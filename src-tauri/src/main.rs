#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod access;
mod agentdock_control;
mod api;
mod bridge;
mod collector;
mod model;
mod os;
mod progress;
mod remote;
mod rollout;
mod sources;
mod storage;
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager,
};

#[derive(Clone)]
pub struct Shared {
    pub snapshot: Arc<Mutex<Value>>,
    pub bridge_status: Arc<Mutex<Value>>,
    pub reaper_status: Arc<Mutex<Value>>,
    pub data: PathBuf,
    pub codex: PathBuf,
    pub bridge_token: Arc<String>,
}
#[tauri::command]
fn get_snapshot(state: tauri::State<'_, Shared>) -> Value {
    api::snapshot(&state)
}
#[tauri::command]
async fn get_detail(id: String, state: tauri::State<'_, Shared>) -> Result<Value, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || api::detail(&state, &id))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
fn get_analytics(days: i64, state: tauri::State<'_, Shared>) -> Result<Value, String> {
    api::analytics(&state, days)
}
#[tauri::command]
fn bind_agent(
    input: storage::BindingInput,
    state: tauri::State<'_, Shared>,
) -> Result<Value, String> {
    api::bind(&state, &input, "user-confirmed-desktop")
}
#[tauri::command]
fn unbind_agent(entity_id: String, state: tauri::State<'_, Shared>) -> Result<(), String> {
    api::unbind(&state, &entity_id).map(|_| ())
}
#[tauri::command]
fn set_agent_archived(
    id: String,
    archived: bool,
    state: tauri::State<'_, Shared>,
) -> Result<Value, String> {
    api::set_archived(&state, &id, archived)
}
#[tauri::command]
fn link_task(
    thread_id: String,
    task_id: Option<String>,
    state: tauri::State<'_, Shared>,
) -> Result<(), String> {
    api::link_task(&state, &thread_id, task_id.as_deref()).map(|_| ())
}
#[tauri::command]
fn get_pairing_token(state: tauri::State<'_, Shared>) -> String {
    (*state.bridge_token).clone()
}
fn spawn_native(program: &str, args: &[String]) -> Result<(), String> {
    let mut c = std::process::Command::new(program);
    c.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x08000000);
    }
    c.spawn().map_err(|e| e.to_string())?;
    Ok(())
}
#[tauri::command]
fn open_conversation(url: String) -> Result<(), String> {
    let id = storage::conversation_id(&url)?;
    spawn_native(
        "rundll32.exe",
        &[
            "url.dll,FileProtocolHandler".into(),
            format!("https://chatgpt.com/c/{id}"),
        ],
    )
}
#[tauri::command]
fn open_data_dir(state: tauri::State<'_, Shared>) -> Result<(), String> {
    spawn_native("explorer.exe", &[state.data.to_string_lossy().into_owned()])
}
fn extension_dir(app: &tauri::AppHandle) -> PathBuf {
    let installed = app
        .path()
        .resource_dir()
        .unwrap_or_default()
        .join("extension");
    if installed.is_dir() {
        installed
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("extension")
    }
}
#[tauri::command]
fn open_extension_dir(app: tauri::AppHandle) -> Result<(), String> {
    spawn_native(
        "explorer.exe",
        &[extension_dir(&app).to_string_lossy().into_owned()],
    )
}

fn browser_executable(browser: &str) -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let program = std::env::var_os("ProgramFiles").map(PathBuf::from);
    let program_x86 = std::env::var_os("ProgramFiles(x86)").map(PathBuf::from);
    let relative = match browser {
        "chrome" => PathBuf::from("Google/Chrome/Application/chrome.exe"),
        "edge" => PathBuf::from("Microsoft/Edge/Application/msedge.exe"),
        _ => return None,
    };
    [local, program, program_x86]
        .into_iter()
        .flatten()
        .map(|root| root.join(&relative))
        .find(|path| path.is_file())
}

#[tauri::command]
fn prepare_browser_attribution(browser: String, app: tauri::AppHandle) -> Result<Value, String> {
    let exe = browser_executable(&browser).ok_or_else(|| match browser.as_str() {
        "chrome" => "没有找到已安装的 Chrome".to_string(),
        "edge" => "没有找到已安装的 Edge".to_string(),
        _ => "只支持 Chrome 或 Edge".to_string(),
    })?;
    let extension = extension_dir(&app);
    spawn_native("explorer.exe", &[extension.to_string_lossy().into_owned()])?;
    let page = if browser == "edge" {
        "edge://extensions/"
    } else {
        "chrome://extensions/"
    };
    spawn_native(&exe.to_string_lossy(), &[page.into()])?;
    Ok(json!({"ok":true,"browser":browser,"extensionDir":extension}))
}

