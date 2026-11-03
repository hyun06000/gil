//! Host-local enrollment, scoped turn handoffs, and one sandbox for all enrolled roots.
//! No Project/Graph writes. This registry is not an authorization or OS-wide security boundary.
use super::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::Write,
};

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Registry {
    roots: BTreeSet<PathBuf>,
    sessions: BTreeMap<String, Bound>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Bound {
    root: PathBuf,
    wait: Option<Value>,
}
struct Store {
    dir: PathBuf,
    _lock: File,
    data: Registry,
}

fn location() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or("HOME unavailable")?;
    let home = PathBuf::from(home);
    if !home.is_absolute() {
        return Err("HOME must be absolute".into());
    }
    Ok(home.join(".gil-host-guard"))
}
fn regular(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_file() && !m.file_type().is_symlink() => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        _ => Err("guard settings must be regular files".into()),
    }
}
impl Store {
    fn open() -> Result<Self, String> {
        let dir = location()?;
        if !dir.exists() {
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            if let Err(error) = builder.create(&dir) {
                if error.kind() != io::ErrorKind::AlreadyExists { return Err(error.to_string()); }
            }
        }
        let meta = fs::symlink_metadata(&dir).map_err(|e| e.to_string())?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err("unsafe guard directory".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if meta.mode() & 0o077 != 0 {
                return Err("guard directory must be private (0700)".into());
            }
        }
        let lock_path = dir.join("registry.lock");
        regular(&lock_path)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let lock = options.open(lock_path).map_err(|e| e.to_string())?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(300);
        loop {
            match lock.try_lock() {
                Ok(()) => break,
                Err(std::fs::TryLockError::WouldBlock) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(_) => return Err("guard registry busy or unreadable; retry this call".into()),
            }
        }
        let file = dir.join("registry.json");
        regular(&file)?;
        let data = if file.exists() {
            if fs::metadata(&file).map_err(|e| e.to_string())?.len() > LIMIT {
                return Err("guard registry too large".into());
            }
            serde_json::from_slice(&fs::read(file).map_err(|e| e.to_string())?)
                .map_err(|_| "damaged guard registry; preserved")?
        } else {
            Registry::default()
        };
        Ok(Self {
            dir,
            _lock: lock,
            data,
        })
    }
    fn save(&self) -> Result<(), String> {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
        let tmp = self.dir.join(format!("registry-{}-{stamp}.tmp", std::process::id()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&tmp).map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec(&self.data).map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        fs::rename(&tmp, self.dir.join("registry.json")).map_err(|e| e.to_string())
    }
}
fn resolved(root: &str) -> Result<PathBuf, String> {
    project_root(&crate::mcp::resolve_guard_root(root)?)
}
fn tool(input: &Value) -> &str {
    input["tool_name"]
        .as_str()
        .unwrap_or("")
        .rsplit("__")
        .next()
        .unwrap_or("")
}
fn context(event: &str, message: &str) -> Value {
    if event == "Stop" { return json!({"systemMessage":message}); }
    json!({"hookSpecificOutput":{"hookEventName":event,"additionalContext":message}})
}
fn correction(root: &Path) -> String {
    let first = json!({"project_root":root,"kind":"question","contract":"objective: 사용자 목표 확인\nnext_action: 목표와 원하는 결과를 사용자에게 질문\ndone_when: 사용자 답변으로 목표 또는 남은 불확실성 확인"});
    format!("GIL 작업이 미완료다: {}. gil_context를 읽고 실제 다음 행동을 수행한다. 목표를 모르면 먼저 gil_open({first})을 연다. 사용자 입력이 필요하면 열린 Question 또는 제안이 등록된 승인 대기 Synthesis에서 gil_await_user(project_root, question)를 호출하고 그 질문으로 응답을 끝낸다. 답변을 받으면 기존 Question을 이어서 닫는다. 채팅 결과·웹 조사만으로 GIL 완료를 선언하지 않는다. 사용자 중단 요청은 존중한다; 사용자는 ‘GIL 일시 중지’ 메시지로 이 채팅의 진행 검사만 일시 해제할 수 있다. 파일 보호는 유지된다.",root.display())
}

fn fingerprint(s: &crate::ReadOnlySession) -> Result<String, String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(fs::read(s.state_path()).map_err(|e| e.to_string())?)
    ))
}
fn waiting(s: &crate::ReadOnlySession) -> bool {
    let project = s.project();
    let cycle = project.current_cycle();
    let Some(node) = cycle.step_now_open().and_then(|id| cycle.steps().node(id)) else {
        return false;
    };
    if node.kind == crate::NodeKind::Question {
        return true;
    }
    if node.kind == crate::NodeKind::Synthesis {
        if let Some(plan) = project.approved_plan() {
            return plan.current(project).is_some_and(|p| p.receipt.is_none());
        }
        return !cfg!(feature = "approved-plan");
    }
    false
}
/// Pure handoff validation: no consent, completion, Project mutation or persistent token.
pub(crate) fn await_user(root: &str, question: &str) -> Result<Value, String> {
    if question.trim().is_empty() || question.len() > 4000 {
        return Err("사용자에게 물을 구체적인 질문이 필요하다 (1..4000 bytes)".into());
    }
    let root = resolved(root)?;
    let s = session(&root)?;
    if !waiting(&s) {
        return Err(correction(&root));
    }
    Ok(
        json!({"project_root":root,"state_digest":fingerprint(&s)?,"question":question,"status":"awaiting_user"}),
    )
}
fn finished(s: &crate::ReadOnlySession) -> bool {
    s.project().current_chain().is_some_and(|c| c.is_closed())
}
fn stop(store: &Store, input: &Value, id: &str) -> Result<Value, String> {
    let Some(bound) = store.data.sessions.get(id) else {
        return Ok(json!({}));
    };
    let s = match session(&bound.root) {
        Ok(s) => s,
        Err(error) => return Ok(json!({"decision":"block","reason":format!("연결된 GIL 프로젝트 상태를 확인하지 못했다: {error}. 완료로 보고하지 않는다. 사용자 중단은 항상 가능하다.")})),
    };
    if finished(&s) {
        return Ok(json!({}));
    }
    if let Some(wait) = &bound.wait {
        let question = wait["question"].as_str().unwrap_or("");
        if waiting(&s)
            && wait["state_digest"] == fingerprint(&s)?
            && !question.is_empty()
            && input["last_assistant_message"]
                .as_str()
                .is_some_and(|m| m.trim_end().ends_with(question))
        {
            return Ok(json!({}));
        }
    }
    Ok(json!({"decision":"block","reason":correction(&bound.root)}))
}
fn response_ok(input: &Value) -> bool {
    fn explicit_success(r: &Value) -> bool {
        if r["isError"] == true || r["is_error"] == true { return false; }
        r["ok"] == true || r["structuredContent"]["ok"] == true || r["structured_content"]["ok"] == true
    }
    let r = &input["tool_response"];
    if r["isError"] == true || r["is_error"] == true { return false; }
    if explicit_success(r) { return true; }
    // Some Host versions encode the MCP envelope as a JSON string/text block. Parse only
    // explicit structured success; never infer success from human-readable receipt prose.
    let text = r.as_str().or_else(|| r["content"].as_array().filter(|a| a.len() == 1)
        .and_then(|a| a[0]["text"].as_str()));
    text.and_then(|text| serde_json::from_str::<Value>(text).ok()).as_ref().is_some_and(explicit_success)
}

