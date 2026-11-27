use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{mpsc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const MAX_CONNECTIONS: usize = 8;
const ASK_WAIT: u64 = 600;
const DETAIL_MAX: usize = 600;
const SETTING: &str = "dz-island-agent-api";

static ACTIVE: AtomicUsize = AtomicUsize::new(0);
static NEXT_ASK: AtomicU64 = AtomicU64::new(1);
static ALERT_PANEL_VISIBLE: AtomicBool = AtomicBool::new(false);
type Decision = (bool, Vec<usize>);
struct PendingAsk { sender: Option<mpsc::Sender<Decision>>, activity: Activity, persistent: bool }
static PENDING: Mutex<Option<HashMap<u64, PendingAsk>>> = Mutex::new(None);

pub fn has_pending_question() -> bool {
    PENDING.lock().map(|map| map.as_ref().is_some_and(|m| !m.is_empty())).unwrap_or(true)
}

pub fn alert_visible() -> bool {
    ALERT_PANEL_VISIBLE.load(Ordering::Relaxed) || has_pending_question()
}

#[tauri::command]
pub fn agent_alert_visibility(window: tauri::Window, visible: bool) {
    if window.label() == "main" { ALERT_PANEL_VISIBLE.store(visible, Ordering::Relaxed); }
}

#[tauri::command]
pub fn agent_pending_questions() -> Vec<Activity> {
    let mut questions: Vec<_> = PENDING.lock().ok()
        .and_then(|map| map.as_ref().map(|map| map.values().map(|pending| pending.activity.clone()).collect()))
        .unwrap_or_default();
    questions.sort_by_key(|activity| activity.ask.as_ref().map(|ask| ask.id));
    questions
}

fn notify_agent(app: &AppHandle, state: &str) {
    use windows::core::PCWSTR;
    use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
    let enabled = crate::utils::get_setting_str(app, &format!("dz-island-agent-sound-{state}"));
    if crate::utils::get_setting_str(app, "dz-island-agent-sounds").as_deref() == Some("false")
        || enabled.as_deref() == Some("false")
        || (state == "done" && enabled.as_deref() != Some("true")) { return; }
    // Static PCM bytes remain alive for asynchronous WinMM playback. This does
    // not depend on WebView autoplay, focus or the frontend being awake.
    let chime: &'static [u8] = match state {
        "needs_input" => include_bytes!("../../public/sounds/agent-needs_input.wav"),
        "error" => include_bytes!("../../public/sounds/agent-error.wav"),
        "done" => include_bytes!("../../public/sounds/agent-done.wav"),
        _ => return,
    };
    let played = unsafe { PlaySoundW(PCWSTR(chime.as_ptr().cast()), None, SND_ASYNC | SND_MEMORY | SND_NODEFAULT) }.as_bool();
    crate::diagnostics::log(&format!("agent {state}: native sound started={played}"));
}

#[derive(Deserialize)]
struct Incoming {
    state: String,
    session: Option<String>,
    name: Option<String>,
    project: Option<String>,
    window: Option<u64>,
    detail: Option<String>,
}

#[derive(Deserialize)]
struct IncomingAsk {
    name: Option<String>,
    project: Option<String>,
    window: Option<u64>,
    tool: Option<String>,
    detail: Option<String>,
    questions: Option<Vec<AskQuestion>>,
}

#[derive(Serialize, Deserialize, Clone)]
struct AskQuestion {
    question: String,
    options: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Ask {
    id: u64,
    tool: Option<String>,
    detail: Option<String>,
    questions: Option<Vec<AskQuestion>>,
}

#[derive(Serialize, Deserialize)]
struct StoredQuestion {
    name: Option<String>, project: Option<String>, window: Option<u64>, ask: Ask,
}

fn persist_questions() {
    let Ok(map) = PENDING.lock() else { return };
    let stored: Vec<_> = map.as_ref().into_iter().flat_map(|map| map.values())
        .filter(|pending| pending.persistent)
        .filter_map(|pending| pending.activity.ask.clone().map(|ask| StoredQuestion {
            name: pending.activity.name.clone(), project: pending.activity.project.clone(), window: pending.activity.window, ask,
        })).collect();
    let result = (|| -> Result<(), String> {
        let dir = data_dir().ok_or("Pasta local indisponível")?;
        let bytes = serde_json::to_vec(&stored).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("agent-pending.tmp"), bytes).map_err(|e| e.to_string())?;
        std::fs::rename(dir.join("agent-pending.tmp"), dir.join("agent-pending.json")).map_err(|e| e.to_string())
    })();
    if let Err(error) = result { crate::diagnostics::log(&format!("agent pending save: {error}")); }
}

fn valid_questions(questions: &[AskQuestion]) -> bool {
    (1..=4).contains(&questions.len()) && questions.iter().all(|q| {
        !q.question.trim().is_empty() && q.question.chars().count() <= DETAIL_MAX
            && (2..=4).contains(&q.options.len())
            && q.options.iter().all(|option| !option.trim().is_empty() && option.chars().count() <= 80)
    })
}

