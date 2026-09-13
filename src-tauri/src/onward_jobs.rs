//! Read-only Onward / JobPilot market-pipeline observability.
//! This module never reads raw job snapshots and never mutates JobPilot state.
use chrono::{DateTime, Utc};
use regex::Regex;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::Command,
};

const TASK_RAW: &str = "Onward Job Market Bootstrap";
const TASK_BUILD: &str = "Onward France Job Index Build";
const TASK_FINANCE: &str = "Onward Finance Official Bootstrap";
const TASK_SYNC: &str = "Onward Job Index VPS Sync";
const TASK_NAMES: [&str; 4] = [TASK_RAW, TASK_BUILD, TASK_FINANCE, TASK_SYNC];

fn root() -> PathBuf {
    std::env::var_os("AGENT_MONITOR_ONWARD_JOB_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\dev\onward-job-data"))
}

fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

fn parse_ms(value: Option<&str>) -> Option<i64> {
    value
        .and_then(|v| DateTime::parse_from_rfc3339(v).ok())
        .map(|v| v.timestamp_millis())
}

fn read_json(path: &Path) -> Value {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(Value::Null)
}

fn tail(path: &Path, max_bytes: u64) -> String {
    let Ok(mut file) = File::open(path) else {
        return String::new();
    };
    let Ok(len) = file.metadata().map(|m| m.len()) else {
        return String::new();
    };
    let start = len.saturating_sub(max_bytes);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return String::new();
    }
    let mut bytes = Vec::with_capacity((len - start) as usize);
    if file.read_to_end(&mut bytes).is_err() {
        return String::new();
    }
    let text = String::from_utf8_lossy(&bytes).into_owned();
    if start == 0 {
        text
    } else {
        text.split_once('\n')
            .map(|(_, rest)| rest.to_owned())
            .unwrap_or_default()
    }
}

fn ps_tasks() -> Vec<Value> {
    #[cfg(not(windows))]
    {
        return Vec::new();
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let names = TASK_NAMES
            .iter()
            .map(|n| format!("'{}'", n.replace('\'', "''")))
            .collect::<Vec<_>>()
            .join(",");
        let script = format!(
            r#"[Console]::OutputEncoding=[System.Text.UTF8Encoding]::new(); $svc=New-Object -ComObject Schedule.Service; $svc.Connect(); $folder=$svc.GetFolder('\'); $names=@({names}); $state=@{{0='Unknown';1='Disabled';2='Queued';3='Ready';4='Running'}}; $rows=foreach($n in $names){{try{{$t=$folder.GetTask($n);[pscustomobject]@{{name=$n;state=$state[[int]$t.State];hidden=[bool]$t.Definition.Settings.Hidden;lastResult=[int64]$t.LastTaskResult;lastRun=if($t.LastRunTime.Year -gt 2000){{$t.LastRunTime.ToUniversalTime().ToString('o')}}else{{$null}};nextRun=if($t.NextRunTime.Year -gt 2000){{$t.NextRunTime.ToUniversalTime().ToString('o')}}else{{$null}}}}}}catch{{}}}};$rows|ConvertTo-Json -Compress"#
        );
        let mut cmd = Command::new("powershell.exe");
        cmd.args(["-NoProfile", "-NonInteractive", "-Command", &script]);
        cmd.creation_flags(0x08000000);
        let Ok(out) = cmd.output() else {
            return Vec::new();
        };
        if !out.status.success() {
            return Vec::new();
        }
        let Ok(value) = serde_json::from_slice::<Value>(&out.stdout) else {
            return Vec::new();
        };
        match value {
            Value::Array(v) => v,
            Value::Object(_) => vec![value],
            _ => Vec::new(),
        }
    }
}

fn task_map() -> HashMap<String, Value> {
    ps_tasks()
        .into_iter()
        .filter_map(|v| {
            let name = v["name"].as_str()?.to_owned();
            Some((name, v))
        })
        .collect()
}

fn task_status(task: Option<&Value>) -> &'static str {
    let Some(t) = task else {
        return "missing";
    };
    if t["state"]
        .as_str()
        .is_some_and(|s| s.eq_ignore_ascii_case("running"))
    {
        return "running";
    }
    if t["lastResult"].as_i64().unwrap_or(1) != 0 {
        return "failed";
    }
    "waiting"
}

