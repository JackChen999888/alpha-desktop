use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Running,
    Completed,
    Attention,
    Failed,
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub agent: String,
    pub session: String,
    #[serde(default)]
    pub project: String,
    pub phase: Phase,
    pub at: i64,
    #[serde(default)]
    pub detail: String,
}
impl Event {
    /// No heartbeat is guaranteed by either client. Old active events are unverified,
    /// never inferred to have completed or failed.
    pub fn effective_phase(&self, now: i64) -> Phase {
        if matches!(self.phase, Phase::Running | Phase::Attention)
            && (now.saturating_sub(self.at) > 30 * 60 * 1000 || self.at > now.saturating_add(5 * 60 * 1000)) {
            Phase::Unknown
        } else { self.phase.clone() }
    }
    pub fn label(&self) -> &str {
        match self.phase {
            Phase::Running => "运行中",
            Phase::Completed => "已完成",
            Phase::Attention => "等待操作",
            Phase::Failed => "失败",
            Phase::Unknown => "状态未知",
        }
    }
}

pub fn from_hook(agent: &str, v: &serde_json::Value, at: i64) -> Result<Event, String> {
    if !["Codex", "Claude Code"].contains(&agent) {
        return Err("未知客户端".into());
    }
    let session = v["session_id"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= 160)
        .ok_or("缺少会话编号")?;
    let name = v["hook_event_name"].as_str().ok_or("缺少事件名称")?;
    let phase = match name {
        "UserPromptSubmit" | "PreToolUse" | "PostToolUse" | "PreModelRequest" | "SubagentStart" => {
            Phase::Running
        }
        "PermissionRequest" | "Interrupt" => Phase::Attention,
        "Notification" => match v["notification_type"].as_str() {
            Some("permission_prompt" | "idle_prompt") => Phase::Attention,
            _ => return Err("非等待操作通知，不改变任务状态".into()),
        },
        "Stop" | "SubagentStop" => Phase::Completed,
        "StopFailure" => Phase::Failed,
        "PostModelRequest" => match v["status"].as_str() {
            Some("failed") => Phase::Failed,
            Some("cancelled") => Phase::Attention,
            _ => Phase::Running,
        },
        "SessionEnd" => Phase::Unknown,
        _ => return Err("未订阅的事件".into()),
    };
    let child = v["agent_id"]
        .as_str()
        .filter(|s| !s.is_empty() && s.len() < 100);
    if matches!(name, "SubagentStart" | "SubagentStop") && child.is_none() {
        return Err("子任务事件缺少编号，不改变父任务状态".into());
    }
    let session = if let Some(id) = child {
        format!("{} / {}", session, id)
    } else {
        session.into()
    };
    Ok(Event {
        agent: agent.into(),
        session,
        project: v["cwd"].as_str().unwrap_or("").chars().take(500).collect(),
        phase,
        at,
        detail: name.into(),
    })
}
pub fn ingest_cli() -> Result<(), String> {
    use std::io::Read;
    let args: Vec<_> = std::env::args().collect();
    let pos = args
        .iter()
        .position(|a| a == "--agent-event")
        .ok_or("缺少参数")?;
    let agent = args.get(pos + 1).ok_or("缺少客户端")?;
    let mut input = String::new();
    std::io::stdin()
        .take(2_000_001)
        .read_to_string(&mut input)
        .map_err(|e| e.to_string())?;
    if input.len() > 2_000_000 {
        return Err("事件过大".into());
    }
    let v = serde_json::from_str(&input).map_err(|_| "事件不是有效JSON")?;
    let event = from_hook(agent, &v, chrono::Utc::now().timestamp_millis())?;
    // Only lifecycle metadata is persisted, never prompts, responses or transcripts.
    let digest = ring::digest::digest(
        &ring::digest::SHA256,
        format!("{}\0{}", event.agent, event.session).as_bytes(),
    );
    let name = digest
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    // Separate event files avoid concurrent hooks overwriting a newer update.
    let folder = super::root().join("events");
    super::save(
        &folder.join(format!("{}-{}-{}.json", event.at, std::process::id(), name)),
        &event,
    )?;
    if let Ok(entries) = std::fs::read_dir(&folder) {
        let mut files: Vec<_> = entries
            .flatten()
            .filter(|e| e.path().extension().and_then(|e| e.to_str()) == Some("json"))
            .collect();
        files.sort_by_key(|f| std::cmp::Reverse(f.file_name()));
        let mut sessions = std::collections::HashSet::new();
        for f in files {
            let filename = f.file_name().to_string_lossy().to_string();
            let Some(key) = filename.rsplit('-').next() else {
                continue;
            };
            if !sessions.insert(key.to_string()) || sessions.len() > 100 {
                let _ = std::fs::remove_file(f.path());
            }
        }
    }
    Ok(())
}