fn restore_questions() {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(1);
    NEXT_ASK.fetch_max(now, Ordering::SeqCst);
    let Some(dir) = data_dir() else { return };
    let path = dir.join("agent-pending.json");
    if std::fs::metadata(&path).map(|meta| meta.len() > 524288).unwrap_or(true) { return; }
    let Some(stored) = std::fs::read(path).ok().and_then(|bytes| serde_json::from_slice::<Vec<StoredQuestion>>(&bytes).ok()) else { return };
    if let Ok(mut map) = PENDING.lock() {
        let map = map.get_or_insert_with(HashMap::new);
        for item in stored.into_iter().take(64) {
            if item.ask.id == 0 || item.ask.id >= 9_007_199_254_740_991
                || !item.ask.questions.as_deref().is_some_and(valid_questions) { continue; }
            NEXT_ASK.fetch_max(item.ask.id + 1, Ordering::SeqCst);
            let activity = Activity { state: "needs_input", session: None, name: item.name, project: item.project,
                window: item.window, detail: None, ask: Some(item.ask), cancelled: None };
            map.insert(activity.ask.as_ref().unwrap().id, PendingAsk { sender: None, activity, persistent: true });
        }
    }
}

#[derive(Serialize, Clone)]
pub struct Activity {
    state: &'static str,
    session: Option<String>,
    name: Option<String>,
    project: Option<String>,
    window: Option<u64>,
    detail: Option<String>,
    ask: Option<Ask>,
    cancelled: Option<u64>,
}

pub fn data_dir() -> Option<PathBuf> {
    crate::utils::portable_data_dir().ok()
}

fn config_path() -> Option<PathBuf> {
    data_dir().map(|dir| dir.join("agent.json"))
}

fn state_of(raw: &str) -> Option<Option<&'static str>> {
    match raw.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "working" | "thinking" | "running" => Some(Some("working")),
        "tool" | "tool_running" => Some(Some("tool")),
        "needs_input" | "input" | "waiting" => Some(Some("needs_input")),
        "done" | "completed" | "complete" | "finished" => Some(Some("done")),
        "error" | "failed" => Some(Some("error")),
        "clear" | "idle" | "stop" => Some(None),
        _ => None,
    }
}

fn clean(text: Option<String>, limit: usize) -> Option<String> {
    let cleaned: String = text?.chars().filter(|c| !c.is_control()).take(limit).collect();
    let cleaned = cleaned.trim().to_string();
    (!cleaned.is_empty()).then_some(cleaned)
}

