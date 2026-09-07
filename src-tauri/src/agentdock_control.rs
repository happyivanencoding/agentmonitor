use crate::{model::now_ms, storage, Shared};
use base64::Engine;
use reqwest::blocking::Client;
use serde_json::{json, Value};
use std::{collections::HashMap, path::PathBuf, thread, time::Duration};
use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN},
};

const STALE_MS: i64 = 24 * 60 * 60 * 1000;
const RETRY_MS: i64 = 60 * 60 * 1000;
const MAX_PER_SWEEP: usize = 100;

fn runtime_root() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("AgentDock")
}

fn unprotect_token() -> Result<String, String> {
    let encoded = std::fs::read_to_string(runtime_root().join("auth-token.dpapi"))
        .map_err(|e| format!("AgentDock auth token is unavailable: {e}"))?;
    let mut encrypted = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .map_err(|e| format!("AgentDock auth token is malformed: {e}"))?;
    let mut entropy_bytes = b"agentdock.startup.v1".to_vec();
    let mut input = CRYPT_INTEGER_BLOB {
        cbData: encrypted.len() as u32,
        pbData: encrypted.as_mut_ptr(),
    };
    let mut entropy = CRYPT_INTEGER_BLOB {
        cbData: entropy_bytes.len() as u32,
        pbData: entropy_bytes.as_mut_ptr(),
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let ok = unsafe {
        CryptUnprotectData(
            &mut input,
            std::ptr::null_mut(),
            &mut entropy,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 || output.pbData.is_null() {
        return Err("Windows could not unlock the current user's AgentDock credential".into());
    }
    let bytes = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) };
    let token = String::from_utf8(bytes.to_vec()).map_err(|e| e.to_string());
    unsafe {
        LocalFree(output.pbData.cast());
    }
    token
}

fn mcp_url() -> Result<String, String> {
    let raw = std::fs::read_to_string(runtime_root().join("runtime.json"))
        .map_err(|e| format!("AgentDock runtime.json is unavailable: {e}"))?;
    let v: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    v["local_mcp_url"]
        .as_str()
        .filter(|s| s.starts_with("http://127.0.0.1:") && s.ends_with("/mcp"))
        .map(str::to_owned)
        .ok_or_else(|| "AgentDock local MCP URL is unavailable".into())
}

struct Mcp {
    client: Client,
    url: String,
    token: String,
    session: Option<String>,
    id: u64,
}

impl Mcp {
    fn new() -> Result<Self, String> {
        Ok(Self {
            client: Client::builder()
                .timeout(Duration::from_secs(20))
                .build()
                .map_err(|e| e.to_string())?,
            url: mcp_url()?,
            token: unprotect_token()?,
            session: None,
            id: 0,
        })
    }

    fn post(&mut self, value: Value) -> Result<Value, String> {
        let mut request = self
            .client
            .post(&self.url)
            .bearer_auth(&self.token)
            .header("Accept", "application/json, text/event-stream")
            .json(&value);
        if let Some(session) = &self.session {
            request = request.header("Mcp-Session-Id", session);
        }
        let response = request.send().map_err(|e| e.to_string())?;
        if let Some(session) = response.headers().get("Mcp-Session-Id") {
            self.session = session.to_str().ok().map(str::to_owned);
        }
        if !response.status().is_success() {
            return Err(format!("AgentDock MCP returned HTTP {}", response.status()));
        }
        let text = response.text().map_err(|e| e.to_string())?;
        if text.trim().is_empty() {
            return Ok(json!({}));
        }
        let json_text = if text.starts_with("event:") {
            text.lines()
                .find_map(|line| line.strip_prefix("data:"))
                .map(str::trim)
                .ok_or_else(|| "AgentDock MCP SSE response had no data frame".to_string())?
                .to_owned()
        } else {
            text
        };
        serde_json::from_str(&json_text).map_err(|e| e.to_string())
    }

    fn rpc(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.id += 1;
        let response = self.post(json!({"jsonrpc":"2.0","id":self.id,"method":method,"params":params}))?;
        if !response["error"].is_null() {
            return Err(response["error"].to_string());
        }
        Ok(response["result"].clone())
    }

    fn initialize(&mut self) -> Result<(), String> {
        self.rpc(
            "initialize",
            json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"agent-monitor-stale-reaper","version":env!("CARGO_PKG_VERSION")}}),
        )?;
        self.post(json!({"jsonrpc":"2.0","method":"notifications/initialized","params":{}}))?;
        Ok(())
    }

    fn close_session(&mut self, id: &str) -> Result<(), String> {
        let result = self.rpc(
            "tools/call",
            json!({"name":"acp_session","arguments":{"action":"close","session_id":id}}),
        )?;
        if result["isError"].as_bool() == Some(true) {
            return Err(result["content"].to_string());
        }
        Ok(())
    }
}

