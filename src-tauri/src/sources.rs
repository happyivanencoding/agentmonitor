use crate::model::*;
use rusqlite::{params, types::ValueRef, Connection, OpenFlags};
use serde_json::{json, Map, Value};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

pub fn home() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_default()
}
pub fn db_path(dir: &Path, stem: &str, default: &str) -> PathBuf {
    std::fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let n = name
                .strip_prefix(&format!("{stem}_"))?
                .strip_suffix(".sqlite")?
                .parse::<u32>()
                .ok()?;
            Some((n, e.path()))
        })
        .max_by_key(|x| x.0)
        .map(|x| x.1)
        .unwrap_or_else(|| dir.join(default))
}
pub fn ro(path: &Path) -> Result<Connection, String> {
    let c = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| {
        format!(
            "Read-only {}: {e}",
            path.file_name().unwrap_or_default().to_string_lossy()
        )
    })?;
    c.busy_timeout(Duration::from_millis(250))
        .map_err(|e| e.to_string())?;
    c.execute_batch("PRAGMA query_only=ON;")
        .map_err(|e| e.to_string())?;
    Ok(c)
}
pub fn columns(c: &Connection, table: &str) -> HashSet<String> {
    c.prepare(&format!("PRAGMA table_info({table})"))
        .and_then(|mut s| s.query_map([], |r| r.get::<_, String>(1))?.collect())
        .unwrap_or_default()
}
fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let mut m = Map::new();
    let s = row.as_ref();
    for (i, n) in s.column_names().iter().enumerate() {
        let v = match row.get_ref(i)? {
            ValueRef::Null => Value::Null,
            ValueRef::Integer(n) => json!(n),
            ValueRef::Real(n) => json!(n),
            ValueRef::Text(b) => Value::String(String::from_utf8_lossy(b).into_owned()),
            ValueRef::Blob(_) => Value::Null,
        };
        m.insert((*n).into(), v);
    }
    Ok(Value::Object(m))
}
pub fn threads(c: &Connection) -> Result<(Vec<Value>, usize), String> {
    let cols = columns(c, "threads");
    if !cols.contains("id") {
        return Err("Unrecognized Codex schema: threads.id missing".into());
    }
    let wanted = [
        "id",
        "rollout_path",
        "created_at",
        "updated_at",
        "created_at_ms",
        "updated_at_ms",
        "source",
        "model_provider",
        "cwd",
        "title",
        "name",
        "tokens_used",
        "model",
        "reasoning_effort",
        "agent_nickname",
        "agent_role",
        "thread_source",
        "archived",
    ];
    let select = wanted
        .iter()
        .map(|key| {
            if !cols.contains(*key) {
                format!("NULL AS {key}")
            } else if matches!(*key, "title" | "name" | "source") {
                format!("substr({key},1,700) AS {key}")
            } else {
                key.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(",");
    let order = if cols.contains("updated_at") {
        "updated_at DESC"
    } else {
        "id"
    };
    let mut s = c
        .prepare(&format!(
            "SELECT {select} FROM threads ORDER BY {order} LIMIT 15000"
        ))
        .map_err(|e| e.to_string())?;
    let rows = s
        .query_map([], map_row)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let count = c
        .query_row("SELECT COUNT(*) FROM threads", [], |r| r.get::<_, i64>(0))
        .unwrap_or(rows.len() as i64) as usize;
    Ok((rows, count))
}
pub fn parents(c: &Connection) -> HashMap<String, String> {
    if !columns(c, "thread_spawn_edges").contains("parent_thread_id") {
        return HashMap::new();
    }
    c.prepare("SELECT child_thread_id,parent_thread_id FROM thread_spawn_edges")
        .and_then(|mut s| {
            s.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
                .collect()
        })
        .unwrap_or_default()
}
pub fn latest_turns(c: &Connection) -> Result<HashMap<String, Turn>, String> {
    let cols = columns(c, "thread_turns");
    if !cols.contains("thread_id") {
        return Err("thread_turns projection unavailable".into());
    }
    let wanted = [
        "thread_id",
        "turn_id",
        "status",
        "started_at",
        "completed_at",
        "duration_ms",
        "error_json",
    ];
    let select = wanted
        .iter()
        .map(|k| {
            if cols.contains(*k) {
                k.to_string()
            } else {
                format!("NULL AS {k}")
            }
        })
        .collect::<Vec<_>>()
        .join(",");
    let mut s = c
        .prepare(&format!(
            "SELECT {select} FROM thread_turns ORDER BY started_at DESC LIMIT 30000"
        ))
        .map_err(|e| e.to_string())?;
    let rows = s.query_map([], map_row).map_err(|e| e.to_string())?;
    let mut result = HashMap::new();
    for v in rows {
        let v = v.map_err(|e| e.to_string())?;
        result.entry(text(&v, "thread_id")).or_insert_with(|| Turn {
            id: text(&v, "turn_id"),
            status: text(&v, "status"),
            start_ms: v.get("started_at").and_then(millis).unwrap_or(0),
            end_ms: v.get("completed_at").and_then(millis),
            duration_ms: v.get("duration_ms").and_then(Value::as_i64),
            error: v.get("error_json").and_then(Value::as_str).map(redact),
            source: "thread_turns".into(),
        });
    }
    Ok(result)
}
/// Project metadata in SQLite. Never transfer entire command outputs, prompts, reasoning, arguments or diffs into the collector.
pub fn items(c: &Connection, id: &str, limit: usize) -> Result<Vec<Event>, String> {
    let sql="SELECT thread_id,turn_id,item_id,created_at_ms,json_object(
    'type',json_extract(item_json,'$.type'),'id',json_extract(item_json,'$.id'),
    'status',json_extract(item_json,'$.status'),'durationMs',json_extract(item_json,'$.durationMs'),
    'command',substr(json_extract(item_json,'$.command'),1,2000),'processId',json_extract(item_json,'$.processId'),
    'server',json_extract(item_json,'$.server'),'tool',json_extract(item_json,'$.tool'),
    'changes',json_array(json_object('path',json_extract(item_json,'$.changes[0].path')))
    ) AS item_json FROM thread_items WHERE thread_id=? AND json_valid(item_json) ORDER BY rollout_ordinal DESC LIMIT ?";
    let mut s = c.prepare_cached(sql).map_err(|e| e.to_string())?;
    let rows = s
        .query_map(params![id, limit.min(4000) as i64], map_row)
        .map_err(|e| e.to_string())?;
    Ok(rows
        .filter_map(Result::ok)
        .filter_map(|r| item_event(&r))
        .collect())
}
#[derive(Default)]
pub struct JsonCache {
    cache: HashMap<PathBuf, (Option<SystemTime>, u64, Value)>,
}
impl JsonCache {
    fn load(&mut self, p: &Path) -> Option<Value> {
        let m = p.metadata().ok()?;
        if m.len() > 4 * 1024 * 1024 {
            return None;
        }
        let modified = m.modified().ok();
        if let Some((time, size, v)) = self.cache.get(p) {
            if *time == modified && *size == m.len() {
                return Some(v.clone());
            }
        }
        let s = std::fs::read_to_string(p).ok()?;
        let v: Value = serde_json::from_str(s.trim_start_matches('\u{feff}')).ok()?;
        self.cache.insert(p.into(), (modified, m.len(), v.clone()));
        Some(v)
    }
    pub fn acp(&mut self, root: &Path) -> Result<Vec<Value>, String> {
        if !root.is_dir() {
            return Err("ACP session directory unavailable".into());
        }
        let mut out = Vec::new();
        for e in walkdir::WalkDir::new(root)
            .max_depth(3)
            .follow_links(false)
            .into_iter()
        {
            let e = e.map_err(|e| e.to_string())?;
            if !e.file_type().is_file()
                || !e.file_name().to_string_lossy().starts_with("acps_")
                || e.path().extension().and_then(|s| s.to_str()) != Some("json")
            {
                continue;
            }
            if let Some(v) = self.load(e.path()) {
                out.push(json!({"id":v["id"],"remoteSessionId":v["remote_session_id"],"parentId":e.path().parent().and_then(|p|p.file_name()).map(|s|s.to_string_lossy().to_string()),"agent":v["agent"],"cwd":v["cwd"],"modeId":v["mode_id"],"status":v["status"],"stopReason":v["last_stop_reason"],"createdAt":v.get("created_at").and_then(millis),"updatedAt":v.get("updated_at").and_then(millis),"closedAt":v.get("closed_at").and_then(millis)}));
            }
        }
        Ok(out)
    }
    pub fn tasks(&mut self, root: &Path) -> Vec<Value> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(root).ok().into_iter().flatten().flatten() {
            if e.path().extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            if let Some(v) = self.load(&e.path()) {
                let steps = v["steps"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .map(|s| json!({"id":s["id"],"title":s["title"],"status":s["status"]}))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                out.push(json!({"id":v["id"],"title":v["title"],"goal":clipped(&text(&v,"goal"),500),"project":v["project"],"status":v["status"],"phase":v["phase"],"steps":steps,"updatedAt":v.get("updated_at").and_then(millis),"createdAt":v.get("created_at").and_then(millis),"completedAt":v.get("completed_at").and_then(millis)}));
            }
        }
        out.sort_by_key(|v| std::cmp::Reverse(v["updatedAt"].as_i64().unwrap_or(0)));
        out.truncate(120);
        out
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn readonly_does_not_allow_writes() {
        let p = std::env::temp_dir().join(format!("am-ro-{}.sqlite", uuid::Uuid::new_v4()));
        {
            let c = Connection::open(&p).unwrap();
            c.execute_batch("CREATE TABLE test(a INTEGER)").unwrap();
        }
        let c = ro(&p).unwrap();
        assert!(c.execute("INSERT INTO test VALUES(1)", []).is_err());
        drop(c);
        std::fs::remove_file(p).unwrap();
    }
    #[test]
    fn schema_drift_becomes_null() {
        let c = Connection::open_in_memory().unwrap();
        c.execute_batch(
            "CREATE TABLE threads(id TEXT,updated_at INTEGER);INSERT INTO threads VALUES('t',1);",
        )
        .unwrap();
        let (rows, _) = threads(&c).unwrap();
        assert!(rows[0]["model"].is_null());
    }
}
