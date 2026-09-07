//! Bounded, incremental JSONL reader. It never retains message/reasoning/output bodies.
use crate::model::*;
use serde_json::Value;
use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
    time::SystemTime,
};
const INITIAL_TAIL: u64 = 1024 * 1024;
const READ_BUDGET: u64 = 2 * 1024 * 1024;

#[derive(Clone, Default)]
pub struct RolloutInfo {
    pub turn: Option<Turn>,
    pub events: Vec<Event>,
    pub last_activity: i64,
    pub token_total: Option<i64>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub partial: bool,
    pub catching_up: bool,
    pub error: Option<String>,
}
#[derive(Default)]
struct Cursor {
    offset: u64,
    mtime: Option<SystemTime>,
    carry: Vec<u8>,
    info: RolloutInfo,
}
#[derive(Default)]
pub struct Reader {
    files: HashMap<String, Cursor>,
}
impl Reader {
    pub fn read(&mut self, path: &str) -> RolloutInfo {
        if path.is_empty() {
            return RolloutInfo::default();
        }
        let c = self.files.entry(path.into()).or_default();
        let result = (|| -> std::io::Result<()> {
            let mut f = File::open(Path::new(path))?;
            let meta = f.metadata()?;
            let len = meta.len();
            let mtime = meta.modified().ok();
            if len < c.offset || (len == c.offset && c.mtime.is_some() && mtime != c.mtime) {
                *c = Cursor::default();
            }
            if c.offset == len && c.mtime == mtime {
                return Ok(());
            }
            let mut skip = false;
            if c.offset == 0 && len > INITIAL_TAIL {
                c.offset = len - INITIAL_TAIL;
                c.info.partial = true;
                skip = true;
            }
            f.seek(SeekFrom::Start(c.offset))?;
            let mut buf = Vec::new();
            f.take(READ_BUDGET).read_to_end(&mut buf)?;
            c.offset += buf.len() as u64;
            c.mtime = mtime;
            if skip {
                if let Some(i) = buf.iter().position(|x| *x == b'\n') {
                    buf.drain(..=i);
                } else {
                    buf.clear();
                }
            }
            c.carry.extend_from_slice(&buf);
            if let Some(last) = c.carry.iter().rposition(|x| *x == b'\n') {
                let complete = c.carry.drain(..=last).collect::<Vec<_>>();
                for line in complete.split(|x| *x == b'\n') {
                    if let Ok(v) = serde_json::from_slice::<Value>(line) {
                        ingest(&mut c.info, &v);
                    }
                }
            }
            // Giant truncated records cannot grow memory without bound. Skip until a newline on next read.
            if c.carry.len() > 8 * 1024 * 1024 {
                c.carry.clear();
                c.info.partial = true;
            }
            c.info.catching_up = c.offset < len;
            c.info.error = None;
            if c.info.events.len() > 600 {
                let n = c.info.events.len() - 600;
                c.info.events.drain(..n);
            }
            Ok(())
        })();
        if let Err(e) = result {
            c.info.error = Some(format!("Rollout read: {}", e.kind()));
        }
        c.info.clone()
    }
}
fn upsert(info: &mut RolloutInfo, event: Event) {
    if let Some(old) = info.events.iter_mut().rev().find(|e| e.id == event.id) {
        if event.end_ms.is_some() {
            old.end_ms = event.end_ms;
            old.duration_ms = event
                .duration_ms
                .or_else(|| event.end_ms.map(|e| (e - old.start_ms).max(0)));
            old.status = event.status;
        } else if event.start_ms < old.start_ms {
            *old = event;
        }
    } else {
        info.events.push(event);
    }
}
fn finish(info: &mut RolloutInfo, id: &str, at: i64, status: &str, duration: Option<i64>) {
    if let Some(e) = info.events.iter_mut().rev().find(|e| e.id == id) {
        e.end_ms = Some(at);
        e.duration_ms = duration.or(Some((at - e.start_ms).max(0)));
        e.status = status.into();
    }
}
pub fn ingest(info: &mut RolloutInfo, v: &Value) {
    let Some(at) = v.get("timestamp").and_then(millis) else {
        return;
    };
    let Some(p) = v.get("payload") else {
        return;
    };
    let outer = text(v, "type");
    let kind = text(p, "type");
    info.last_activity = info.last_activity.max(at);
    if outer == "turn_context" {
        info.model = p.get("model").and_then(Value::as_str).map(str::to_owned);
        info.effort = p
            .get("effort")
            .or_else(|| p.get("reasoning_effort"))
            .and_then(Value::as_str)
            .map(str::to_owned);
    }
    if outer == "event_msg" {
        match kind.as_str() {
            "task_started" => {
                let t = Turn {
                    id: text(p, "turn_id"),
                    status: "inProgress".into(),
                    start_ms: p.get("started_at").and_then(millis).unwrap_or(at),
                    source: "rollout".into(),
                    ..Default::default()
                };
                if info
                    .turn
                    .as_ref()
                    .map(|x| x.start_ms <= t.start_ms)
                    .unwrap_or(true)
                {
                    info.turn = Some(t);
                }
            }
            "task_complete" | "task_failed" | "turn_aborted" => {
                let id = text(p, "turn_id");
                let previous = info.turn.as_ref().filter(|t| t.id == id);
                let start = p
                    .get("started_at")
                    .and_then(millis)
                    .or_else(|| previous.map(|t| t.start_ms))
                    .unwrap_or(at);
                let end = p.get("completed_at").and_then(millis).unwrap_or(at);
                let status = match kind.as_str() {
                    "task_complete" => "completed",
                    "task_failed" => "failed",
                    _ => "interrupted",
                };
                let t = Turn {
                    id,
                    status: status.into(),
                    start_ms: start,
                    end_ms: Some(end),
                    duration_ms: p
                        .get("duration_ms")
                        .and_then(Value::as_i64)
                        .or_else(|| previous.map(|t| end - t.start_ms)),
                    error: p.get("error").map(|s| redact(&s.to_string())),
                    source: "rollout".into(),
                };
                if info
                    .turn
                    .as_ref()
                    .map(|x| x.start_ms <= end)
                    .unwrap_or(true)
                {
                    info.turn = Some(t);
                }
            }
            "token_count" => {
                if let Some(total) = p
                    .pointer("/info/total_token_usage/total_tokens")
                    .and_then(Value::as_i64)
                {
                    let delta = info
                        .token_total
                        .filter(|prev| total >= *prev)
                        .map(|prev| total - prev);
                    info.token_total = Some(total);
                    if delta.map(|d| d > 0).unwrap_or(false) {
                        info.events.push(Event {
                            id: format!("token:{at}:{total}"),
                            kind: "tokens".into(),
                            name: "token usage update".into(),
                            status: "observed".into(),
                            start_ms: at,
                            token_delta: delta,
                            turn_id: info.turn.as_ref().map(|t| t.id.clone()),
                            source: "rollout".into(),
                            ..Default::default()
                        });
                    }
                }
            }
            "exec_command_begin" | "mcp_tool_call_begin" | "apply_patch_begin" => {
                let id = text(p, "call_id");
                if id.is_empty() {
                    return;
                }
                let command = p.get("command").map(|v| {
                    if let Some(a) = v.as_array() {
                        a.iter()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join(" ")
                    } else {
                        v.as_str().unwrap_or("").into()
                    }
                });
                upsert(
                    info,
                    Event {
                        id,
                        kind: "tool".into(),
                        name: kind.trim_end_matches("_begin").into(),
                        status: "inProgress".into(),
                        start_ms: at,
                        command: command.map(|s| redact(&s)),
                        turn_id: info.turn.as_ref().map(|t| t.id.clone()),
                        source: "rollout".into(),
                        ..Default::default()
                    },
                );
            }
            "exec_command_end" | "mcp_tool_call_end" | "apply_patch_end" => {
                let failed = p
                    .get("exit_code")
                    .and_then(Value::as_i64)
                    .map(|n| n != 0)
                    .unwrap_or(false)
                    || p.get("success") == Some(&Value::Bool(false));
                finish(
                    info,
                    &text(p, "call_id"),
                    at,
                    if failed { "failed" } else { "completed" },
                    p.get("duration_ms").and_then(Value::as_i64),
                );
            }
            _ => {}
        }
    }
    if outer == "response_item" {
        match kind.as_str() {
            "function_call" | "custom_tool_call" => {
                let id = text(p, "call_id");
                if id.is_empty() {
                    return;
                }
                let args = p
                    .get("arguments")
                    .and_then(Value::as_str)
                    .and_then(|s| serde_json::from_str::<Value>(s).ok());
                let command = args
                    .as_ref()
                    .and_then(|a| a.get("cmd").or_else(|| a.get("command")))
                    .and_then(Value::as_str)
                    .map(redact);
                upsert(
                    info,
                    Event {
                        id,
                        kind: "tool".into(),
                        name: clipped(&text(p, "name"), 150),
                        status: "inProgress".into(),
                        start_ms: at,
                        command,
                        turn_id: info.turn.as_ref().map(|t| t.id.clone()),
                        source: "rollout".into(),
                        ..Default::default()
                    },
                );
            }
            "function_call_output" | "custom_tool_call_output" => {
                finish(info, &text(p, "call_id"), at, "returned", None)
            }
            "reasoning" => {
                // Metadata only. Do not access summary, encrypted_content or reasoning text.
                info.events.push(Event {
                    id: format!("activity:{at}"),
                    kind: "reasoning".into(),
                    name: "model activity".into(),
                    status: "observed".into(),
                    start_ms: at,
                    turn_id: info.turn.as_ref().map(|t| t.id.clone()),
                    source: "rollout".into(),
                    ..Default::default()
                });
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn lifecycle_without_projection() {
        let mut i = RolloutInfo::default();
        ingest(
            &mut i,
            &json!({"timestamp":"2026-09-06T10:00:00Z","type":"event_msg","payload":{"type":"task_started","turn_id":"t"}}),
        );
        assert_eq!(i.turn.as_ref().unwrap().status, "inProgress");
        ingest(
            &mut i,
            &json!({"timestamp":"2026-09-06T10:01:00Z","type":"event_msg","payload":{"type":"task_complete","turn_id":"t","duration_ms":60000}}),
        );
        assert_eq!(i.turn.unwrap().status, "completed");
    }
    #[test]
    fn first_cumulative_token_event_is_not_delta() {
        let mut i = RolloutInfo::default();
        let v = json!({"timestamp":"2026-09-06T10:00:00Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"total_tokens":72810}}}});
        ingest(&mut i, &v);
        assert!(i.events.is_empty());
        ingest(&mut i, &v);
        assert!(i.events.is_empty());
    }
    #[test]
    fn no_private_reasoning_retained() {
        let mut i = RolloutInfo::default();
        ingest(
            &mut i,
            &json!({"timestamp":"2026-09-06T10:00:00Z","type":"response_item","payload":{"type":"reasoning","summary":"PRIVATE","encrypted_content":"PRIVATE"}}),
        );
        assert!(!serde_json::to_string(&i.events)
            .unwrap()
            .contains("PRIVATE"));
    }
    #[test]
    fn tool_pair_duration() {
        let mut i = RolloutInfo::default();
        for (timestamp, payload) in [
            (
                "2026-09-06T10:00:00Z",
                json!({"type":"function_call","call_id":"x","name":"exec_command","arguments":"{\"cmd\":\"python --version\"}"}),
            ),
            (
                "2026-09-06T10:00:02Z",
                json!({"type":"function_call_output","call_id":"x","output":"do not retain"}),
            ),
        ] {
            ingest(
                &mut i,
                &json!({"timestamp":timestamp,"type":"response_item","payload":payload}),
            );
        }
        assert_eq!(i.events[0].duration_ms, Some(2000));
        assert_eq!(i.events[0].status, "returned");
    }
}
