//! Cheap native Task refresh, independent of the slow process/token collector.
use crate::{model, sources, storage, Shared};
use serde_json::{json, Value};
use std::{collections::{HashMap, HashSet}, path::PathBuf, time::Duration};
use tauri::Emitter;

pub fn apply(snapshot: &mut Value, tasks: &[Value], at: i64) {
    let by_id: HashMap<&str, &Value> = tasks.iter().filter_map(|t| t["id"].as_str().map(|id| (id, t))).collect();
    let live_agents: HashSet<String> = snapshot["agents"].as_array().into_iter().flatten()
        .filter(|a| !a["archived"].as_bool().unwrap_or(false) && !matches!(a["status"].as_str().unwrap_or(""), "COMPLETED"|"FAILED"|"CRASHED"|"IDLE"))
        .filter_map(|a| a["id"].as_str().map(str::to_owned)).collect();
    if let Some(requests) = snapshot["requests"].as_array_mut() {
        for request in requests {
            // Only refresh already-proven Task references. No title, project or temporal inference.
            let mut ids: Vec<String> = request["plans"].as_array().into_iter().flatten()
                .filter_map(|p| p["id"].as_str().map(str::to_owned)).collect();
            for tool in request["tools"].as_array().into_iter().flatten() {
                if let Some(id) = tool["taskId"].as_str().filter(|id| !id.is_empty()) {
                    if !ids.iter().any(|x| x == id) { ids.push(id.to_owned()); }
                }
            }
            if let Some(id) = request["agentdockTaskId"].as_str() {
                if !ids.iter().any(|x| x == id) { ids.push(id.to_owned()); }
            }
            let tools = request["tools"].as_array().cloned().unwrap_or_default();
            let mut plans = Vec::new();
            let mut total = 0usize; let mut completed = 0usize; let mut current = None;
            let mut active = tools.iter().any(|t| t["status"] == "running") || request["agentIds"].as_array().into_iter().flatten().filter_map(Value::as_str).any(|id| live_agents.contains(id));
            for id in ids {
                let Some(task) = by_id.get(id.as_str()) else { continue; };
                let mut plan = (*task).clone();
                let relation = if tools.iter().any(|t| t["taskId"] == id && t["action"] == "create") { "created" }
                    else if tools.iter().any(|t| t["taskId"] == id && t["action"] == "checkpoint") { "checkpointed" } else { "observed" };
                plan["requestRelation"] = json!(relation);
                active |= plan["status"] == "active";
                request["lastActivityMs"] = json!(request["lastActivityMs"].as_i64().unwrap_or(0).max(plan["updatedAt"].as_i64().unwrap_or(0)));
                for step in plan["steps"].as_array().into_iter().flatten() {
                    total += 1; completed += usize::from(step["status"] == "completed");
                    if step["status"] == "in_progress" { current = step["title"].as_str().map(str::to_owned); }
                }
                plans.push(plan);
            }
            request["plan"] = plans.first().cloned().unwrap_or(Value::Null);
            request["progress"] = json!({"completed":completed,"total":total,"currentStep":current,"revisionCount":plans.len().saturating_sub(1)});
            request["progressProtocol"] = json!(if !plans.is_empty() {"claimed"} else if !tools.is_empty() {"unclaimed"} else {"not-needed"});
            request["plans"] = json!(plans);
            request["status"] = json!(if active { "active" } else if request["completedAt"].as_i64().is_some() { "completed" } else { "responding" });
        }
    }
    if let Some(agents) = snapshot["agents"].as_array_mut() {
        for agent in agents {
            if let Some(task) = agent["task"]["id"].as_str().and_then(|id| by_id.get(id)) { agent["task"] = (*task).clone(); }
        }
    }
    snapshot["tasks"] = json!(tasks);
    snapshot["taskProgressAt"] = json!(at);
    snapshot["sources"]["taskProgress"] = json!({"healthy":true,"updatedAt":at,"refreshMs":2000});
    // Expose local plans on cold start without pretending Agent/token collection has finished.
    if snapshot["generatedAt"].is_null() {
        snapshot["version"] = json!(env!("CARGO_PKG_VERSION"));
        snapshot["loading"] = json!(false);
        snapshot["metricsLoading"] = json!(true);
    }
}

/// Merge fresh Monitor-owned records without erasing slow collector Agent joins.
fn merge_requests(snapshot: &mut Value, fresh: &[Value]) {
    let old: HashMap<String, Value> = snapshot["requests"].as_array().into_iter().flatten()
        .filter_map(|r| r["id"].as_str().map(|id| (id.to_owned(),r.clone()))).collect();
    let mut out = Vec::new();
    for record in fresh {
        let Some(id) = record["id"].as_str() else { continue; };
        let mut value = old.get(id).cloned().unwrap_or_else(|| json!({"project":"unknown","agentIds":[],"plans":[]}));
        for field in ["id","conversationId","userMessageId","conversationTitle","prompt","startedAt","completedAt","updatedAt","agentdockTaskId","projectHint","tools"] {
            if let Some(data) = record.get(field) { value[field] = data.clone(); }
        }
        if value["project"] == "unknown" {
            if let Some(hint) = value["projectHint"].as_str().filter(|s| !s.is_empty()) { value["project"] = json!(hint); }
        }
        value["lastActivityMs"] = json!(value["lastActivityMs"].as_i64().unwrap_or(0).max(value["updatedAt"].as_i64().unwrap_or(0)));
        out.push(value);
    }
    snapshot["requests"] = json!(out);
}