fn hook(input: &Value, exe: &Path, host: &str) -> Result<Value, String> {
    let event = input["hook_event_name"]
        .as_str()
        .ok_or("missing hook_event_name")?;
    // Host comes from the installed hook argv, never from model-controlled tool arguments.
    // Old unqualified bindings remain on disk but cannot attach another Host's chat.
    let session_id = input["session_id"].as_str().filter(|s| !s.is_empty() && s.len() <= 200);
    let key = session_id.map(|id| format!("{host}:{id}"));
    let id = key.as_deref().unwrap_or("");
    if id.is_empty() && matches!(event, "Stop" | "UserPromptSubmit") {
        return Ok(context(event, "GIL 세션 식별자가 없어 진행 검사를 적용하지 않았다. 파일 보호는 별도로 유지한다."));
    }
    // A global matcher being invoked does not itself establish GIL scope.
    if event == "PreToolUse" && !matches!(tool(input),
        "Bash" | "exec_command" | "Write" | "Edit" | "MultiEdit" | "NotebookEdit" | "apply_patch" | "wrap_up") {
        return Ok(json!({}));
    }
    let mut store = Store::open()?;
    match event {
        "UserPromptSubmit" => {
            // Only a real user prompt can detach. No tool parameter or assistant final text can.
            if input["prompt"]
                .as_str()
                .is_some_and(|p| p.trim() == "GIL 일시 중지")
            {
                store.data.sessions.remove(id);
                store.save()?;
                return Ok(context(
                    event,
                    "이 채팅의 GIL 진행 검사를 일시 해제했다. 프로젝트 파일 쓰기 보호는 유지한다. 재개하려면 gil_context를 호출한다. 완료·승인은 기록하지 않았다.",
                ));
            }
            // Once the recorded Chain has closed, later unrelated user requests in this
            // chat must not receive an "unfinished GIL" instruction. File protection remains.
            if store.data.sessions.get(id).is_some_and(|bound| session(&bound.root).is_ok_and(|s| finished(&s))) {
                store.data.sessions.remove(id);
                store.save()?;
                return Ok(json!({}));
            }
            if let Some(bound) = store.data.sessions.get_mut(id) {
                bound.wait = None;
                let message = correction(&bound.root);
                store.save()?;
                return Ok(context(event, &message));
            }
        }
        "PostToolUse" if response_ok(input) => {
            let name = tool(input);
            if matches!(
                name,
                "gil_start" | "gil_context" | "gil_open" | "gil_close" | "gil_await_user"
            ) {
                if id.is_empty() { return Err("missing session_id; project enrollment not confirmed".into()); }
                let root = resolved(
                    input["tool_input"]["project_root"]
                        .as_str()
                        .ok_or("missing project_root")?,
                )?;
                // A real validated GIL project is required even if another MCP mimics a name.
                drop(session(&root)?);
                store.data.roots.insert(root.clone());
                let wait = if name == "gil_await_user" {
                    Some(await_user(
                        root.to_str().ok_or("non-UTF8 root")?,
                        input["tool_input"]["question"]
                            .as_str()
                            .ok_or("missing question")?,
                    )?)
                } else {
                    None
                };
                store.data.sessions.insert(
                    id.to_owned(),
                    Bound {
                        root: root.clone(),
                        wait,
                    },
                );
                store.save()?;
                return Ok(context(
                    event,
                    &format!(
                        "GIL 보호 폴더 자동 연결 완료: {}. 이 채팅의 사용자 대기·종료 검사를 적용한다. {}",
                        root.display(),
                        correction(&root)
                    ),
                ));
            }
        }
        "Stop" => return stop(&store, input, id),
        "PreToolUse" => {
            let name = tool(input);
            if store.data.roots.is_empty() {
                return Ok(json!({}));
            }
            if name == "wrap_up" {
                // Claude Code may expose a structured turn-end tool before Stop.
                let verdict = stop(
                    &store,
                    &json!({"last_assistant_message":input["tool_input"]["ask"]}),
                    id,
                )?;
                if let Some(reason) = verdict["reason"].as_str() {
                    return Ok(deny(reason));
                }
            }
            if matches!(name, "Bash" | "exec_command") && !store.data.roots.is_empty() {
                let args = &input["tool_input"];
                if args["run_in_background"] == true {
                    return Ok(deny("GIL 보호 명령은 foreground로 끝내야 한다."));
                }
                let command = args["command"]
                    .as_str()
                    .or_else(|| args["cmd"].as_str())
                    .ok_or("missing shell command")?;
                let mut runner = vec!["run", "--auto"];
                if host != "legacy" { runner.extend(["--host", host]); }
                runner.push("--");
                let prefix = format!("{} ", invocation(exe, &runner)?);
                if let Some(arg) = command.strip_prefix(&prefix) {
                    if let Some(body) = arg.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
                        let decoded = body.replace("'\\''", "'");
                        if !decoded.contains('\0')
                            && shell_quote(&decoded) == arg
                            && !body.replace("'\\''", "").contains('\'')
                        {
                            return Ok(json!({}));
                        }
                    }
                }
                runner.push(command);
                let wrapped = invocation(exe, &runner)?;
                if host == "claude" && name == "Bash" {
                    // Claude evaluates permissions on the replacement input. Ask, never allow:
                    // do not run the payload in the hook or silently widen Host permissions.
                    let mut updated = args.clone();
                    updated["command"] = json!(wrapped);
                    return Ok(json!({"hookSpecificOutput":{
                        "hookEventName":"PreToolUse", "permissionDecision":"ask",
                        "permissionDecisionReason":"등록된 GIL 폴더만 쓰기 보호하는 명령이다. 이 채팅에 GIL 진행 의무를 추가하지 않는다. 외부 개발·설치 작업은 가능하며 Host 승인을 그대로 요청한다.",
                        "updatedInput":updated
                    }}));
                }
                let note = if host == "codex" {
                    "Codex: 정확한 래퍼 전체에 sandbox_permissions=require_escalated와 구체적인 justification으로 정상 Host 승인을 요청한다."
                } else {
                    "Host가 확인되지 않았다. 설치된 Core의 write-guard install --auto --host codex 또는 --host claude로 Host를 명시해야 한다. 다른 Host의 승인 필드를 추측하지 않는다."
                };
                return Ok(deny(&format!(
                    "GIL 파일 보호: 외부 작업에는 GIL 단계를 요구하지 않는다. {note} 승인 경로가 없거나 거절되면 중단한다.\n{wrapped}"
                )));
            }
            let mut normalized = input.clone();
            normalized["tool_name"] = json!(name);
            // Protect the registry itself from the same direct file tools.
            let guard = check(&store.dir, exe, &normalized)?;
            if guard["hookSpecificOutput"]["permissionDecision"] == "deny" {
                return Ok(guard);
            }
            for root in &store.data.roots {
                let verdict = check(root, exe, &normalized)?;
                if verdict["hookSpecificOutput"]["permissionDecision"] == "deny" {
                    return Ok(verdict);
                }
            }
        }
        _ => {}
    }
    Ok(json!({}))
}