fn merge_hooks(
    mut doc: serde_json::Value,
    agent: &str,
    command: &str,
) -> Result<serde_json::Value, String> {
    use serde_json::json;
    let obj = doc.as_object_mut().ok_or("Hook配置必须是JSON对象")?;
    let hooks = obj
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or("hooks格式异常")?;
    let mut events = vec![
        "UserPromptSubmit",
        "PreToolUse",
        "PostToolUse",
        "PermissionRequest",
        "Stop",
        "SubagentStart",
        "SubagentStop",
        "SessionEnd",
    ];
    if agent == "Codex" {
        events.extend(["PreModelRequest", "PostModelRequest", "Interrupt"]);
    } else {
        events.extend(["Notification", "StopFailure"]);
    }
    for name in events {
        let groups = hooks
            .entry(name)
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or("Hook事件格式异常")?;
        for group in groups.iter_mut() {
            if let Some(handlers) = group["hooks"].as_array_mut() {
                handlers.retain(|h| {
                    !h["command"]
                        .as_str()
                        .unwrap_or("")
                        .contains("desktop-island-event.exe")
                        && !h["command"]
                            .as_str()
                            .unwrap_or("")
                            .contains("alpha-agent-hook.ps1")
                });
            }
        }
        groups.retain(|g| g["hooks"].as_array().is_none_or(|h| !h.is_empty()));
        let mut handler = json!({"type":"command","command":command,"timeout":3});
        if agent == "Codex" {
            handler["commandWindows"] = json!(command);
        }
        let mut group = json!({"hooks":[handler]});
        if name == "Notification" {
            group["matcher"] = json!("permission_prompt|idle_prompt");
        }
        groups.push(group);
    }
    Ok(doc)
}
pub fn install_hooks() -> Result<String, String> {
    let exe = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .with_file_name("desktop-island-event.exe");
    if !exe.is_file() {
        return Err("缺少状态接入程序，请重新安装Alpha".into());
    }
    let script = exe.with_file_name("alpha-agent-hook.ps1");
    crate::model::atomic_write(
        &script,
        br#"param([string]$Agent)
$ErrorActionPreference = 'SilentlyContinue'
$OutputEncoding = New-Object System.Text.UTF8Encoding($false)
[Console]::InputEncoding = $OutputEncoding
[Console]::OutputEncoding = $OutputEncoding
$payload = [Console]::In.ReadToEnd()
if ($payload.Length -le 2000000) {
    $null = $payload | & (Join-Path $PSScriptRoot 'desktop-island-event.exe') --agent-event $Agent
}
[Console]::Out.WriteLine('{}')
exit 0
"#,
    )
    .map_err(|e| e.to_string())?;
    let home =
        std::path::PathBuf::from(std::env::var("USERPROFILE").map_err(|_| "未找到用户目录")?);
    let codex = std::env::var("CODEX_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| home.join(".codex"));
    let claude = std::env::var("CLAUDE_CONFIG_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| home.join(".claude"));
    let mut writes = Vec::new();
    for (path, agent) in [
        (codex.join("hooks.json"), "Codex"),
        (claude.join("settings.json"), "Claude Code"),
    ] {
        let old = if path.exists() {
            Some(std::fs::read(&path).map_err(|e| e.to_string())?)
        } else {
            None
        };
        let doc = match &old {
            Some(b) => serde_json::from_slice(b)
                .map_err(|_| format!("{} 格式异常，未改动", path.display()))?,
            None => serde_json::json!({}),
        };
        let command = format!(
            "powershell.exe -NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -File \"{}\" -Agent \"{}\"",
            script.to_string_lossy().replace('\\', "/"),
            agent
        );
        let next = merge_hooks(doc, agent, &command)?;
        writes.push((path, old, next));
    }
    for (path, old, next) in writes {
        if let Some(old) = old {
            let backup = path.with_extension(format!(
                "island-backup-{}.json",
                chrono::Utc::now().timestamp_millis()
            ));
            crate::model::atomic_write(&backup, &old).map_err(|e| e.to_string())?;
        }
        super::save(&path, &next)?;
    }
    Ok("接入已安装，原配置已备份。请在 Codex 的 Hooks 设置中审核并信任；新会话生效。".into())
}
pub fn open_application(path: &str) -> Result<(), String> {
    if path.starts_with("https://") {
        let u = url::Url::parse(path).map_err(|_| "网址无效")?;
        if u.host_str().is_none() || !u.username().is_empty() || u.password().is_some() {
            return Err("网址无效".into());
        }
        #[cfg(windows)]
        unsafe {
            use windows::{
                core::PCWSTR,
                Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
            };
            let s: Vec<_> = path.encode_utf16().chain(Some(0)).collect();
            let r = ShellExecuteW(None, None, PCWSTR(s.as_ptr()), None, None, SW_SHOWNORMAL);
            if r.0 as isize <= 32 {
                return Err("无法打开网页".into());
            }
        }
        return Ok(());
    }
    let p = std::path::Path::new(path.trim());
    if !p.is_absolute() || !p.is_file() {
        return Err("请选择存在的应用或快捷方式完整路径".into());
    }
    if !p
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| ["exe", "lnk"].iter().any(|e| s.eq_ignore_ascii_case(e)))
    {
        return Err("仅支持 exe 或 lnk".into());
    }
    #[cfg(windows)]
    unsafe {
        use windows::{
            core::PCWSTR,
            Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
        };
        let wide: Vec<u16> = p
            .as_os_str()
            .to_string_lossy()
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let work: Vec<u16> = p
            .parent()
            .unwrap()
            .to_string_lossy()
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let result = ShellExecuteW(
            None,
            None,
            PCWSTR(wide.as_ptr()),
            None,
            PCWSTR(work.as_ptr()),
            SW_SHOWNORMAL,
        );
        if result.0 as isize <= 32 {
            return Err("Windows无法打开该应用".into());
        }
    }
    Ok(())
}
#[cfg(test)]
mod hook_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn hook_does_not_persist_prompt_or_transcript() {
        let e=from_hook("Codex",&json!({"session_id":"a","hook_event_name":"UserPromptSubmit","prompt":"SECRET","transcript_path":"PRIVATE"}),2).unwrap();
        let s = serde_json::to_string(&e).unwrap();
        assert!(!s.contains("SECRET"));
        assert!(!s.contains("PRIVATE"));
    }
    #[test]
    fn merge_is_idempotent_and_keeps_existing_hooks() {
        let doc = json!({"env":{"keep":"yes"},"hooks":{"Stop":[{"hooks":[{"type":"command","command":"original"}]}]}});
        let merged =
            merge_hooks(doc, "Codex", "desktop-island-event.exe --agent-event Codex").unwrap();
        let twice = merge_hooks(
            merged.clone(),
            "Codex",
            "desktop-island-event.exe --agent-event Codex",
        )
        .unwrap();
        assert_eq!(merged, twice);
        assert_eq!(twice["env"]["keep"], "yes");
        assert_eq!(twice["hooks"]["Stop"][0]["hooks"][0]["command"], "original");
    }
    #[test]
    fn lifecycle_and_subagents() {
        let e=from_hook("Codex",&json!({"session_id":"a","agent_id":"child","hook_event_name":"PostModelRequest","status":"failed"}),2).unwrap();
        assert_eq!(e.phase, Phase::Failed);
        assert_eq!(e.session, "a / child");
        assert!(from_hook("Fake", &json!({}), 2).is_err());
    }
    #[test]
    fn parallel_projects_and_children_keep_independent_lifecycles() {
        let mut state=State::default();
        for agent in ["Codex","Claude Code"] {
            for (project,session) in [("G:/project-a","a"),("G:/project-b","b")] {
                for (i,name) in ["UserPromptSubmit","PermissionRequest","PreToolUse","Stop"].into_iter().enumerate() {
                    let e=from_hook(agent,&json!({"session_id":session,"cwd":project,"hook_event_name":name}),i as i64+1).unwrap();
                    state.apply(e);
                }
            }
            let child=from_hook(agent,&json!({"session_id":"a","agent_id":"child","cwd":"G:/project-a","hook_event_name":"SubagentStop"}),5).unwrap();
            state.apply(child);
            let parent=from_hook(agent,&json!({"session_id":"a","cwd":"G:/project-a","hook_event_name":"UserPromptSubmit"}),6).unwrap();
            state.apply(parent);
            assert!(from_hook(agent,&json!({"session_id":"a","hook_event_name":"SubagentStop"}),7).is_err());
            assert!(from_hook(agent,&json!({"session_id":"a","hook_event_name":"Notification","notification_type":"auth_success"}),8).is_err());
        }
        assert_eq!(state.tasks.len(),6);assert_eq!(state.counts_at(10),[2,4,0,0]);
        let event=from_hook("Claude Code",&json!({"session_id":"b","cwd":"G:/project-b","hook_event_name":"StopFailure"}),9).unwrap();state.apply(event);
        let event=from_hook("Codex",&json!({"session_id":"b","cwd":"G:/project-b","hook_event_name":"Notification","notification_type":"permission_prompt"}),9).unwrap();state.apply(event);
        assert_eq!(state.counts_at(10),[2,2,1,1]);
        assert!(state.tasks.iter().all(|e| !e.project.is_empty()));
    }
}
#[derive(Default)]
pub struct State {
    pub tasks: Vec<Event>,
    pub errors: usize,
}
impl State {
    pub fn read() -> Self {
        EventCache::default().read(&super::root().join("events"))
    }
    pub fn apply(&mut self, e: Event) {
        if e.session.is_empty() || e.agent.len() > 100 || e.session.len() > 263 {
            return;
        }
        if let Some(old) = self
            .tasks
            .iter_mut()
            .find(|v| v.agent == e.agent && v.session == e.session)
        {
            if e.at > old.at {
                *old = e;
            }
        } else {
            self.tasks.push(e);
        }
    }
    pub fn counts(&self) -> [usize; 4] {
        self.counts_at(chrono::Utc::now().timestamp_millis())
    }
    pub fn counts_at(&self, now: i64) -> [usize; 4] {
        let mut n = [0; 4];
        for t in &self.tasks {
            n[match t.effective_phase(now) {
                Phase::Running => 0,
                Phase::Completed => 1,
                Phase::Attention => 2,
                Phase::Failed => 3,
                Phase::Unknown => continue,
            }] += 1;
        }
        n
    }
}

