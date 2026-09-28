//! One Rust MCP process: ten shared domain actions, native fallback, and the MCP App.
//! stdout belongs exclusively to rmcp; all blocking Core/OS operations use workers.
use rmcp::{ServerHandler, ServiceExt, ErrorData, RoleServer};
use rmcp::model::*;
use rmcp::service::RequestContext;
use std::sync::{Arc, Mutex};
use serde_json::Value;
mod monitor;
mod bindings;
mod actions;
mod native;
mod surface;
use monitor::Monitor;

#[derive(Debug, Clone)]
pub struct GilServer {
    monitor: Arc<Monitor>,
    native_gate: Arc<Mutex<()>>,
}
impl Default for GilServer { fn default() -> Self { Self::new() } }
impl GilServer {
    pub fn new() -> Self {
        Self { monitor: Arc::new(Monitor::default()), native_gate: Arc::new(Mutex::new(())) }
    }
    pub fn status_said(root: &str) -> Result<String, String> {
        crate::command::action(&["status".into()], Some(std::path::Path::new(root)), "")
    }
    async fn dispatch(&self, name: String, input: Value) -> CallToolResult {
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
        let mut got = monitor::run(move || {
            let scope = input["scope_id"].as_str().unwrap_or_default();
            match name.as_str() {
                "gil_monitor_prepare" => monitor.prepare(input["project_root"].as_str().unwrap_or_default()),
                "gil_monitor_read" | "show_gil_monitor" => monitor.view(scope),
                "gil_monitor_detail" => monitor.detail(scope, input["step_ref"].as_str().unwrap_or_default()),
                "gil_monitor_poll" => monitor.poll(scope),
                _ => Err(monitor::Refusal::new("unavailable", "알 수 없는 Monitor 동작")),
            }
        }).await;
        if show && got.is_error != Some(true) {
            got.content = vec![ContentBlock::text("GIL Monitor 데이터를 준비했다. App은 가로보기로 시작하고 지원되는 Host에 fullscreen을 한 번 자동 요청한다. 그대로라면 모니터 펼치기를 누른다. 이 응답만으로 화면 표시나 지속 표시 성공을 판정하지 않는다.")];
        }
        got
    }
}
impl ServerHandler for GilServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().enable_resources().build())
            .with_server_info(Implementation::new("gil", env!("CARGO_PKG_VERSION")))
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
        if !surface::valid_input(&tool, &input) { return Err(ErrorData::invalid_params("Invalid GIL tool arguments", None)); }
        Ok(self.dispatch(request.name.into_owned(), input).await.into())
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
        let server = GilServer::new();
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
        match service.waiting().await {
            Ok(_) => Ok(()),
            Err(err) => Err(format!("MCP 연결이 끊겼다: {err}")),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 기존 status 와 명시적으로 추가한 read-only Monitor 문만 있다.
    #[test]
    fn the_complete_seventeen_tool_surface_is_registered() {
        let tools = surface::tools();
        let mut names: Vec<&str> = tools.iter().map(|one| one.name.as_ref()).collect();
        names.sort();
        assert_eq!(names, vec!["gil_close", "gil_companion_status", "gil_context", "gil_cycle", "gil_help", "gil_monitor_detail", "gil_monitor_poll", "gil_monitor_prepare", "gil_monitor_read", "gil_open", "gil_restore", "gil_revisit", "gil_start", "gil_status", "gil_story", "show_gil_companion", "show_gil_monitor"]);
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
