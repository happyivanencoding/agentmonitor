use crate::model::*;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

pub fn data_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("AgentMonitor")
}
pub fn open(path: &Path) -> Result<Connection, String> {
    let c = Connection::open(path).map_err(|e| e.to_string())?;
    c.busy_timeout(Duration::from_millis(1200))
        .map_err(|e| e.to_string())?;
    Ok(c)
}
pub fn init(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let c = open(&dir.join("monitor.sqlite"))?;
    c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;
    CREATE TABLE IF NOT EXISTS bindings(entity_id TEXT PRIMARY KEY, conversation_id TEXT NOT NULL, title TEXT NOT NULL, url TEXT NOT NULL, source TEXT NOT NULL, observed_at INTEGER NOT NULL);
    CREATE TABLE IF NOT EXISTS task_links(thread_id TEXT PRIMARY KEY,task_id TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS monitor_tasks(id TEXT PRIMARY KEY,conversation_id TEXT NOT NULL,title TEXT NOT NULL,goal TEXT NOT NULL,source_message_id TEXT NOT NULL,created_at INTEGER NOT NULL,updated_at INTEGER NOT NULL);
    CREATE INDEX IF NOT EXISTS monitor_tasks_conversation ON monitor_tasks(conversation_id,updated_at DESC);
    CREATE TABLE IF NOT EXISTS monitor_task_members(task_id TEXT NOT NULL,entity_id TEXT NOT NULL,user_message_id TEXT NOT NULL,linked_at INTEGER NOT NULL,PRIMARY KEY(task_id,entity_id,user_message_id));
    CREATE TABLE IF NOT EXISTS chat_requests(id TEXT PRIMARY KEY,conversation_id TEXT NOT NULL,user_message_id TEXT NOT NULL,conversation_title TEXT NOT NULL,prompt TEXT NOT NULL,started_at INTEGER NOT NULL,completed_at INTEGER,updated_at INTEGER NOT NULL,agentdock_task_id TEXT,project_hint TEXT,UNIQUE(conversation_id,user_message_id));
    CREATE INDEX IF NOT EXISTS chat_requests_time ON chat_requests(started_at DESC);
    CREATE INDEX IF NOT EXISTS chat_requests_conversation ON chat_requests(conversation_id,started_at DESC);
    CREATE TABLE IF NOT EXISTS request_tools(request_id TEXT NOT NULL,invocation_id TEXT NOT NULL,tool_name TEXT NOT NULL,action TEXT,args_summary TEXT,project_hint TEXT,entity_id TEXT,task_id TEXT,status TEXT NOT NULL,started_at INTEGER NOT NULL,finished_at INTEGER,ok INTEGER,PRIMARY KEY(request_id,invocation_id));
    CREATE INDEX IF NOT EXISTS request_tools_request ON request_tools(request_id,started_at);
    CREATE TABLE IF NOT EXISTS auto_stop_attempts(entity_id TEXT PRIMARY KEY,attempted_at INTEGER NOT NULL,ok INTEGER NOT NULL,detail TEXT NOT NULL);
    CREATE TABLE IF NOT EXISTS agent_dispositions(entity_id TEXT PRIMARY KEY,disposition TEXT NOT NULL CHECK(disposition IN ('archived')),updated_at INTEGER NOT NULL);
    CREATE TABLE IF NOT EXISTS observations(thread_id TEXT PRIMARY KEY,tokens INTEGER NOT NULL,last_seen INTEGER NOT NULL);
    CREATE TABLE IF NOT EXISTS usage_samples(id INTEGER PRIMARY KEY,thread_id TEXT NOT NULL,at_ms INTEGER NOT NULL,delta INTEGER NOT NULL CHECK(delta>0),model TEXT,project TEXT);
    CREATE INDEX IF NOT EXISTS usage_time ON usage_samples(at_ms);
    CREATE TABLE IF NOT EXISTS events(thread_id TEXT NOT NULL,event_id TEXT NOT NULL,at_ms INTEGER NOT NULL,json TEXT NOT NULL,PRIMARY KEY(thread_id,event_id));
    CREATE INDEX IF NOT EXISTS event_time ON events(at_ms);
    CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);
    PRAGMA user_version=1;").map_err(|e|e.to_string())?;
    let old_membership_schema: Option<String> = c.query_row(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='monitor_task_members'",
        [],
        |r| r.get(0),
    ).optional().map_err(|e| e.to_string())?;
    if old_membership_schema.as_deref().map(|s| s.contains("entity_id TEXT NOT NULL UNIQUE")).unwrap_or(false) {
        c.execute_batch("BEGIN; ALTER TABLE monitor_task_members RENAME TO monitor_task_members_old; CREATE TABLE monitor_task_members(task_id TEXT NOT NULL,entity_id TEXT NOT NULL,user_message_id TEXT NOT NULL,linked_at INTEGER NOT NULL,PRIMARY KEY(task_id,entity_id,user_message_id)); INSERT OR IGNORE INTO monitor_task_members SELECT task_id,entity_id,user_message_id,linked_at FROM monitor_task_members_old; DROP TABLE monitor_task_members_old; COMMIT;").map_err(|e| e.to_string())?;
    }
    let cutoff = now_ms() - 90 * 24 * 3600 * 1000;
    c.execute("DELETE FROM usage_samples WHERE at_ms < ?", [cutoff])
        .map_err(|e| e.to_string())?;
    c.execute("DELETE FROM events WHERE at_ms < ?", [cutoff])
        .map_err(|e| e.to_string())?;
    c.execute("DELETE FROM request_tools WHERE request_id IN (SELECT id FROM chat_requests WHERE started_at < ?)", [cutoff])
        .map_err(|e| e.to_string())?;
    c.execute("DELETE FROM chat_requests WHERE started_at < ?", [cutoff])
        .map_err(|e| e.to_string())?;
    c.execute(
        "INSERT OR IGNORE INTO settings VALUES('first_observed_at',?)",
        [now_ms().to_string()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BindingInput {
    pub entity_id: String,
    pub conversation_url: String,
    pub title: String,
    #[serde(default)]
    pub user_message_id: Option<String>,
    #[serde(default)]
    pub user_message_text: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RequestObservationInput {
    pub conversation_url: String,
    pub conversation_title: String,
    pub user_message_id: String,
    pub user_message_text: String,
    pub phase: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RequestToolInput {
    pub conversation_url: String,
    pub conversation_title: String,
    pub user_message_id: String,
    pub user_message_text: String,
    pub invocation_id: String,
    pub tool_name: String,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub args_summary: Option<String>,
    #[serde(default)]
    pub project_hint: Option<String>,
    #[serde(default)]
    pub entity_id: Option<String>,
    #[serde(default)]
    pub task_id: Option<String>,
    pub phase: String,
    #[serde(default)]
    pub ok: Option<bool>,
}

fn valid_message_id(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 160 && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '.'))
}

fn ensure_chat_request(c: &Connection, cid: &str, input: &RequestObservationInput, at: i64) -> Result<String, String> {
    if !valid_message_id(&input.user_message_id) || input.user_message_text.chars().count() > 4000 || input.conversation_title.chars().count() > 250 {
        return Err("Request observation metadata is invalid or too long".into());
    }
    if let Some(id) = c.query_row(
        "SELECT id FROM chat_requests WHERE conversation_id=? AND user_message_id=?",
        params![cid,input.user_message_id],
        |r| r.get::<_,String>(0),
    ).optional().map_err(|e| e.to_string())? {
        c.execute(
            "UPDATE chat_requests SET conversation_title=?,prompt=?,updated_at=? WHERE id=?",
            params![clipped_chars(&input.conversation_title,250),clipped_chars(&input.user_message_text,4000),at,id],
        ).map_err(|e| e.to_string())?;
        return Ok(id);
    }
    let id = format!("req_{}", uuid::Uuid::new_v4().simple());
    c.execute(
        "INSERT INTO chat_requests(id,conversation_id,user_message_id,conversation_title,prompt,started_at,completed_at,updated_at) VALUES(?,?,?,?,?,?,NULL,?)",
        params![id,cid,input.user_message_id,clipped_chars(&input.conversation_title,250),clipped_chars(&input.user_message_text,4000),at,at],
    ).map_err(|e| e.to_string())?;
    Ok(id)
}

pub fn observe_request(path: &Path, input: &RequestObservationInput) -> Result<Value, String> {
    let cid = conversation_id(&input.conversation_url)?;
    if !matches!(input.phase.as_str(), "started" | "completed") {
        return Err("Request phase must be started or completed".into());
    }
    let c = open(path)?;
    let at = now_ms();
    let id = ensure_chat_request(&c, &cid, input, at)?;
    if input.phase == "completed" {
        c.execute("UPDATE chat_requests SET completed_at=COALESCE(completed_at,?),updated_at=? WHERE id=?", params![at,at,id]).map_err(|e| e.to_string())?;
    }
    Ok(json!({"ok":true,"requestId":id,"conversationId":cid}))
}

pub fn observe_request_tool(path: &Path, input: &RequestToolInput) -> Result<Value, String> {
    let cid = conversation_id(&input.conversation_url)?;
    if !matches!(input.phase.as_str(), "started" | "completed") || !valid_message_id(&input.user_message_id) || !valid_message_id(&input.invocation_id) {
        return Err("Invalid request tool metadata".into());
    }
    if input.tool_name.is_empty() || input.tool_name.len() > 160 || !input.tool_name.starts_with("AgentDock.") {
        return Err("Only AgentDock tool activity is accepted".into());
    }
    let request_input = RequestObservationInput {
        conversation_url: input.conversation_url.clone(),
        conversation_title: input.conversation_title.clone(),
        user_message_id: input.user_message_id.clone(),
        user_message_text: input.user_message_text.clone(),
        phase: "started".into(),
    };
    let c = open(path)?;
    let at = now_ms();
    let request_id = ensure_chat_request(&c, &cid, &request_input, at)?;
    let task_id = input.task_id.as_deref().filter(|s| s.starts_with("tsk_") && s.len() <= 100).map(str::to_owned);
    let entity_id = input.entity_id.as_deref().filter(|s| (s.starts_with("acps_") || uuid::Uuid::parse_str(s).is_ok()) && s.len() <= 100).map(str::to_owned);
    let project_hint = input.project_hint.as_deref().filter(|s| !s.trim().is_empty()).map(|s| clipped_chars(s, 500));
    let action = input.action.as_deref().map(|s| clipped_chars(s, 80));
    let args_summary = input.args_summary.as_deref().map(|s| clipped_chars(s, 1200));
    if input.phase == "started" {
        c.execute(
            "INSERT INTO request_tools(request_id,invocation_id,tool_name,action,args_summary,project_hint,entity_id,task_id,status,started_at,finished_at,ok) VALUES(?,?,?,?,?,?,?,?,?, ?,NULL,NULL) ON CONFLICT(request_id,invocation_id) DO UPDATE SET tool_name=excluded.tool_name,action=excluded.action,args_summary=excluded.args_summary,project_hint=COALESCE(excluded.project_hint,request_tools.project_hint),entity_id=COALESCE(excluded.entity_id,request_tools.entity_id),task_id=COALESCE(excluded.task_id,request_tools.task_id),status='running'",
            params![request_id,input.invocation_id,input.tool_name,action,args_summary,project_hint,entity_id,task_id,"running",at],
        ).map_err(|e| e.to_string())?;
    } else {
        c.execute(
            "INSERT INTO request_tools(request_id,invocation_id,tool_name,action,args_summary,project_hint,entity_id,task_id,status,started_at,finished_at,ok) VALUES(?,?,?,?,?,?,?,?,?, ?,?,?) ON CONFLICT(request_id,invocation_id) DO UPDATE SET action=COALESCE(excluded.action,request_tools.action),args_summary=COALESCE(excluded.args_summary,request_tools.args_summary),project_hint=COALESCE(excluded.project_hint,request_tools.project_hint),entity_id=COALESCE(excluded.entity_id,request_tools.entity_id),task_id=COALESCE(excluded.task_id,request_tools.task_id),status=excluded.status,finished_at=excluded.finished_at,ok=excluded.ok",
            params![request_id,input.invocation_id,input.tool_name,action,args_summary,project_hint,entity_id,task_id,if input.ok==Some(false){"failed"}else{"completed"},at,at,input.ok.map(|v|if v{1}else{0})],
        ).map_err(|e| e.to_string())?;
    }
    if let Some(tid) = task_id.as_deref() {
        c.execute("UPDATE chat_requests SET agentdock_task_id=COALESCE(agentdock_task_id,?),updated_at=? WHERE id=?", params![tid,at,request_id]).map_err(|e| e.to_string())?;
    }
    if let Some(p) = project_hint.as_deref() {
        c.execute("UPDATE chat_requests SET project_hint=COALESCE(project_hint,?),updated_at=? WHERE id=?", params![p,at,request_id]).map_err(|e| e.to_string())?;
    }
    Ok(json!({"ok":true,"requestId":request_id,"conversationId":cid,"taskId":task_id}))
}

pub fn chat_requests(c: &Connection) -> Result<Vec<Value>, String> {
    let mut s = c.prepare("SELECT id,conversation_id,user_message_id,conversation_title,prompt,started_at,completed_at,updated_at,agentdock_task_id,project_hint FROM chat_requests ORDER BY started_at DESC LIMIT 800").map_err(|e| e.to_string())?;
    let rows=s.query_map([],|r|Ok(json!({
        "id":r.get::<_,String>(0)?,"conversationId":r.get::<_,String>(1)?,"userMessageId":r.get::<_,String>(2)?,"conversationTitle":r.get::<_,String>(3)?,"prompt":r.get::<_,String>(4)?,"startedAt":r.get::<_,i64>(5)?,"completedAt":r.get::<_,Option<i64>>(6)?,"updatedAt":r.get::<_,i64>(7)?,"agentdockTaskId":r.get::<_,Option<String>>(8)?,"projectHint":r.get::<_,Option<String>>(9)?
    }))).map_err(|e| e.to_string())?;
    let mut out=rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
    for req in &mut out {
        let id=req["id"].as_str().unwrap_or_default();
        let mut ts=c.prepare("SELECT invocation_id,tool_name,action,args_summary,project_hint,entity_id,task_id,status,started_at,finished_at,ok FROM request_tools WHERE request_id=? ORDER BY started_at LIMIT 80").map_err(|e| e.to_string())?;
        let tools=ts.query_map([id],|r|Ok(json!({
            "invocationId":r.get::<_,String>(0)?,"toolName":r.get::<_,String>(1)?,"action":r.get::<_,Option<String>>(2)?,"argsSummary":r.get::<_,Option<String>>(3)?,"projectHint":r.get::<_,Option<String>>(4)?,"entityId":r.get::<_,Option<String>>(5)?,"taskId":r.get::<_,Option<String>>(6)?,"status":r.get::<_,String>(7)?,"startedAt":r.get::<_,i64>(8)?,"finishedAt":r.get::<_,Option<i64>>(9)?,"ok":r.get::<_,Option<i64>>(10)?.map(|v|v!=0)
        }))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
        req["tools"]=json!(tools);
    }
    Ok(out)
}

pub fn conversation_id(raw: &str) -> Result<String, String> {
    let u = url::Url::parse(raw)
        .map_err(|_| "Enter a valid HTTPS ChatGPT conversation URL".to_string())?;
    if u.scheme() != "https"
        || u.host_str() != Some("chatgpt.com")
        || !u.username().is_empty()
        || u.password().is_some()
        || u.port().is_some()
    {
        return Err("Only https://chatgpt.com conversation URLs are allowed".into());
    }
    let segments = u.path_segments().ok_or("Invalid path")?.collect::<Vec<_>>();
    let pos = segments
        .iter()
        .position(|s| *s == "c")
        .ok_or("URL must identify a /c/<conversation-id> conversation")?;
    let id = segments.get(pos + 1).ok_or("Missing conversation ID")?;
    if uuid::Uuid::parse_str(id).is_err() || pos + 2 != segments.len() {
        return Err("Invalid conversation ID".into());
    }
    Ok((*id).to_owned())
}
pub fn bind(path: &Path, input: &BindingInput, source: &str) -> Result<Value, String> {
    let cid = conversation_id(&input.conversation_url)?;
    if input.entity_id.len() > 100 || input.title.chars().count() > 250 {
        return Err("Binding metadata is too long".into());
    }
    let c = open(path)?;
    let n=c.execute("INSERT INTO bindings(entity_id,conversation_id,title,url,source,observed_at) VALUES(?,?,?,?,?,?) ON CONFLICT(entity_id) DO UPDATE SET title=excluded.title,url=excluded.url,observed_at=excluded.observed_at WHERE bindings.conversation_id=excluded.conversation_id",
        params![input.entity_id,cid,input.title,format!("https://chatgpt.com/c/{cid}"),source,now_ms()]).map_err(|e|e.to_string())?;
    if n == 0 {
        return Err("CONFLICT: this entity already belongs to another conversation. Remove the binding explicitly in the desktop app before rebinding.".into());
    }
    let monitor_task_id = if source == "browser-tool-result" {
        observe_monitor_task(&c, &cid, input)?
    } else {
        None
    };
    Ok(json!({"ok":true,"conversationId":cid,"source":source,"monitorTaskId":monitor_task_id}))
}

fn clipped_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn task_title(text: &str) -> String {
    let clean = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let clean = clean.trim_matches(|c: char| c.is_whitespace() || "，。！？,.!?：:；;".contains(c));
    let mut title = clipped_chars(clean, 54);
    if clean.chars().count() > 54 {
        title.push('…');
    }
    if title.is_empty() { "未命名任务".into() } else { title }
}

fn continuation(text: &str) -> bool {
    let t = text.trim().to_ascii_lowercase();
    ["继续", "好的", "好，", "好,", "好 ", "那就", "就按", "对的", "可以", "然后", "再", "同时", "与此同时", "补充", "改成", "换成", "替代", "只要", "只需要", "记得", "别忘了", "continue", "yes", "ok", "then"].iter().any(|p| t.starts_with(p))
}

fn explicit_new_task(text: &str) -> bool {
    let t = text.trim().to_ascii_lowercase();
    ["新任务", "另外帮我", "另外，帮我", "另外,帮我", "另外我想", "现在我要", "接下来我要", "接下来帮我", "还有一件事", "顺便帮我", "另一个任务", "换个任务", "另外一个项目", "new task", "another task"].iter().any(|p| t.starts_with(p))
}

fn informational_query(text: &str) -> bool {
    let t = text.trim().to_ascii_lowercase();
    let directive = ["帮我", "请帮", "给我", "我要", "我想", "我希望", "替我", "开始", "继续", "执行", "运行", "实现", "修改", "修正", "加入", "添加", "删除", "构建", "搭建", "部署", "安装", "更新", "整理", "写一", "做一", "生成一", "帮忙", "please ", "create ", "build ", "implement ", "run ", "continue ", "update "]
        .iter()
        .any(|p| t.starts_with(p) || t.contains("帮我"));
    if directive {
        return false;
    }
    t.ends_with('?')
        || t.ends_with('？')
        || ["是什么", "为什么", "哪本", "哪个", "哪些", "多少", "怎么样", "有没有", "是否", "好吗", "了吗", "到哪", "进度如何", "怎么回事", "什么意思"]
            .iter()
            .any(|p| t.contains(p))
}

fn observe_monitor_task(c: &Connection, cid: &str, input: &BindingInput) -> Result<Option<String>, String> {
    let Some(message_id) = input.user_message_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) else { return Ok(None); };
    let Some(text) = input.user_message_text.as_deref().map(str::trim).filter(|s| !s.is_empty()) else { return Ok(None); };
    if message_id.len() > 120 || text.chars().count() > 2000 { return Err("Task attribution metadata is too long".into()); }
    if informational_query(text) { return Ok(None); }

    if let Some(existing) = c.query_row("SELECT task_id FROM monitor_task_members WHERE entity_id=? AND user_message_id=?", params![input.entity_id,message_id], |r| r.get::<_,String>(0)).optional().map_err(|e|e.to_string())? {
        return Ok(Some(existing));
    }

    let latest = c.query_row(
        "SELECT id,updated_at FROM monitor_tasks WHERE conversation_id=? ORDER BY updated_at DESC LIMIT 1",
        [cid],
        |r| Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?)),
    ).optional().map_err(|e|e.to_string())?;
    let at = now_ms();
    let reuse = latest.as_ref().map(|(_,updated)| continuation(text) || (!explicit_new_task(text) && at-*updated < 6*3600*1000)).unwrap_or(false);
    let task_id = if reuse {
        latest.unwrap().0
    } else {
        let id = format!("mt_{}", uuid::Uuid::new_v4().simple());
        c.execute(
            "INSERT INTO monitor_tasks(id,conversation_id,title,goal,source_message_id,created_at,updated_at) VALUES(?,?,?,?,?,?,?)",
            params![id,cid,task_title(text),clipped_chars(text,500),message_id,at,at],
        ).map_err(|e|e.to_string())?;
        id
    };
    c.execute(
        "INSERT OR IGNORE INTO monitor_task_members(task_id,entity_id,user_message_id,linked_at) VALUES(?,?,?,?)",
        params![task_id,input.entity_id,message_id,at],
    ).map_err(|e|e.to_string())?;
    c.execute("UPDATE monitor_tasks SET updated_at=? WHERE id=?", params![at,task_id]).map_err(|e|e.to_string())?;
    Ok(Some(task_id))
}

pub fn monitor_tasks(c: &Connection) -> Result<Vec<Value>, String> {
    let mut s=c.prepare("SELECT id,conversation_id,title,goal,source_message_id,created_at,updated_at FROM monitor_tasks ORDER BY created_at DESC").map_err(|e|e.to_string())?;
    let rows=s.query_map([],|r|Ok(json!({"id":r.get::<_,String>(0)?,"conversationId":r.get::<_,String>(1)?,"title":r.get::<_,String>(2)?,"goal":r.get::<_,String>(3)?,"sourceMessageId":r.get::<_,String>(4)?,"createdAt":r.get::<_,i64>(5)?,"updatedAt":r.get::<_,i64>(6)?}))).map_err(|e|e.to_string())?;
    let mut out=rows.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
    for task in &mut out {
        let id=task["id"].as_str().unwrap_or_default();
        let mut ms=c.prepare("SELECT entity_id,user_message_id,linked_at FROM monitor_task_members WHERE task_id=? ORDER BY linked_at").map_err(|e|e.to_string())?;
        let members=ms.query_map([id],|r|Ok(json!({"entityId":r.get::<_,String>(0)?,"userMessageId":r.get::<_,String>(1)?,"linkedAt":r.get::<_,i64>(2)?}))).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
        task["members"]=json!(members);
        task["origin"]=json!("monitor-auto");
    }
    Ok(out)
}

pub fn auto_stop_attempts(c: &Connection) -> Vec<(String,i64,bool)> {
    c.prepare("SELECT entity_id,attempted_at,ok FROM auto_stop_attempts").and_then(|mut s|s.query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get::<_,i64>(2)?!=0)))?.collect()).unwrap_or_default()
}

