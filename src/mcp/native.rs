//! The native fallback remains explicit; reading/showing the MCP App never calls this.
use crate::capability::{self, AgentCoreState, Companion, Host, HostSurface, MonitorIntent, Ports, Policy};
use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::json;

struct UnverifiedHost;
impl Host for UnverifiedHost {
    fn surface(&self) -> HostSurface { HostSurface::Unverified }
    fn open(&self, _: MonitorIntent) -> bool { false }
}
struct Clock;
impl capability::Clock for Clock {
    fn sleep(&self, ms: u64) { std::thread::sleep(std::time::Duration::from_millis(ms)); }
}

pub fn call(show: bool) -> CallToolResult {
    let Some(platform) = crate::launcher::this_machine() else {
        return super::actions::refusal("이 기계의 Companion 연결은 아직 지원하지 않는다");
    };
    let companion = crate::launcher::PlatformCompanion::new(platform);
    let agent = AgentCoreState::of_this_process();
    let (data, said, is_error) = if show {
        let settled = capability::open_monitor(&Ports { agent, host: &UnverifiedHost, companion: &companion,
            clock: &Clock, policy: Policy::default() });
        let said = crate::say::outcome(settled.outcome);
        (json!({"result": settled.outcome, "agent_surface": settled.installation.agent_surface,
            "monitor_surface": settled.installation.monitor_surface, "said": said}), said, settled.outcome.is_error())
    } else {
        let seen = companion.state();
        let installation = capability::installation_of(agent, HostSurface::Unverified, Some(seen.state));
        let version = seen.descriptor.as_ref().map(|d| d.app_version.as_str());
        let said = crate::say::state(seen.state, version);
        (json!({"agent_surface": installation.agent_surface, "monitor_surface": installation.monitor_surface,
            "companion_state": seen.state, "agent_state": agent, "appVersion": version, "said": said}), said, false)
    };
    if crate::say::assert_no_leak(&said).is_err() {
        return CallToolResult::error(vec![ContentBlock::text("Monitor 응답을 안전하게 전달하지 못했다.")]);
    }
    let mut out = CallToolResult::success(vec![ContentBlock::text(said)]);
    out.structured_content = Some(data);
    // The original availability tool omitted isError on success.
    out.is_error = if show { Some(is_error) } else { None };
    out
}