fn random_token() -> Option<String> {
    use windows::Win32::Security::Cryptography::{BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG};
    let mut bytes = [0u8; 24];
    unsafe { BCryptGenRandom(None, &mut bytes, BCRYPT_USE_SYSTEM_PREFERRED_RNG) }.ok().ok()?;
    Some(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn reply(stream: &mut TcpStream, status: &str) {
    let _ = write!(stream, "HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
}

fn reply_json(stream: &mut TcpStream, body: &str) {
    let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
}

pub fn answer(id: u64, allow: bool, choices: Vec<usize>) -> Result<(), String> {
    let pending = {
        let mut map = PENDING.lock().map_err(|e| e.to_string())?;
        let map = map.as_mut().ok_or("Pergunta não está mais pendente")?;
        let pending = map.get(&id).ok_or("Pergunta não está mais pendente")?;
        if pending.persistent {
            let questions = pending.activity.ask.as_ref().and_then(|ask| ask.questions.as_ref()).ok_or("Pergunta inválida")?;
            if allow && (choices.len() != questions.len() || choices.iter().zip(questions).any(|(&index, q)| index >= q.options.len())) {
                return Err("Opções de resposta inválidas".into());
            }
            let answers: Vec<_> = questions.iter().enumerate().map(|(i,q)| serde_json::json!({
                "question": q.question, "answer": choices.get(i).and_then(|index| q.options.get(*index))
            })).collect();
            let dir = data_dir().ok_or("Pasta local indisponível")?.join("agent-answers");
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let response = serde_json::json!({"id":id,"status":if allow {"answered"}else{"dismissed"},"answers":answers});
            std::fs::write(dir.join(format!("{id}.json")), response.to_string()).map_err(|e| e.to_string())?;
        }
        map.remove(&id).unwrap()
    };
    let sender = pending.sender;
    if let Some(sender) = sender {
        let _ = sender.send((allow, choices));
    }
    if pending.persistent { persist_questions(); }
    Ok(())
}

fn wait_for_answer(stream: &mut TcpStream, id: u64, rx: &mpsc::Receiver<Decision>, persistent: bool) -> Option<Decision> {
    let _ = stream.set_nonblocking(true);
    let mut waited = 0;
    while waited < ASK_WAIT {
        if let Ok(decision) = rx.recv_timeout(Duration::from_secs(1)) {
            let _ = stream.set_nonblocking(false);
            return Some(decision);
        }
        waited += 1;
        let mut probe = [0u8; 1];
        if matches!(stream.peek(&mut probe), Ok(0)) {
            break;
        }
    }
    if !persistent { if let Ok(mut map) = PENDING.lock() {
        if let Some(map) = map.as_mut() {
            map.remove(&id);
        }
    } }
    let _ = stream.set_nonblocking(false);
    None
}

fn handle(mut stream: TcpStream, app: &AppHandle, token: &str) {
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    let request = match crate::agent_http::read_request(&mut stream, token, Duration::from_secs(2)) {
        Ok(request) => request,
        Err(status) => return reply(&mut stream, status),
    };
    let ask = request.ask;
    let body = request.body;
    if ask {
        let Ok(incoming) = serde_json::from_slice::<IncomingAsk>(&body) else {
            return reply(&mut stream, "400 Bad Request");
        };
        let persistent = incoming.questions.as_deref().is_some_and(valid_questions);
        if incoming.questions.is_some() && !persistent { return reply(&mut stream, "400 Bad Request"); }
        if PENDING.lock().map(|map| map.as_ref().is_some_and(|map| map.len() >= 64)).unwrap_or(true) { return reply(&mut stream, "429 Too Many Requests"); }
        let id = NEXT_ASK.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel();
        let (name, project, window) = (clean(incoming.name, 40), clean(incoming.project, 40), incoming.window);
        let activity = Activity {
                state: "needs_input",
                session: None,
                name: name.clone(),
                project: project.clone(),
                window,
                detail: None,
                ask: Some(Ask {
                    id,
                    tool: clean(incoming.tool, 40),
                    detail: clean(incoming.detail, DETAIL_MAX),
                    questions: incoming.questions.map(|list| {
                        list.into_iter()
                            .take(4)
                            .filter_map(|q| {
                                let question = clean(Some(q.question), DETAIL_MAX)?;
                                Some(AskQuestion { question, options: q.options.into_iter().filter_map(|o| clean(Some(o), 80)).take(4).collect() })
                            })
                            .collect()
                    }),
                }),
                cancelled: None,
            };
        if let Ok(mut map) = PENDING.lock() {
            let map = map.get_or_insert_with(HashMap::new);
            if map.len() >= 64 { return reply(&mut stream, "429 Too Many Requests"); }
            map.insert(id, PendingAsk { sender: Some(tx), activity: activity.clone(), persistent });
        } else { return reply(&mut stream, "503 Service Unavailable"); }
        if persistent { persist_questions(); }
        notify_agent(app, "needs_input");
        let _ = app.emit("agent-activity", activity);
        return match wait_for_answer(&mut stream, id, &rx, persistent) {
            Some((true, choices)) if !choices.is_empty() => reply_json(&mut stream, &serde_json::json!({ "decision": "allow", "choices": choices }).to_string()),
            Some((true, _)) => reply_json(&mut stream, r#"{"decision":"allow"}"#),
            Some((false, _)) => reply_json(&mut stream, r#"{"decision":"deny"}"#),
            None if persistent => reply_json(&mut stream, &serde_json::json!({"decision":"pending","id":id}).to_string()),
            None => {
                let _ = app.emit("agent-activity", Activity { state: "working", session: None, name, project, window, detail: None, ask: None, cancelled: Some(id) });
                reply(&mut stream, "204 No Content");
            }
        };
    }

    let Ok(incoming) = serde_json::from_slice::<Incoming>(&body) else {
        return reply(&mut stream, "400 Bad Request");
    };
    let Some(state) = state_of(&incoming.state) else {
        return reply(&mut stream, "400 Bad Request");
    };
    let activity = state.map(|state| Activity {
        state,
        session: clean(incoming.session, 80),
        name: clean(incoming.name, 40),
        project: clean(incoming.project, 40),
        window: incoming.window,
        detail: clean(incoming.detail, DETAIL_MAX),
        ask: None,
        cancelled: None,
    });
    if let Some(activity) = &activity { notify_agent(app, activity.state); }
    let _ = app.emit("agent-activity", activity);
    reply(&mut stream, "204 No Content");
}

type HookGroup = (&'static str, Option<&'static str>);

#[cfg(test)]
mod question_recovery_tests {
    use super::*;
    #[test]
    fn pending_question_survives_missed_event_and_disappears_after_answer() {
        let id = u64::MAX - 1;
        let (sender, receiver) = mpsc::channel();
        let activity = Activity {
            state: "needs_input", session: None, name: Some("Codex".into()),
            project: None, window: None, detail: None,
            ask: Some(Ask { id, tool: None, detail: None,
                questions: Some(vec![AskQuestion { question: "Teste".into(), options: vec!["Sim".into(), "Não".into()] }]) }),
            cancelled: None,
        };
        PENDING.lock().unwrap().get_or_insert_with(HashMap::new).insert(id, PendingAsk { sender:Some(sender), activity, persistent:false });
        assert!(agent_pending_questions().iter().any(|item| item.ask.as_ref().is_some_and(|ask| ask.id == id)));
        answer(id, true, vec![0]).unwrap();
        assert_eq!(receiver.recv_timeout(Duration::from_secs(1)).unwrap(), (true, vec![0]));
        assert!(!agent_pending_questions().iter().any(|item| item.ask.as_ref().is_some_and(|ask| ask.id == id)));
    }
    #[test]
    fn durable_question_restores_without_client_and_archives_late_answer() {
        let id = 1_700_000_000_123;
        let activity = Activity {
            state:"needs_input", session:None, name:Some("Codex".into()), project:None,
            window:None, detail:None, cancelled:None,
            ask:Some(Ask { id, tool:None, detail:None, questions:Some(vec![AskQuestion {
                question:"Pergunta que deve continuar aberta".into(), options:vec!["Sim".into(),"Não".into()]
            }]) }),
        };
        PENDING.lock().unwrap().get_or_insert_with(HashMap::new).insert(id, PendingAsk { sender:None, activity, persistent:true });
        persist_questions();
        PENDING.lock().unwrap().as_mut().unwrap().remove(&id);
        restore_questions();
        assert!(agent_pending_questions().iter().any(|item| item.ask.as_ref().is_some_and(|ask| ask.id == id)));
        assert!(answer(id, true, vec![99]).is_err());
        answer(id, true, vec![1]).unwrap();
        let bytes = std::fs::read(data_dir().unwrap().join("agent-answers").join(format!("{id}.json"))).unwrap();
        let response: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(response["answers"][0]["answer"], "Não");
        assert!(!agent_pending_questions().iter().any(|item| item.ask.as_ref().is_some_and(|ask| ask.id == id)));
    }
}

fn hook_groups(provider: &str) -> &'static [HookGroup] {
    match provider {
        "Codex" => &[
            ("UserPromptSubmit", None), ("PermissionRequest", None),
            ("PreToolUse", Some("request_user_input|AskUserQuestion")),
            ("PostToolUse", Some("request_user_input|AskUserQuestion")),
            ("Stop", None), ("Interrupt", None),
        ],
        "Claude Code" => &[
            ("UserPromptSubmit", None), ("PermissionRequest", None),
            ("PreToolUse", Some("AskUserQuestion")), ("PostToolUse", Some("AskUserQuestion")),
            ("PostToolUseFailure", None), ("Notification", None), ("Stop", None),
        ],
        "Gemini CLI" => &[
            ("BeforeAgent", None), ("Notification", None), ("AfterAgent", None),
        ],
        _ => &[],
    }
}

fn hook_entry(exe: &str, provider: &str) -> serde_json::Value {
    if provider == "Claude Code" {
        return serde_json::json!({ "type": "command", "command": exe,
            "args": ["agent", "hook", "--name", provider], "timeout": ASK_WAIT + 10 });
    }
    // PowerShell runs the absolute executable path while passing through the hook's stdin.
    let path = exe.replace('\\', "/").replace('\'', "''");
    let statement = format!("& '{path}' agent hook --name '{provider}'");
    let command = format!("powershell.exe -NoProfile -NonInteractive -Command \"{statement}\"");
    let timeout = if provider == "Gemini CLI" { 4000 } else { 4 };
    serde_json::json!({ "type": "command", "command": command, "timeout": timeout })
}

fn is_ours(hook: &serde_json::Value, exe: &str) -> bool {
    let Some(command) = hook["command"].as_str() else { return false };
    let path = exe.replace('\\', "/").replace('\'', "''");
    let legacy_exe = std::path::Path::new(exe).with_file_name("Nectar.exe");
    let legacy_exe = legacy_exe.to_string_lossy();
    let legacy_path = legacy_exe.replace('\\', "/").replace('\'', "''");
    ((command.eq_ignore_ascii_case(exe) || command.eq_ignore_ascii_case(&legacy_exe))
        && hook["args"][0].as_str() == Some("agent"))
        || ((command.contains(&path) || command.contains(&legacy_path)) && command.contains("agent hook --name"))
}

fn sync_hooks(path: PathBuf, provider: &str, enable: bool, exe: &str) {
    let text = std::fs::read_to_string(&path).ok();
    if text.is_none() && path.exists() { return; }
    let mut root: serde_json::Value = match text.as_deref().map(str::trim) {
        Some(t) if !t.is_empty() => match serde_json::from_str(t) {
            Ok(value) => value,
            Err(_) => return,
        },
        _ if enable => serde_json::json!({}),
        _ => return,
    };
    let original = root.clone();
    let Some(object) = root.as_object_mut() else { return };

    let hooks = object.entry("hooks").or_insert_with(|| serde_json::json!({}));
    let Some(hooks) = hooks.as_object_mut() else { return };
    for event in hooks.values_mut() {
        if let Some(groups) = event.as_array_mut() {
            for group in groups.iter_mut() {
                if let Some(handlers) = group["hooks"].as_array_mut() {
                    handlers.retain(|hook| !is_ours(hook, exe));
                }
            }
            groups.retain(|group| group["hooks"].as_array().is_none_or(|handlers| !handlers.is_empty()));
        }
    }
    if enable {
        for &(event, matcher) in hook_groups(provider) {
            let mut group = serde_json::json!({ "hooks": [hook_entry(exe, provider)] });
            if let Some(matcher) = matcher {
                group["matcher"] = matcher.into();
            }
            match hooks.entry(event).or_insert_with(|| serde_json::json!([])).as_array_mut() {
                Some(groups) => groups.push(group),
                None => return,
            }
        }
    }
    hooks.retain(|_, event| event.as_array().map(|groups| !groups.is_empty()).unwrap_or(true));
    if hooks.is_empty() {
        object.remove("hooks");
    }
    if provider == "Gemini CLI" {
        if enable {
            let servers = object.entry("mcpServers").or_insert_with(|| serde_json::json!({}));
            if let Some(servers) = servers.as_object_mut() {
                let legacy = std::path::Path::new(exe).with_file_name("Nectar.exe");
                if servers.get("nectar-island").is_some_and(|server| {
                    server["command"].as_str().is_some_and(|path| std::path::Path::new(path) == legacy)
                        && server["args"][0].as_str() == Some("agent")
                }) {
                    servers.remove("nectar-island");
                }
                servers.entry("dz-island").or_insert_with(|| serde_json::json!({
                    "command": exe, "args": ["agent", "mcp", "--name", "Gemini CLI"], "timeout": 135000
                }));
            }
        } else if let Some(servers) = object.get_mut("mcpServers").and_then(serde_json::Value::as_object_mut) {
            if servers.get("dz-island").is_some_and(|server| server["command"].as_str().is_some_and(|path| {
                let legacy = std::path::Path::new(exe).with_file_name("Nectar.exe");
                std::path::Path::new(path) == std::path::Path::new(exe) || std::path::Path::new(path) == legacy
            }) && server["args"][0].as_str() == Some("agent")) {
                servers.remove("dz-island");
            }
        }
    }
    if root == original {
        return;
    }

    let Ok(mut out) = serde_json::to_string_pretty(&root) else { return };
    out.push('\n');
    if let Some(text) = &text {
        let backup = path.with_extension("json.dz-island-backup");
        if !backup.exists() {
            let _ = std::fs::write(&backup, text);
        }
    }
    let temp = path.with_extension("json.dz-island-tmp");
    if std::fs::write(&temp, out).is_ok() && std::fs::rename(&temp, &path).is_err() {
        let _ = std::fs::remove_file(&temp);
    }
}

pub fn set_hooks(enable: bool) {
    let Ok(exe) = std::env::current_exe() else { return };
    if cfg!(debug_assertions) || exe.components().any(|c| c.as_os_str() == "target") {
        return;
    }
    let Some(home) = std::env::var_os("USERPROFILE").map(PathBuf::from) else { return };
    let codex = std::env::var_os("CODEX_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".codex"));
    for (provider, path) in [
        ("Codex", codex.join("hooks.json")),
        ("Claude Code", home.join(".claude").join("settings.json")),
        ("Gemini CLI", home.join(".gemini").join("settings.json")),
    ] {
        if path.parent().is_some_and(|parent| parent.is_dir() || (enable && std::fs::create_dir_all(parent).is_ok())) {
            sync_hooks(path, provider, enable, &exe.to_string_lossy());
        }
    }
}

pub fn start(app: AppHandle) {
    let enabled = crate::utils::get_setting_str(&app, SETTING).as_deref() == Some("true");
    if !enabled {
        return;
    }
    restore_questions();
    // A local question bridge does not need to modify other agents' hook files.
    if crate::utils::get_setting_str(&app, "dz-island-agent-hooks").as_deref() != Some("false") {
        std::thread::spawn(|| set_hooks(true));
    }
    let Some(token) = random_token() else { return };
    let Ok(listener) = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))) else { return };
    let Ok(addr) = listener.local_addr() else { return };
    let Some(path) = config_path() else { return };
    let config = serde_json::json!({ "port": addr.port(), "token": token }).to_string();
    if std::fs::write(&path, config).is_err() {
        return;
    }

    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            if !stream.peer_addr().map(|a| a.ip().is_loopback()).unwrap_or(false) {
                continue;
            }
            if ACTIVE.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
                ACTIVE.fetch_sub(1, Ordering::SeqCst);
                continue;
            }
            let app = app.clone();
            let token = token.clone();
            std::thread::spawn(move || {
                handle(stream, &app, &token);
                ACTIVE.fetch_sub(1, Ordering::SeqCst);
            });
        }
    });
}