fn autostart_enabled() -> bool {
    let mut c = std::process::Command::new("reg.exe");
    c.args([
        "query",
        r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
        "/v",
        "AgentMonitor",
    ]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x08000000);
    }
    c.output().map(|o| o.status.success()).unwrap_or(false)
}
#[tauri::command]
fn set_autostart(enabled: bool) -> Result<(), String> {
    let mut c = std::process::Command::new("reg.exe");
    let path = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    if enabled {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        c.args([
            "add",
            path,
            "/v",
            "AgentMonitor",
            "/t",
            "REG_SZ",
            "/d",
            &format!("\"{}\" --minimized", exe.to_string_lossy()),
            "/f",
        ]);
    } else {
        if !autostart_enabled() {
            return Ok(());
        }
        c.args(["delete", path, "/v", "AgentMonitor", "/f"]);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x08000000);
    }
    let out = c.output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err("Windows could not update this user's startup entry".into())
    }
}
#[tauri::command]
fn get_setup_info(app: tauri::AppHandle, state: tauri::State<'_, Shared>) -> Value {
    json!({"dataDir":state.data,"extensionDir":extension_dir(&app),"autostart":autostart_enabled(),"bridge":state.bridge_status.lock().map(|s|s.clone()).unwrap_or(Value::Null),"version":env!("CARGO_PKG_VERSION"),"remote":access::Config::read(&state.data).ok().flatten().map(|c|json!({"publicUrl":c.origin(),"webPort":remote::PORT}))})
}
#[tauri::command]
fn export_diagnostics(state: tauri::State<'_, Shared>) -> Result<String, String> {
    let s = state.snapshot.lock().map_err(|e| e.to_string())?;
    let v = json!({"appVersion":env!("CARGO_PKG_VERSION"),"generatedAt":s["generatedAt"],"collectionMs":s["collectionMs"],"stale":s["stale"],"stats":s["stats"],"sourceHealth":{"codex":s["sources"]["codex"]["healthy"],"history":s["sources"]["history"]["healthy"],"acp":s["sources"]["acp"]["healthy"],"locks":s["sources"]["locks"]["healthy"],"agentdock":s["sources"]["agentdock"]["healthy"]},"note":"No conversation titles, IDs, commands, paths, prompts, credentials or tool output are included."});
    let p = state.data.join("diagnostics.json");
    std::fs::write(
        &p,
        serde_json::to_vec_pretty(&v).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(p.to_string_lossy().into_owned())
}
#[cfg(debug_assertions)]
fn start_native_qa(app: &tauri::AppHandle, data: PathBuf) {
    use tauri::Listener;
    let report_path = data.join("native-qa.json");
    let _ = std::fs::write(&report_path, "{\"status\":\"starting\"}");
    app.listen("agent-monitor:qa", move |event| {
        let _ = std::fs::write(&report_path, event.payload());
    });
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if let Some(i) = args.iter().position(|a| a == "--probe") {
        let Some(output) = args.get(i + 1) else {
            return;
        };
        let output = PathBuf::from(output);
        let dir = output
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join("probe-data");
        let result = (|| -> Result<Value, String> {
            storage::init(&dir)?;
            let mut c = collector::Collector::new(dir);
            let mut value = c.collect()?;
            if args.iter().any(|s| s == "--twice") {
                std::thread::sleep(Duration::from_secs(3));
                value = c.collect()?;
            }
            Ok(value)
        })();
        let value = result.unwrap_or_else(|e| json!({"probeError":e}));
        let _ = std::fs::write(
            output,
            serde_json::to_vec_pretty(&value).unwrap_or_default(),
        );
        return;
    }
    let data = storage::data_dir();
    if let Err(e) = storage::init(&data) {
        let _ = std::fs::write(data.join("startup-error.txt"), e);
        return;
    }
    let token = match bridge::token(&data) {
        Ok(t) => t,
        Err(e) => {
            let _ = std::fs::write(data.join("startup-error.txt"), e);
            return;
        }
    };
    let shared = Shared {
        snapshot: Arc::new(Mutex::new(
            json!({"loading":true,"stale":false,"agents":[],"processes":[],"tasks":[],"bindings":[]}),
        )),
        bridge_status: Arc::new(Mutex::new(
            json!({"healthy":false,"status":"starting","port":bridge::PORT}),
        )),
        reaper_status: Arc::new(Mutex::new(
            json!({"healthy":true,"thresholdHours":24,"status":"starting","lastSweepAt":null,"closedSessions":0,"expiredLocalThreads":0,"failed":0}),
        )),
        data: data.clone(),
        codex: std::env::var_os("AGENT_MONITOR_CODEX_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| sources::home().join(".codex")),
        bridge_token: Arc::new(token),
    };
    let minimized = args.iter().any(|a| a == "--minimized");
    #[allow(unused_mut)]
    let mut context = tauri::generate_context!();
    // Explicit developer-only CDP opt-in. Never compiled into the installed release.
    #[cfg(debug_assertions)]
    if let Ok(port) = std::env::var("AGENT_MONITOR_CDP_PORT")
        .unwrap_or_default()
        .parse::<u16>()
    {
        if port >= 1024 {
            for window in &mut context.config_mut().app.windows {
                window.additional_browser_args = Some(format!("--remote-debugging-port={port}"));
            }
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .manage(shared.clone())
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            get_detail,
            get_analytics,
            bind_agent,
            unbind_agent,
            set_agent_archived,
            link_task,
            get_pairing_token,
            open_conversation,
            open_data_dir,
            open_extension_dir,
            prepare_browser_attribution,
            set_autostart,
            get_setup_info,
            export_diagnostics
        ])
        .setup(move |app| {
            let show = MenuItem::with_id(app, "show", "Open Agent Monitor", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Agent Monitor", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            let mut tray = TrayIconBuilder::new()
                .tooltip("Agent Monitor · local agent observability")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            bridge::start(shared.clone());
            agentdock_control::start(shared.clone());
            let installed_web = app.path().resource_dir()?.join("web");
            let web_root = if installed_web.join("index.html").is_file() {
                installed_web
            } else {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap()
                    .join("dist")
            };
            remote::start(shared.clone(), web_root);
            progress::start(shared.clone(), app.handle().clone());
            let handle = app.handle().clone();
            let state = shared.clone();
            std::thread::spawn(move || {
                let mut collector = collector::Collector::new(state.data.clone());
                loop {
                    let result = collector.collect();
                    let mut snapshot = match result {
                        Ok(mut v) => {
                            v["loading"] = json!(false);
                            v["collectionCompletedAt"] = json!(model::now_ms());
                            v
                        }
                        Err(e) => {
                            let mut v = state
                                .snapshot
                                .lock()
                                .map(|v| v.clone())
                                .unwrap_or_else(|_| json!({}));
                            v["loading"] = json!(false);
                            v["stale"] = json!(true);
                            v["collectorError"] = json!(e);
                            v["lastAttemptAt"] = json!(model::now_ms());
                            v
                        }
                    };
                    snapshot["sources"]["bridge"] = state
                        .bridge_status
                        .lock()
                        .map(|s| s.clone())
                        .unwrap_or(Value::Null);
                    snapshot["sources"]["autoStop"] = state
                        .reaper_status
                        .lock()
                        .map(|s| s.clone())
                        .unwrap_or(Value::Null);
                    if let Ok(mut s) = state.snapshot.lock() {
                        progress::preserve_latest(&mut snapshot, &s);
                        *s = snapshot.clone();
                    }
                    let _ = handle.emit("monitor:snapshot", &snapshot);
                    std::thread::sleep(Duration::from_millis(2500));
                }
            });
            #[cfg(debug_assertions)]
            if std::env::args().any(|a| a == "--qa-native") {
                start_native_qa(app.handle(), shared.data.clone());
            }
            if minimized {
                if let Some(w) = app.get_webview_window("main") {
                    w.hide()?;
                }
            }
            Ok(())
        })
        .on_page_load(|_webview, _payload| {
            #[cfg(debug_assertions)]
            if matches!(_payload.event(), tauri::webview::PageLoadEvent::Finished)
                && std::env::args().any(|a| a == "--qa-native")
            {
                let _ = _webview.eval(include_str!("../../scripts/qa-native.js"));
            }
        })
        .on_window_event(|w, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = w.hide();
            }
        })
        .run(context)
        .expect("Agent Monitor desktop runtime failed");
}
