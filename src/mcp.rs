//! One Rust MCP process: ten shared domain actions, native fallback, and the MCP App.
//! stdout belongs exclusively to rmcp; all blocking Core/OS operations use workers.
use rmcp::{ServerHandler, ServiceExt, ErrorData, RoleServer};
use rmcp::model::*;
use rmcp::service::RequestContext;
use std::sync::{Arc, Mutex};
use serde_json::Value;
mod monitor;
mod presence;
mod update;
mod bindings;
mod actions;
mod hangul;
mod native;
mod surface;
mod visuals;
mod guidance;
#[cfg(feature = "product-safety")]
mod recovery;
#[cfg(feature = "approved-plan")]
mod plans;
#[cfg(feature = "visual-authoring")]
mod authoring;
#[cfg(feature = "visual-authoring")]
mod migration;
#[cfg(all(feature = "visual-authoring", windows))]
mod workspace;
use monitor::Monitor;

#[derive(Debug, Clone)]
pub struct GilServer {
    #[cfg(feature = "product-safety")]
    recovery_grants: Arc<recovery::Grants>,
    monitor: Arc<Monitor>,
    #[cfg(feature = "approved-plan")]
    plan_grants: Arc<plans::Grants>,
    native_gate: Arc<Mutex<()>>,
    companion_session: Arc<Mutex<crate::companion_ipc::McpSession>>,
}
impl Default for GilServer { fn default() -> Self { Self::new() } }
impl GilServer {
    pub fn new() -> Self {
        Self::with_companion_endpoint(None)
    }
    /// Deployment/test adapter, not an MCP tool parameter. None discovers an installed cohort;
    /// existing Plugin installs with no cohort receipt remain opted out.
    pub fn with_companion_endpoint(endpoint: Option<std::path::PathBuf>) -> Self {
        Self { #[cfg(feature = "product-safety")] recovery_grants: Arc::new(recovery::Grants::default()),
            #[cfg(feature = "approved-plan")] plan_grants: Arc::new(plans::Grants::default()),
            monitor: Arc::new(Monitor::default()), native_gate: Arc::new(Mutex::new(())),
            companion_session: Arc::new(Mutex::new(crate::companion_ipc::McpSession::new(endpoint))) }
    }
    async fn dispatch_with_session(&self, name: String, mut input: Value) -> CallToolResult {
        // These operations actually open/create a Project on success. Help topics, listing,
        // initialization, workspace planning and native-window requests do not qualify.
        let project_use = input.get("project_root").and_then(Value::as_str).is_some()
            && matches!(name.as_str(), "gil_start" | "gil_status" | "gil_context" | "gil_story"
                | "gil_open" | "gil_close" | "gil_restore" | "gil_revisit" | "gil_cycle"
                | "gil_monitor_prepare" | "gil_report_visuals");
        // Use the same resolved root for the action, typed authoring and Monitor preparation.
        // Workspace creation/migration targets are intentionally outside this fallback.
        let normalized_note = if project_use || actions::NAMES.contains(&name.as_str()) || name == "gil_monitor_status" {
            input.get("project_root").and_then(Value::as_str)
                .filter(|root| std::path::Path::new(root).is_absolute())
                .and_then(|root| actions::resolve_root(root).ok())
        } else { None };
        if let Some((root, _)) = &normalized_note { input["project_root"] = serde_json::json!(root); }
        let mut result = self.dispatch(name.clone(), input.clone()).await;
        let succeeded = result.is_error != Some(true)
            && result.structured_content.as_ref().is_none_or(|v| v["ok"] != false);
        if name == "gil_start" && succeeded
            && result.structured_content.as_ref().is_some_and(|v| v["ok"] == true) {
            // Creation is already committed. A display preparation failure must never turn
            // that success into a failed start or encourage a duplicate creation.
            let monitor = self.monitor.clone();
            let root = input["project_root"].as_str().unwrap_or_default().to_owned();
            let prepared = monitor::run(move || monitor.prepare(&root)).await;
            if let Some(scope) = prepared.structured_content.as_ref()
                .filter(|_| prepared.is_error != Some(true))
                .and_then(|v| v["scope_id"].as_str()) {
                let call = format!("show_gil_monitor({})", serde_json::json!({"scope_id": scope}));
                // Some Hosts expose structuredContent without the surrounding text blocks.
                // Keep the CLI receipt intact, but carry the same prepared handoff there too.
                if let Some(fields) = result.structured_content.as_mut().and_then(serde_json::Value::as_object_mut) {
                    fields.insert("next_call".into(), serde_json::json!({
                        "name": "show_gil_monitor", "arguments": {"scope_id": scope}
                    }));
                }
                // The handoff also closes the receipt, after the body's `실행: gil open` line, so the
                // last word of the start receipt is the one next call on every Host. `said` itself
                // stays byte-identical to the CLI receipt.
                result.content.push(ContentBlock::text(format!(
                    "Monitor: 다음 호출은 하나다 — {call}. 위 ‘실행’의 gil open은 그 다음이다. Codex와 Claude Desktop의 Claude Code 탭에서 같다.")));
                guidance::prepend(&mut result, format!(
                    "GIL 시작은 성공했다. Monitor 준비도 완료했다. 다음 호출은 하나다: {call}. 이 호출은 Host 종류와 무관하게 gil_open보다 먼저이며, 인자는 그대로 복사한다. 그 뒤 같은 project_root로 첫 Interview의 gil_open을 호출한다. gil_start나 gil_monitor_prepare를 반복하지 않는다. 화면이 보인다고 아직 단정하지 않는다."));
            } else {
                let detail = prepared.structured_content.as_ref()
                    .and_then(|v| v["said"].as_str()).unwrap_or("Monitor 준비 결과를 확인하지 못했다");
                guidance::prepend(&mut result, format!(
                    "GIL 시작은 성공했으나 Monitor 준비는 실패했다: {detail}. gil_start를 반복하지 않는다. 표시 실패를 사용자에게 알리고 기록된 Interview를 이어간다. Monitor 재시도는 일시적 실패일 때만 gil_monitor_prepare로 한 번 한다."));
            }
        } else if name == "gil_monitor_prepare" && result.is_error != Some(true) {
            if let Some(scope) = result.structured_content.as_ref().and_then(|v| v["scope_id"].as_str()) {
                let hint = format!("다음 호출: show_gil_monitor({}). 준비만으로 화면이 표시되지는 않는다.", serde_json::json!({"scope_id": scope}));
                guidance::prepend(&mut result, hint);
            }
        }
        guidance::after(&name, &mut result);
        if project_use && succeeded && name != "gil_start" && name != "gil_monitor_prepare" {
            let monitor = self.monitor.clone();
            let root = input["project_root"].as_str().unwrap_or_default().to_owned();
            let health = monitor::run(move || {
                let status = monitor.presence_status(&root)?;
                if status["should_open"] == true { monitor.prepare(&root)?; }
                Ok(status)
            }).await;
            if health.is_error != Some(true) {
                if let Some(status) = health.structured_content.as_ref() {
                    if status["display_mode"] == "inline" {
                        guidance::prepend(&mut result, "Monitor는 inline 미리보기로만 확인됐다. fullscreen 표시 완료라고 보고하지 않는다. Host가 자동 확대를 받아들이지 않으면 위젯 머리의 ‘다른 패널에 표시’ 또는 Host의 확대 버튼을 안내한다. 표시 문제와 GIL 진행은 별개다; 아래 다음 GIL 호출을 계속한다.");
                    } else if status["state"] == "unconfirmed" {
                        guidance::prepend(&mut result, "MCP App Monitor가 두 번의 표시 요청에도 응답하지 않았다. 자동 재열기를 멈추고 표시 문제를 알린다. Companion을 자동으로 열지 않는다.");
                    } else if status["should_open"] == true {
                        // A lost/Host-rejected start receipt can be recovered through context,
                        // including on Hosts that expose only structuredContent to the model.
                        if name == "gil_context" {
                            if let Some(fields) = result.structured_content.as_mut().and_then(Value::as_object_mut) {
                                fields.insert("next_call".into(), serde_json::json!({
                                    "name": "show_gil_monitor", "arguments": {"scope_id": status["scope_id"]}
                                }));
                            }
                        }
                        guidance::prepend(&mut result, format!(
                            "MCP App Monitor가 표시 중이라는 신호가 없다 ({}). 작업을 계속하기 전에 show_gil_monitor({})를 한 번 호출한다. Companion으로 대체하지 않는다. 호출 성공만으로 표시됐다고 말하지 않는다.",
                            status["state"], serde_json::json!({"scope_id":status["scope_id"]})));
                    }
                }
            } else {
                guidance::prepend(&mut result, "MCP App Monitor 표시 상태를 확인하지 못했다. 표시 성공으로 보고하지 않는다. 기록된 작업 결과는 유지한다.");
            }
        }
        if project_use && succeeded {
            let session = self.companion_session.clone();
            let _ = tokio::task::spawn_blocking(move || {
                if let Ok(mut session) = session.lock() {
                    if let Err(err) = session.used() {
                        // Auxiliary icon failure must not replay or change a successful action.
                        eprintln!("GIL Companion session unavailable ({:?}); Project result unchanged", err.kind());
                    }
                }
            }).await;
        }
        if succeeded {
            if let Some((_, Some(note))) = normalized_note { guidance::prepend(&mut result, note); }
        }
        result
    }
    pub fn status_said(root: &str) -> Result<String, String> {
        crate::command::action(&["status".into()], Some(std::path::Path::new(root)), "")
    }
    async fn dispatch(&self, name: String, input: Value) -> CallToolResult {
        if name == "gil_await_user" {
            return tokio::task::spawn_blocking(move || match crate::write_guard::automatic::await_user(
                input["project_root"].as_str().unwrap_or_default(), input["question"].as_str().unwrap_or_default()) {
                Ok(wait) => {
                    let mut result = CallToolResult::success(vec![ContentBlock::text(format!("사용자 입력 대기: {}. 응답 마지막에 이 질문을 그대로 적는다. 사용자 답변 후 gil_context로 같은 단계를 이어간다. 완료·승인 기록은 바뀌지 않았다.", wait["question"].as_str().unwrap_or_default()))]);
                    result.structured_content = Some(serde_json::json!({"ok":true,"handoff":wait})); result
                }
                Err(error) => actions::refusal(error),
            }).await.unwrap_or_else(|_| actions::refusal("사용자 대기 상태를 확인하지 못했다. gil_context로 현재 단계를 확인한다."));
        }
        #[cfg(feature = "product-safety")]
        if name.starts_with("gil_revisit_") {
            let monitor = self.monitor.clone(); let grants = self.recovery_grants.clone();
            return tokio::task::spawn_blocking(move || recovery::call(&monitor, &grants, &name, input)).await
                .unwrap_or_else(|_| actions::refusal("복원 결과가 불확실하다. Monitor에서 다시 읽는다. 기존 보존본을 삭제하지 않는다."));
        }
        #[cfg(feature = "approved-plan")]
        if name.starts_with("gil_plan_") {
            let monitor = self.monitor.clone();
            let grants = self.plan_grants.clone();
            return tokio::task::spawn_blocking(move || plans::call(&monitor, &grants, &name, input)).await
                .unwrap_or_else(|_| actions::refusal("승인 기록 결과가 불확실하다. gil_plan_read로 확인한다."));
        }
        #[cfg(feature = "visual-authoring")]
        if name.starts_with("gil_migration_") {
            return tokio::task::spawn_blocking(move || migration::call(&name, input)).await
                .unwrap_or_else(|_| actions::refusal("이관 결과가 불확실하다. 기록과 백업을 보존하고 gil_migration_status만 읽는다. 자동 재시도하지 않는다."));
        }
        #[cfg(all(feature = "visual-authoring", windows))]
        if name == workspace::NAME {
            return tokio::task::spawn_blocking(move || workspace::call(input)).await
                .unwrap_or_else(|_| actions::refusal("작업 폴더 준비 결과가 불확실하다. 폴더 존재 여부를 확인하고 반복하지 않는다."));
        }
        #[cfg(feature = "visual-authoring")]
        if name == "gil_report_visuals" || (name == "gil_close" && input.get("visuals").is_some()) {
            return tokio::task::spawn_blocking(move || authoring::call(&name, input)).await
                .unwrap_or_else(|_| actions::refusal("확정 여부를 알 수 없다. gil_report_visuals로 확인하고 close를 반복하지 않는다."));
        }
        if actions::NAMES.contains(&name.as_str()) {
            return tokio::task::spawn_blocking(move || actions::call(&name, &input)).await
                .unwrap_or_else(|_| actions::refusal("GIL 동작을 끝내지 못했다. 상태를 확인한 뒤 이어간다."));
        }
        if matches!(name.as_str(), "gil_companion_status" | "show_gil_companion") {
            let gate = self.native_gate.clone();
            return tokio::task::spawn_blocking(move || {
                let Ok(_guard) = gate.lock() else { return actions::refusal("Monitor 연결을 확인하지 못했다."); };
                native::call(name == "show_gil_companion")
            }).await.unwrap_or_else(|_| actions::refusal("Monitor 연결을 확인하지 못했다."));
        }
        let monitor = self.monitor.clone();
        let show = name == "show_gil_monitor";
        let visual = matches!(name.as_str(), "gil_monitor_report_visuals" | "gil_monitor_report_image");
        let mut got = monitor::run(move || {
            let scope = input["scope_id"].as_str().unwrap_or_default();
            match name.as_str() {
                "gil_monitor_report_visuals" => visuals::call(&monitor, &input, false),
                "gil_monitor_report_image" => visuals::call(&monitor, &input, true),
                "gil_monitor_prepare" => monitor.prepare(input["project_root"].as_str().unwrap_or_default()),
                "gil_monitor_read" => monitor.view(scope),
                "show_gil_monitor" => monitor.show(scope),
                "gil_monitor_status" => monitor.presence_status(input["project_root"].as_str().unwrap_or_default()),
                "gil_monitor_detail" => monitor.detail(scope, input["step_ref"].as_str().unwrap_or_default()),
                "gil_monitor_poll" => monitor.app_poll(scope, &input),
                _ => Err(monitor::Refusal::new("unavailable", "알 수 없는 Monitor 동작")),
            }
        }).await;
        if visual { got = visuals::bounded(got); }
        if show && got.is_error != Some(true) {
            got.content = vec![ContentBlock::text("GIL Monitor 데이터를 준비했다. App은 가로보기로 시작하고 fullscreen을 광고한 Host에는 연결 직후와 Monitor 준비 뒤 스스로 fullscreen을 요청하며, 그래도 inline이면 사용자의 첫 클릭에 다시 요청한다. 그대로라면 사용자가 Monitor를 한 번 클릭하거나 모니터 펼치기를 누른다. 이 응답만으로 화면 표시나 지속 표시 성공을 판정하지 않는다.")];
        }
        got
    }
}
impl ServerHandler for GilServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().enable_resources().build())
            .with_server_info(Implementation::new("gil", env!("CARGO_PKG_VERSION")))
            .with_instructions(guidance::INSTRUCTIONS)
    }
    fn get_tool(&self, name: &str) -> Option<Tool> {
        surface::tools().iter().find(|t| t.name == name).cloned()
    }
    async fn list_tools(&self, _: Option<PaginatedRequestParams>, _: RequestContext<RoleServer>) -> Result<ListToolsResult, ErrorData> {
        let mut result = ListToolsResult::default();
        result.tools = surface::tools().to_vec();
        Ok(result)
    }
    async fn call_tool(&self, request: CallToolRequestParams, _: RequestContext<RoleServer>) -> Result<CallToolResponse, ErrorData> {
        let tool = self.get_tool(&request.name).ok_or_else(|| ErrorData::invalid_params("Unknown GIL tool", None))?;
        let input = Value::Object(request.arguments.unwrap_or_default());
        if !surface::valid_input(&tool, &input) { return Err(ErrorData::invalid_params(surface::rejection(&tool, &input), None)); }
        Ok(self.dispatch_with_session(request.name.into_owned(), input).await.into())
    }
    async fn list_resources(&self, _: Option<PaginatedRequestParams>, _: RequestContext<RoleServer>) -> Result<ListResourcesResult, ErrorData> {
        let mut result = ListResourcesResult::default();
        result.resources = vec![surface::resource()];
        Ok(result)
    }
    async fn read_resource(&self, request: ReadResourceRequestParams, _: RequestContext<RoleServer>) -> Result<ReadResourceResponse, ErrorData> {
        if request.uri != surface::uri() { return Err(ErrorData::resource_not_found("Unknown GIL resource", None)); }
        Ok(ReadResourceResult::new(vec![surface::contents()]).into())
    }
}

