use crate::{model::*, os, rollout::Reader, sources, storage};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::Instant,
};

#[derive(Default)]
struct Observation {
    fingerprint: String,
    stable_since: i64,
    owner: Option<(u32, u64)>,
    lost_at: Option<i64>,
    active: bool,
}
pub struct Collector {
    pub codex: PathBuf,
    pub agentdock: PathBuf,
    pub data: PathBuf,
    json: sources::JsonCache,
    rollouts: Reader,
    processes: os::ProcessReader,
    observations: HashMap<String, Observation>,
    item_cache: HashMap<String, (i64, Vec<Event>)>,
    persisted: HashMap<String, String>,
    locks: HashMap<String, Vec<u32>>,
    lock_errors: Vec<String>,
    lock_at: i64,
    last_acp: Vec<Value>,
}
fn updated(v: &Value) -> i64 {
    v.get("updated_at_ms")
        .and_then(Value::as_i64)
        .or_else(|| v.get("updated_at").and_then(millis))
        .unwrap_or(0)
}
fn created(v: &Value) -> i64 {
    v.get("created_at_ms")
        .and_then(Value::as_i64)
        .or_else(|| v.get("created_at").and_then(millis))
        .unwrap_or(0)
}
fn project(cwd: &str) -> String {
    let normalized = cwd.replace('\\', "/");
    let parts = normalized
        .split('/')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>();
    if let Some(i) = parts.iter().position(|p| p.eq_ignore_ascii_case("dev")) {
        if let Some(name) = parts.get(i + 1) {
            return (*name).into();
        }
    }
    parts
        .last()
        .map(|s| (*s).to_owned())
        .unwrap_or_else(|| "unknown workspace".into())
}
fn title(v: &Value) -> String {
    for k in ["name", "agent_nickname"] {
        let s = text(v, k);
        if !s.is_empty() {
            return clipped(&s, 110);
        }
    }
    let t = text(v, "title");
    let first = t.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    if !first.is_empty() && !first.starts_with('#') {
        return clipped(first, 110);
    }
    format!("Codex thread 路 {}", clipped(&text(v, "id"), 8))
}
fn tool_event(e: &Event) -> bool {
    matches!(
        e.kind.as_str(),
        "tool"
            | "commandExecution"
            | "mcpToolCall"
            | "fileChange"
            | "collabAgentToolCall"
            | "dynamicToolCall"
            | "webSearch"
    )
}
impl Collector {
    pub fn new(data: PathBuf) -> Self {
        let h = sources::home();
        Self {
            codex: std::env::var_os("AGENT_MONITOR_CODEX_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| h.join(".codex")),
            agentdock: std::env::var_os("AGENT_MONITOR_AGENTDOCK_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| h.join(".agentdock")),
            data,
            json: Default::default(),
            rollouts: Default::default(),
            processes: os::ProcessReader::new(),
            observations: Default::default(),
            item_cache: Default::default(),
            persisted: Default::default(),
            locks: Default::default(),
            lock_errors: vec![],
            lock_at: 0,
            last_acp: vec![],
        }
    }
    pub fn collect(&mut self) -> Result<Value, String> {
        let started = Instant::now();
        let at = now_ms();
        let state_path = sources::db_path(&self.codex, "state", "state_5.sqlite");
        let history_path =
            sources::db_path(&self.codex, "thread_history", "thread_history_1.sqlite");
        let state = sources::ro(&state_path)?;
        let (metadata, total_threads) = sources::threads(&state)?;
        let mut parents = sources::parents(&state);
        for row in &metadata {
            if let Ok(s) = serde_json::from_str::<Value>(&text(row, "source")) {
                if let Some(p) = s
                    .pointer("/subagent/thread_spawn/parent_thread_id")
                    .and_then(Value::as_str)
                {
                    parents.entry(text(row, "id")).or_insert_with(|| p.into());
                }
            }
        }
        let hc = sources::ro(&history_path);
        let (history, history_error) = match hc {
            Ok(c) => (Some(c), None),
            Err(e) => (None, Some(e)),
        };
        let projected_result = history.as_ref().map(sources::latest_turns);
        let mut projection_error = history_error;
        let projected = match projected_result {
            Some(Ok(v)) => v,
            Some(Err(e)) => {
                projection_error = Some(e);
                HashMap::new()
            }
            None => HashMap::new(),
        };
        let acp_result = self.json.acp(&self.agentdock.join("acp/sessions"));
        let acp_error = acp_result.as_ref().err().cloned();
        if let Ok(v) = acp_result {
            self.last_acp = v;
        }
        let acp = self.last_acp.clone();
        let tasks = self.json.tasks(&self.agentdock.join("tasks"));
        let mut acp_by_thread: HashMap<String, Vec<Value>> = HashMap::new();
        for a in &acp {
            acp_by_thread
                .entry(text(a, "remoteSessionId"))
                .or_default()
                .push(a.clone());
        }
        let processes = self.processes.refresh();
        if at - self.lock_at > 10_000 {
            (self.locks, self.lock_errors) =
                os::lock_owners(&self.codex.join("thread-writer-locks"));
            self.lock_at = at;
        }
        let proc_by_pid: HashMap<u32, &Value> = processes
            .iter()
            .filter_map(|p| p["pid"].as_u64().map(|id| (id as u32, p)))
            .collect();
        let mut selected: HashSet<String> = acp_by_thread.keys().cloned().collect();
        selected.extend(metadata.iter().take(120).map(|v| text(v, "id")));
        selected.extend(
            projected
                .iter()
                .filter(|(_, t)| active(&t.status))
                .map(|(id, _)| id.clone()),
        );
        selected.extend(self.locks.keys().cloned());
        // Include exact ancestors, and recent direct descendants. No cwd/time caller inference.
        for _ in 0..12 {
            let old = selected.len();
            for (child, parent) in &parents {
                if selected.contains(child) {
                    selected.insert(parent.clone());
                }
            }
            if old == selected.len() {
                break;
            }
        }
        for row in &metadata {
            let id = text(row, "id");
            if updated(row) > at - 86_400_000
                && parents
                    .get(&id)
                    .map(|p| selected.contains(p))
                    .unwrap_or(false)
            {
                selected.insert(id);
            }
        }
        let mut own = storage::open(&self.data.join("monitor.sqlite"))?;
        let bindings = storage::bindings(&own)?;
        let links: HashMap<String, String> = storage::task_links(&own).into_iter().collect();
        let archived_at: HashMap<String, i64> = storage::archived(&own).into_iter().collect();
        let binding_map: HashMap<String, Value> = bindings
            .iter()
            .map(|b| (text(b, "entityId"), b.clone()))
            .collect();
        let mut thread_bindings: HashMap<String, Value> = HashMap::new();
        let mut conflicts = HashSet::new();
        for row in &metadata {
            let id = text(row, "id");
            let mut candidates = Vec::new();
            if let Some(b) = binding_map.get(&id) {
                candidates.push(b.clone());
            }
            for a in acp_by_thread.get(&id).into_iter().flatten() {
                if let Some(b) = binding_map.get(&text(a, "id")) {
                    candidates.push(b.clone());
                }
            }
            let cids = candidates
                .iter()
                .map(|b| text(b, "conversationId"))
                .collect::<HashSet<_>>();
            if cids.len() == 1 {
                thread_bindings.insert(id, candidates[0].clone());
            } else if cids.len() > 1 {
                conflicts.insert(id);
            }
        }
        for _ in 0..12 {
            let mut inherited = Vec::new();
            for (child, parent) in &parents {
                if !thread_bindings.contains_key(child) && !conflicts.contains(child) {
                    if let Some(b) = thread_bindings.get(parent) {
                        let mut b = b.clone();
                        b["inheritedFromThread"] = json!(parent);
                        inherited.push((child.clone(), b));
                    }
                }
            }
            if inherited.is_empty() {
                break;
            }
            thread_bindings.extend(inherited);
        }
        let tx = own.transaction().map_err(|e| e.to_string())?;
        let mut agents = Vec::new();
        let mut seen = HashSet::new();
        let mut item_read_errors = 0usize;
        for row in &metadata {
            let id = text(row, "id");
            if !selected.contains(&id) {
                continue;
            }
            seen.insert(id.clone());
            let sessions = acp_by_thread.get(&id).cloned().unwrap_or_default();
            let roll = self.rollouts.read(&text(row, "rollout_path"));
            let pt = projected.get(&id).cloned();
            let turn = match (pt, roll.turn.clone()) {
                (Some(p), Some(r)) => {
                    if r.start_ms > p.start_ms
                        || (r.id == p.id && r.end_ms.unwrap_or(0) > p.end_ms.unwrap_or(0))
                    {
                        Some(r)
                    } else {
                        Some(p)
                    }
                }
                (p, r) => p.or(r),
            };
            let cache_key = updated(row).max(roll.last_activity);
            let events = if projected.contains_key(&id) {
                if self
                    .item_cache
                    .get(&id)
                    .map(|(k, _)| *k == cache_key)
                    .unwrap_or(false)
                {
                    self.item_cache[&id].1.clone()
                } else if let Some(h) = &history {
                    match sources::items(h, &id, 40) {
                        Ok(v) => {
                            self.item_cache.insert(id.clone(), (cache_key, v.clone()));
                            v
                        }
                        Err(_) => {
                            item_read_errors += 1;
                            self.item_cache
                                .get(&id)
                                .map(|(_, v)| v.clone())
                                .unwrap_or_default()
                        }
                    }
                } else {
                    vec![]
                }
            } else {
                roll.events.clone()
            };
            let mut events = events;
            events.sort_by_key(|e| e.start_ms);
            let lifecycle_time = turn
                .as_ref()
                .map(|t| t.end_ms.unwrap_or(t.start_ms))
                .unwrap_or(0);
            let last_activity = events
                .iter()
                .map(|e| e.end_ms.unwrap_or(e.start_ms))
                .max()
                .unwrap_or(0)
                .max(roll.last_activity)
                .max(lifecycle_time);
            let last_activity = if last_activity > 0 {
                last_activity
            } else {
                updated(row)
            };
            let current_tool = turn
                .as_ref()
                .filter(|t| active(&t.status))
                .and_then(|t| {
                    events.iter().rev().find(|e| {
                        tool_event(e)
                            && (active(&e.status) || e.status.to_lowercase().contains("approval"))
                            && e.turn_id.as_deref() == Some(t.id.as_str())
                    })
                })
                .cloned();
            let last_tool = events.iter().rev().find(|e| tool_event(e)).cloned();
            let last_command = events.iter().rev().find_map(|e| e.command.clone());
            let last_file = events.iter().rev().find_map(|e| e.file.clone());
            let owners = self
                .locks
                .get(&id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(|p| proc_by_pid.contains_key(p))
                .collect::<Vec<_>>();
            let host = if owners.len() == 1 {
                proc_by_pid.get(&owners[0]).copied().cloned()
            } else {
                None
            };
            let mut descendants: HashSet<u32> = owners.iter().copied().collect();
            for _ in 0..6 {
                for p in &processes {
                    if p["parentPid"]
                        .as_u64()
                        .map(|id| descendants.contains(&(id as u32)))
                        .unwrap_or(false)
                    {
                        descendants.insert(p["pid"].as_u64().unwrap_or(0) as u32);
                    }
                }
            }
            let child_processes = processes
                .iter()
                .filter(|p| {
                    p["pid"]
                        .as_u64()
                        .map(|id| {
                            descendants.contains(&(id as u32)) && !owners.contains(&(id as u32))
                        })
                        .unwrap_or(false)
                })
                .cloned()
                .collect::<Vec<_>>();
            let child_busy = child_processes
                .iter()
                .any(|p| p["cpu"].as_f64().unwrap_or(0.0) > 0.2);
            let token_total = row["tokens_used"].as_i64();
            let fingerprint = format!(
                "{}:{}:{}:{}",
                token_total.unwrap_or(-1),
                last_activity,
                turn.as_ref().map(|t| t.status.as_str()).unwrap_or(""),
                current_tool.as_ref().map(|t| t.id.as_str()).unwrap_or("")
            );
            let observation = self.observations.entry(id.clone()).or_default();
            if observation.fingerprint != fingerprint {
                observation.stable_since = at;
                observation.fingerprint = fingerprint.clone();
            }
            if let Some(h) = &host {
                observation.owner = Some((
                    h["pid"].as_u64().unwrap_or(0) as u32,
                    h["startedAt"].as_u64().unwrap_or(0),
                ));
                observation.lost_at = None;
            } else if let Some((pid, start)) = observation.owner {
                let same = proc_by_pid
                    .get(&pid)
                    .map(|p| p["startedAt"].as_u64() == Some(start))
                    .unwrap_or(false);
                if !same && observation.active && self.lock_errors.is_empty() {
                    observation.lost_at.get_or_insert(at);
                }
            }
            let transport_closed = !sessions.is_empty()
                && sessions.iter().all(|s| s["status"] == "closed")
                && turn
                    .as_ref()
                    .map(|t| {
                        sessions
                            .iter()
                            .filter_map(|s| s["closedAt"].as_i64())
                            .max()
                            .unwrap_or(0)
                            >= t.start_ms
                    })
                    .unwrap_or(false);
            let (status, reasons) = classify(&Evidence {
                turn: turn.as_ref(),
                tool: current_tool.as_ref(),
                now: at,
                last_activity,
                stable_ms: at - observation.stable_since,
                lost_observed_owner: observation.lost_at.map(|t| at - t >= 5000).unwrap_or(false),
                child_busy,
                readable: turn.is_some() && (projected.contains_key(&id) || roll.error.is_none()),
                transport_closed,
            });
            observation.active = turn.as_ref().map(|t| active(&t.status)).unwrap_or(false);
            let cwd = text(row, "cwd").replace("\\\\?\\", "");
            let project = project(&cwd);
            let model = roll
                .model
                .as_ref()
                .filter(|s| !s.is_empty())
                .cloned()
                .or_else(|| row["model"].as_str().map(str::to_owned));
            let effort = roll
                .effort
                .clone()
                .or_else(|| row["reasoning_effort"].as_str().map(str::to_owned));
            if let Some(tokens) = token_total {
                storage::sample(
                    &tx,
                    &id,
                    tokens,
                    at,
                    model.as_deref().unwrap_or("unknown"),
                    &project,
                )?;
            }
            if self.persisted.get(&id) != Some(&fingerprint) {
                storage::save_events(&tx, &id, &events)?;
                if let Some(t) = &turn {
                    storage::save_events(
                        &tx,
                        &id,
                        &[Event {
                            id: format!("turn:{}", t.id),
                            kind: "turn".into(),
                            name: "Codex turn".into(),
                            status: t.status.clone(),
                            start_ms: t.start_ms,
                            end_ms: t.end_ms,
                            duration_ms: t.duration_ms,
                            turn_id: Some(t.id.clone()),
                            source: t.source.clone(),
                            ..Default::default()
                        }],
                    )?;
                }
                self.persisted.insert(id.clone(), fingerprint);
            }
            let task = links
                .get(&id)
                .and_then(|tid| tasks.iter().find(|t| t["id"].as_str() == Some(tid)))
                .cloned();
            let source = if !sessions.is_empty() {
                "AgentDock ACP".into()
            } else if parents.contains_key(&id) {
                "Codex subagent".into()
            } else {
                let s = text(row, "source");
                if s.starts_with('{') {
                    "Codex local".into()
                } else {
                    format!("Codex 路 {s}")
                }
            };
            let archived_at_ms = archived_at.get(&id).copied();
            let is_archived = archived_at_ms.map(|t| t >= last_activity).unwrap_or(false);
            let mut agent = json!({"id":id,"title":title(row),"source":source,"project":project,"cwd":cwd,"acpSessions":sessions,
                "parentThreadId":parents.get(&id),"childThreadIds":parents.iter().filter(|(_,p)|**p==id).map(|(c,_)|c).collect::<Vec<_>>(),
                "binding":thread_bindings.get(&id),"attributionConflict":conflicts.contains(&id),"model":model,"threadModel":row["model"],"modelProvider":row["model_provider"],"reasoningEffort":effort,"tokensUsed":token_total,
                "tokenSource":"Codex threads.tokens_used (cumulative)","modelSource":if roll.model.is_some(){"rollout turn_context"}else{"Codex threads.model"},
                "status":status,"statusEvidence":reasons,"statusConfidence":if matches!(status,"COMPLETED"|"FAILED"|"IDLE"|"WAITING_TOOL"|"WAITING_USER"){"observed"}else{"inferred / limited"},
                "turn":turn,"currentTool":current_tool,"lastTool":last_tool,"lastCommand":last_command,"lastFile":last_file,
                "lastActivityMs":last_activity,"createdAt":created(row),"updatedAt":updated(row),"hostPid":host.as_ref().and_then(|p|p["pid"].as_u64()),"hostPids":owners,"hostProcess":host,"childProcesses":child_processes,
                "processAttribution":"Windows thread-writer-lock owner; child processes are host-scoped, not assigned to an individual logical thread",
                "task":task,"taskAssociation":if links.contains_key(&id){"explicit"}else{"none"},"rolloutPartial":roll.partial,"rolloutCatchingUp":roll.catching_up,"sourceWarning":roll.error,"agentRole":row["agent_role"],"agentNickname":row["agent_nickname"]});
            agent["archived"] = json!(is_archived);
            agent["archivedAt"] = if is_archived { json!(archived_at_ms) } else { Value::Null };
            agents.push(agent);
        }
        // Preserve orphaned ACP records instead of silently hiding missing logical threads.
        for (tid, sessions) in &acp_by_thread {
            if seen.contains(tid) {
                continue;
            }
            let a = &sessions[0];
            let id = if tid.is_empty(){format!("acp:{}",text(a,"id"))}else{tid.clone()};
            let last_activity = a["updatedAt"].as_i64().unwrap_or(0);
            let archived_at_ms = archived_at.get(&id).copied();
            let is_archived = archived_at_ms.map(|t| t >= last_activity).unwrap_or(false);
            agents.push(json!({"id":id,"title":"Unresolved ACP thread","source":"AgentDock ACP","project":project(&text(a,"cwd")),"cwd":a["cwd"],"acpSessions":sessions,"status":"UNKNOWN","statusEvidence":["ACP exists but its remote_session_id is missing from readable Codex metadata"],"archived":is_archived,"archivedAt":if is_archived{archived_at_ms}else{None},"lastActivityMs":last_activity,"tokensUsed":null,"model":null,"binding":null,"childThreadIds":[],"childProcesses":[],"hostPids":[]}));
        }
        tx.commit().map_err(|e| e.to_string())?;
        let mut monitor_tasks = storage::monitor_tasks(&own)?;
        for task in &mut monitor_tasks {
            let mut linked = Vec::<String>::new();
            let mut projects = Vec::<String>::new();
            let mut latest = task["updatedAt"].as_i64().unwrap_or(0);
            let mut any_live = false;
            for member in task["members"].as_array().into_iter().flatten() {
                let entity = member["entityId"].as_str().unwrap_or_default();
                if let Some(agent) = agents.iter().find(|a| {
                    a["id"].as_str() == Some(entity)
                        || a["acpSessions"]
                            .as_array()
                            .map(|s| s.iter().any(|x| x["id"].as_str() == Some(entity)))
                            .unwrap_or(false)
                }) {
                    if let Some(id) = agent["id"].as_str() {
                        if !linked.iter().any(|x| x == id) {
                            linked.push(id.to_owned());
                        }
                    }
                    if let Some(p) = agent["project"].as_str().filter(|p| !p.is_empty()) {
                        if !projects.iter().any(|x| x == p) {
                            projects.push(p.to_owned());
                        }
                    }
                    latest = latest.max(agent["lastActivityMs"].as_i64().unwrap_or(0));
                    let status = agent["status"].as_str().unwrap_or("");
                    if !agent["archived"].as_bool().unwrap_or(false)
                        && !matches!(status, "COMPLETED" | "FAILED" | "CRASHED" | "IDLE")
                    {
                        any_live = true;
                    }
                }
            }
            task["agentIds"] = json!(linked);
            task["projects"] = json!(projects);
            task["project"] = json!(task["projects"].as_array().and_then(|p| p.first()).and_then(|p| p.as_str()).unwrap_or("unknown"));
            task["updatedAt"] = json!(latest);
            task["status"] = json!(if any_live { "active" } else { "completed" });
            task["completedAt"] = if any_live { Value::Null } else { json!(latest) };
        }
        let mut requests = storage::chat_requests(&own)?;
        for request in &mut requests {
            let mut linked_agents = Vec::<String>::new();
            let mut project_name = request["projectHint"].as_str().filter(|s| !s.is_empty()).map(project);
            let mut last_activity = request["updatedAt"].as_i64().unwrap_or(0);
            let mut execution_live = false;
            let mut task_ids = Vec::<String>::new();
            if let Some(tid) = request["agentdockTaskId"].as_str().filter(|s| s.starts_with("tsk_")) { task_ids.push(tid.to_owned()); }
            for tool in request["tools"].as_array().into_iter().flatten() {
                if let Some(tid) = tool["taskId"].as_str().filter(|s| s.starts_with("tsk_")) { if !task_ids.iter().any(|x| x==tid) { task_ids.push(tid.to_owned()); } }
            }
            if let Some(first) = task_ids.first() { request["agentdockTaskId"] = json!(first); }
            let mut plans = Vec::<Value>::new();
            let mut completed_steps = 0usize;
            let mut total_steps = 0usize;
            let mut current_step: Option<String> = None;
            for tid in &task_ids {
                if let Some(task) = tasks.iter().find(|t| t["id"].as_str() == Some(tid.as_str())) {
                    let mut linked = task.clone();
                    let observations = request["tools"].as_array();
                    let created = observations.map(|tools| tools.iter().any(|tool| tool["taskId"].as_str() == Some(tid.as_str()) && tool["action"].as_str() == Some("create"))).unwrap_or(false);
                    let checkpointed = observations.map(|tools| tools.iter().any(|tool| tool["taskId"].as_str() == Some(tid.as_str()) && tool["action"].as_str() == Some("checkpoint"))).unwrap_or(false);
                    linked["requestRelation"] = json!(if created {"created"} else if checkpointed {"checkpointed"} else {"observed"});
                    plans.push(linked);
                    if project_name.is_none() { project_name = task["project"].as_str().filter(|p| !p.is_empty()).map(str::to_owned); }
                    last_activity = last_activity.max(task["updatedAt"].as_i64().unwrap_or(0));
                    execution_live |= task["status"].as_str() == Some("active");
                    let steps = task["steps"].as_array().cloned().unwrap_or_default();
                    completed_steps += steps.iter().filter(|s| s["status"].as_str() == Some("completed")).count();
                    total_steps += steps.len();
                    if let Some(step) = steps.iter().find(|s| s["status"].as_str() == Some("in_progress")).and_then(|s| s["title"].as_str()) { current_step = Some(step.to_owned()); }
                }
            }
            request["plans"] = json!(plans);
            if let Some(first) = request["plans"].as_array().and_then(|x| x.first()).cloned() { request["plan"] = first; }
            request["progress"] = json!({"completed":completed_steps,"total":total_steps,"currentStep":current_step,"revisionCount":request["plans"].as_array().map(|x|x.len().saturating_sub(1)).unwrap_or(0)});
            for tool in request["tools"].as_array().into_iter().flatten() {
                execution_live |= tool["status"].as_str() == Some("running");
                last_activity = last_activity.max(tool["finishedAt"].as_i64().unwrap_or_else(|| tool["startedAt"].as_i64().unwrap_or(0)));
                if project_name.is_none() { project_name = tool["projectHint"].as_str().filter(|s| !s.is_empty()).map(project); }
                if let Some(entity) = tool["entityId"].as_str() {
                    if let Some(agent) = agents.iter().find(|a| a["id"].as_str() == Some(entity) || a["acpSessions"].as_array().map(|ss| ss.iter().any(|x| x["id"].as_str() == Some(entity))).unwrap_or(false)) {
                        if let Some(id) = agent["id"].as_str() { if !linked_agents.iter().any(|x| x == id) { linked_agents.push(id.to_owned()); } }
                        if project_name.is_none() { project_name = agent["project"].as_str().filter(|p| !p.is_empty()).map(str::to_owned); }
                        last_activity = last_activity.max(agent["lastActivityMs"].as_i64().unwrap_or(0));
                        let st = agent["status"].as_str().unwrap_or("");
                        execution_live |= !agent["archived"].as_bool().unwrap_or(false) && !matches!(st,"COMPLETED"|"FAILED"|"CRASHED"|"IDLE");
                    }
                }
            }
            let cid = request["conversationId"].as_str().unwrap_or_default();
            let mid = request["userMessageId"].as_str().unwrap_or_default();
            for mt in &monitor_tasks {
                if mt["conversationId"].as_str() != Some(cid) { continue; }
                for member in mt["members"].as_array().into_iter().flatten().filter(|m| m["userMessageId"].as_str() == Some(mid)) {
                    if let Some(entity) = member["entityId"].as_str() {
                        if let Some(agent) = agents.iter().find(|a| a["id"].as_str() == Some(entity) || a["acpSessions"].as_array().map(|ss| ss.iter().any(|x| x["id"].as_str() == Some(entity))).unwrap_or(false)) {
                            if let Some(id) = agent["id"].as_str() { if !linked_agents.iter().any(|x| x == id) { linked_agents.push(id.to_owned()); } }
                            if project_name.is_none() { project_name = agent["project"].as_str().filter(|p| !p.is_empty()).map(str::to_owned); }
                            last_activity = last_activity.max(agent["lastActivityMs"].as_i64().unwrap_or(0));
                            let st = agent["status"].as_str().unwrap_or("");
                            execution_live |= !agent["archived"].as_bool().unwrap_or(false) && !matches!(st,"COMPLETED"|"FAILED"|"CRASHED"|"IDLE");
                        }
                    }
                }
            }
            request["agentIds"] = json!(linked_agents);
            request["project"] = json!(project_name.unwrap_or_else(|| "unknown".into()));
            request["lastActivityMs"] = json!(last_activity);
            let response_done = request["completedAt"].as_i64().is_some();
            request["status"] = json!(if execution_live {"active"} else if response_done {"completed"} else {"responding"});
            let has_execution = request["tools"].as_array().map(|x| !x.is_empty()).unwrap_or(false) || !request["agentIds"].as_array().unwrap_or(&Vec::new()).is_empty();
            let has_plan = request["plans"].as_array().map(|x| !x.is_empty()).unwrap_or(false);
            request["progressProtocol"] = json!(if has_plan {"claimed"} else if has_execution {"unclaimed"} else {"not-needed"});
        }
        let mut conversation_projects = HashMap::<String,String>::new();
        for request in &requests {
            if let (Some(cid), Some(p)) = (request["conversationId"].as_str(), request["project"].as_str().filter(|p| *p != "unknown")) {
                conversation_projects.entry(cid.to_owned()).or_insert_with(|| p.to_owned());
            }
        }
        for request in &mut requests {
            if request["project"].as_str() == Some("unknown") {
                if let Some(p) = request["conversationId"].as_str().and_then(|cid| conversation_projects.get(cid)) { request["project"] = json!(p); }
            }
        }
        let analytics = storage::analytics(&own, 1);
        fn rank(s: &str) -> u8 {
            match s {
                "CRASHED" | "FAILED" => 0,
                "SUSPECTED_STALLED" => 1,
                "RUNNING" | "WAITING_MODEL" => 2,
                "WAITING_TOOL" | "WAITING_USER" => 3,
                "UNKNOWN" => 4,
                _ => 5,
            }
        }
        agents.sort_by(|a, b| {
            a["archived"]
                .as_bool()
                .unwrap_or(false)
                .cmp(&b["archived"].as_bool().unwrap_or(false))
                .then(rank(a["status"].as_str().unwrap_or("")).cmp(&rank(
                    b["status"].as_str().unwrap_or(""),
                )))
                .then(
                    b["lastActivityMs"]
                        .as_i64()
                        .cmp(&a["lastActivityMs"].as_i64()),
                )
        });
        let count = |statuses: &[&str]| {
            agents
                .iter()
                .filter(|a| {
                    !a["archived"].as_bool().unwrap_or(false)
                        && statuses.contains(&a["status"].as_str().unwrap_or(""))
                })
                .count()
        };
        let stats = json!({"active":count(&["RUNNING","WAITING_MODEL"]),"waiting":count(&["WAITING_TOOL","WAITING_USER"]),"stalled":count(&["SUSPECTED_STALLED"]),"failed":count(&["FAILED","CRASHED"]),"completed":count(&["COMPLETED"]),"observedTokensToday":analytics["observedTokens"],"unassignedAcp":agents.iter().filter(|a|!a["archived"].as_bool().unwrap_or(false)&&a["source"]=="AgentDock ACP"&&a["binding"].is_null()).count()});
        let policy_file = sources::home().join(".agentdock/policies/global-execution.md");
        let policy_state = sources::home().join(".agentdock/skill-store/state/execution-progress.json");
        let policy_version = std::fs::read_to_string(&policy_state).ok().and_then(|s|serde_json::from_str::<Value>(&s).ok()).and_then(|v|v["active_version"].as_str().map(str::to_owned));
        let progress_policy = json!({"healthy":policy_file.is_file() && policy_version.is_some(),"path":policy_file,"activeVersion":policy_version,"indexedSkill":"execution-progress"});
        Ok(
            json!({"version":env!("CARGO_PKG_VERSION"),"generatedAt":at,"collectionMs":started.elapsed().as_millis(),"stale":false,"agents":agents,"processes":processes,"tasks":tasks,"monitorTasks":monitor_tasks,"requests":requests,"bindings":bindings,"stats":stats,"analyticsToday":analytics,
            "sources":{"codex":{"healthy":true,"path":state_path,"threadCount":total_threads,"loadedThreads":seen.len(),"readOnly":true},"history":{"healthy":projection_error.is_none(),"path":history_path,"error":projection_error,"projectedThreadCount":projected.len(),"itemReadErrors":item_read_errors},"acp":{"healthy":acp_error.is_none(),"count":acp.len(),"error":acp_error},"locks":{"healthy":self.lock_errors.is_empty(),"mappedThreads":self.locks.values().filter(|v|!v.is_empty()).count(),"errors":self.lock_errors},"agentdock":os::agentdock_health(&sources::home().join("AppData/Local/AgentDock/runtime.json")),"progressPolicy":progress_policy},
            "settings":{"dataDir":self.data,"codexHome":self.codex,"agentdockHome":self.agentdock,"refreshMs":2500},"coverage":"All known ACP records + recent local threads + active projected turns + lock owners and exact ancestors. First rollout tail is bounded to 1 MiB; historical windows may be partial."}),
        )
    }
}

pub fn detail(data: &Path, codex: &Path, id: &str) -> Result<Value, String> {
    if uuid::Uuid::parse_str(id).is_err() {
        return Ok(json!({"events":[],"partial":true,"notice":"No resolved Codex thread"}));
    }
    let own = storage::open(&data.join("monitor.sqlite"))?;
    let mut events = storage::saved_events(&own, id)?;
    let path = sources::db_path(codex, "thread_history", "thread_history_1.sqlite");
    let mut projection = false;
    let mut warning = None;
    if let Ok(c) = sources::ro(&path) {
        match sources::items(&c, id, 4000) {
            Ok(rows) => {
                projection = !rows.is_empty();
                if projection {
                    events.retain(|e| {
                        e.kind == "turn" || e.kind == "tokens" || e.source == "thread_items"
                    });
                }
                events.extend(rows)
            }
            Err(e) => warning = Some(e),
        }
    }
    let mut by_id = HashMap::new();
    for e in events {
        by_id.insert(e.id.clone(), e);
    }
    let mut events = by_id.into_values().collect::<Vec<_>>();
    events.sort_by_key(|e| e.start_ms);
    Ok(
        json!({"events":events,"projectionAvailable":projection,"partial":true,"notice":"Projection is limited to the latest 4,000 items; rollout-only history begins at the initial tail or first observation. Instantaneous activity is not shown as an invented duration.","warning":warning}),
    )
}