fn project_name() -> Option<String> {
    let dir = std::env::var_os("CLAUDE_PROJECT_DIR").map(PathBuf::from).or_else(|| std::env::current_dir().ok())?;
    dir.file_name().map(|n| n.to_string_lossy().into_owned())
}

fn ancestor_window() -> Option<u64> {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
    use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindow, GetWindowThreadProcessId, IsWindowVisible, GW_OWNER};

    let mut parents: HashMap<u32, (u32, String)> = HashMap::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?;
        let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut more = Process32FirstW(snapshot, &mut entry).is_ok();
        while more {
            let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
            parents.insert(entry.th32ProcessID, (entry.th32ParentProcessID, String::from_utf16_lossy(&entry.szExeFile[..len]).to_ascii_lowercase()));
            more = Process32NextW(snapshot, &mut entry).is_ok();
        }
        let _ = windows::Win32::Foundation::CloseHandle(snapshot);
    }

    let mut chain = Vec::new();
    let mut pid = std::process::id();
    while let Some((parent, _)) = parents.get(&pid) {
        match parents.get(parent) {
            Some((_, name)) if name != "explorer.exe" && !chain.contains(parent) => chain.push(*parent),
            _ => break,
        }
        pid = *parent;
    }

    struct Search {
        chain: Vec<u32>,
        found: Vec<(usize, u64)>,
    }
    unsafe extern "system" fn visit(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let search = &mut *(lparam.0 as *mut Search);
        if IsWindowVisible(hwnd).as_bool() && GetWindow(hwnd, GW_OWNER).map(|o| o.0.is_null()).unwrap_or(true) {
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if let Some(rank) = search.chain.iter().position(|&p| p == pid) {
                search.found.push((rank, hwnd.0 as usize as u64));
            }
        }
        BOOL(1)
    }
    let mut search = Search { chain, found: Vec::new() };
    unsafe {
        let _ = EnumWindows(Some(visit), LPARAM(&mut search as *mut Search as isize));
    }
    search.found.into_iter().min_by_key(|(rank, _)| *rank).map(|(_, hwnd)| hwnd)
}