/// A bounded worker keeps event-directory I/O off the rendering thread.
pub struct Watcher {
    tx: std::sync::mpsc::SyncSender<()>,
    rx: std::sync::mpsc::Receiver<State>,
    pending: bool,
    at: std::time::Instant,
}
impl Watcher {
    pub fn start(folder: std::path::PathBuf) -> Self {
        let (tx, requests)=std::sync::mpsc::sync_channel(1);
        let (results, rx)=std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut cache=EventCache::default();
            while requests.recv().is_ok() {
                if results.send(cache.read(&folder)).is_err(){break;}
            }
        });
        Self{tx,rx,pending:false,at:std::time::Instant::now()-std::time::Duration::from_secs(3)}
    }
    pub fn poll(&mut self) -> Option<State> {
        let result=match self.rx.try_recv(){
            Ok(state)=>{self.pending=false;Some(state)},
            Err(std::sync::mpsc::TryRecvError::Disconnected)=>{self.pending=false;None},
            Err(_)=>None,
        };
        if !self.pending && self.at.elapsed()>=std::time::Duration::from_secs(2) && self.tx.try_send(()).is_ok() {
            self.pending=true;self.at=std::time::Instant::now();
        }
        result
    }
}
#[derive(Default)]
struct EventCache {
    entries: std::collections::HashMap<std::path::PathBuf,((std::time::SystemTime,u64),Event)>,
    reads: usize,
}
impl EventCache {
    fn read(&mut self, folder: &std::path::Path) -> State {
        let mut state=State::default();
        let mut seen=std::collections::HashSet::new();
        if let Ok(files)=std::fs::read_dir(folder){
            for file in files.flatten().take(1000){
                let path=file.path();
                if path.extension().and_then(|v|v.to_str())!=Some("json"){continue;}
                let Ok(meta)=file.metadata() else {state.errors+=1;continue;};
                if !meta.is_file() || meta.len()>16384 {state.errors+=1;continue;}
                let stamp=meta.modified().ok().map(|at|(at,meta.len()));
                let cached=self.entries.get(&path).filter(|(old,_)|Some(*old)==stamp);
                let event=if let Some((_,event))=cached{Some(event.clone())}else{
                    self.reads+=1;
                    std::fs::read(&path).ok().and_then(|bytes|serde_json::from_slice::<Event>(&bytes).ok())
                };
                if let Some(event)=event{
                    if let Some(stamp)=stamp{self.entries.insert(path.clone(),(stamp,event.clone()));}
                    seen.insert(path);state.apply(event);
                }else{state.errors+=1;}
            }
        }
        self.entries.retain(|path,_|seen.contains(path));
        state.tasks.sort_by(|a,b|b.at.cmp(&a.at));state.tasks.truncate(100);state
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_active_states_are_unverified_and_recover_on_new_event() {
        let now=2_000_000;
        let mut state=State::default();
        let base=Event{agent:"Codex".into(),session:"parent".into(),project:"fixture".into(),phase:Phase::Running,at:1,detail:String::new()};
        state.apply(base.clone());
        state.apply(Event{session:"waiting".into(),phase:Phase::Attention,..base.clone()});
        state.apply(Event{session:"done".into(),phase:Phase::Completed,..base.clone()});
        assert_eq!(state.counts_at(now),[0,1,0,0]);
        assert_eq!(state.tasks[0].effective_phase(now),Phase::Unknown);
        assert_eq!(state.tasks[0].phase,Phase::Running,"Keep the observed event, do not fabricate completion");
        state.apply(Event{at:now,..base.clone()});assert_eq!(state.counts_at(now),[1,1,0,0]);
        assert_eq!(Event{at:now+300_001,..base}.effective_phase(now),Phase::Unknown);
    }
    #[test]
    fn cache_reuses_unchanged_files_and_observes_changes() {
        let folder=std::env::temp_dir().join(format!("alpha-event-cache-{}-{}",std::process::id(),chrono::Utc::now().timestamp_nanos_opt().unwrap()));
        std::fs::create_dir_all(&folder).unwrap();
        let path=folder.join("fixture.json");
        let mut event=Event{agent:"Codex".into(),session:"fixture".into(),project:String::new(),phase:Phase::Running,at:1,detail:String::new()};
        super::super::save(&path,&event).unwrap();
        let mut cache=EventCache::default();
        assert_eq!(cache.read(&folder).tasks.len(),1);
        for _ in 0..60 {assert_eq!(cache.read(&folder).tasks.len(),1);}
        assert_eq!(cache.reads,1);
        event.at=2;event.phase=Phase::Completed;event.detail="Stop".into();
        super::super::save(&path,&event).unwrap();
        assert_eq!(cache.read(&folder).tasks[0].phase,Phase::Completed);assert_eq!(cache.reads,2);
        std::fs::write(&path,b"invalid").unwrap();assert_eq!(cache.read(&folder).errors,1);
        assert!(cache.entries.is_empty());
        std::fs::remove_file(&path).unwrap();assert!(cache.read(&folder).tasks.is_empty());
        std::fs::remove_dir(&folder).unwrap();
    }
    #[test]
    fn sessions_do_not_overwrite_and_old_events_ignored() {
        let mut s = State::default();
        let e = Event {
            agent: "Codex".into(),
            session: "1".into(),
            project: String::new(),
            phase: Phase::Completed,
            at: 20,
            detail: String::new(),
        };
        s.apply(e.clone());
        s.apply(Event {
            at: 10,
            phase: Phase::Running,
            ..e.clone()
        });
        s.apply(Event {
            session: "2".into(),
            phase: Phase::Running,
            ..e
        });
        assert_eq!(s.counts_at(20), [1, 1, 0, 0]);
    }
}