fn snapshot(state: &Shared) -> Value {
    state
        .snapshot
        .lock()
        .map(|v| v.clone())
        .unwrap_or_else(|_| json!({}))
}

fn attempts(state: &Shared) -> HashMap<String, (i64, bool)> {
    storage::open(&state.data.join("monitor.sqlite"))
        .map(|c| {
            storage::auto_stop_attempts(&c)
                .into_iter()
                .map(|(id, at, ok)| (id, (at, ok)))
                .collect()
        })
        .unwrap_or_default()
}

fn set_status(state: &Shared, value: Value) {
    if let Ok(mut status) = state.reaper_status.lock() {
        *status = value;
    }
}

pub fn start(state: Shared) {
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(8));
        loop {
            let at = now_ms();
            let snapshot = snapshot(&state);
            let previous = attempts(&state);
            let mut owners: HashMap<String, String> = HashMap::new();
            let mut sessions = Vec::new();
            let mut expire_only = Vec::new();

            for agent in snapshot["agents"].as_array().into_iter().flatten() {
                let last = agent["lastActivityMs"].as_i64().unwrap_or(at);
                if at - last <= STALE_MS {
                    continue;
                }
                let thread_id = agent["id"].as_str().unwrap_or_default().to_owned();
                let mut has_open_acp = false;
                for acp in agent["acpSessions"].as_array().into_iter().flatten() {
                    if acp["status"].as_str() == Some("closed") {
                        continue;
                    }
                    let Some(id) = acp["id"].as_str() else { continue };
                    has_open_acp = true;
                    let retry = previous
                        .get(id)
                        .map(|(attempted, ok)| !*ok && at - *attempted >= RETRY_MS)
                        .unwrap_or(true);
                    if retry && sessions.len() < MAX_PER_SWEEP {
                        sessions.push(id.to_owned());
                        owners.insert(id.to_owned(), thread_id.clone());
                    }
                }
                if !has_open_acp
                    && !agent["archived"].as_bool().unwrap_or(false)
                    && agent["status"].as_str() == Some("SUSPECTED_STALLED")
                {
                    expire_only.push(thread_id);
                }
            }

            let mut closed = 0usize;
            let mut failed = 0usize;
            if !sessions.is_empty() {
                match Mcp::new().and_then(|mut mcp| {
                    mcp.initialize()?;
                    Ok(mcp)
                }) {
                    Ok(mut mcp) => {
                        for id in &sessions {
                            match mcp.close_session(id) {
                                Ok(()) => {
                                    closed += 1;
                                    let _ = storage::record_auto_stop(&state.data.join("monitor.sqlite"), id, true, "AgentDock acp_session close succeeded after 24h inactivity");
                                    if let Some(owner) = owners.get(id) {
                                        let _ = storage::set_archived(&state.data.join("monitor.sqlite"), owner, true);
                                    }
                                }
                                Err(error) => {
                                    failed += 1;
                                    let _ = storage::record_auto_stop(&state.data.join("monitor.sqlite"), id, false, &error);
                                }
                            }
                        }
                    }
                    Err(error) => {
                        failed = sessions.len();
                        for id in &sessions {
                            let _ = storage::record_auto_stop(&state.data.join("monitor.sqlite"), id, false, &error);
                        }
                    }
                }
            }

            let mut expired = 0usize;
            for id in expire_only {
                if storage::set_archived(&state.data.join("monitor.sqlite"), &id, true).is_ok() {
                    expired += 1;
                    let _ = storage::record_auto_stop(&state.data.join("monitor.sqlite"), &id, true, "Stale local logical thread expired after 24h; no per-thread ACP control exists");
                }
            }

            set_status(
                &state,
                json!({"healthy":failed==0,"thresholdHours":24,"lastSweepAt":at,"closedSessions":closed,"expiredLocalThreads":expired,"failed":failed,"pendingBatch":sessions.len()}),
            );
            thread::sleep(Duration::from_secs(60));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_threshold_is_one_day() {
        assert_eq!(STALE_MS, 86_400_000);
    }
}