fn task_json(name: &str, tasks: &HashMap<String, Value>, label: &str) -> Value {
    let task = tasks.get(name);
    json!({
        "id": name,
        "label": label,
        "status": task_status(task),
        "state": task.and_then(|v|v["state"].as_str()).unwrap_or("Missing"),
        "hidden": task.and_then(|v|v["hidden"].as_bool()).unwrap_or(false),
        "lastResult": task.and_then(|v|v["lastResult"].as_i64()),
        "lastRunAt": parse_ms(task.and_then(|v|v["lastRun"].as_str())),
        "nextRunAt": parse_ms(task.and_then(|v|v["nextRun"].as_str())),
    })
}

fn contribution_map(summary: &Value) -> HashMap<String, i64> {
    summary["providerContribution"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| {
            Some((
                v["provider"].as_str()?.to_owned(),
                v["primaryAttributedJobs"].as_i64().unwrap_or(0),
            ))
        })
        .collect()
}

fn due_status(source: &Value, task: Option<&Value>) -> &'static str {
    let task_running = task
        .and_then(|t| t["state"].as_str())
        .is_some_and(|s| s.eq_ignore_ascii_case("running"));
    let completed = parse_ms(source["completedAt"].as_str());
    let next = parse_ms(source["nextRefreshAt"].as_str());
    let error_at = parse_ms(source["lastErrorAt"].as_str());
    if error_at.is_some() && error_at > completed {
        return "failed";
    }
    if next.is_some_and(|v| v > now_ms()) {
        return if completed.is_some() {
            "success"
        } else {
            "waiting"
        };
    }
    if task_running {
        return "running";
    }
    if completed.is_some() {
        return "success";
    }
    "waiting"
}

fn source_row(
    id: &str,
    label: &str,
    state: &Value,
    task: Option<&Value>,
    recent_jobs: i64,
    error_count: i64,
    last_error: Option<&str>,
    last_batch_added: Option<i64>,
    last_batch_at: Option<i64>,
) -> Value {
    let base = if state.is_null() {
        json!({})
    } else {
        state.clone()
    };
    json!({
        "id": id,
        "label": label,
        "status": due_status(&base, task),
        "lastSuccessAt": parse_ms(base["completedAt"].as_str()).or_else(||parse_ms(base["lastSuccessAt"].as_str())),
        "nextRefreshAt": parse_ms(base["nextRefreshAt"].as_str()),
        "recentJobs": recent_jobs,
        "errorCount": error_count,
        "lastError": last_error.or_else(||base["lastError"].as_str()),
        "lastBatchAdded": last_batch_added,
        "lastBatchAt": last_batch_at,
    })
}

#[derive(Default, Clone)]
struct LogHealth {
    errors: i64,
    last_error: Option<String>,
    last_saved: Option<i64>,
    last_saved_at: Option<i64>,
}

fn provider_log_health(text: &str) -> HashMap<String, LogHealth> {
    let mut out: HashMap<String, LogHealth> = HashMap::new();
    let error = Regex::new(
        r"^\[([^\]]+)\]\s+(?i:(greenhouse|ashby|lever|workable|smartrecruiters|arbeitnow))[^:]*:\s*ERROR\s*(.*)$",
    )
    .unwrap();
    let saved = Regex::new(
        r"^\[([^\]]+)\]\s+(?i:(greenhouse|ashby|lever|workable|smartrecruiters|arbeitnow))[^:]*:\s*saved\s+(\d+)\s+recent",
    )
    .unwrap();
    for line in text.lines() {
        if let Some(c) = error.captures(line) {
            let id = c.get(2).unwrap().as_str().to_ascii_lowercase();
            let entry = out.entry(id).or_default();
            entry.errors += 1;
            entry.last_error = Some(
                c.get(3)
                    .map(|m| m.as_str().trim())
                    .unwrap_or("error")
                    .chars()
                    .take(180)
                    .collect(),
            );
        }
        if let Some(c) = saved.captures(line) {
            let id = c.get(2).unwrap().as_str().to_ascii_lowercase();
            let at = parse_ms(c.get(1).map(|m| m.as_str()));
            let count = c.get(3).and_then(|m| m.as_str().parse::<i64>().ok());
            let entry = out.entry(id).or_default();
            if at >= entry.last_saved_at {
                entry.last_saved_at = at;
                entry.last_saved = count;
            }
        }
    }
    out
}

