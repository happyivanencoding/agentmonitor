//! Same-origin web/PWA service. Only Monitor metadata operations; no native command or file APIs.
use crate::{
    access::{Access, Config},
    api, storage, Shared,
};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};
pub const PORT: u16 = 43218;
const MAX_BODY: u64 = 16 * 1024;
fn header(r: &Request, name: &'static str) -> String {
    r.headers()
        .iter()
        .find(|h| h.field.equiv(name))
        .map(|h| h.value.as_str().to_owned())
        .unwrap_or_default()
}
fn is_local(host: &str, forwarded: bool) -> bool {
    !forwarded && matches!(host, "127.0.0.1:43218" | "localhost:43218")
}
fn forwarded(r: &Request) -> bool {
    r.headers().iter().any(|h| {
        h.field
            .as_str()
            .as_str()
            .to_ascii_lowercase()
            .starts_with("cf-")
            || h.field.equiv("X-Forwarded-For")
            || h.field.equiv("Forwarded")
            || h.field.equiv("X-Forwarded-Host")
            || h.field.equiv("X-Forwarded-Proto")
    })
}
fn request_origin(host: &str, local: bool, access: Option<&Access>) -> Option<String> {
    if local {
        Some(format!("http://{host}"))
    } else {
        access
            .filter(|a| a.config.host() == host)
            .map(|a| a.config.origin().to_owned())
    }
}
fn respond(r: Request, status: u16, content_type: &str, body: Vec<u8>, static_asset: bool) {
    let accepts_gzip = header(&r, "Accept-Encoding")
        .split(',')
        .any(|v| v.trim() == "gzip");
    let compressed = if accepts_gzip
        && body.len() > 1024
        && (content_type.starts_with("text/") || content_type.starts_with("application/json"))
    {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder
            .write_all(&body)
            .ok()
            .and_then(|_| encoder.finish().ok())
    } else {
        None
    };
    let is_gzip = compressed.is_some();
    let mut response =
        Response::from_data(compressed.unwrap_or(body)).with_status_code(StatusCode(status));
    if is_gzip {
        response.add_header(Header::from_bytes("Content-Encoding", "gzip").unwrap());
    }
    response.add_header(Header::from_bytes("Vary", "Accept-Encoding").unwrap());
    for(k,v)in [
        ("Content-Type",content_type),
        ("Cache-Control",if static_asset {"private, max-age=0, must-revalidate"}else{"no-store"}),
        ("X-Content-Type-Options","nosniff"),
        ("Referrer-Policy","no-referrer"),
        ("X-Frame-Options","DENY"),
        ("Content-Security-Policy","default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; worker-src 'self'; manifest-src 'self'; frame-ancestors 'none'; base-uri 'none'; object-src 'none'")
    ] { response.add_header(Header::from_bytes(k,v).unwrap()); }
    let _ = r.respond(response);
}
fn json_response(r: Request, status: u16, v: Value) {
    respond(
        r,
        status,
        "application/json; charset=utf-8",
        serde_json::to_vec(&v).unwrap_or_default(),
        false,
    );
}
fn error(r: Request, status: u16, message: &str) {
    json_response(r, status, json!({"error":message}));
}
fn asset_path(path: &str) -> Option<&str> {
    if matches!(path, "/" | "/index.html") {
        return Some("index.html");
    }
    let rel = path.strip_prefix('/')?;
    let safe = rel.split('/').all(|s| {
        !s.is_empty()
            && s != "."
            && s != ".."
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    });
    if safe
        && (matches!(
            rel,
            "manifest.webmanifest" | "sw.js" | "offline.html" | "favicon.ico"
        ) || rel.starts_with("assets/")
            || rel.starts_with("icons/"))
    {
        Some(rel)
    } else {
        None
    }
}
fn mime(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "webmanifest" => "application/manifest+json",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}
fn handle(mut r: Request, state: &Shared, root: &Path, access: Option<&Access>) {
    let host = header(&r, "Host").to_ascii_lowercase();
    let local = is_local(&host, forwarded(&r));
    let Some(origin) = request_origin(&host, local, access) else {
        return error(r, 403, "Host is not configured for Agent Monitor");
    };
    let origin_header = header(&r, "Origin");
    if !origin_header.is_empty() && origin_header != origin {
        return error(r, 403, "Cross-origin requests are not allowed");
    }
    let identity = if local {
        json!({"kind":"local","label":"这台电脑"})
    } else {
        let token = header(&r, "Cf-Access-Jwt-Assertion");
        match access.unwrap().validate(&token) {
            Ok(claims) => {
                json!({"kind":"cloudflare-access","label":claims.get("email").and_then(Value::as_str).filter(|s|!s.is_empty()).unwrap_or("Authorized service")})
            }
            Err(_) => {
                return error(
                    r,
                    401,
                    "Cloudflare Access session required or expired; reload to sign in",
                )
            }
        }
    };
    let url = match url::Url::parse(&format!("http://monitor.local{}", r.url())) {
        Ok(u) => u,
        Err(_) => return error(r, 400, "Invalid URL"),
    };
    let path = url.path();
    if r.method() == &Method::Get {
        let result = match path {
            "/api/snapshot" => {
                let snapshot = api::snapshot(state);
                let version = snapshot["generatedAt"]
                    .as_i64()
                    .unwrap_or(0)
                    .max(snapshot["lastAttemptAt"].as_i64().unwrap_or(0))
                    .max(snapshot["taskProgressAt"].as_i64().unwrap_or(0))
                    .max(snapshot["collectionCompletedAt"].as_i64().unwrap_or(0));
                let since = url
                    .query_pairs()
                    .find(|(k, _)| k == "since")
                    .and_then(|(_, v)| v.parse::<i64>().ok());
                if version > 0 && since == Some(version) {
                    return respond(r, 204, "application/json", Vec::new(), false);
                }
                Ok(snapshot)
            }
            "/api/detail" => {
                let id = url
                    .query_pairs()
                    .find(|(k, _)| k == "id")
                    .map(|(_, v)| v.into_owned())
                    .unwrap_or_default();
                api::detail(state, &id)
            }
            "/api/analytics" => {
                let days = url
                    .query_pairs()
                    .find(|(k, _)| k == "days")
                    .and_then(|(_, v)| v.parse::<i64>().ok())
                    .unwrap_or(1);
                api::analytics(state, days)
            }
            "/api/setup" => Ok(
                json!({"version":env!("CARGO_PKG_VERSION"),"transport":"web","identity":identity,"publicUrl":access.map(|a|a.config.origin()),"hostName":std::env::var("COMPUTERNAME").unwrap_or_else(|_|"Home PC".into()),"webPort":PORT,"capabilities":{"monitor":true,"attribution":true,"agentControl":false,"nativeSettings":false}}),
            ),
            _ => {
                let Some(relative) = asset_path(path) else {
                    return error(r, 404, "Not found");
                };
                match std::fs::read(root.join(relative)) {
                    Ok(body) => respond(r, 200, mime(relative), body, true),
                    Err(_) => error(
                        r,
                        404,
                        "Web asset not installed; build or reinstall Agent Monitor",
                    ),
                }
                return;
            }
        };
        return match result {
            Ok(v) => json_response(r, 200, v),
            Err(e) => error(r, 400, &e),
        };
    }
    if r.method() != &Method::Post {
        return error(r, 405, "Method not allowed");
    }
    if origin_header != origin || header(&r, "X-Agent-Monitor") != "1" {
        return error(r, 403, "Same-origin Monitor client required");
    }
    if !header(&r, "Content-Type")
        .to_ascii_lowercase()
        .starts_with("application/json")
    {
        return error(r, 415, "JSON required");
    }
    if r.body_length().is_some_and(|n| n as u64 > MAX_BODY) {
        return error(r, 413, "Request too large");
    }
    let mut bytes = Vec::new();
    if r.as_reader()
        .take(MAX_BODY + 1)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return error(r, 400, "Unreadable body");
    }
    if bytes.len() as u64 > MAX_BODY {
        return error(r, 413, "Request too large");
    }
    let v: Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => return error(r, 400, "Invalid JSON"),
    };
    let result = match path {
        "/api/bind" => serde_json::from_value::<storage::BindingInput>(v)
            .map_err(|e| e.to_string())
            .and_then(|input| api::bind(state, &input, "user-confirmed-web")),
        "/api/unbind" => v["entityId"]
            .as_str()
            .ok_or_else(|| "entityId required".to_string())
            .and_then(|id| api::unbind(state, id)),
        "/api/archive" => v["id"]
            .as_str()
            .ok_or_else(|| "id required".to_string())
            .and_then(|id| api::set_archived(state, id, v["archived"].as_bool().unwrap_or(true))),
        "/api/task" => v["threadId"]
            .as_str()
            .ok_or_else(|| "threadId required".to_string())
            .and_then(|id| api::link_task(state, id, v["taskId"].as_str())),
        _ => return error(r, 404, "Operation unavailable on the web"),
    };
    match result {
        Ok(v) => json_response(r, 200, v),
        Err(e) => error(r, 400, &e),
    }
}
pub fn start(state: Shared, root: PathBuf) {
    std::thread::spawn(move || {
        let access = match Config::read(&state.data).and_then(|c| c.map(Access::new).transpose()) {
            Ok(c) => c,
            Err(e) => {
                let _ = std::fs::write(state.data.join("remote-error.txt"), e);
                return;
            }
        };
        let server = match Server::http(("127.0.0.1", PORT)) {
            Ok(s) => Arc::new(s),
            Err(e) => {
                let _ = std::fs::write(state.data.join("remote-error.txt"), e.to_string());
                return;
            }
        };
        let _ = std::fs::remove_file(state.data.join("remote-error.txt"));
        let access = Arc::new(access);
        // Four readers are ample for this one-user local service and avoid spawning a thread per request.
        for _ in 0..4 {
            let server = server.clone();
            let state = state.clone();
            let root = root.clone();
            let access = access.clone();
            std::thread::spawn(move || {
                for r in server.incoming_requests() {
                    handle(r, &state, &root, access.as_ref().as_ref());
                }
            });
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tunnel_is_never_local_just_because_tcp_is_loopback() {
        assert!(is_local("127.0.0.1:43218", false));
        assert!(!is_local("127.0.0.1:43218", true));
        assert!(!is_local("monitor.example.com", false));
        assert!(!is_local("127.0.0.1.attacker:43218", false));
    }
    #[test]
    fn static_routes_cannot_read_data_or_secrets() {
        for p in [
            "/bridge.key",
            "/remote.json",
            "/../monitor.sqlite",
            "/assets/../../bridge.key",
            "/assets/%2e%2e/file",
            "/C:/Windows/file",
            "//server/file",
        ] {
            assert!(asset_path(p).is_none(), "{p}");
        }
        assert_eq!(asset_path("/"), Some("index.html"));
        assert_eq!(
            asset_path("/assets/index-a1.js"),
            Some("assets/index-a1.js")
        );
    }
    #[test]
    fn public_host_is_disabled_without_access() {
        assert!(request_origin("monitor.example.com", false, None).is_none());
        assert_eq!(
            request_origin("localhost:43218", true, None).unwrap(),
            "http://localhost:43218"
        );
    }
}
