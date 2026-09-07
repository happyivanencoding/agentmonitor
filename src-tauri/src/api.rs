//! Operations shared by the Tauri IPC and authenticated web transport.
use crate::{bridge, collector, storage, Shared};
use serde_json::{json, Value};

pub fn snapshot(state: &Shared) -> Value {
    state.snapshot.lock().map(|v|v.clone()).unwrap_or_else(|_|json!({"stale":true,"collectorError":"Snapshot unavailable","agents":[],"processes":[],"tasks":[],"bindings":[]}))
}
pub fn detail(state: &Shared, id: &str) -> Result<Value, String> {
    if !bridge::entity_known(state, id) {
        return Err("Unknown logical thread".into());
    }
    collector::detail(&state.data, &state.codex, id)
}
pub fn analytics(state: &Shared, days: i64) -> Result<Value, String> {
    let c = storage::open(&state.data.join("monitor.sqlite"))?;
    Ok(storage::analytics(&c, days.clamp(1, 30)))
}
pub fn bind(state: &Shared, input: &storage::BindingInput, source: &str) -> Result<Value, String> {
    if !bridge::entity_known(state, &input.entity_id) {
        return Err("Select a known ACP or logical thread".into());
    }
    storage::bind(&state.data.join("monitor.sqlite"), input, source)
}
pub fn unbind(state: &Shared, entity_id: &str) -> Result<Value, String> {
    let c = storage::open(&state.data.join("monitor.sqlite"))?;
    c.execute("DELETE FROM bindings WHERE entity_id=?", [entity_id])
        .map_err(|e| e.to_string())?;
    Ok(Value::Null)
}
pub fn set_archived(state: &Shared, id: &str, archived: bool) -> Result<Value, String> {
    if !bridge::entity_known(state, id) {
        return Err("Unknown logical thread".into());
    }
    let at = storage::set_archived(&state.data.join("monitor.sqlite"), id, archived)?;
    Ok(json!({"ok":true,"id":id,"archived":archived,"updatedAt":at}))
}

pub fn link_task(state: &Shared, thread_id: &str, task_id: Option<&str>) -> Result<Value, String> {
    if !bridge::entity_known(state, thread_id) {
        return Err("Unknown thread".into());
    }
    let c = storage::open(&state.data.join("monitor.sqlite"))?;
    if let Some(task) = task_id {
        let valid = state
            .snapshot
            .lock()
            .ok()
            .and_then(|s| {
                s["tasks"]
                    .as_array()
                    .map(|a| a.iter().any(|t| t["id"].as_str() == Some(task)))
            })
            .unwrap_or(false);
        if !valid {
            return Err("Unknown task".into());
        }
        c.execute("INSERT INTO task_links VALUES(?,?) ON CONFLICT(thread_id) DO UPDATE SET task_id=excluded.task_id",rusqlite::params![thread_id,task]).map_err(|e|e.to_string())?;
    } else {
        c.execute("DELETE FROM task_links WHERE thread_id=?", [thread_id])
            .map_err(|e| e.to_string())?;
    }
    Ok(Value::Null)
}