pub fn record_auto_stop(path: &Path, entity_id: &str, ok: bool, detail: &str) -> Result<(), String> {
    let c=open(path)?;
    c.execute("INSERT INTO auto_stop_attempts(entity_id,attempted_at,ok,detail) VALUES(?,?,?,?) ON CONFLICT(entity_id) DO UPDATE SET attempted_at=excluded.attempted_at,ok=excluded.ok,detail=excluded.detail",params![entity_id,now_ms(),if ok{1}else{0},clipped_chars(detail,500)]).map_err(|e|e.to_string())?;
    Ok(())
}
pub fn bindings(c: &Connection) -> Result<Vec<Value>, String> {
    let mut s = c
        .prepare("SELECT entity_id,conversation_id,title,url,source,observed_at FROM bindings")
        .map_err(|e| e.to_string())?;
    let rows=s.query_map([],|r|Ok(json!({"entityId":r.get::<_,String>(0)?,"conversationId":r.get::<_,String>(1)?,"title":r.get::<_,String>(2)?,"url":r.get::<_,String>(3)?,"source":r.get::<_,String>(4)?,"observedAt":r.get::<_,i64>(5)?}))).map_err(|e|e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}
pub fn task_links(c: &Connection) -> Vec<(String, String)> {
    c.prepare("SELECT thread_id,task_id FROM task_links")
        .and_then(|mut s| s.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect())
        .unwrap_or_default()
}

pub fn archived(c: &Connection) -> Vec<(String, i64)> {
    c.prepare("SELECT entity_id,updated_at FROM agent_dispositions WHERE disposition='archived'")
        .and_then(|mut s| s.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect())
        .unwrap_or_default()
}

pub fn set_archived(path: &Path, entity_id: &str, archived: bool) -> Result<i64, String> {
    let c = open(path)?;
    if archived {
        let at = now_ms();
        c.execute(
            "INSERT INTO agent_dispositions(entity_id,disposition,updated_at) VALUES(?,'archived',?) ON CONFLICT(entity_id) DO UPDATE SET disposition='archived',updated_at=excluded.updated_at",
            params![entity_id, at],
        )
        .map_err(|e| e.to_string())?;
        Ok(at)
    } else {
        c.execute("DELETE FROM agent_dispositions WHERE entity_id=?", [entity_id])
            .map_err(|e| e.to_string())?;
        Ok(now_ms())
    }
}
/// Re-baseline on first sight, negative counters, >30s observation gaps, and local midnight.
pub fn sample_delta(previous: Option<(i64, i64)>, total: i64, at: i64) -> Option<i64> {
    let (old, last) = previous?;
    let same_day = chrono::DateTime::from_timestamp_millis(at)?
        .with_timezone(&chrono::Local)
        .date_naive()
        == chrono::DateTime::from_timestamp_millis(last)?
            .with_timezone(&chrono::Local)
            .date_naive();
    if !same_day || at - last > 30_000 || at < last || total <= old {
        None
    } else {
        Some(total - old)
    }
}
pub fn sample(
    c: &Connection,
    id: &str,
    total: i64,
    at: i64,
    model: &str,
    project: &str,
) -> Result<(), String> {
    let prev = c
        .query_row(
            "SELECT tokens,last_seen FROM observations WHERE thread_id=?",
            [id],
            |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if let Some(delta) = sample_delta(prev, total, at) {
        c.execute(
            "INSERT INTO usage_samples(thread_id,at_ms,delta,model,project) VALUES(?,?,?,?,?)",
            params![id, at, delta, model, project],
        )
        .map_err(|e| e.to_string())?;
    }
    c.execute("INSERT INTO observations VALUES(?,?,?) ON CONFLICT(thread_id) DO UPDATE SET tokens=excluded.tokens,last_seen=excluded.last_seen",params![id,total,at]).map_err(|e|e.to_string())?;
    Ok(())
}
pub fn save_events(c: &Connection, id: &str, events: &[Event]) -> Result<(), String> {
    let mut s=c.prepare_cached("INSERT INTO events VALUES(?,?,?,?) ON CONFLICT(thread_id,event_id) DO UPDATE SET at_ms=excluded.at_ms,json=excluded.json WHERE events.json!=excluded.json").map_err(|e|e.to_string())?;
    for e in events {
        s.execute(params![
            id,
            e.id,
            e.start_ms,
            serde_json::to_string(e).map_err(|e| e.to_string())?
        ])
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
pub fn saved_events(c: &Connection, id: &str) -> Result<Vec<Event>, String> {
    let mut s = c
        .prepare("SELECT json FROM events WHERE thread_id=? ORDER BY at_ms DESC LIMIT 4000")
        .map_err(|e| e.to_string())?;
    let rows = s
        .query_map([id], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    Ok(rows
        .filter_map(Result::ok)
        .filter_map(|s| serde_json::from_str(&s).ok())
        .collect())
}
pub fn analytics(c: &Connection, days: i64) -> Value {
    let now = chrono::Local::now();
    let start = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .and_then(|n| n.and_local_timezone(chrono::Local).earliest())
        .map(|n| n.timestamp_millis())
        .unwrap_or(now_ms())
        - (days.clamp(1, 30) - 1) * 86400000;
    let total = c
        .query_row(
            "SELECT COALESCE(SUM(delta),0) FROM usage_samples WHERE at_ms>=?",
            [start],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0);
    fn group(c: &Connection, field: &str, start: i64) -> Vec<Value> {
        let sql=format!("SELECT {field},SUM(delta) FROM usage_samples WHERE at_ms>=? GROUP BY {field} ORDER BY SUM(delta) DESC LIMIT 20");
        c.prepare(&sql).and_then(|mut s|s.query_map([start],|r|Ok(json!({"name":r.get::<_,Option<String>>(0)?.unwrap_or_else(||"unknown".into()),"tokens":r.get::<_,i64>(1)?})))?.collect()).unwrap_or_default()
    }
    let first = c
        .query_row(
            "SELECT value FROM settings WHERE key='first_observed_at'",
            [],
            |r| r.get::<_, String>(0),
        )
        .ok()
        .and_then(|s| s.parse::<i64>().ok());
    let turns:Vec<Value>=c.prepare("SELECT json FROM events WHERE at_ms>=? AND event_id LIKE 'turn:%' ORDER BY at_ms DESC LIMIT 1000").and_then(|mut s|s.query_map([start],|r|r.get::<_,String>(0))?.collect::<Result<Vec<_>,_>>()).unwrap_or_default().iter().filter_map(|s|serde_json::from_str(s).ok()).collect();
    json!({"days":days,"startMs":start,"observedTokens":total,"firstObservedAt":first,"byModel":group(c,"model",start),"byProject":group(c,"project",start),"byThread":group(c,"thread_id",start),"completedTurns":turns.iter().filter(|t|t["status"]=="completed").count(),"failedTurns":turns.iter().filter(|t|t["status"]=="failed").count(),"coverage":"continuous-observation-deltas; gaps and first lifetime samples excluded"})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_lifetime_as_today() {
        assert_eq!(sample_delta(None, 72810, 1788692259000), None);
    }
    #[test]
    fn exact_positive_delta() {
        assert_eq!(
            sample_delta(Some((72810, 1788692259000)), 72910, 1788692262000),
            Some(100)
        );
    }
    #[test]
    fn reset_and_gap_rebaseline() {
        assert_eq!(
            sample_delta(Some((800, 1788692259000)), 20, 1788692262000),
            None
        );
        assert_eq!(
            sample_delta(Some((800, 1788692259000)), 1000, 1788692300000),
            None
        );
    }
    #[test]
    fn url_validation() {
        assert!(
            conversation_id("https://chatgpt.com/c/12345678-1234-1234-1234-123456789abc").is_ok()
        );
        assert!(
            conversation_id("https://evil.com/c/12345678-1234-1234-1234-123456789abc").is_err()
        );
        assert!(conversation_id(
            "https://chatgpt.com@evil.com/c/12345678-1234-1234-1234-123456789abc"
        )
        .is_err());
        assert!(conversation_id("javascript:alert(1)").is_err());
    }
    #[test]
    fn conflicts_never_silently_reassign() {
        let p = std::env::temp_dir().join(format!("agentmonitor-test-{}", uuid::Uuid::new_v4()));
        init(&p).unwrap();
        let db = p.join("monitor.sqlite");
        let mut b = BindingInput {
            entity_id: "acps_test".into(),
            title: "test".into(),
            conversation_url: "https://chatgpt.com/c/12345678-1234-1234-1234-123456789abc".into(),
            user_message_id: None,
            user_message_text: None,
        };
        bind(&db, &b, "test").unwrap();
        b.conversation_url = "https://chatgpt.com/c/12345678-1234-1234-1234-123456789abd".into();
        assert!(bind(&db, &b, "test").is_err());
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn archived_disposition_roundtrip() {
        let p = std::env::temp_dir().join(format!("agentmonitor-test-{}", uuid::Uuid::new_v4()));
        init(&p).unwrap();
        let db = p.join("monitor.sqlite");
        let at = set_archived(&db, "thread-1", true).unwrap();
        let c = open(&db).unwrap();
        assert_eq!(archived(&c), vec![("thread-1".to_string(), at)]);
        drop(c);
        set_archived(&db, "thread-1", false).unwrap();
        assert!(archived(&open(&db).unwrap()).is_empty());
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn automatic_monitor_task_reuses_continuation_and_splits_explicit_new_goal() {
        let p = std::env::temp_dir().join(format!("agentmonitor-task-test-{}", uuid::Uuid::new_v4()));
        init(&p).unwrap();
        let db = p.join("monitor.sqlite");
        let url = "https://chatgpt.com/c/12345678-1234-1234-1234-123456789abc".to_string();
        let first = BindingInput { entity_id:"acps_first0001".into(), conversation_url:url.clone(), title:"Experiment".into(), user_message_id:Some("u1".into()), user_message_text:Some("帮我做 GPT-6 替代 GPT-5.6 的实验".into()) };
        let follow = BindingInput { entity_id:"acps_second002".into(), conversation_url:url.clone(), title:"Experiment".into(), user_message_id:Some("u2".into()), user_message_text:Some("继续完成实验，只是提前告诉我 ACP ID".into()) };
        let next = BindingInput { entity_id:"acps_third0003".into(), conversation_url:url, title:"Experiment".into(), user_message_id:Some("u3".into()), user_message_text:Some("另外帮我整理项目文档并提交".into()) };
        let a=bind(&db,&first,"browser-tool-result").unwrap()["monitorTaskId"].as_str().unwrap().to_string();
        let b=bind(&db,&follow,"browser-tool-result").unwrap()["monitorTaskId"].as_str().unwrap().to_string();
        let c=bind(&db,&next,"browser-tool-result").unwrap()["monitorTaskId"].as_str().unwrap().to_string();
        assert_eq!(a,b);
        assert_ne!(a,c);
        assert_eq!(monitor_tasks(&open(&db).unwrap()).unwrap().len(),2);
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn informational_query_does_not_become_a_monitor_task() {
        let p = std::env::temp_dir().join(format!("agentmonitor-query-test-{}", uuid::Uuid::new_v4()));
        init(&p).unwrap();
        let db = p.join("monitor.sqlite");
        let q = BindingInput { entity_id:"acps_query0001".into(), conversation_url:"https://chatgpt.com/c/12345678-1234-1234-1234-123456789abc".into(), title:"Query".into(), user_message_id:Some("q1".into()), user_message_text:Some("最新一次实验小说是哪本？我审核了吗？".into()) };
        assert!(bind(&db,&q,"browser-tool-result").unwrap()["monitorTaskId"].is_null());
        assert!(monitor_tasks(&open(&db).unwrap()).unwrap().is_empty());
        let _ = std::fs::remove_dir_all(p);
    }

    #[test]
    fn every_observed_user_message_is_one_request() {
        let p = std::env::temp_dir().join(format!("agentmonitor-request-test-{}", uuid::Uuid::new_v4()));
        init(&p).unwrap();let db=p.join("monitor.sqlite");let url="https://chatgpt.com/c/12345678-1234-1234-1234-123456789abc".to_string();
        let started=RequestObservationInput{conversation_url:url.clone(),conversation_title:"A".into(),user_message_id:"user-1".into(),user_message_text:"这只是一个问题".into(),phase:"started".into()};
        observe_request(&db,&started).unwrap();observe_request(&db,&started).unwrap();
        let mut completed=started.clone();completed.phase="completed".into();observe_request(&db,&completed).unwrap();
        let rows=chat_requests(&open(&db).unwrap()).unwrap();assert_eq!(rows.len(),1);assert!(rows[0]["completedAt"].as_i64().is_some());
        let _=std::fs::remove_dir_all(p);
    }

    #[test]
    fn request_tools_keep_initial_and_supplemental_task_ids() {
        let p = std::env::temp_dir().join(format!("agentmonitor-request-tool-test-{}", uuid::Uuid::new_v4()));
        init(&p).unwrap();let db=p.join("monitor.sqlite");let url="https://chatgpt.com/c/12345678-1234-1234-1234-123456789abc".to_string();
        let make=|inv:&str,task:&str|RequestToolInput{conversation_url:url.clone(),conversation_title:"Work".into(),user_message_id:"user-2".into(),user_message_text:"完成复杂开发".into(),invocation_id:inv.into(),tool_name:"AgentDock.task_manage".into(),action:Some("create".into()),args_summary:None,project_hint:Some("C:\\dev\\agent-monitor".into()),entity_id:None,task_id:Some(task.into()),phase:"completed".into(),ok:Some(true)};
        observe_request_tool(&db,&make("inv-1","tsk_initial")).unwrap();observe_request_tool(&db,&make("inv-2","tsk_supplement")).unwrap();
        let rows=chat_requests(&open(&db).unwrap()).unwrap();assert_eq!(rows.len(),1);assert_eq!(rows[0]["agentdockTaskId"],"tsk_initial");
        let ids=rows[0]["tools"].as_array().unwrap().iter().filter_map(|t|t["taskId"].as_str()).collect::<Vec<_>>();assert_eq!(ids,vec!["tsk_initial","tsk_supplement"]);
        let _=std::fs::remove_dir_all(p);
    }

    #[test]
    fn one_acp_can_participate_in_two_explicit_tasks() {
        let p = std::env::temp_dir().join(format!("agentmonitor-reuse-test-{}", uuid::Uuid::new_v4()));
        init(&p).unwrap();
        let db = p.join("monitor.sqlite");
        let url="https://chatgpt.com/c/12345678-1234-1234-1234-123456789abc".to_string();
        let first=BindingInput{entity_id:"acps_reused001".into(),conversation_url:url.clone(),title:"Work".into(),user_message_id:Some("u1".into()),user_message_text:Some("帮我完成第一个实验".into())};
        let second=BindingInput{entity_id:"acps_reused001".into(),conversation_url:url,title:"Work".into(),user_message_id:Some("u2".into()),user_message_text:Some("另外帮我整理文档并提交".into())};
        let a=bind(&db,&first,"browser-tool-result").unwrap()["monitorTaskId"].as_str().unwrap().to_string();
        let b=bind(&db,&second,"browser-tool-result").unwrap()["monitorTaskId"].as_str().unwrap().to_string();
        assert_ne!(a,b);
        assert_eq!(monitor_tasks(&open(&db).unwrap()).unwrap().len(),2);
        let _ = std::fs::remove_dir_all(p);
    }
}
