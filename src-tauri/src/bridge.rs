//! Write-only, authenticated loopback attribution bridge. No shell/filesystem/proxy APIs.
use crate::{
    model::now_ms,
    sources,
    storage::{self, BindingInput, RequestObservationInput, RequestToolInput},
    Shared,
};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    path::Path,
    sync::OnceLock,
};
use tiny_http::{Header, Method, Response, Server, StatusCode};
pub const PORT: u16 = 43217;
const MAX_BODY: u64 = 16 * 1024;

pub fn token(dir: &Path) -> Result<String, String> {
    let p = dir.join("bridge.key");
    if let Ok(s) = std::fs::read_to_string(&p) {
        if s.trim().len() == 64 {
            return Ok(s.trim().into());
        }
    }
    let value = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&p)
        .map_err(|e| format!("Cannot initialize bridge credential: {e}"))?;
    f.write_all(value.as_bytes()).map_err(|e| e.to_string())?;
    Ok(value)
}
pub fn valid_origin(origin: &str) -> bool {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"^chrome-extension://[a-p]{32}$").unwrap())
        .is_match(origin)
}
pub fn valid_host(host: &str) -> bool {
    host == format!("127.0.0.1:{PORT}") || host == format!("localhost:{PORT}")
}
fn constant_equal(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes().zip(b.bytes()).fold(0u8, |n, (a, b)| n | (a ^ b)) == 0
}
pub fn entity_known(state: &Shared, id: &str) -> bool {
    if let Ok(v) = state.snapshot.lock() {
        if v["agents"]
            .as_array()
            .map(|a| {
                a.iter().any(|a| {
                    a["id"].as_str() == Some(id)
                        || a["acpSessions"]
                            .as_array()
                            .map(|s| s.iter().any(|s| s["id"].as_str() == Some(id)))
                            .unwrap_or(false)
                })
            })
            .unwrap_or(false)
        {
            return true;
        }
    }
    // An extension can observe the tool result before the next collector tick.
    if id.starts_with("acps_")
        && id.len() <= 80
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        let root = sources::home().join(".agentdock/acp/sessions");
        for e in walkdir::WalkDir::new(root)
            .max_depth(3)
            .follow_links(false)
            .into_iter()
            .flatten()
        {
            if e.file_type().is_file() && e.file_name().to_string_lossy() == format!("{id}.json") {
                return std::fs::read_to_string(e.path())
                    .ok()
                    .and_then(|s| serde_json::from_str::<Value>(&s).ok())
                    .map(|v| v["id"].as_str() == Some(id))
                    .unwrap_or(false);
            }
        }
    }
    false
}
pub fn start(state: Shared) {
    std::thread::spawn(move || {
        let server = match Server::http(("127.0.0.1", PORT)) {
            Ok(s) => s,
            Err(e) => {
                if let Ok(mut status) = state.bridge_status.lock() {
                    *status = json!({"healthy":false,"port":PORT,"error":e.to_string()});
                }
                return;
            }
        };
        if let Ok(mut status) = state.bridge_status.lock() {
            *status = json!({"healthy":true,"port":PORT,"lastExtensionSeen":null});
        }
        for mut request in server.incoming_requests() {
            let header = |key: &'static str| {
                request
                    .headers()
                    .iter()
                    .find(|h| h.field.equiv(key))
                    .map(|h| h.value.as_str().to_owned())
                    .unwrap_or_default()
            };
            let host = header("Host");
            let origin = header("Origin");
            let authorization = header("Authorization");
            let content_type = header("Content-Type");
            let route = request.url().to_owned();
            let method = request.method().clone();
            let respond = |request: tiny_http::Request, code: u16, value: Value, cors: bool| {
                let mut response = Response::from_string(value.to_string())
                    .with_status_code(StatusCode(code))
                    .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
                    .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap());
                if cors && !origin.is_empty() {
                    response.add_header(
                        Header::from_bytes("Access-Control-Allow-Origin", origin.as_str()).unwrap(),
                    );
                    response.add_header(Header::from_bytes("Vary", "Origin").unwrap());
                    response.add_header(
                        Header::from_bytes(
                            "Access-Control-Allow-Headers",
                            "authorization, content-type",
                        )
                        .unwrap(),
                    );
                    response.add_header(
                        Header::from_bytes("Access-Control-Allow-Methods", "POST, OPTIONS")
                            .unwrap(),
                    );
                }
                let _ = request.respond(response);
            };
            if !valid_host(&host) {
                respond(request, 403, json!({"error":"Invalid Host"}), false);
                continue;
            }
            if !origin.is_empty() && !valid_origin(&origin) {
                respond(request, 403, json!({"error":"Origin denied"}), false);
                continue;
            }
            if method == Method::Get && route == "/health" {
                respond(
                    request,
                    200,
                    json!({"app":"Agent Monitor","version":env!("CARGO_PKG_VERSION")}),
                    false,
                );
                continue;
            }
            if method == Method::Options {
                if valid_origin(&origin)
                    && matches!(
                        route.as_str(),
                        "/v1/bind" | "/v1/bind/manual" | "/v1/heartbeat" | "/v1/request" | "/v1/tool"
                    )
                {
                    respond(request, 204, json!({}), true);
                } else {
                    respond(request, 403, json!({"error":"Preflight denied"}), false);
                }
                continue;
            }
            if !constant_equal(&authorization, &format!("Bearer {}", state.bridge_token)) {
                respond(
                    request,
                    401,
                    json!({"error":"Pair the extension with the desktop token"}),
                    true,
                );
                continue;
            }
            if method != Method::Post
                || !matches!(
                    route.as_str(),
                    "/v1/bind" | "/v1/bind/manual" | "/v1/heartbeat" | "/v1/request" | "/v1/tool"
                )
            {
                respond(request, 404, json!({"error":"No such endpoint"}), true);
                continue;
            }
            if !content_type
                .to_ascii_lowercase()
                .starts_with("application/json")
            {
                respond(request, 415, json!({"error":"JSON required"}), true);
                continue;
            }
            if request
                .body_length()
                .map(|n| n as u64 > MAX_BODY)
                .unwrap_or(false)
            {
                respond(request, 413, json!({"error":"Body too large"}), true);
                continue;
            }
            let mut bytes = Vec::new();
            if request
                .as_reader()
                .take(MAX_BODY + 1)
                .read_to_end(&mut bytes)
                .is_err()
                || bytes.len() as u64 > MAX_BODY
            {
                respond(
                    request,
                    413,
                    json!({"error":"Body unreadable or too large"}),
                    true,
                );
                continue;
            }
            let value = match serde_json::from_slice::<Value>(&bytes) {
                Ok(v) => v,
                Err(_) => {
                    respond(request, 400, json!({"error":"Malformed JSON"}), true);
                    continue;
                }
            };
            if route == "/v1/heartbeat" {
                let at = now_ms();
                if let Ok(mut s) = state.bridge_status.lock() {
                    s["lastExtensionSeen"] = json!(at);
                    s["extensionOrigin"] = json!(origin);
                    if let Some(version) = value["extensionVersion"].as_str().filter(|v| v.len() <= 32) {
                        s["extensionVersion"] = json!(version);
                    }
                    if let Some(error) = value["captureError"].as_str().filter(|v| v.len() <= 200) {
                        s["captureError"] = json!(error);
                    }
                    if let Some(observer) = value["pageObserver"].as_str().filter(|v| v.len() <= 80) {
                        s["pageObserver"] = json!(observer);
                        s["lastPageObserverSeen"] = json!(at);
                        if observer != "observer-ready" { s["captureStatus"] = json!(observer); }
                    }
                }
                respond(request, 200, json!({"ok":true,"version":env!("CARGO_PKG_VERSION")}), true);
                continue;
            }
            if route == "/v1/request" {
                let input = match serde_json::from_value::<RequestObservationInput>(value) {
                    Ok(i) => i,
                    Err(_) => {
                        respond(request, 400, json!({"error":"Missing request observation fields"}), true);
                        continue;
                    }
                };
                match storage::observe_request(&state.data.join("monitor.sqlite"), &input) {
                    Ok(v) => {
                        if let Ok(mut s) = state.bridge_status.lock() {
                            s["lastExtensionSeen"] = json!(now_ms());
                            s["lastRequestAt"] = json!(now_ms());
                        }
                        respond(request, 200, v, true)
                    }
                    Err(e) => respond(request, 400, json!({"error":e}), true),
                }
                continue;
            }
            if route == "/v1/tool" {
                let input = match serde_json::from_value::<RequestToolInput>(value) {
                    Ok(i) => i,
                    Err(_) => {
                        respond(request, 400, json!({"error":"Missing request tool fields"}), true);
                        continue;
                    }
                };
                match storage::observe_request_tool(&state.data.join("monitor.sqlite"), &input) {
                    Ok(v) => {
                        if let Ok(mut s) = state.bridge_status.lock() {
                            s["lastExtensionSeen"] = json!(now_ms());
                            s["lastToolAt"] = json!(now_ms());
                        }
                        respond(request, 200, v, true)
                    }
                    Err(e) => respond(request, 400, json!({"error":e}), true),
                }
                continue;
            }
            let input = match serde_json::from_value::<BindingInput>(value) {
                Ok(i) => i,
                Err(_) => {
                    respond(
                        request,
                        400,
                        json!({"error":"Missing binding fields"}),
                        true,
                    );
                    continue;
                }
            };
            if !entity_known(&state, &input.entity_id) {
                respond(
                    request,
                    422,
                    json!({"error":"Entity not yet present in local ACP/thread data"}),
                    true,
                );
                continue;
            }
            let source = if route.ends_with("/manual") {
                "user-confirmed-browser-reference"
            } else {
                "browser-tool-result"
            };
            match storage::bind(&state.data.join("monitor.sqlite"), &input, source) {
                Ok(v) => {
                    if let Ok(mut s) = state.bridge_status.lock() {
                        s["lastExtensionSeen"] = json!(now_ms());
                        s["lastBindingAt"] = json!(now_ms());
                    }
                    respond(request, 200, v, true)
                }
                Err(e) => respond(
                    request,
                    if e.starts_with("CONFLICT") { 409 } else { 400 },
                    json!({"error":e}),
                    true,
                ),
            }
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn origin_boundary() {
        assert!(valid_origin(
            "chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        ));
        assert!(!valid_origin("https://chatgpt.com"));
        assert!(!valid_origin("null"));
        assert!(!valid_origin("chrome-extension://bad"));
    }
    #[test]
    fn host_boundary() {
        assert!(valid_host("127.0.0.1:43217"));
        assert!(!valid_host("evil.test:43217"));
        assert!(!valid_host("0.0.0.0:43217"));
    }
    #[test]
    fn auth_is_exact() {
        assert!(constant_equal("abc", "abc"));
        assert!(!constant_equal("abc", "abd"));
        assert!(!constant_equal("abc", "ab"));
    }
}