pub(super) fn dispatch(args: &[String]) -> Result<u8, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let host = if args.len() == 4 && args[2] == "--host" && matches!(args[0].as_str(), "hook" | "config") {
        match args[3].as_str() {
            "claude" | "codex" => args[3].as_str(),
            _ => return Err("host must be codex or claude".into()),
        }
    } else { "legacy" };
    match args.first().map(String::as_str) {
        Some("config") if args.len() == 2 || host != "legacy" => {
            println!("{}", config(&exe, host)?); Ok(0)
        }
        Some("install") if args.len() == 4 && args[2] == "--host" => {
            println!("{}", install(&exe, &args[3])?); Ok(0)
        }
        Some("hook") if args.len() == 2 || host != "legacy" => {
            let mut bytes = Vec::new(); io::stdin().take(LIMIT+1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
            if bytes.len() as u64 > LIMIT { return Err("hook input too large".into()); }
            let input: Value = serde_json::from_slice(&bytes).map_err(|_| "invalid hook JSON")?;
            let value = match hook(&input, &exe, host) {
                Ok(value) => value,
                Err(error) => match input["hook_event_name"].as_str() {
                    Some("PreToolUse") if tool(&input) == "wrap_up" => context("PreToolUse", &format!("GIL 진행 연결을 확인하지 못했다: {error}. GIL 완료 확인으로 간주하지 않는다.")),
                    Some("PreToolUse") => deny(&error),
                    // A broken shared registry is not evidence this chat is bound. Do not
                    // impose a global conversation gate. File tools still fail closed above.
                    Some(event @ ("UserPromptSubmit" | "Stop")) => context(event, &format!("GIL 진행 검사 상태를 확인하지 못했다: {error}. GIL 완료로 보고하지 않는다. 파일 보호는 유지한다.")),
                    _ => context(input["hook_event_name"].as_str().unwrap_or("PostToolUse"), &format!("GIL 훅 연결 실패: {error}. 보호 적용 완료로 보고하지 않는다.")),
                }
            };
            println!("{value}"); Ok(0)
        }
        Some("run") if args.len() == 4 && args[2] == "--" => {
            let store = Store::open()?;
            let roots: Vec<_> = store.data.roots.iter().cloned().collect();
            let protected = vec![store.dir.clone()]; drop(store);
            run_roots(&roots, &args[3], &protected, "legacy")
        }
        Some("run") if args.len() == 6 && args[2] == "--host" && matches!(args[3].as_str(), "codex" | "claude") && args[4] == "--" => {
            let store = Store::open()?;
            let roots: Vec<_> = store.data.roots.iter().cloned().collect();
            let protected = vec![store.dir.clone()]; drop(store);
            run_roots(&roots, &args[5], &protected, &args[3])
        }
        Some("status") if args.len() == 2 => {
            let store = Store::open()?;
            println!("{}",json!({"registered_roots":store.data.roots,"bound_sessions":store.data.sessions.len(),"current_core":exe,"installed_hooks":installed_hooks(&exe, &store.dir)?,"host_activation":"not_proven_by_this_command"})); Ok(0)
        }
        _ => Err("usage: gil write-guard config|hook|status --auto; gil write-guard run --auto -- COMMAND".into()),
    }
}

