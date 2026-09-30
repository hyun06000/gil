//! **MCP stdio 표면의 시험** — 진짜 프로세스를 세워 진짜 frame 을 주고받는다.
//!
//! 여기서 재는 것은 배선이지 domain 이 아니다. GIL 이 무엇을 옳다고 판정하는가는 이미 다른
//! 시험들이 잡았다. 이 파일이 지키는 것은 넷이다.
//!
//! 1. **같은 글자** — 같은 Project 에서 CLI 와 MCP 의 `said` 가 byte 로 같다
//! 2. **정확 경로** — 자식 자리를 받고 부모 Project 로 물러서지 않는다
//! 3. **깨끗한 stdout** — JSON-RPC frame 말고는 한 글자도 없다
//! 4. **명시적 tool 표** — 기존 12개 + 읽기 전용 MCP App 5개, UI 쓰기 문은 없다

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

fn gil() -> PathBuf {
    // 시험이 부르는 것은 **지금 지은 그 binary** 다.
    let mut at = std::env::current_exe().expect("시험 실행 파일");
    at.pop();
    if at.ends_with("deps") {
        at.pop();
    }
    at.join(format!("gil{}", std::env::consts::EXE_SUFFIX))
}

fn scratch(name: &str) -> PathBuf {
    let at = std::env::temp_dir().join(format!(
        "gil-mcp-test-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&at);
    std::fs::create_dir_all(&at).expect("자리");
    at
}

/// 진짜 Project 하나를 만든다 — CLI 로. 시험이 저장 형식을 손으로 짓지 않는다.
fn started(at: &Path) {
    let done = Command::new(gil())
        .arg("start")
        .current_dir(at)
        .output()
        .expect("gil start");
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
}

fn cli_status(at: &Path) -> String {
    let done = Command::new(gil())
        .arg("status")
        .current_dir(at)
        .output()
        .expect("gil status");
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    String::from_utf8(done.stdout).expect("utf-8")
}

// ── MCP 손님 ───────────────────────────────────────────────────────────────

struct Client {
    kid: Child,
    out: BufReader<std::process::ChildStdout>,
    next: i64,
    settings: Option<PathBuf>,
}

impl Client {
    /// **Node 없이** 선다 — Rust binary 하나가 전부다.
    fn serve() -> Self {
        let settings = scratch("client-settings");
        let mut client = Self::serve_with(&settings);
        client.settings = Some(settings);
        client
    }

    fn serve_with(settings: &Path) -> Self {
        let mut kid = Command::new(gil())
            .args(["mcp", "--serve"])
            // No Node, cargo, repository executable, or global gil on PATH.
            .env("PATH", "/usr/bin:/bin")
            .env("GIL_MONITOR_STATE_DIR", settings.join("bindings"))
            // cwd 를 **다른 자리**에 둔다. 요청 결과가 여기에 흔들리면 그것이 결함이다.
            .current_dir(std::env::temp_dir())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("gil mcp --serve");
        let out = BufReader::new(kid.stdout.take().expect("stdout"));
        Self { kid, out, next: 1, settings: None }
    }

    fn send(&mut self, frame: &serde_json::Value) {
        let line = serde_json::to_string(frame).expect("frame");
        let stdin = self.kid.stdin.as_mut().expect("stdin");
        writeln!(stdin, "{line}").expect("보낸다");
        stdin.flush().expect("흘린다");
    }

    fn read(&mut self) -> serde_json::Value {
        let mut line = String::new();
        self.out.read_line(&mut line).expect("받는다");
        assert!(!line.trim().is_empty(), "빈 줄을 받았다");
        serde_json::from_str(&line)
            .unwrap_or_else(|err| panic!("JSON-RPC 가 아닌 것이 stdout 에 있다: {line:?} ({err})"))
    }

    fn request(&mut self, method: &str, params: serde_json::Value) -> serde_json::Value {
        let id = self.next;
        self.next += 1;
        self.send(&serde_json::json!({
            "jsonrpc": "2.0", "id": id, "method": method, "params": params
        }));
        let got = self.read();
        assert_eq!(got["id"], serde_json::json!(id), "id 가 어긋났다");
        got
    }

    fn initialize(&mut self) -> serde_json::Value {
        let got = self.request(
            "initialize",
            serde_json::json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "gil-test", "version": "0" }
            }),
        );
        self.send(&serde_json::json!({
            "jsonrpc": "2.0", "method": "notifications/initialized"
        }));
        got
    }

    fn status(&mut self, root: &str) -> serde_json::Value {
        self.request(
            "tools/call",
            serde_json::json!({
                "name": "gil_status",
                "arguments": { "project_root": root }
            }),
        )
    }

    /// stdin 을 닫고 **깨끗이** 끝나는지 본다 — 남은 stdout 도 함께 돌려준다.
    fn eof(mut self) -> (std::process::ExitStatus, String) {
        drop(self.kid.stdin.take());
        let mut rest = String::new();
        use std::io::Read as _;
        self.out.read_to_string(&mut rest).ok();
        let done = self.kid.wait().expect("끝난다");
        if let Some(settings) = self.settings.take() { std::fs::remove_dir_all(settings).unwrap(); }
        (done, rest)
    }
}