/// stdin/stdout 에 서서 EOF 까지 답한다.
///
/// 돌아오는 것은 **사람에게 적을 말이 아니다** — 이 함수가 끝나는 자리는 프로세스의 끝이고,
/// stdout 은 이미 JSON-RPC 가 다 썼다. 그래서 오류만 문자열로 돌려주고, 부르는 쪽이 그것을
/// stderr 로 적는다.
pub fn serve_stdio() -> Result<(), String> {
    // stdin/stdout 은 blocking pool 위에서 돈다 — IO driver(=`net`)는 필요 없다. 다만
    // rmcp 가 요청 수명에 타이머를 쓰므로 시계는 켠다. 끄면 첫 요청에서 패닉한다.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .map_err(|err| format!("MCP 실행기를 세우지 못했다: {err}"))?;

    runtime.block_on(async {
        let server = GilServer::with_companion_endpoint(std::env::var_os(crate::companion_ipc::ENV_ENDPOINT).map(Into::into));
        let companion_session = server.companion_session.clone();
        server.monitor.reap();
        let service = match server.serve(rmcp::transport::stdio()).await {
            Ok(service) => service,
            // **손님이 말없이 떠난 것은 고장이 아니다.** Host 가 server 를 세워 두고
            // 초기화 없이 닫는 일은 흔하다 — 조용히 0 으로 끝낸다.
            Err(rmcp::service::ServerInitializeError::ConnectionClosed(_)) => return Ok(()),
            Err(err) => return Err(format!("MCP 연결을 세우지 못했다: {err}")),
        };
        // stdin 이 닫히면 여기서 깨끗이 돌아온다. `Closed`·`Cancelled` 는 **끝난 것**이지
        // 실패가 아니다. 매달려 기다리지 않는다.
        let result = match service.waiting().await {
            Ok(_) => Ok(()),
            Err(err) => Err(format!("MCP 연결이 끊겼다: {err}")),
        };
        // rmcp handler clones may outlive waiting(); the transport owns this disconnect.
        if let Ok(mut session) = companion_session.lock() { session.close(); }
        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 기본 표(`tools.json`)는 20개다. feature 별 등록은 **그 feature 가 켜졌을 때만** 더해진다.
    /// 기본 표와 feature 표를 섞지 않는다 — 각 집합을 따로 고정한다.
    const DEFAULT_SURFACE: [&str; 20] = ["gil_close", "gil_companion_status", "gil_context", "gil_cycle", "gil_help", "gil_monitor_detail", "gil_monitor_poll", "gil_monitor_prepare", "gil_monitor_read", "gil_monitor_report_image", "gil_monitor_report_visuals", "gil_monitor_status", "gil_open", "gil_restore", "gil_revisit", "gil_start", "gil_status", "gil_story", "show_gil_companion", "show_gil_monitor"];
    const VISUAL_SURFACE: [&str; 5] = ["gil_report_visuals", "gil_migration_check", "gil_migration_prepare", "gil_migration_apply", "gil_migration_status"];
    /// M6-D 승인 계획 도구 — 모델이 부르는 셋과 App 전용 결정 둘(src/mcp/plans.rs).
    const PLAN_MODEL_SURFACE: [&str; 3] = ["gil_plan_propose", "gil_plan_read", "gil_plan_request_interview"];
    const PLAN_APP_SURFACE: [&str; 2] = ["gil_plan_review", "gil_plan_decide"];

    fn registered_names() -> Vec<String> {
        let mut names: Vec<String> = surface::tools().iter().map(|one| one.name.to_string()).collect();
        names.sort();
        names
    }

    fn visibility(name: &str) -> Vec<String> {
        let tool = surface::tools().iter().find(|t| t.name == name).unwrap_or_else(|| panic!("{name} 가 등록되지 않았다"));
        let value = serde_json::to_value(tool).expect("tool json");
        value["_meta"]["ui"]["visibility"].as_array().unwrap_or_else(|| panic!("{name} 에 visibility 가 없다"))
            .iter().map(|v| v.as_str().expect("visibility 문자열").to_owned()).collect()
    }

    /// 등록된 도구 이름은 feature 별 기대 집합과 **정확히** 같다. 이름 하나가 빠지거나 남으면 실패한다.
    #[test]
    fn the_registered_tool_names_match_the_enabled_features_exactly() {
        let mut expected: Vec<String> = DEFAULT_SURFACE.iter().map(|s| s.to_string()).collect();
        if cfg!(feature = "visual-authoring") {
            expected.extend(VISUAL_SURFACE.iter().map(|s| s.to_string()));
        }
        if cfg!(all(feature = "visual-authoring", windows)) {
            expected.push("gil_prepare_workspace".to_string());
        }
        if cfg!(feature = "approved-plan") {
            expected.extend(PLAN_MODEL_SURFACE.iter().chain(PLAN_APP_SURFACE.iter()).map(|s| s.to_string()));
        }
        if cfg!(feature = "product-safety") { expected.extend(["gil_revisit_apply".into(), "gil_revisit_review".into()]); }
        expected.push("gil_await_user".to_string());
        expected.sort();
        assert_eq!(registered_names(), expected);
    }

    /// 기본 표 파일 자체에는 승인 계획 도구가 없다 — 승인 계획은 feature 가 더하는 것이지 기본 계약이 아니다.
    #[test]
    fn the_checked_in_default_catalog_never_lists_plan_or_visual_tools() {
        let catalog: Vec<serde_json::Value> = serde_json::from_str(include_str!("mcp/tools.json")).expect("tools.json");
        let mut names: Vec<&str> = catalog.iter().map(|t| t["name"].as_str().expect("name")).collect();
        names.sort();
        assert_eq!(names, DEFAULT_SURFACE);
        for name in PLAN_MODEL_SURFACE.iter().chain(PLAN_APP_SURFACE.iter()).chain(VISUAL_SURFACE.iter()) {
            assert!(!names.contains(name), "{name} 이 기본 표에 들어왔다");
        }
    }

    /// 승인 계획 도구는 approved-plan 이 켜질 때만 다섯 개가 등록되고, 그 가시성은 모델 셋 / App 둘이다.
    /// 기본·visual-authoring 만의 빌드에서는 하나도 없다.
    #[test]
    fn plan_tools_are_registered_only_in_the_approved_plan_profile_with_their_visibility() {
        let names = registered_names();
        let present: Vec<&String> = names.iter().filter(|n| n.starts_with("gil_plan_")).collect();
        if cfg!(feature = "approved-plan") {
            assert_eq!(present.len(), 5, "{present:?}");
            for name in PLAN_MODEL_SURFACE {
                assert_eq!(visibility(name), ["model"], "{name} 는 모델만 부른다");
            }
            for name in PLAN_APP_SURFACE {
                assert_eq!(visibility(name), ["app"], "{name} 는 App 전용 결정 도구다");
            }
        } else {
            assert!(present.is_empty(), "approved-plan 없이 승인 계획 도구가 등록됐다: {present:?}");
        }
    }

    /// 기본 표의 가시성은 체크인된 파일 그대로다 — 모델 전용·App 전용·둘 다를 뒤섞지 않는다.
    #[test]
    fn default_surface_visibility_matches_the_checked_in_catalog() {
        let catalog: Vec<serde_json::Value> = serde_json::from_str(include_str!("mcp/tools.json")).expect("tools.json");
        for tool in &catalog {
            let name = tool["name"].as_str().expect("name");
            let declared: Vec<String> = tool["_meta"]["ui"]["visibility"].as_array().expect("visibility")
                .iter().map(|v| v.as_str().unwrap().to_owned()).collect();
            assert_eq!(visibility(name), declared, "{name}");
        }
        for name in ["gil_monitor_read", "gil_monitor_detail", "gil_monitor_poll", "gil_monitor_report_visuals", "gil_monitor_report_image"] {
            assert_eq!(visibility(name), ["app"], "{name}");
        }
        for name in ["gil_start", "gil_open", "gil_close", "gil_context", "gil_status"] {
            assert_eq!(visibility(name), ["model"], "{name}");
        }
    }

    #[test]
    fn the_tool_asks_for_a_project_root() {
        let tools = surface::tools();
        let status = tools.iter().find(|t| t.name == "gil_status").expect("status");
        let schema = serde_json::to_string(&status.input_schema).expect("schema");
        assert!(schema.contains("project_root"), "{schema}");
    }

    /// 판 번호를 두 자리에 적었으므로 **두 자리가 같다는 것**을 시험이 지킨다.
    #[test]
    fn the_version_it_reports_is_the_crate_version() {
        let info = GilServer::new().get_info();
        assert_eq!(info.server_info.name, "gil");
        assert_eq!(
            info.server_info.version,
            env!("CARGO_PKG_VERSION"),
            "Cargo.toml 의 판과 MCP 가 말하는 판이 갈렸다"
        );
    }

    #[test]
    fn a_relative_project_root_is_refused() {
        let said = GilServer::status_said("some/where").unwrap_err();
        assert_eq!(said, crate::RootError::NotAbsolute.to_string());
        assert!(!said.contains('/'), "거절문에 경로가 새어 나왔다");
    }

    #[test]
    fn a_place_without_a_project_is_refused() {
        let at = std::env::temp_dir().join(format!("gil-mcp-empty-{}", std::process::id()));
        std::fs::create_dir_all(&at).expect("자리");
        let said = GilServer::status_said(at.to_str().expect("utf-8")).unwrap_err();
        assert_eq!(said, crate::RootError::NoProjectHere.to_string());
        std::fs::remove_dir_all(&at).ok();
    }
}

/// Reuse the exact MCP path resolution (including Hangul normalization) in Host hooks.
pub(crate) fn resolve_guard_root(root: &str) -> Result<std::path::PathBuf, String> {
    actions::resolve_root(root).map(|(root, _)| std::path::PathBuf::from(root))
}