fn config(exe: &Path, host: &str) -> Result<Value, String> {
    let command = if host == "legacy" { invocation(exe, &["hook", "--auto"])? }
        else { invocation(exe, &["hook", "--auto", "--host", host])? };
    let mut hooks = serde_json::Map::new();
    for event in ["PreToolUse", "PostToolUse", "UserPromptSubmit", "Stop"] {
        let mut entry = json!({"hooks":[{"type":"command","command":command,"timeout":15}]});
        if event == "PreToolUse" { entry["matcher"] = json!("^(Bash|exec_command|apply_patch|Write|Edit|MultiEdit|NotebookEdit)$|(^|__)wrap_up$"); }
        if event == "PostToolUse" { entry["matcher"] = json!("(^|__)(gil_start|gil_context|gil_open|gil_close|gil_await_user)$"); }
        hooks.insert(event.into(), json!([entry]));
    }
    Ok(json!({"hooks":hooks}))
}

fn install(exe: &Path, host: &str) -> Result<Value, String> {
    if !cfg!(target_os = "macos") {
        return Err("automatic Host guard installation currently supports macOS only".into());
    }
    let hosts: &[&str] = match host {
        "both" => &["codex", "claude"],
        "codex" => &["codex"],
        "claude" => &["claude"],
        _ => return Err("host must be codex, claude, or both".into()),
    };
    let store = Store::open()?;
    let home = store.dir.parent().ok_or("missing home")?;
    let mut updated = Vec::new();
    for host in hosts {
        let config = config(exe, host)?;
        let path = home.join(if *host == "codex" {
            ".codex/hooks.json"
        } else {
            ".claude/settings.json"
        });
        regular(&path)?;
        let old = if path.exists() {
            fs::read(&path).map_err(|e| e.to_string())?
        } else {
            b"{}".to_vec()
        };
        let mut settings: Value =
            serde_json::from_slice(&old).map_err(|_| "invalid existing Host JSON; preserved")?;
        let object = settings
            .as_object_mut()
            .ok_or("Host settings must be an object")?;
        let hooks = object
            .entry("hooks")
            .or_insert(json!({}))
            .as_object_mut()
            .ok_or("invalid existing hooks; preserved")?;
        for (event, entries) in config["hooks"].as_object().unwrap() {
            let list = hooks
                .entry(event)
                .or_insert(json!([]))
                .as_array_mut()
                .ok_or("invalid hook event; preserved")?;
            // Replace only a previously generated, single-command automatic GIL entry.
            list.retain(|entry| {
                let Some(commands) = entry["hooks"].as_array() else {
                    return true;
                };
                !(commands.len() == 1
                    && commands[0]["type"] == "command"
                    && commands[0]["command"].as_str().is_some_and(generated_hook))
            });
            list.extend(entries.as_array().unwrap().iter().cloned());
        }
        let mut next = serde_json::to_vec_pretty(&settings).map_err(|e| e.to_string())?;
        next.push(b'\n');
        if next == old {
            updated.push(json!({"host":host,"settings":path,"changed":false}));
            continue;
        }
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let backup = store.dir.join(format!("{host}-settings-{stamp}.json"));
        let mut opts = OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        opts.open(&backup)
            .and_then(|mut f| f.write_all(&old))
            .map_err(|e| e.to_string())?;
        let parent = path.parent().ok_or("missing Host directory")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let tmp = parent.join(format!(".gil-hooks-{stamp}.tmp"));
        opts.open(&tmp)
            .and_then(|mut f| {
                f.write_all(&next)?;
                f.sync_all()
            })
            .map_err(|e| e.to_string())?;
        fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
        updated.push(json!({"host":host,"settings":path,"backup":backup,"changed":true}));
    }
    Ok(
        json!({"updated":updated,"activation":"Restart/new Host session required. Review and trust Codex hooks using /hooks. This command does not edit trust or permission settings."}),
    )
}