fn text_of(reply: &serde_json::Value) -> String {
    reply["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("text 가 없다: {reply}"))
        .to_string()
}

// ── 시험 ───────────────────────────────────────────────────────────────────

#[test]
fn initialize_succeeds_and_says_who_is_answering() {
    let mut mcp = Client::serve();
    let got = mcp.initialize();
    assert_eq!(got["jsonrpc"], "2.0");
    assert!(got["result"]["serverInfo"]["name"].is_string(), "{got}");
    assert!(got["result"]["capabilities"]["tools"].is_object(), "{got}");
    let (done, _) = mcp.eof();
    assert!(done.success());
}

#[test]
fn tools_list_preserves_seventeen_tools_and_the_read_only_app_boundary() {
    let mut mcp = Client::serve();
    mcp.initialize();
    let got = mcp.request("tools/list", serde_json::json!({}));
    let tools = got["result"]["tools"].as_array().expect("tools");
    let mut names: Vec<_> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    names.sort();
    assert_eq!(names, ["gil_close", "gil_companion_status", "gil_context", "gil_cycle", "gil_help", "gil_monitor_detail", "gil_monitor_poll", "gil_monitor_prepare", "gil_monitor_read", "gil_open", "gil_restore", "gil_revisit", "gil_start", "gil_status", "gil_story", "show_gil_companion", "show_gil_monitor"]);
    for tool in tools.iter().filter(|t| t["name"].as_str().unwrap().starts_with("gil_monitor_") || t["name"] == "show_gil_monitor") {
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
    }
    mcp.eof();
}

#[test]
fn the_app_resource_is_embedded_and_binds_both_metadata_keys() {
    let mut mcp = Client::serve();
    let init = mcp.initialize();
    assert!(init["result"]["capabilities"]["resources"].is_object());
    let got = mcp.request("tools/list", serde_json::json!({}));
    let show = got["result"]["tools"].as_array().unwrap().iter().find(|t| t["name"] == "show_gil_monitor").unwrap();
    let uri = show["_meta"]["ui"]["resourceUri"].clone();
    assert_eq!(uri, show["_meta"]["ui/resourceUri"]);
    let listed = mcp.request("resources/list", serde_json::json!({}));
    assert_eq!(listed["result"]["resources"][0]["uri"], uri);
    let resource = mcp.request("resources/read", serde_json::json!({"uri": uri}));
    let content = &resource["result"]["contents"][0];
    assert_eq!(content["mimeType"], "text/html;profile=mcp-app");
    assert_eq!(content["text"], include_str!("../plugins/gil-companion-prototype/assets/monitor.html"));
    assert_eq!(content["_meta"]["ui"]["csp"]["connectDomains"], serde_json::json!([]));
    let rejected = mcp.request("resources/read", serde_json::json!({"uri": "file:///etc/passwd"}));
    assert!(rejected["error"].is_object());
    mcp.eof();
}

#[test]
fn native_actions_keep_the_wrapper_and_reject_extra_monitor_inputs() {
    let at = scratch("native-actions");
    let mut mcp = Client::serve(); mcp.initialize();
    let start = monitor_call(&mut mcp, "gil_start", serde_json::json!({"project_root": at}));
    assert_eq!(start["structuredContent"]["ok"], true);
    assert_eq!(start["structuredContent"]["exit_code"], 0);
    let refused = monitor_call(&mut mcp, "gil_open", serde_json::json!({"project_root": at, "kind": "--help"}));
    assert_eq!(refused["isError"], true);
    assert_eq!(refused["structuredContent"]["exit_code"], serde_json::Value::Null);
    assert_eq!(refused["structuredContent"]["problem"], "Cycle 종류 는 '-' 로 시작할 수 없다");
    let root = serde_json::json!({"project_root": at});
    let prepare = monitor_call(&mut mcp, "gil_monitor_prepare", root);
    let scope = prepare["structuredContent"]["scope_id"].clone();
    let invalid = mcp.request("tools/call", serde_json::json!({"name": "gil_monitor_read", "arguments": {"scope_id": scope, "project_root": at}}));
    assert!(invalid["error"].is_object());
    let view = monitor_call(&mut mcp, "show_gil_monitor", serde_json::json!({"scope_id": scope}));
    assert_ne!(view["isError"], true);
    assert!(view["content"][0]["text"].as_str().unwrap().contains("화면 표시나 지속 표시 성공을 판정하지 않는다"));
    mcp.eof(); std::fs::remove_dir_all(at).unwrap();
}

fn monitor_call(client: &mut Client, name: &str, arguments: serde_json::Value) -> serde_json::Value {
    client.request("tools/call", serde_json::json!({ "name": name, "arguments": arguments }))["result"].clone()
}

fn record_question(at: &Path, answer: &str) {
    for (args, input) in [
        (vec!["open", "question"], "objective: 재시작 검수\nnext_action: 질문한다\ndone_when: 답을 기록했다\n".to_owned()),
        (vec!["close"], format!("question: 어느 Project 인가\nchoices: 하나 / 둘\nresponse: {answer}\n")),
    ] {
        let mut process = Command::new(gil()).args(args).current_dir(at).stdin(Stdio::piped())
            .stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        process.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
        let done = process.wait_with_output().unwrap();
        assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    }
}

#[test]
fn a_new_process_restores_each_old_scope_without_prepare_or_a_project_path() {
    let at = scratch("automatic-resume");
    let a = at.join("a/same-name"); let b = at.join("b/same-name");
    for root in [&a, &b] { std::fs::create_dir_all(root).unwrap(); started(root); }
    record_question(&a, "첫 번째 기록"); record_question(&b, "두 번째 기록");
    let original_a = std::fs::read(a.join(gil::STATE_PATH)).unwrap();
    let original_b = std::fs::read(b.join(gil::STATE_PATH)).unwrap();
    let mut old = Client::serve_with(&at.join("settings")); old.initialize();
    let mut captured = Vec::new();
    for root in [&a, &b] {
        let prepared = monitor_call(&mut old, "gil_monitor_prepare", serde_json::json!({ "project_root": root }));
        assert_ne!(prepared["isError"], true, "{prepared}");
        let scope = prepared["structuredContent"]["scope_id"].clone();
        let detail = monitor_call(&mut old, "gil_monitor_detail", serde_json::json!({ "scope_id": scope, "step_ref": "step:C1/S1" }));
        let view = monitor_call(&mut old, "gil_monitor_read", serde_json::json!({ "scope_id": scope }));
        assert_ne!(view["isError"], true, "{view}");
        captured.push((scope, detail["structuredContent"].clone(), view["structuredContent"].clone()));
    }
    assert_ne!(captured[0].0, captured[1].0); assert_ne!(captured[0].1, captured[1].1);
    assert!(old.eof().0.success());

    let mut fresh = Client::serve_with(&at.join("settings")); fresh.initialize();
    // No root, prepare, current directory, Companion selection, or Agent intervention.
    for (scope, expected_detail, expected_view) in &captured {
        let detail = monitor_call(&mut fresh, "gil_monitor_detail", serde_json::json!({ "scope_id": scope, "step_ref": "step:C1/S1" }));
        assert_eq!(&detail["structuredContent"], expected_detail);
        let view = monitor_call(&mut fresh, "gil_monitor_read", serde_json::json!({ "scope_id": scope }));
        assert_eq!(view["structuredContent"]["view"]["timeline"], expected_view["view"]["timeline"]);
        assert_ne!(view["structuredContent"]["revision"], expected_view["revision"]);
        assert!(!view.to_string().contains(at.to_str().unwrap()));
    }
    assert_eq!(std::fs::read(a.join(gil::STATE_PATH)).unwrap(), original_a);
    assert_eq!(std::fs::read(b.join(gil::STATE_PATH)).unwrap(), original_b);
    assert!(fresh.eof().0.success()); std::fs::remove_dir_all(at).unwrap();
}

#[test]
fn restart_does_not_open_a_replacement_even_after_it_is_explicitly_prepared() {
    let at = scratch("replacement-safety"); let root = at.join("project");
    std::fs::create_dir(&root).unwrap(); started(&root);
    let mut old = Client::serve_with(&at.join("settings")); old.initialize();
    let prepared = monitor_call(&mut old, "gil_monitor_prepare", serde_json::json!({ "project_root": root }));
    let scope = prepared["structuredContent"]["scope_id"].clone();
    old.eof();
    std::fs::rename(&root, at.join("original-project")).unwrap();
    let mut fresh = Client::serve_with(&at.join("settings")); fresh.initialize();
    let missing = monitor_call(&mut fresh, "gil_monitor_read", serde_json::json!({ "scope_id": scope }));
    assert_eq!(missing["structuredContent"]["code"], "project_missing");
    std::fs::create_dir(&root).unwrap(); started(&root);
    let replaced = monitor_call(&mut fresh, "gil_monitor_prepare", serde_json::json!({ "project_root": root }));
    assert_ne!(replaced["isError"], true, "{replaced}");
    assert_ne!(replaced["structuredContent"]["scope_id"], scope);
    let refused = monitor_call(&mut fresh, "gil_monitor_read", serde_json::json!({ "scope_id": scope }));
    assert_eq!(refused["structuredContent"]["code"], "project_moved");
    assert!(!refused.to_string().contains(at.to_str().unwrap()));
    fresh.eof(); std::fs::remove_dir_all(at).unwrap();
}

#[test]
fn monitor_uses_exact_scope_and_canonical_view_without_touching_project_bytes() {
    let at = scratch("monitor-read-only");
    started(&at);
    std::fs::write(at.join("work.txt"), "원문 세계").unwrap();
    let state = std::fs::read(at.join(gil::STATE_PATH)).unwrap();
    let mut mcp = Client::serve(); mcp.initialize();
    let ready = monitor_call(&mut mcp, "gil_monitor_prepare", serde_json::json!({ "project_root": at }));
    assert_ne!(ready["isError"], true, "{ready}");
    assert!(!ready.to_string().contains(at.to_str().unwrap()));
    let scope = ready["structuredContent"]["scope_id"].clone();
    let seen = monitor_call(&mut mcp, "gil_monitor_read", serde_json::json!({ "scope_id": scope }));
    assert_ne!(seen["isError"], true, "{seen}");
    let mut actual = seen["structuredContent"]["view"].clone();
    let session = gil::ProjectSession::open_read_only(gil::RuleSet::builtin().unwrap(), at.join(gil::STATE_PATH)).unwrap();
    let view = gil::monitor_view_v1(&session.monitor().unwrap()).unwrap();
    let mut expected = serde_json::to_value(&view).unwrap();
    actual["captured_at_unix_ms"] = serde_json::json!(0);
    expected["captured_at_unix_ms"] = serde_json::json!(0);
    assert_eq!(actual, expected);
    drop(session);
    let hint = monitor_call(&mut mcp, "gil_monitor_poll", serde_json::json!({ "scope_id": scope }));
    assert_eq!(hint["structuredContent"].as_object().unwrap().len(), 3);
    assert!(hint["structuredContent"]["revision"].is_string());
    assert!(hint["structuredContent"].get("view").is_none());
    assert_eq!(std::fs::read(at.join(gil::STATE_PATH)).unwrap(), state);
    assert_eq!(std::fs::read_to_string(at.join("work.txt")).unwrap(), "원문 세계");
    // Unknown identity is well-formed. A malformed scope is a schema rejection,
    // matching the public JS adapter, and must not enter the binding registry.
    let malformed = mcp.request("tools/call", serde_json::json!({
        "name": "gil_monitor_read", "arguments": {"scope_id": "project:unknown"}
    }));
    assert_eq!(malformed["error"]["code"], -32602);
    let unknown = monitor_call(&mut mcp, "gil_monitor_read", serde_json::json!({ "scope_id": format!("project:{}", "0".repeat(64)) }));
    assert_eq!(unknown["structuredContent"]["code"], "reconnect_required");
    let child = at.join("child"); std::fs::create_dir(&child).unwrap();
    let refused = monitor_call(&mut mcp, "gil_monitor_prepare", serde_json::json!({ "project_root": child }));
    assert_eq!(refused["structuredContent"]["code"], "not_a_project");
    assert!(!refused.to_string().contains(at.to_str().unwrap()));
    mcp.eof(); std::fs::remove_dir_all(at).unwrap();
}

#[test]
fn monitor_report_is_complete_and_a_busy_project_is_not_recovered_or_rewritten() {
    let at = scratch("monitor-detail"); started(&at);
    let mut opening = Command::new(gil()).args(["open", "question"]).current_dir(&at)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    opening.stdin.take().unwrap().write_all("objective: Monitor 검수\nnext_action: 질문한다\ndone_when: 답을 기록했다\n".as_bytes()).unwrap();
    let opened = opening.wait_with_output().unwrap();
    assert!(opened.status.success(), "{}", String::from_utf8_lossy(&opened.stderr));
    let mut close = Command::new(gil()).args(["close"]).current_dir(&at)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    close.stdin.take().unwrap().write_all("question: 무엇을 만들까\nchoices: 검수 / 다음\nresponse: <script>원문</script>\n".as_bytes()).unwrap();
    let done = close.wait_with_output().unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let before = std::fs::read(at.join(gil::STATE_PATH)).unwrap();
    let mut mcp = Client::serve(); mcp.initialize();
    let ready = monitor_call(&mut mcp, "gil_monitor_prepare", serde_json::json!({ "project_root": at }));
    let scope = ready["structuredContent"]["scope_id"].clone();
    let detail = monitor_call(&mut mcp, "gil_monitor_detail", serde_json::json!({ "scope_id": scope, "step_ref": "step:C1/S1" }));
    assert_ne!(detail["isError"], true, "{detail}");
    let fields = detail["structuredContent"]["detail"]["report"]["fields"].as_array().unwrap();
    assert_eq!(fields.iter().map(|f| f["name"].as_str().unwrap()).collect::<Vec<_>>(), ["choices", "question", "response"]);
    assert_eq!(fields[2]["value"], "<script>원문</script>");
    let missing = monitor_call(&mut mcp, "gil_monitor_detail", serde_json::json!({ "scope_id": scope, "step_ref": "step:C2/S1" }));
    assert_eq!(missing["structuredContent"]["code"], "not_found");
    let locked = gil::ProjectSession::open(gil::RuleSet::builtin().unwrap(), at.join(gil::STATE_PATH)).unwrap();
    let busy = monitor_call(&mut mcp, "gil_monitor_read", serde_json::json!({ "scope_id": scope }));
    assert_eq!(busy["structuredContent"]["code"], "busy");
    drop(locked);
    assert_eq!(std::fs::read(at.join(gil::STATE_PATH)).unwrap(), before);
    mcp.eof(); std::fs::remove_dir_all(at).unwrap();
}

/// **이 조각의 목표 문장.** 같은 Project 에서 두 진입점이 같은 글자를 낸다.
#[test]
fn the_cli_and_the_mcp_say_the_very_same_bytes() {
    let at = scratch("same");
    started(&at);
    let by_cli = cli_status(&at);

    let mut mcp = Client::serve();
    mcp.initialize();
    let got = mcp.status(at.to_str().expect("utf-8"));
    assert_eq!(got["result"]["isError"], serde_json::json!(false), "{got}");
    let by_mcp = text_of(&got);
    mcp.eof();

    assert_eq!(
        by_mcp.as_bytes(),
        by_cli.as_bytes(),
        "같은 자리에서 다른 말을 했다\nCLI:\n{by_cli}\nMCP:\n{by_mcp}"
    );
    std::fs::remove_dir_all(&at).ok();
}

/// 부모가 Project 여도 **자식은 자식이다.** 말없이 위로 물러서지 않는다.
#[test]
fn a_child_directory_never_opens_the_parent_project() {
    let parent = scratch("parent");
    started(&parent);
    let child = parent.join("tmp");
    std::fs::create_dir_all(&child).expect("자리");

    let before = std::fs::read(parent.join(".gil/state.yaml")).expect("기록");
    let by_cli_at_parent = cli_status(&parent);

    let mut mcp = Client::serve();
    mcp.initialize();
    let got = mcp.status(child.to_str().expect("utf-8"));
    assert_eq!(got["result"]["isError"], serde_json::json!(true), "{got}");
    let said = text_of(&got);
    mcp.eof();

    // 거절이지, 부모의 상태가 아니다.
    assert_ne!(said, by_cli_at_parent, "자식 요청이 부모 Project 를 열었다");
    assert!(said.contains("위로 거슬러 오르지 않았다"), "{said}");
    // 거절문에 절대경로를 싣지 않는다.
    assert!(!said.contains('/'), "거절문에 경로가 새어 나왔다: {said}");

    // 부모의 bytes 와 Journey 는 그대로다.
    let after = std::fs::read(parent.join(".gil/state.yaml")).expect("기록");
    assert_eq!(before, after, "부모의 기록이 바뀌었다");
    assert_eq!(cli_status(&parent), by_cli_at_parent, "부모의 자리가 움직였다");

    std::fs::remove_dir_all(&parent).ok();
}

#[test]
fn a_relative_project_root_is_refused() {
    let mut mcp = Client::serve();
    mcp.initialize();
    let got = mcp.status("some/where");
    assert_eq!(got["result"]["isError"], serde_json::json!(true), "{got}");
    let said = text_of(&got);
    assert!(said.contains("온전한 경로"), "{said}");
    mcp.eof();
}

#[test]
fn starting_receipt_uses_the_requested_root_not_the_servers_directory() {
    let base = scratch("start-receipt");
    let cli_root = base.join("cli-project");
    let mcp_root = base.join("mcp-project");
    std::fs::create_dir(&cli_root).unwrap();
    std::fs::create_dir(&mcp_root).unwrap();
    let cli = Command::new(gil()).arg("start").current_dir(&cli_root).output().unwrap();
    assert!(cli.status.success());
    let mut mcp = Client::serve();
    mcp.initialize();
    let reply = mcp.request("tools/call", serde_json::json!({
        "name": "gil_start", "arguments": {"project_root": mcp_root}
    }));
    assert_eq!(reply["result"]["structuredContent"]["said"], String::from_utf8(cli.stdout).unwrap());
    assert!(text_of(&reply).ends_with("기록: .gil/state.yaml\n"));
    mcp.eof();
    std::fs::remove_dir_all(base).unwrap();
}

#[test]
fn a_place_without_a_project_is_refused() {
    let empty = scratch("empty");
    let mut mcp = Client::serve();
    mcp.initialize();
    let got = mcp.status(empty.to_str().expect("utf-8"));
    assert_eq!(got["result"]["isError"], serde_json::json!(true), "{got}");
    assert!(text_of(&got).contains("걷기가 없다"), "{got}");
    mcp.eof();
    std::fs::remove_dir_all(&empty).ok();
}

/// server 의 cwd 가 **다른 GIL Project** 여도 답은 준 자리의 것이다.
#[test]
fn the_servers_own_working_directory_never_leaks_into_the_answer() {
    let asked = scratch("asked");
    let elsewhere = scratch("elsewhere");
    started(&asked);
    started(&elsewhere);

    let mut kid = Command::new(gil())
        .args(["mcp", "--serve"])
        .current_dir(&elsewhere) // ← 다른 Project 안에 서 있다
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("serve");
    let out = BufReader::new(kid.stdout.take().expect("stdout"));
    let mut mcp = Client { kid, out, next: 1, settings: None };
    mcp.initialize();
    let got = mcp.status(asked.to_str().expect("utf-8"));
    let said = text_of(&got);
    mcp.eof();

    assert_eq!(said.as_bytes(), cli_status(&asked).as_bytes(), "{said}");
    std::fs::remove_dir_all(&asked).ok();
    std::fs::remove_dir_all(&elsewhere).ok();
}

/// stdin 이 닫히면 **깨끗이** 끝난다 — 매달리지 않고, 뒤에 쓰레기를 남기지 않는다.
#[test]
fn closing_stdin_ends_the_server_cleanly_and_leaves_nothing_behind() {
    let at = scratch("eof");
    started(&at);
    let mut mcp = Client::serve();
    mcp.initialize();
    mcp.status(at.to_str().expect("utf-8"));
    let (done, rest) = mcp.eof();
    assert!(done.success(), "깨끗이 끝나지 않았다: {done:?}");
    assert!(rest.trim().is_empty(), "마지막 frame 뒤에 글자가 남았다: {rest:?}");
    std::fs::remove_dir_all(&at).ok();
}

/// **stdout 에는 JSON-RPC 말고 아무것도 없다.** 진단 한 줄이 새면 여기서 걸린다.
#[test]
fn stdout_carries_json_rpc_frames_and_nothing_else() {
    let at = scratch("clean");
    started(&at);

    let mut kid = Command::new(gil())
        .args(["mcp", "--serve"])
        .current_dir(std::env::temp_dir())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("serve");
    {
        let stdin = kid.stdin.as_mut().expect("stdin");
        for line in [
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#.to_string(),
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.to_string(),
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#.to_string(),
            serde_json::json!({
                "jsonrpc":"2.0","id":3,"method":"tools/call",
                "params":{"name":"gil_status","arguments":{"project_root": at.to_str().expect("utf-8")}}
            }).to_string(),
            // 일부러 거절당할 것 하나 — 거절의 말이 stdout 으로 새는지 본다.
            serde_json::json!({
                "jsonrpc":"2.0","id":4,"method":"tools/call",
                "params":{"name":"gil_status","arguments":{"project_root":"relative/path"}}
            }).to_string(),
        ] {
            writeln!(stdin, "{line}").expect("보낸다");
        }
    }
    drop(kid.stdin.take());
    let done = kid.wait_with_output().expect("끝난다");
    assert!(done.status.success());

    let said = String::from_utf8(done.stdout).expect("utf-8");
    let mut frames = 0;
    for line in said.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let one: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|err| panic!("stdout 에 JSON-RPC 가 아닌 줄이 있다: {line:?} ({err})"));
        assert_eq!(one["jsonrpc"], "2.0", "frame 이 아니다: {line}");
        frames += 1;
    }
    assert_eq!(frames, 4, "답이 넷이어야 한다:\n{said}");

    std::fs::remove_dir_all(&at).ok();
}

