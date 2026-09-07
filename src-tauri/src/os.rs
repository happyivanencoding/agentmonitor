use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::Path,
    time::Duration,
};
use sysinfo::{ProcessesToUpdate, System};

pub struct ProcessReader {
    system: System,
    initialized: bool,
}
impl ProcessReader {
    pub fn new() -> Self {
        Self {
            system: System::new(),
            initialized: false,
        }
    }
    pub fn refresh(&mut self) -> Vec<Value> {
        self.system.refresh_processes(ProcessesToUpdate::All, true);
        let mut result = Vec::new();
        let mut included = HashSet::new();
        let known = [
            "codex",
            "agentdock",
            "node",
            "python",
            "git",
            "gradle",
            "java",
            "adb",
            "chatgpt",
            "chrome",
            "msedge",
            "pwsh",
            "powershell",
            "agent-monitor",
        ];
        for (id, p) in self.system.processes() {
            let name = p.name().to_string_lossy().to_lowercase();
            if known.iter().any(|n| name.starts_with(n)) {
                included.insert(*id);
            }
        }
        for _ in 0..4 {
            for (id, p) in self.system.processes() {
                if p.parent().map(|p| included.contains(&p)).unwrap_or(false) {
                    included.insert(*id);
                }
            }
        }
        for (id, p) in self.system.processes() {
            if !included.contains(id) {
                continue;
            }
            let name = p.name().to_string_lossy().to_string();
            let exe = p.exe().map(|s| s.to_string_lossy().to_string());
            let lower = exe.as_deref().unwrap_or("").to_lowercase();
            let role = if name.eq_ignore_ascii_case("codex.exe")
                && lower.contains("agentclientprotocol")
            {
                "ACP host"
            } else if name.eq_ignore_ascii_case("codex.exe") {
                "Codex host"
            } else if name.to_lowercase().starts_with("agentdock") {
                "AgentDock"
            } else if name.to_lowercase().contains("chrome")
                || name.to_lowercase().contains("msedge")
                || name.eq_ignore_ascii_case("ChatGPT.exe")
            {
                "Browser / WebView"
            } else {
                "Supporting process"
            };
            result.push(json!({"pid":id.as_u32(),"parentPid":p.parent().map(|p|p.as_u32()),"name":name,"exe":exe,"role":role,"cpu":if self.initialized{Some(p.cpu_usage())}else{None},"memory":p.memory(),"startedAt":p.start_time()*1000,"status":p.status().to_string()}));
        }
        self.initialized = true;
        result.sort_by_key(|p| p["pid"].as_u64().unwrap_or(0));
        result
    }
}

#[cfg(windows)]
mod rm {
    use std::ffi::c_void;
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct UniqueProcess {
        pid: u32,
        start: FileTime,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct ProcessInfo {
        process: UniqueProcess,
        name: [u16; 256],
        service: [u16; 64],
        app_type: i32,
        status: u32,
        session: u32,
        restartable: i32,
    }
    impl Default for ProcessInfo {
        fn default() -> Self {
            unsafe { std::mem::zeroed() }
        }
    }
    #[link(name = "Rstrtmgr")]
    extern "system" {
        fn RmStartSession(handle: *mut u32, flags: u32, key: *mut u16) -> u32;
        fn RmRegisterResources(
            handle: u32,
            nfiles: u32,
            files: *const *const u16,
            napps: u32,
            apps: *const c_void,
            nservices: u32,
            services: *const *const u16,
        ) -> u32;
        fn RmGetList(
            handle: u32,
            needed: *mut u32,
            count: *mut u32,
            apps: *mut ProcessInfo,
            reasons: *mut u32,
        ) -> u32;
        fn RmEndSession(handle: u32) -> u32;
    }
    pub fn owners(path: &std::path::Path) -> Result<Vec<u32>, String> {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut h = 0;
        let mut key = [0u16; 33];
        unsafe {
            let rc = RmStartSession(&mut h, 0, key.as_mut_ptr());
            if rc != 0 {
                return Err(format!("RestartManager start {rc}"));
            }
            struct Guard(u32);
            impl Drop for Guard {
                fn drop(&mut self) {
                    unsafe {
                        RmEndSession(self.0);
                    }
                }
            }
            let _guard = Guard(h);
            let filename = wide.as_ptr();
            let rc = RmRegisterResources(h, 1, &filename, 0, std::ptr::null(), 0, std::ptr::null());
            if rc != 0 {
                return Err(format!("RestartManager register {rc}"));
            }
            let (mut need, mut count, mut reasons) = (0, 0, 0);
            let rc = RmGetList(h, &mut need, &mut count, std::ptr::null_mut(), &mut reasons);
            if rc == 0 {
                return Ok(Vec::new());
            }
            if rc != 234 {
                return Err(format!("RestartManager query {rc}"));
            }
            let mut infos = vec![ProcessInfo::default(); need as usize];
            count = need;
            let rc = RmGetList(h, &mut need, &mut count, infos.as_mut_ptr(), &mut reasons);
            if rc != 0 {
                return Err(format!("RestartManager list {rc}"));
            }
            Ok(infos
                .iter()
                .take(count as usize)
                .map(|p| p.process.pid)
                .collect())
        }
    }
}
pub fn lock_owners(dir: &Path) -> (HashMap<String, Vec<u32>>, Vec<String>) {
    let mut map = HashMap::new();
    let mut errors = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (map, vec!["Thread lock directory unavailable".into()]);
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let id = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if uuid::Uuid::parse_str(id).is_err() {
            continue;
        }
        #[cfg(windows)]
        match rm::owners(&path) {
            Ok(pids) => {
                map.insert(id.into(), pids);
            }
            Err(e) => errors.push(e),
        }
        #[cfg(not(windows))]
        {
            errors.push("Lock-owner mapping requires Windows Restart Manager".into());
            break;
        }
    }
    (map, errors)
}
pub fn agentdock_health(runtime: &Path) -> Value {
    let config = std::fs::read_to_string(runtime)
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok());
    let port = config
        .as_ref()
        .and_then(|v| v["port"].as_u64())
        .unwrap_or(8765) as u16;
    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    let result = (|| -> std::io::Result<bool> {
        let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(300))?;
        stream.set_read_timeout(Some(Duration::from_millis(400)))?;
        stream.set_write_timeout(Some(Duration::from_millis(300)))?;
        stream.write_all(
            format!("GET /healthz HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )?;
        let mut buf = [0u8; 2048];
        let n = stream.read(&mut buf)?;
        Ok(String::from_utf8_lossy(&buf[..n])
            .lines()
            .next()
            .unwrap_or("")
            .contains(" 200 "))
    })();
    json!({"healthy":matches!(result,Ok(true)),"port":port,"source":"runtime.json + loopback /healthz","error":result.err().map(|e|e.kind().to_string())})
}