fn flags(args: &[String]) -> (Option<String>, Option<String>) {
    let (mut name, mut project) = (None, None);
    let mut rest = args.iter();
    while let Some(flag) = rest.next() {
        match flag.as_str() {
            "--name" | "-n" => name = rest.next().cloned(),
            "--project" | "-p" => project = rest.next().cloned(),
            _ => {}
        }
    }
    (name, project.or_else(project_name))
}

fn post(path: &str, body: String, wait: u64) -> Option<String> {
    let config: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(config_path()?).ok()?).ok()?;
    let port = config.get("port")?.as_u64()? as u16;
    let token = config.get("token")?.as_str()?;

    let mut stream = TcpStream::connect_timeout(&SocketAddr::from((Ipv4Addr::LOCALHOST, port)), Duration::from_secs(1)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(wait))).ok()?;
    write!(
        stream,
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    Some(response)
}

fn send(args: &[String]) -> Option<()> {
    send_with(args, None)
}

fn send_with(args: &[String], detail: Option<String>) -> Option<()> {
    let state = args.first()?.clone();
    let (name, project) = flags(&args[1..]);
    let body = serde_json::json!({ "state": state, "name": name, "project": project, "window": ancestor_window(), "detail": detail }).to_string();
    post("/activity", body, 2)?.starts_with("HTTP/1.1 204").then_some(())
}