// ── T1.1 시작과 관측을 한 흐름으로 ──────────────────────────────────────────
//
// Skill 이 「GIL 프로젝트를 시작하자」 한 요청에 잇는 도구 차례를 **같은 stdio 연결**에서
// 그대로 밟는다. 여기서 재는 것은 배선의 성질 넷이다.
//
//   1. start → prepare → show 가 한 연결에서 이어지고, 첫 View 는 열린 Interview 하나다
//   2. 이미 시작된 자리의 두 번째 start 는 거절되고 기록은 byte 로 같다 — 재초기화 없음
//   3. 화면 재시도(prepare/show 반복)는 같은 scope 를 돌려주고 binding 도 하나뿐이다
//   4. 시작 실패와 App 조회의 typed 거절은 서로 다른 답이며, 어느 쪽도 Project 를 만들거나 바꾸지 않는다
//
// 이것은 **stdio 도구 연쇄**의 시험이다. 실제 Host 의 카드 렌더링·fullscreen 전환·화면 표시는
// 여기서 재지 않는다 — 그것은 App 진입점 시험(`mcp-app/display.test.mjs`)과 사용자 화면
// 확인이 따로 맡는다. 아래 5의 알 수 없는 scope 거절도 Host 렌더링 실패의 시험이 아니다.