pub fn preserve_latest(incoming: &mut Value, current: &Value) {
    if current["taskProgressAt"].as_i64().unwrap_or(0) > incoming["taskProgressAt"].as_i64().unwrap_or(0) {
        if let Some(requests) = current["requests"].as_array() { merge_requests(incoming, requests); }
        if let Some(tasks) = current["tasks"].as_array() { apply(incoming, tasks, current["taskProgressAt"].as_i64().unwrap_or(0)); }
    }
}

pub fn start(state: Shared, app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let root = std::env::var_os("AGENT_MONITOR_AGENTDOCK_HOME").map(PathBuf::from)
            .unwrap_or_else(|| sources::home().join(".agentdock")).join("tasks");
        let mut cache = sources::JsonCache::default();
        loop {
            if root.is_dir() {
                let tasks = cache.tasks(&root);
                let requests = sources::ro(&state.data.join("monitor.sqlite")).and_then(|c| storage::chat_requests(&c));
                let published = if let Ok(mut snapshot) = state.snapshot.lock() {
                    match requests {
                        Ok(ref rows) => { merge_requests(&mut snapshot, rows); snapshot["sources"]["requestProgress"] = json!({"healthy":true}); }
                        Err(ref error) => { snapshot["sources"]["requestProgress"] = json!({"healthy":false,"error":error}); }
                    }
                    apply(&mut snapshot, &tasks, model::now_ms());
                    snapshot["sources"]["bridge"] = state.bridge_status.lock().map(|s| s.clone()).unwrap_or(Value::Null);
                    Some(snapshot.clone())
                } else { None };
                if let Some(snapshot) = published { let _ = app.emit("monitor:snapshot", &snapshot); }
            }
            std::thread::sleep(Duration::from_millis(2000));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn task(status: &str, step: &str) -> Value { json!({"id":"tsk_fixture","status":status,"updatedAt":200,"steps":[{"id":"s","title":"Real step","status":step}]}) }
    fn snapshot() -> Value { json!({"generatedAt":10,"stats":{"observedTokensToday":42},"agents":[],"tasks":[],"requests":[{"id":"r","completedAt":20,"tools":[{"taskId":"tsk_fixture","action":"checkpoint","status":"completed"}],"plans":[],"agentIds":[]},{"id":"unrelated","tools":[],"plans":[]}]}) }
    #[test]
    fn checkpoint_refreshes_only_the_exact_request_and_preserves_metrics_age() {
        let mut s=snapshot();apply(&mut s,&[task("active","in_progress")],300);
        assert_eq!(s["requests"][0]["progress"]["currentStep"],"Real step");
        assert_eq!(s["requests"][0]["plans"][0]["requestRelation"],"checkpointed");
        assert_eq!(s["requests"][1]["plans"],json!([]));assert_eq!(s["generatedAt"],10);assert_eq!(s["stats"]["observedTokensToday"],42);
    }
    #[test]
    fn completed_task_closes_request_but_blocked_step_is_not_completed() {
        let mut s=snapshot();apply(&mut s,&[task("blocked","pending")],100);
        assert_eq!(s["tasks"][0]["status"],"blocked");assert_eq!(s["requests"][0]["progress"]["completed"],0);
        apply(&mut s,&[task("completed","completed")],200);
        assert_eq!(s["requests"][0]["progress"]["completed"],1);assert_eq!(s["requests"][0]["status"],"completed");
    }
    #[test]
    fn slow_collector_cannot_regress_newer_step_status() {
        let mut old=snapshot();apply(&mut old,&[task("active","in_progress")],100);
        let mut current=snapshot();apply(&mut current,&[task("completed","completed")],300);
        preserve_latest(&mut old,&current);assert_eq!(old["tasks"][0]["status"],"completed");assert_eq!(old["taskProgressAt"],300);
    }
    #[test]
    fn cold_start_exposes_plans_without_claiming_metrics_ready() {
        let mut s=json!({"loading":true,"agents":[],"tasks":[]});apply(&mut s,&[task("active","pending")],100);
        assert_eq!(s["loading"],false);assert_eq!(s["metricsLoading"],true);assert!(s["generatedAt"].is_null());
    }
    #[test]
    fn new_request_and_tool_enter_fast_snapshot_without_waiting_for_agent_scan() {
        let mut s=snapshot();let raw=json!({"id":"new","conversationId":"conversation-fixture","agentdockTaskId":"tsk_fixture","tools":[{"taskId":"tsk_fixture","action":"checkpoint","status":"completed"}],"completedAt":30,"updatedAt":40});
        merge_requests(&mut s,&[raw]);apply(&mut s,&[task("completed","completed")],300);
        assert_eq!(s["requests"][0]["agentIds"],json!([]));assert_eq!(s["requests"][0]["progress"]["completed"],1);assert_eq!(s["requests"][0]["status"],"completed");assert_eq!(s["generatedAt"],10);
    }

}