fn raw_runs(text: &str) -> Vec<Value> {
    let re = Regex::new(r"^\[([^\]]+)\]\s+run complete; files=(\d+); recent=(\d+); undated=(\d+)")
        .unwrap();
    let mut values: Vec<(i64, i64, i64)> = Vec::new();
    for line in text.lines() {
        let Some(c) = re.captures(line) else {
            continue;
        };
        let Some(at) = parse_ms(c.get(1).map(|m| m.as_str())) else {
            continue;
        };
        let files = c
            .get(2)
            .and_then(|m| m.as_str().parse::<i64>().ok())
            .unwrap_or(0);
        let total = c
            .get(3)
            .and_then(|m| m.as_str().parse::<i64>().ok())
            .unwrap_or(0);
        values.push((at, files, total));
    }
    let start = values.len().saturating_sub(25);
    values[start..]
        .iter()
        .enumerate()
        .map(|(i, (at, files, total))| {
            let absolute = start + i;
            let added = if absolute > 0 {
                total.saturating_sub(values[absolute - 1].2)
            } else {
                0
            };
            json!({"at":at,"files":files,"total":total,"added":added})
        })
        .collect()
}

fn sync_runs(text: &str) -> Vec<Value> {
    let re = Regex::new(r"^\[([^\]]+)\]\s+export\s+(\{.*\})$").unwrap();
    let mut out = Vec::new();
    for line in text.lines() {
        let Some(c) = re.captures(line.trim_start_matches('\u{feff}')) else {
            continue;
        };
        let Some(at) = parse_ms(c.get(1).map(|m| m.as_str())) else {
            continue;
        };
        if let Ok(v) = serde_json::from_str::<Value>(c.get(2).unwrap().as_str()) {
            out.push(json!({"at":at,"scanned":v["scanned"],"upserts":v["upserts"],"deletes":v["deletes"]}));
        }
    }
    out.into_iter()
        .rev()
        .take(20)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn last_sync_import(text: &str) -> Option<Value> {
    let re = Regex::new(r"^\[([^\]]+)\]\s+import\s+(\{.*\})$").unwrap();
    let mut latest = None;
    for line in text.lines() {
        let Some(c) = re.captures(line.trim_start_matches('\u{feff}')) else {
            continue;
        };
        let at = parse_ms(c.get(1).map(|m| m.as_str()));
        let Ok(v) = serde_json::from_str::<Value>(c.get(2).unwrap().as_str()) else {
            continue;
        };
        latest = Some(json!({
            "at":at,
            "ok":v["ok"],
            "upserts":v["upserts"],
            "deletes":v["deletes"],
            "activeJobs":v["activeJobs"],
            "bytes":v["bytes"],
            "syncedAt":parse_ms(v["syncedAt"].as_str()),
        }));
    }
    latest
}

fn build_runs(text: &str) -> Vec<Value> {
    let built = Regex::new(r#""builtAt"\s*:\s*"([^"]+)""#).unwrap();
    let canonical = Regex::new(r#""canonicalTotal"\s*:\s*(\d+)"#).unwrap();
    let mut current_at = None;
    let mut out: Vec<(i64, i64)> = Vec::new();
    for line in text.lines() {
        if let Some(c) = built.captures(line) {
            current_at = parse_ms(c.get(1).map(|m| m.as_str()));
        }
        if let Some(c) = canonical.captures(line) {
            if let (Some(at), Some(total)) = (
                current_at,
                c.get(1).and_then(|m| m.as_str().parse::<i64>().ok()),
            ) {
                if out.last().map(|x| x.0) != Some(at) {
                    out.push((at, total));
                }
            }
        }
    }
    let start = out.len().saturating_sub(16);
    out[start..]
        .iter()
        .enumerate()
        .map(|(i, (at, total))| {
            let absolute = start + i;
            let delta = if absolute > 0 {
                *total - out[absolute - 1].1
            } else {
                0
            };
            json!({"at":at,"total":total,"delta":delta})
        })
        .collect()
}

fn finance_label(id: &str) -> String {
    match id {
        "bnp-paribas" => "BNP Paribas",
        "societe-generale" => "Société Générale",
        "amundi" => "Amundi",
        "caceis" => "CACEIS",
        "credit-agricole-cib" => "Crédit Agricole CIB",
        "axa" => "AXA",
        "citi" => "Citi",
        "blackrock" => "BlackRock",
        "rothschild" => "Rothschild & Co",
        "hsbc" => "HSBC",
        "credit-mutuel-arkea" => "Crédit Mutuel Arkéa",
        "ardian" => "Ardian",
        "tikehau-capital" => "Tikehau Capital",
        _ => id,
    }
    .to_owned()
}

fn finance_rows(finance: &Value, finance_task: Option<&Value>) -> Vec<Value> {
    let mut rows = Vec::new();
    if let Some(obj) = finance["sources"].as_object() {
        for (id, v) in obj {
            rows.push(json!({
                "id":id,
                "label":finance_label(id),
                "status":due_status(v,finance_task),
                "recordCount":v["recordCount"].as_i64().unwrap_or(0),
                "lastSuccessAt":parse_ms(v["completedAt"].as_str()),
                "nextRefreshAt":parse_ms(v["nextRefreshAt"].as_str()),
                "lastError":v["lastError"].as_str(),
            }));
        }
    }
    let bpce = &finance["bpce"];
    if !bpce.is_null() {
        rows.push(json!({
            "id":"bpce-natixis",
            "label":"Groupe BPCE / Natixis",
            "status":due_status(bpce,finance_task),
            "recordCount":bpce["seenJobIds"].as_array().map(|a|a.len()).unwrap_or(0),
            "lastSuccessAt":parse_ms(bpce["completedAt"].as_str()),
            "nextRefreshAt":parse_ms(bpce["nextRefreshAt"].as_str()),
            "lastError":bpce["lastError"].as_str(),
        }));
    }
    rows.sort_by(|a, b| {
        b["recordCount"]
            .as_i64()
            .unwrap_or(0)
            .cmp(&a["recordCount"].as_i64().unwrap_or(0))
    });
    rows
}

fn finance_recent_total(finance: &Value) -> i64 {
    let base = finance["sources"]
        .as_object()
        .map(|o| {
            o.values()
                .map(|v| v["recordCount"].as_i64().unwrap_or(0))
                .sum()
        })
        .unwrap_or(0);
    base + finance["bpce"]["seenJobIds"]
        .as_array()
        .map(|a| a.len() as i64)
        .unwrap_or(0)
}

pub fn snapshot() -> Result<Value, String> {
    let root = root();
    if !root.is_dir() {
        return Ok(
            json!({"available":false,"generatedAt":now_ms(),"error":"Onward job-data directory not found"}),
        );
    }
    let state = read_json(&root.join("state.json"));
    let finance = read_json(&root.join("finance-official-state.json"));
    let summary = read_json(&root.join("derived").join("summary-14d.json"));
    let build = read_json(&root.join("derived").join("build-state.json"));
    let collector_log = tail(&root.join("collector.log"), 512 * 1024);
    let finance_log = tail(&root.join("finance-official.log"), 256 * 1024);
    let sync_log = tail(&root.join("sync").join("sync.log"), 512 * 1024);
    let build_log = tail(&root.join("derived").join("index-build.log"), 512 * 1024);
    let tasks = task_map();
    let contribution = contribution_map(&summary);
    let log_health = provider_log_health(&collector_log);
    let raw_task = tasks.get(TASK_RAW);
    let finance_task = tasks.get(TASK_FINANCE);
    let ats = &state["ats"];
    let smart = &state["smartRecruiters"];
    let arbeit = &state["arbeitnow"];
    let provider_health = |id: &str| log_health.get(id).cloned().unwrap_or_default();
    let mut sources = Vec::new();
    sources.push(source_row(
        "france-travail",
        "France Travail",
        &state["franceTravail"],
        raw_task,
        *contribution.get("france-travail").unwrap_or(&0),
        0,
        None,
        None,
        None,
    ));
    let h = provider_health("smartrecruiters");
    sources.push(source_row(
        "smartrecruiters",
        "SmartRecruiters",
        smart,
        raw_task,
        *contribution.get("smartrecruiters").unwrap_or(&0),
        h.errors,
        h.last_error.as_deref(),
        h.last_saved,
        h.last_saved_at,
    ));
    for (id, label) in [
        ("greenhouse", "Greenhouse"),
        ("ashby", "Ashby"),
        ("lever", "Lever"),
        ("workable", "Workable"),
    ] {
        let h = provider_health(id);
        sources.push(source_row(
            id,
            label,
            ats,
            raw_task,
            *contribution.get(id).unwrap_or(&0),
            h.errors,
            h.last_error.as_deref(),
            h.last_saved,
            h.last_saved_at,
        ));
    }
    let h = provider_health("arbeitnow");
    sources.push(source_row(
        "arbeitnow",
        "Arbeitnow",
        arbeit,
        raw_task,
        *contribution.get("arbeitnow").unwrap_or(&0),
        h.errors,
        h.last_error.as_deref(),
        h.last_saved,
        h.last_saved_at,
    ));
    sources.push(json!({
        "id":"finance-official","label":"Finance official careers","status":if task_status(finance_task)=="failed"{"failed"}else if task_status(finance_task)=="running"{"running"}else if parse_ms(finance["lastRunAt"].as_str()).is_some(){"success"}else{"waiting"},
        "lastSuccessAt":parse_ms(finance["lastRunAt"].as_str()),"nextRefreshAt":Value::Null,
        "recentJobs":*contribution.get("finance-official").unwrap_or(&0),"errorCount":0,"lastError":Value::Null,
        "lastBatchAdded":Value::Null,"lastBatchAt":Value::Null,
    }));
    let raw_history = raw_runs(&collector_log);
    let sync_history = sync_runs(&sync_log);
    let build_history = build_runs(&build_log);
    let last_import = last_sync_import(&sync_log).unwrap_or_else(|| json!({}));
    let sync_task = tasks.get(TASK_SYNC);
    let sync_last_result = sync_task.and_then(|v| v["lastResult"].as_i64());
    let import_at = last_import["at"].as_i64();
    let vps_fresh = import_at.is_some_and(|at| now_ms() - at < 45 * 60 * 1000);
    let vps_ok =
        last_import["ok"].as_bool().unwrap_or(false) && sync_last_result == Some(0) && vps_fresh;
    let task_rows = vec![
        task_json(TASK_RAW, &tasks, "Raw market collectors"),
        task_json(TASK_FINANCE, &tasks, "Finance official collectors"),
        task_json(TASK_BUILD, &tasks, "Derived France Job Index"),
        task_json(TASK_SYNC, &tasks, "VPS incremental sync"),
    ];
    let contracts = summary["contracts"].clone();
    let industries = summary["industries"]
        .as_array()
        .map(|a| Value::Array(a.iter().take(10).cloned().collect()))
        .unwrap_or_else(|| json!([]));
    let providers = summary["providerContribution"].clone();
    let stage = summary["contracts"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|v| v["name"] == "Stage")
        .and_then(|v| v["count"].as_i64())
        .unwrap_or(0);
    let alternance = summary["contracts"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|v| v["name"] == "Alternance")
        .and_then(|v| v["count"].as_i64())
        .unwrap_or(0);
    Ok(json!({
        "available":true,
        "generatedAt":now_ms(),
        "bootstrapDays":state["bootstrapWindow"]["days"].as_i64().unwrap_or(0),
        "totals":{
            "rawRecords":state["totals"]["franceRecentRecords"].as_i64().unwrap_or(0),
            "rawFiles":state["totals"]["files"].as_i64().unwrap_or(0),
            "canonicalAll":build["crossPlatformTotal"].as_i64().unwrap_or(0),
            "unique14d":summary["counts"]["independentJobsAfterCrossPlatformDedupe"].as_i64().unwrap_or(0),
            "paris14d":summary["counts"]["parisJobs"].as_i64().unwrap_or(0),
            "ileDeFrance14d":summary["counts"]["ileDeFranceJobs"].as_i64().unwrap_or(0),
            "stage14d":stage,
            "alternance14d":alternance,
            "financeOfficialCurrent":finance_recent_total(&finance),
            "duplicatePct":summary["duplicateRates"]["rawToFinalDuplicatePct"].as_f64().unwrap_or(0.0),
        },
        "builtAt":parse_ms(build["builtAt"].as_str()),
        "lastRawRunAt":parse_ms(state["lastRunAt"].as_str()),
        "lastFinanceRunAt":parse_ms(finance["lastRunAt"].as_str()),
        "tasks":task_rows,
        "sources":sources,
        "financeSources":finance_rows(&finance,finance_task),
        "rawRuns":raw_history,
        "syncRuns":sync_history,
        "buildRuns":build_history,
        "contracts":contracts,
        "industries":industries,
        "providers":providers,
        "vps":{
            "healthy":vps_ok,
            "status":if vps_ok{"connected"}else if sync_last_result.is_some_and(|v|v!=0){"failed"}else{"stale"},
            "lastImportAt":import_at,
            "syncedAt":last_import["syncedAt"],
            "activeJobs":last_import["activeJobs"],
            "bytes":last_import["bytes"],
            "lastUpserts":last_import["upserts"],
            "lastDeletes":last_import["deletes"],
            "taskLastResult":sync_last_result,
            "taskNextRunAt":parse_ms(sync_task.and_then(|v|v["nextRun"].as_str())),
        },
        "coverage":{
            "windowDays":summary["windowDays"],
            "summaryBuiltAt":parse_ms(summary["builtAt"].as_str()),
            "parseErrors":summary["parseErrors"],
            "note":"Dashboard reads only derived state, summaries, bounded log tails and Task Scheduler metadata; raw snapshots are not scanned."
        },
        "financeLogRecent":finance_log.lines().rev().take(8).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_raw_run_deltas() {
        let rows=raw_runs("[2026-09-13T10:00:00Z] run complete; files=10; recent=100; undated=0\n[2026-09-13T10:03:00Z] run complete; files=12; recent=107; undated=0\n");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1]["added"], 7);
    }
    #[test]
    fn parses_sync_delta() {
        let rows = sync_runs(
            "[2026-09-13T10:07:00Z] export {\"scanned\":100,\"upserts\":5,\"deletes\":2}\n",
        );
        assert_eq!(rows[0]["upserts"], 5);
        assert_eq!(rows[0]["deletes"], 2);
    }

    #[test]
    fn parses_provider_batch_and_error_evidence() {
        let health = provider_log_health(
            "[2026-09-13T10:00:00Z] smartrecruiters/Example: saved 12 recent + 0 undated France records\n[2026-09-13T10:01:00Z] greenhouse/OldBoard: ERROR HTTP 404 Not Found\n",
        );
        assert_eq!(health["smartrecruiters"].last_saved, Some(12));
        assert!(health["smartrecruiters"].last_saved_at.is_some());
        assert_eq!(health["greenhouse"].errors, 1);
        assert!(health["greenhouse"]
            .last_error
            .as_deref()
            .unwrap_or("")
            .contains("404"));
    }

    #[test]
    #[cfg(windows)]
    fn live_snapshot_is_read_only_and_structured_when_local_data_exists() {
        if !root().is_dir() {
            return;
        }
        let value = snapshot().expect("read-only Onward snapshot");
        assert_eq!(value["available"], true);
        assert!(value["totals"]["canonicalAll"].as_i64().unwrap_or(0) > 1_000);
        let tasks = value["tasks"].as_array().expect("scheduled tasks");
        assert_eq!(tasks.len(), 4);
        assert!(tasks
            .iter()
            .all(|task| task["hidden"].as_bool() == Some(true)));
        assert!(tasks
            .iter()
            .all(|task| task["status"].as_str() != Some("missing")));
        assert!(value["sources"].as_array().map(|v| v.len()).unwrap_or(0) >= 8);
        assert!(value["vps"]["activeJobs"].as_i64().unwrap_or(0) > 1_000);
        assert_eq!(value["vps"]["healthy"], true);
    }
}
