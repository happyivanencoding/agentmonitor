use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::OnceLock;

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
pub fn text(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}
pub fn millis(v: &Value) -> Option<i64> {
    if let Some(n) = v.as_i64() {
        return Some(if n.abs() < 100_000_000_000 {
            n * 1000
        } else {
            n
        });
    }
    v.as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|t| t.timestamp_millis())
}
pub fn clipped(s: &str, limit: usize) -> String {
    if s.chars().count() <= limit {
        s.to_owned()
    } else {
        s.chars().take(limit).collect::<String>() + "…"
    }
}
/// Do not persist prompts, output bodies, reasoning contents or diffs. Only commands pass here.
pub fn redact(s: &str) -> String {
    static KEY: OnceLock<regex::Regex> = OnceLock::new();
    static BEARER: OnceLock<regex::Regex> = OnceLock::new();
    let re = KEY.get_or_init(|| regex::Regex::new(r#"(?i)((?:api[_-]?key|access[_-]?token|password|passwd|secret|authorization|cookie)\s*[=:]\s*)[^\s,;]+"#).unwrap());
    let bearer = BEARER.get_or_init(|| {
        regex::Regex::new(r"(?i)(bearer\s+)[a-z0-9._~+/=-]+|\bsk-[a-zA-Z0-9_-]{12,}").unwrap()
    });
    clipped(
        &bearer.replace_all(&re.replace_all(s, "${1}[REDACTED]"), "[REDACTED]"),
        1600,
    )
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub status: String,
    pub start_ms: i64,
    pub end_ms: Option<i64>,
    pub duration_ms: Option<i64>,
    pub command: Option<String>,
    pub file: Option<String>,
    pub token_delta: Option<i64>,
    pub turn_id: Option<String>,
    pub tool_session_id: Option<String>,
    pub source: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Turn {
    pub id: String,
    pub status: String,
    pub start_ms: i64,
    pub end_ms: Option<i64>,
    pub duration_ms: Option<i64>,
    pub error: Option<String>,
    pub source: String,
}
pub fn active(s: &str) -> bool {
    matches!(
        s,
        "inProgress" | "in_progress" | "running" | "started" | "pending"
    )
}

pub struct Evidence<'a> {
    pub turn: Option<&'a Turn>,
    pub tool: Option<&'a Event>,
    pub now: i64,
    pub last_activity: i64,
    pub stable_ms: i64,
    pub lost_observed_owner: bool,
    pub child_busy: bool,
    pub readable: bool,
    pub transport_closed: bool,
}
/// Conservative, explainable classifier. CPU is deliberately not an input.
pub fn classify(e: &Evidence<'_>) -> (&'static str, Vec<String>) {
    if !e.readable {
        return (
            "UNKNOWN",
            vec!["Structured source unavailable; retained observations may be stale".into()],
        );
    }
    let Some(t) = e.turn else {
        return (
            "UNKNOWN",
            vec!["No recorded turn lifecycle; transport state is not execution state".into()],
        );
    };
    match t.status.as_str() {
        "completed" => {
            return (
                "COMPLETED",
                vec![format!("Terminal turn recorded by {}", t.source)],
            )
        }
        "failed" => return ("FAILED", vec!["Turn has an explicit failure record".into()]),
        "interrupted" | "cancelled" | "aborted" => {
            return (
                "IDLE",
                vec!["Last turn was interrupted or cancelled, not completed".into()],
            )
        }
        _ => {}
    }
    if !active(&t.status) {
        return (
            "UNKNOWN",
            vec![format!("Unrecognized turn status: {}", t.status)],
        );
    }
    if e.lost_observed_owner {
        return ("CRASHED",vec!["Previously verified lock-owner process disappeared while turn remained active across observations".into()]);
    }
    if let Some(tool) = e.tool {
        if matches!(
            tool.status.as_str(),
            "waitingApproval" | "awaitingApproval" | "waitingUser" | "requires_action"
        ) {
            return (
                "WAITING_USER",
                vec!["Explicit approval/user-action state".into()],
            );
        }
    }
    if e.transport_closed {
        return (
            "UNKNOWN",
            vec!["ACP transport closed without a matching terminal-turn record".into()],
        );
    }
    let threshold = if e.tool.is_some() {
        30 * 60 * 1000
    } else {
        15 * 60 * 1000
    };
    if e.now - e.last_activity > threshold && e.stable_ms >= 60_000 && !e.child_busy {
        return ("SUSPECTED_STALLED",vec![
            "Recorded turn remains active without a terminal event".into(),
            format!("No structured activity for {} minutes",(e.now-e.last_activity)/60_000),
            "Token/activity counters unchanged over repeated observations; no positively observed child progress".into(),
            "This is a suspicion, not proof of failure; long reasoning/network/tool waits remain possible".into()]);
    }
    if e.now - e.last_activity > threshold && e.stable_ms < 60_000 {
        return (
            "UNKNOWN",
            vec![
                "Stale active-turn record; gathering fresh observations before classifying".into(),
            ],
        );
    }
    if let Some(tool) = e.tool {
        if active(&tool.status) {
            return (
                "WAITING_TOOL",
                vec![format!("Open {} tool item", tool.name)],
            );
        }
    }
    ("WAITING_MODEL",vec!["Active turn; no open tool item currently recorded (model/network activity is not separately observable)".into()])
}

pub fn item_event(row: &Value) -> Option<Event> {
    let j: Value = serde_json::from_str(row.get("item_json")?.as_str()?).ok()?;
    let kind = text(&j, "type");
    if matches!(kind.as_str(), "userMessage" | "functionCallOutput") {
        return None;
    }
    let start = row.get("created_at_ms").and_then(Value::as_i64)?;
    let duration = j.get("durationMs").and_then(Value::as_i64);
    let status = if text(&j, "status").is_empty() {
        "observed".into()
    } else {
        text(&j, "status")
    };
    let file = j
        .get("changes")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(|v| v.get("path"))
        .and_then(Value::as_str)
        .map(|s| clipped(s, 600));
    let name = match kind.as_str() {
        "commandExecution" => "shell".into(),
        "fileChange" => "file edit".into(),
        "reasoning" => "model activity".into(),
        "agentMessage" => "agent message".into(),
        "mcpToolCall" => format!("{} · {}", text(&j, "server"), text(&j, "tool")),
        "collabAgentToolCall" => format!("subagent · {}", text(&j, "tool")),
        _ => kind.clone(),
    };
    Some(Event {
        id: text(row, "item_id"),
        kind,
        name: clipped(&name, 150),
        status: status.clone(),
        start_ms: start,
        end_ms: duration.map(|d| start + d),
        duration_ms: duration,
        command: j.get("command").and_then(Value::as_str).map(redact),
        file,
        token_delta: None,
        turn_id: Some(text(row, "turn_id")),
        tool_session_id: j
            .get("processId")
            .and_then(Value::as_str)
            .map(str::to_owned),
        source: "thread_items".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn run(status: &str, age: i64, stable: i64, closed: bool, lost: bool) -> String {
        let t = Turn {
            status: status.into(),
            source: "fixture".into(),
            ..Default::default()
        };
        classify(&Evidence {
            turn: Some(&t),
            tool: None,
            now: 2_000_000,
            last_activity: 2_000_000 - age,
            stable_ms: stable,
            lost_observed_owner: lost,
            child_busy: false,
            readable: true,
            transport_closed: closed,
        })
        .0
        .into()
    }
    #[test]
    fn completed_wins_over_transport_and_process() {
        assert_eq!(run("completed", 2_000_000, 90_000, true, true), "COMPLETED");
    }
    #[test]
    fn closed_does_not_equal_success() {
        assert_eq!(run("inProgress", 0, 0, true, false), "UNKNOWN");
    }
    #[test]
    fn stale_active_requires_repeated_observations() {
        assert_eq!(run("inProgress", 1_000_000, 0, false, false), "UNKNOWN");
        assert_eq!(
            run("inProgress", 1_000_000, 61_000, false, false),
            "SUSPECTED_STALLED"
        );
    }
    #[test]
    fn long_model_wait_not_dead() {
        assert_eq!(
            run("inProgress", 300_000, 90_000, false, false),
            "WAITING_MODEL"
        );
    }
    #[test]
    fn interrupted_not_failed() {
        assert_eq!(run("interrupted", 1_000_000, 90_000, false, false), "IDLE");
    }
    #[test]
    fn observed_owner_loss_can_crash() {
        assert_eq!(run("inProgress", 60_000, 60_000, false, true), "CRASHED");
    }
    #[test]
    fn redacts_command_secrets() {
        let s =
            redact("API_KEY=abcdef password=hunter2 Authorization=Bearer sk-123456789abcdefghijk");
        assert!(!s.contains("abcdef"));
        assert!(!s.contains("hunter2"));
        assert!(!s.contains("sk-123"));
    }
    #[test]
    fn timestamps_seconds_and_millis() {
        assert_eq!(millis(&serde_json::json!(1788692259)), Some(1788692259000));
        assert_eq!(
            millis(&serde_json::json!(1788692259000_i64)),
            Some(1788692259000)
        );
    }
    #[test]
    fn stale_open_tool_is_unknown_during_warmup() {
        let t = Turn {
            status: "inProgress".into(),
            ..Default::default()
        };
        let tool = Event {
            status: "inProgress".into(),
            name: "shell".into(),
            ..Default::default()
        };
        let e = Evidence {
            turn: Some(&t),
            tool: Some(&tool),
            now: 4_000_000,
            last_activity: 1_000_000,
            stable_ms: 0,
            lost_observed_owner: false,
            child_busy: false,
            readable: true,
            transport_closed: false,
        };
        assert_eq!(classify(&e).0, "UNKNOWN");
    }
}