fn agent_call(client: &mut Client, name: &str, arguments: serde_json::Value) -> serde_json::Value {
    client.request("tools/call", serde_json::json!({ "name": name, "arguments": arguments }))["result"].clone()
}

fn binding_files(settings: &Path) -> usize {
    std::fs::read_dir(settings.join("bindings")).map(|dir| dir.filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json")).count()).unwrap_or(0)
}

#[test]
fn starting_a_project_leads_to_one_monitor_scope_and_the_first_interview_updates_it() {
    let at = scratch("start-and-watch");
    let root = at.join("project");
    std::fs::create_dir(&root).unwrap();
    let settings = at.join("settings");
    let mut mcp = Client::serve_with(&settings); mcp.initialize();

    // 1. 시작 — Skill 의 첫 걸음. 요청한 root 의 receipt 를 그대로 받는다.
    let started = agent_call(&mut mcp, "gil_start", serde_json::json!({ "project_root": root }));
    assert_eq!(started["isError"], false, "{started}");
    assert_eq!(started["structuredContent"]["ok"], true);
    let receipt = started["structuredContent"]["said"].as_str().unwrap();
    assert!(receipt.starts_with("프로젝트를 시작했다.\n현재: cycle:C1 · interview\n"), "{receipt}");
    assert!(receipt.contains("실행\n  gil open\n"), "{receipt}");
    let state = std::fs::read(root.join(gil::STATE_PATH)).unwrap();
    assert_eq!(binding_files(&settings), 0, "시작만으로는 Monitor binding 을 만들지 않는다");

    // 2. 준비 → 표시 — 같은 연결, 별도 요청 없음. 첫 View 는 Step 이 없는 열린 Interview 다.
    let prepared = agent_call(&mut mcp, "gil_monitor_prepare", serde_json::json!({ "project_root": root }));
    assert_ne!(prepared["isError"], true, "{prepared}");
    let scope = prepared["structuredContent"]["scope_id"].clone();
    assert_eq!(prepared["structuredContent"]["label"], "project");
    let shown = agent_call(&mut mcp, "show_gil_monitor", serde_json::json!({ "scope_id": scope }));
    assert_ne!(shown["isError"], true, "{shown}");
    assert!(shown["content"][0]["text"].as_str().unwrap().contains("fullscreen을 한 번 자동 요청"), "{shown}");
    let view = &shown["structuredContent"]["view"];
    assert_eq!(view["schema_version"], 1);
    assert_eq!(view["current"]["cycle_ref"], "cycle:C1");
    assert!(view["current"]["step_ref"].is_null());
    assert_eq!(view["timeline"].as_array().unwrap().len(), 1);
    assert_eq!(view["timeline"][0]["kind"], "interview");
    assert_eq!(view["timeline"][0]["state"], "open");
    assert_eq!(view["timeline"][0]["steps"], serde_json::json!([]));
    assert_eq!(shown["structuredContent"]["scope_id"], scope);
    assert!(!shown.to_string().contains(at.to_str().unwrap()), "App 응답에 경로가 새어 나왔다");
    let first_hint = shown["structuredContent"]["revision"].as_str().unwrap().to_owned();
    let watching = shown["structuredContent"]["watching"] == true;
    assert_eq!(binding_files(&settings), 1);

    // 3. 두 번째 start — 이미 걷고 있다. 거절이며 기록은 byte 로 같다.
    let again = agent_call(&mut mcp, "gil_start", serde_json::json!({ "project_root": root }));
    assert_eq!(again["isError"], true, "{again}");
    assert_eq!(again["structuredContent"]["ok"], false);
    let problem = again["structuredContent"]["problem"].as_str().unwrap();
    assert!(problem.contains("여기서 새로 시작할 수 없다.") && problem.contains("이미 걷고 있다"), "{problem}");
    assert!(problem.contains("gil status"), "복구 안내가 없다: {problem}");
    assert_eq!(std::fs::read(root.join(gil::STATE_PATH)).unwrap(), state, "거절된 start 가 기록을 바꿨다");

    // 4. 화면 재시도 — prepare/show 를 다시 불러도 같은 scope, binding 하나, 기록 불변.
    let retried = agent_call(&mut mcp, "gil_monitor_prepare", serde_json::json!({ "project_root": root }));
    assert_eq!(retried["structuredContent"]["scope_id"], scope, "재시도가 새 scope 를 만들었다");
    let reshown = agent_call(&mut mcp, "show_gil_monitor", serde_json::json!({ "scope_id": scope }));
    assert_ne!(reshown["isError"], true, "{reshown}");
    assert_eq!(reshown["structuredContent"]["view"]["timeline"], view["timeline"]);
    assert_eq!(binding_files(&settings), 1, "재시도가 binding 을 늘렸다");
    assert_eq!(std::fs::read(root.join(gil::STATE_PATH)).unwrap(), state);

    // 5. App 조회의 typed 거절(알 수 없는 scope)은 Project 를 만지지 않는다 — 그 뒤 start 도 여전히
    //    거절이다. Host 가 카드를 그리지 못한 경우를 흉내 내는 것이 아니다.
    let lost = agent_call(&mut mcp, "show_gil_monitor", serde_json::json!({ "scope_id": format!("project:{}", "0".repeat(64)) }));
    assert_eq!(lost["isError"], true);
    assert_eq!(lost["structuredContent"]["code"], "reconnect_required");
    assert!(lost["structuredContent"].get("ok").is_none(), "Monitor 거절이 Agent 동작의 모양을 흉내 냈다");
    assert_eq!(std::fs::read(root.join(gil::STATE_PATH)).unwrap(), state);
    let still = agent_call(&mut mcp, "gil_start", serde_json::json!({ "project_root": root }));
    assert_eq!(still["structuredContent"]["ok"], false);
    assert_eq!(std::fs::read(root.join(gil::STATE_PATH)).unwrap(), state);

    // 6. 시작 실패는 화면 이전에 끝난다 — 없는 자리는 start 도 prepare 도 거절이고 binding 은 그대로다.
    let nowhere = at.join("missing");
    let failed = agent_call(&mut mcp, "gil_start", serde_json::json!({ "project_root": nowhere }));
    assert_eq!(failed["structuredContent"]["ok"], false);
    assert_eq!(failed["structuredContent"]["problem"], "Project 자리를 열 수 없다");
    assert!(!nowhere.exists(), "실패한 start 가 폴더를 만들었다");
    let unprepared = agent_call(&mut mcp, "gil_monitor_prepare", serde_json::json!({ "project_root": nowhere }));
    assert_eq!(unprepared["isError"], true);
    assert_eq!(unprepared["structuredContent"]["code"], "unreadable");
    assert_eq!(binding_files(&settings), 1);

    // 7. 첫 Interview Step — Skill 의 셋째 걸음. 렌더 도구를 다시 부르지 않아도 View 가 바뀐다.
    let opened = agent_call(&mut mcp, "gil_open", serde_json::json!({ "project_root": root, "kind": "question",
        "contract": "objective: 사용자의 목표를 확인한다\nnext_action: 무엇을 만들고 싶은지 묻는다\ndone_when: 사용자의 원문 응답을 얻는다\n" }));
    assert_eq!(opened["structuredContent"]["ok"], true, "{opened}");
    let updated = agent_call(&mut mcp, "gil_monitor_read", serde_json::json!({ "scope_id": scope }));
    assert_ne!(updated["isError"], true, "{updated}");
    let now = &updated["structuredContent"]["view"];
    assert_eq!(now["current"]["step_ref"], "step:C1/S1");
    assert_eq!(now["timeline"][0]["steps"][0]["step_ref"], "step:C1/S1");
    assert_eq!(now["timeline"][0]["steps"][0]["state"], "open");
    assert_eq!(now["timeline"][0]["steps"][0]["kind"], "question");
    assert_eq!(now["current_will"]["objective"], "사용자의 목표를 확인한다");

    // 8. 변화 hint — watcher 가 서 있으면 첫 Step 의 기록이 유한한 시간 안에 새 revision 이 된다.
    //    OS 사건이 막힌 환경이면 hint 없이도 위 완전 재조회가 이미 새 사실을 읽었다(§8.3).
    //    어느 분기를 밟았는지는 `--nocapture` 로 보이게 적는다.
    if watching {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut hint = first_hint.clone();
        while hint == first_hint && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(200));
            let polled = agent_call(&mut mcp, "gil_monitor_poll", serde_json::json!({ "scope_id": scope }));
            hint = polled["structuredContent"]["revision"].as_str().unwrap().to_owned();
        }
        assert_ne!(hint, first_hint, "첫 Step 을 열었는데 30초 안에 변화 hint 가 오지 않았다");
        eprintln!("watcher branch: live — 실제 OS 감시가 첫 Step 의 hint 를 냈다");
    } else {
        eprintln!("watcher branch: unavailable — hint 없이 완전 재조회로만 갱신을 확인했다");
    }

    assert!(mcp.eof().0.success());
    std::fs::remove_dir_all(at).unwrap();
}