fn ask(args: &[String]) {
    let mut input = String::new();
    if std::io::stdin().take(1_048_576).read_to_string(&mut input).is_err() { return; }
    let Ok(event) = serde_json::from_str(&input) else { return };
    ask_from_event(args, event);
}

fn ask_from_event(args: &[String], event: serde_json::Value) {
    let (name, project) = flags(args);
    let tool_input = &event["tool_input"];
    let detail: Option<String>;
    let mut asked: Option<Vec<(String, Vec<String>)>> = None;
    if event["tool_name"].as_str() == Some("AskUserQuestion") {
        let list = tool_input["questions"].as_array().cloned().unwrap_or_default();
        let parsed: Vec<(String, Vec<String>)> = list
            .iter()
            .filter(|q| q["multiSelect"].as_bool() != Some(true))
            .filter_map(|q| {
                let labels: Vec<String> = q["options"].as_array()?.iter().filter_map(|o| o["label"].as_str().map(str::to_string)).collect();
                (2..=4).contains(&labels.len()).then(|| Some((q["question"].as_str()?.to_string(), labels))).flatten()
            })
            .collect();
        if list.is_empty() || list.len() > 4 || parsed.len() != list.len() {
            let mut forwarded = vec!["needs-input".to_string()];
            forwarded.extend(args.iter().cloned());
            let _ = send_with(&forwarded, list.first().and_then(|q| q["question"].as_str()).map(str::to_string));
            return;
        }
        detail = None;
        asked = Some(parsed);
    } else {
        detail = ["command", "file_path", "url", "pattern", "description"]
            .iter()
            .find_map(|key| tool_input.get(*key).and_then(|v| v.as_str()))
            .map(str::to_string)
            .or_else(|| (!tool_input.is_null()).then(|| tool_input.to_string()));
    }
    let detail = detail.map(|d| d.chars().take(DETAIL_MAX).collect::<String>());
    let questions = asked.as_ref().map(|list| list.iter().map(|(q, o)| serde_json::json!({ "question": q, "options": o })).collect::<Vec<_>>());
    let body = serde_json::json!({
        "name": name,
        "project": project,
        "window": ancestor_window(),
        "tool": event["tool_name"].as_str(),
        "detail": detail,
        "questions": questions,
    })
    .to_string();

    let Some(response) = post("/ask", body, ASK_WAIT + 5) else { return };
    let decision = response.split("\r\n\r\n").nth(1).and_then(|b| serde_json::from_str::<serde_json::Value>(b).ok());
    if let Some(asked) = asked {
        let Some(choices) = decision.as_ref().and_then(|d| d["choices"].as_array()) else { return };
        if choices.len() != asked.len() {
            return;
        }
        let mut answers = serde_json::Map::new();
        for ((question, labels), choice) in asked.iter().zip(choices) {
            let Some(label) = choice.as_u64().and_then(|i| labels.get(i as usize)) else { return };
            answers.insert(question.clone(), serde_json::Value::String(label.clone()));
        }
        let updated = serde_json::json!({ "questions": tool_input["questions"], "answers": answers });
        println!(
            "{}",
            serde_json::json!({ "hookSpecificOutput": { "hookEventName": "PermissionRequest", "decision": { "behavior": "allow", "updatedInput": updated } } })
        );
        return;
    }
    let behavior = match decision.as_ref().and_then(|d| d["decision"].as_str()) {
        Some("allow") => serde_json::json!({ "behavior": "allow" }),
        Some("deny") => serde_json::json!({ "behavior": "deny", "message": "Denied from DZ Island" }),
        _ => return,
    };
    println!("{}", serde_json::json!({ "hookSpecificOutput": { "hookEventName": "PermissionRequest", "decision": behavior } }));
}