// Only replace our canonical generated command, never an arbitrary shell suffix or user hook.
fn generated_hook(command: &str) -> bool {
    let Some(rest) = command.strip_prefix('\'') else { return false; };
    let mut executable = String::new();
    let mut tail = rest;
    loop {
        let Some(end) = tail.find('\'') else { return false; };
        executable.push_str(&tail[..end]);
        tail = &tail[end + 1..];
        if let Some(next) = tail.strip_prefix("\\''") {
            executable.push('\'');
            tail = next;
        } else { break; }
    }
    let path = Path::new(&executable);
    if !path.is_absolute() || !matches!(path.file_stem().and_then(|s| s.to_str()), Some("gil" | "gil-write-guard")) { return false; }
    ["legacy", "codex", "claude"].iter().any(|host| {
        let args = if *host == "legacy" { vec!["hook", "--auto"] }
            else { vec!["hook", "--auto", "--host", host] };
        invocation(path, &args).is_ok_and(|expected| expected == command)
    })
}

// Read-only update check. Matching paths prove configuration agreement, not loaded Host state.
fn installed_hooks(exe: &Path, dir: &Path) -> Result<Value, String> {
    let home = dir.parent().ok_or("missing home")?;
    let mut result = serde_json::Map::new();
    for (host, relative) in [("codex", ".codex/hooks.json"), ("claude", ".claude/settings.json")] {
        let path = home.join(relative);
        let expected = config(exe, host)?;
        let settings: Value = match fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice(&bytes) {
                Ok(value) => value,
                Err(_) => { result.insert(host.into(), json!({"status":"invalid_settings"})); continue; }
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                result.insert(host.into(), json!({"status":"not_installed"})); continue;
            },
            Err(_) => { result.insert(host.into(), json!({"status":"unreadable_settings"})); continue; }
        };
        let matched = expected["hooks"].as_object().unwrap().iter().all(|(event, entries)| {
            let generated: Vec<_> = settings["hooks"][event].as_array().into_iter().flatten()
                .filter(|entry| entry["hooks"].as_array().is_some_and(|hooks| hooks.iter()
                    .any(|h| h["command"].as_str().is_some_and(generated_hook)))) .collect();
            generated.len() == 1 && *generated[0] == entries[0]
        });
        result.insert(host.into(), json!({"status":if matched {"matches_current_core"} else {"refresh_required"}}));
    }
    Ok(Value::Object(result))
}