fn handle_hook(args: &[String]) {
    let mut input = String::new();
    if std::io::stdin().take(1_048_576).read_to_string(&mut input).is_err() { return; }
    let Ok(event) = serde_json::from_str::<serde_json::Value>(&input) else { return };
    let (name, project) = flags(args);
    let tool = event["tool_name"].as_str().unwrap_or("ferramenta");
    let event_name = event["hook_event_name"].as_str().unwrap_or_default();
    if name.as_deref() == Some("Claude Code") && event_name == "PermissionRequest" && tool == "AskUserQuestion" {
        ask_from_event(args, event);
        return;
    }
    let text = |value: &serde_json::Value| value.as_str().unwrap_or_default().to_owned();
    let (state, detail) = match event_name {
        "UserPromptSubmit" | "BeforeAgent" | "PostToolUse" => ("working", None),
        "Stop" | "AfterAgent" => ("done", Some("Tarefa concluída.".to_owned())),
        "Interrupt" => ("error", Some("Tarefa interrompida.".to_owned())),
        "PermissionRequest" => ("needs-input", Some(format!("Permissão solicitada para {tool}. Responda no agente."))),
        "PreToolUse" => {
            let questions = event["tool_input"]["questions"].as_array().map(|questions| {
                questions.iter().filter_map(|q| q["question"].as_str()).take(4).collect::<Vec<_>>().join(" • ")
            }).unwrap_or_default();
            let question = if questions.is_empty() { text(&event["tool_input"]["question"]) } else { questions };
            ("needs-input", Some(if question.is_empty() { "O agente tem uma pergunta. Responda na conversa.".into() } else { question }))
        }
        "Notification" => {
            let kind = event["notification_type"].as_str().unwrap_or_default().to_ascii_lowercase();
            let message = text(&event["message"]);
            let state = if kind.contains("error") { "error" } else { "needs-input" };
            (state, Some(if message.is_empty() { "Aviso do agente.".into() } else { message }))
        }
        "PostToolUseFailure" => ("error", Some(format!("Falha em {tool}."))),
        _ => return,
    };
    let project = event["cwd"].as_str().and_then(|cwd| std::path::Path::new(cwd).file_name())
        .map(|name| name.to_string_lossy().into_owned()).or(project);
    let window = if matches!(state, "needs-input" | "error" | "done") { ancestor_window() } else { None };
    let body = serde_json::json!({
        "state": state, "name": name, "project": project, "detail": detail, "window": window,
        "session": event["session_id"].as_str(),
    }).to_string();
    // Observational hooks always allow the agent's original UI and approval flow to proceed.
    let _ = post("/activity", body, 2);
}


fn island_tool_result(text: String, error: bool) -> serde_json::Value {
    serde_json::json!({ "content": [{ "type": "text", "text": text }], "isError": error })
}

fn island_mcp_call(request: &serde_json::Value, agent: &str) -> serde_json::Value {
    let args = &request["params"]["arguments"];
    if request["params"]["name"] != "ask_island" {
        return island_tool_result("Unknown DZ Island tool.".into(), true);
    }
    let Ok(questions) = serde_json::from_value::<Vec<AskQuestion>>(args["questions"].clone()) else {
        return island_tool_result("Provide questions and their answer options.".into(), true);
    };
    if !(1..=4).contains(&questions.len()) || questions.iter().any(|q| {
        q.question.trim().is_empty() || q.question.chars().count() > 500
            || !(2..=4).contains(&q.options.len())
            || q.options.iter().any(|o| o.trim().is_empty() || o.chars().count() > 80)
    }) {
        return island_tool_result("Use 1 to 4 questions with 2 to 4 short choices per question.".into(), true);
    }
    let body = serde_json::json!({
        "name": agent, "questions": questions, "project": project_name(), "window": ancestor_window()
    }).to_string();
    let answer = post("/ask", body, ASK_WAIT + 3)
        .and_then(|response| response.split_once("\r\n\r\n").and_then(|(_, body)| serde_json::from_str::<serde_json::Value>(body).ok()));
    let selected = answer.as_ref().and_then(|answer| {
        let choices = answer["choices"].as_array()?;
        if choices.len() != questions.len() { return None; }
        questions.iter().zip(choices).map(|(question, choice)| {
            let selected = question.options.get(usize::try_from(choice.as_u64()?).ok()?)?;
            Some(serde_json::json!({ "question": question.question, "answer": selected }))
        }).collect::<Option<Vec<_>>>()
    });
    match selected {
        Some(answers) => island_tool_result(serde_json::json!({ "answers": answers }).to_string(), false),
        None => island_tool_result("DZ Island received no answer. Ask in the original chat instead.".into(), true),
    }
}

fn run_island_mcp(agent: &str) {
    use std::io::BufRead;
    for line in std::io::stdin().lock().lines().map_while(Result::ok) {
        let Ok(request) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
        let Some(id) = request.get("id") else { continue };
        let result = match request["method"].as_str() {
            Some("initialize") => serde_json::json!({
                "protocolVersion": request["params"]["protocolVersion"].as_str().unwrap_or("2024-11-05"),
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "dz-island", "version": env!("CARGO_PKG_VERSION") },
                "instructions": "For user choices in this local session, prefer ask_island. It shows options in the DZ Island island and returns the answer. Use normal agent approval UI for tool permissions. If DZ Island gives no answer, use the original chat."
            }),
            Some("ping") => serde_json::json!({}),
            Some("tools/list") => serde_json::json!({ "tools": [{
                "name": "ask_island",
                "description": "Present one or more multiple-choice questions to the user in the local DZ Island island and wait for the user's answer. Use for questions and decisions, not tool permissions.",
                "inputSchema": {
                    "type": "object", "required": ["questions"], "additionalProperties": false,
                    "properties": { "questions": { "type": "array", "minItems": 1, "maxItems": 4,
                        "items": { "type": "object", "required": ["question", "options"], "additionalProperties": false,
                            "properties": { "question": { "type": "string", "maxLength": 500 },
                                "options": { "type": "array", "minItems": 2, "maxItems": 4, "items": { "type": "string", "maxLength": 80 } } } } } }
                }
            }] }),
            Some("tools/call") => island_mcp_call(&request, agent),
            _ => {
                println!("{}", serde_json::json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": "Method not found" } }));
                let _ = std::io::stdout().flush();
                continue;
            }
        };
        println!("{}", serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result }));
        let _ = std::io::stdout().flush();
    }
}

pub fn run_cli(args: &[String]) -> Option<i32> {
    if args.get(1).map(String::as_str) != Some("agent") {
        return None;
    }
    match args.get(2).map(String::as_str) {
        Some("hook") => { handle_hook(&args[3..]); return Some(0); }
        Some("mcp") => {
            let (name, _) = flags(&args[3..]);
            run_island_mcp(name.as_deref().unwrap_or("Codex"));
            return Some(0);
        }
        Some("install-hooks") => {
            set_hooks(true);
            return Some(0);
        }
        Some("remove-hooks") => {
            set_hooks(false);
            return Some(0);
        }
        _ => {}
    }
    if args.get(2).map(String::as_str) == Some("ask") {
        ask(&args[3..]);
        return Some(0);
    }
    Some(if send(&args[2..]).is_some() { 0 } else { 1 })
}
