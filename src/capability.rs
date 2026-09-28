//! **설치 capability 의 판정과 조율** — 무엇이 실제로 가능한지 하나의 자리에서 정한다.
//!
//! `plugins/gil-companion-prototype/capability.mjs` 를 그대로 옮긴 것이다. 값의 이름, 표면 선택
//! 순서, 유한한 재시도, `why` 의 두 글자까지 같다 — 같아야 두 문(JS·Rust)이 같은 사실에 같은
//! 답을 하고, 이전이 끝난 날 JS 를 지워도 사람이 알아채지 못한다.
//!
//! 이 파일은 바깥 세계에 손대지 않는다. process 를 띄우지도, 파일을 읽지도, 시계를 보지도
//! 않는다. 그 셋은 전부 [`Ports`] 로 들어온다. 그래야 결정적인 시계와 가짜 launcher 로 시험할
//! 수 있고, 시험을 위해 production protocol 을 약하게 만들 필요가 없다.
//!
//! # 왜 한 자리인가
//!
//! 같은 판정을 두 군데에 적으면 한쪽이 낡는다. Host surface 의 유무와 Companion 의 네 상태는
//! 따로 물어볼 수 있지만 **"Monitor 를 열 수 있는가"는 하나의 답**이어야 한다.

use serde::{Deserialize, Serialize};

pub use crate::companion::InstallationState as CompanionState;

/// Agent 가 GIL 의 typed action 을 쓸 수 있는가.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSurface {
    Ready,
    Unavailable,
}

/// 사람이 지속형 Monitor 를 열 수 있는 표면. 선택 순서가 그대로 이 순서다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MonitorSurface {
    PersistentHost,
    NativeCompanion,
    Unavailable,
}

/// Agent Core probe 가 가르는 세 값. Companion 의 네 값과 **뜻이 다르다** — 그쪽은 "사람이 볼
/// 창이 떠 있는가", 이쪽은 "Agent 가 부를 Core 가 이 판과 맞는가".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentCoreState {
    Ready,
    OutdatedAgent,
    Unavailable,
}

impl AgentCoreState {
    /// **이 프로세스가 곧 Core 다.** JS 는 실린 binary 를 다시 띄워 물었지만, Rust MCP 안에서
    /// 그 물음의 답은 자기 자신의 descriptor 에 있다. 자기 실행 파일을 다시 spawn 하지 않는다.
    ///
    /// 판정은 `core.mjs` 와 같다 — 이름부터 다르면 낡은 판이 아니라 **다른 물건**(unavailable),
    /// 이름은 맞는데 범위가 어긋나면 `outdated_agent`.
    pub fn of(descriptor: &crate::agent::DescriptorV1) -> Self {
        if descriptor.product != crate::agent::PRODUCT {
            return Self::Unavailable;
        }
        let protocol_ok = descriptor.protocol.min <= crate::agent::PROTOCOL_VERSION
            && crate::agent::PROTOCOL_VERSION <= descriptor.protocol.max;
        let surface_ok = descriptor.action_surface.min <= crate::agent::ACTION_SURFACE
            && crate::agent::ACTION_SURFACE <= descriptor.action_surface.max;
        if protocol_ok && surface_ok {
            Self::Ready
        } else {
            Self::OutdatedAgent
        }
    }

    pub fn of_this_process() -> Self {
        Self::of(&crate::agent::DescriptorV1::current())
    }
}

/// Host 의 지속형 surface 를 **실제로 확인했는가**.
///
/// `Unverified` 는 "이 Host 가 PiP 를 지원하지 않는다"는 뜻이 **아니다**. 아직 정식 probe 로
/// 확인하지 않았다는 뜻이다. `Unsupported` 라는 값을 만들지 않는다 — 그렇게 적으면 아직 하지
/// 않은 probe 의 결과를 코드가 먼저 단정하게 된다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostSurface {
    Verified,
    Unverified,
}

/// coordinator 가 실제로 한 일. 문자열을 뜯어 뜻을 짐작하지 않도록 값으로 가른다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    OpenedPersistentHost,
    FocusedExistingCompanion,
    StartedAndOpenedCompanion,
    NeedsCompanionInstall,
    NeedsCompanionUpdate,
    MonitorUnavailable,
}

impl Outcome {
    pub const ALL: [Self; 6] = [
        Self::OpenedPersistentHost,
        Self::FocusedExistingCompanion,
        Self::StartedAndOpenedCompanion,
        Self::NeedsCompanionInstall,
        Self::NeedsCompanionUpdate,
        Self::MonitorUnavailable,
    ];

    /// 열지 못한 것을 열었다고 말하지 않는다 — MCP 응답의 `isError` 가 이것이다.
    pub fn is_error(self) -> bool {
        matches!(
            self,
            Self::NeedsCompanionInstall | Self::NeedsCompanionUpdate | Self::MonitorUnavailable
        )
    }
}

/// `monitor_unavailable` 이 왜 그런지. 두 값뿐이며 JS 와 같은 글자다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Why {
    LaunchFailed,
    NotReadyAfterLaunch,
}

/// 유한한 재시도. 무한 polling 도 background busy loop 도 만들지 않는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    pub probe_timeout_ms: u64,
    pub ready_attempts: u32,
    pub ready_gap_ms: u64,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            probe_timeout_ms: 2_000,
            ready_attempts: 30,
            ready_gap_ms: 100,
        }
    }
}

/// 설치 완료 조건의 read model (Distribution Model §2).
///
/// `monitor_surface` 는 **지금 창이 떠 있는가**가 아니라 **사람이 지속형 Monitor 를 열 수
/// 있는가**를 말한다. 그래서 호환판이 설치됐지만 꺼져 있는 `stopped` 도 `native_companion` 이다
/// — 열면 되기 때문이다. 반대로 `outdated` 는 설치는 됐어도 열어서는 안 되므로 표면이 없다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Installation {
    pub agent_surface: AgentSurface,
    pub monitor_surface: MonitorSurface,
}

pub fn installation_of(
    agent: AgentCoreState,
    host: HostSurface,
    companion: Option<CompanionState>,
) -> Installation {
    Installation {
        // **호출자가 넘긴 boolean 이 아니다.** Core 가 지금 답했을 때만 ready 다.
        agent_surface: match agent {
            AgentCoreState::Ready => AgentSurface::Ready,
            _ => AgentSurface::Unavailable,
        },
        monitor_surface: monitor_surface_of(host, companion),
    }
}

fn monitor_surface_of(host: HostSurface, companion: Option<CompanionState>) -> MonitorSurface {
    if host == HostSurface::Verified {
        return MonitorSurface::PersistentHost;
    }
    match companion {
        Some(CompanionState::Ready | CompanionState::Stopped) => MonitorSurface::NativeCompanion,
        _ => MonitorSurface::Unavailable,
    }
}

/// 설치가 완료됐는가 — 둘 다 있어야 한다. inline 과 text 는 여기에 세지 않는다.
pub fn is_complete(installation: Installation) -> bool {
    installation.agent_surface == AgentSurface::Ready
        && installation.monitor_surface != MonitorSurface::Unavailable
}

/// 사용자가 처음 말한 것. **메모리 안에서만** 산다.
///
/// Graph·Journey·Memory·Will·Project 어디에도 적지 않는다. 이 요청 하나가 끝나면 함께
/// 사라진다. `Serialize` 를 일부러 붙이지 않는다 — 적을 수 없게.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorIntent {
    _private: (),
}

impl MonitorIntent {
    pub fn open_monitor() -> Self {
        Self { _private: () }
    }
}

/// Companion 을 관측한 결과 — 상태와, 있다면 그 descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub state: CompanionState,
    pub descriptor: Option<crate::companion::DescriptorV1>,
}

/// coordinator 가 끝에 돌려주는 것.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settled {
    pub outcome: Outcome,
    pub installation: Installation,
    pub why: Option<Why>,
}

// ── 바깥 세계로 난 문 ────────────────────────────────────────────────

/// Host 의 지속형 surface.
pub trait Host {
    fn surface(&self) -> HostSurface;
    /// 확인된 지속형 Host surface 를 연다. `Unverified` 에서는 불리지 않는다.
    fn open(&self, intent: MonitorIntent) -> bool;
}

/// 네 상태와 세 동작. platform 이 이것을 [`crate::launcher::Platform`] 위에 짓는다.
pub trait Companion {
    fn state(&self) -> Seen;
    fn launch(&self, intent: MonitorIntent) -> bool;
    fn focus(&self, intent: MonitorIntent) -> bool;
    /// 실행 뒤 **새 challenge** 로 다시 묻는다. 한 번의 성공을 계속 재사용하지 않는다.
    fn probe_ready(&self, descriptor: &crate::companion::DescriptorV1) -> bool;
}

/// 결정적 시계를 끼울 자리.
pub trait Clock {
    fn sleep(&self, ms: u64);
}

pub struct Ports<'a> {
    pub agent: AgentCoreState,
    pub host: &'a dyn Host,
    pub companion: &'a dyn Companion,
    pub clock: &'a dyn Clock,
    pub policy: Policy,
}

/// "GIL Monitor 열기" 하나를 끝까지 조율한다.
///
/// `persistent_host → native_companion → unavailable` 순서를 그대로 밟는다. Companion 이
/// 꺼져 있으면 실행하고, **fresh challenge 로 다시 확인한 뒤**, 그제서야 원래 요청을 이어서
/// 수행한다. 실행 명령이 성공했다는 사실만으로 완료라고 답하지 않는다.
pub fn open_monitor(ports: &Ports<'_>) -> Settled {
    let intent = MonitorIntent::open_monitor();
    // Agent 와 Monitor 는 **서로 독립이다.** 여기서는 보고를 위해 물어보기만 한다.
    let agent = ports.agent;

    // ① 지속형 Host surface 를 **확인된 경우에만** 쓴다. 확인되지 않았으면 Companion 으로 간다.
    let host = ports.host.surface();
    if host == HostSurface::Verified {
        ports.host.open(intent);
        return settled(Outcome::OpenedPersistentHost, agent, host, None, None);
    }

    // ② Companion 의 네 상태. 경로가 있다는 사실만으로 열지 않는다.
    let seen = ports.companion.state();
    match seen.state {
        CompanionState::Missing => {
            return settled(Outcome::NeedsCompanionInstall, agent, host, Some(seen.state), None);
        }
        // `outdated` 를 `missing` 이나 `stopped` 로 뭉개지 않는다. 지금 판을 ready 처럼 열지도 않는다.
        CompanionState::Outdated => {
            return settled(Outcome::NeedsCompanionUpdate, agent, host, Some(seen.state), None);
        }
        CompanionState::Ready => {
            ports.companion.focus(intent);
            return settled(Outcome::FocusedExistingCompanion, agent, host, Some(seen.state), None);
        }
        CompanionState::Stopped => {}
    }

    // ③ `stopped` — 실행하고, 다시 확인하고, 원래 요청을 이어서 수행한다.
    if !ports.companion.launch(intent) {
        return settled(
            Outcome::MonitorUnavailable,
            agent,
            host,
            Some(CompanionState::Stopped),
            Some(Why::LaunchFailed),
        );
    }
    let descriptor = seen.descriptor.as_ref();
    if !until_ready(ports, descriptor) {
        // 실행 명령은 성공했지만 handshake 가 오지 않았다. 원래 요청을 성공으로 표시하지 않는다.
        return settled(
            Outcome::MonitorUnavailable,
            agent,
            host,
            Some(CompanionState::Stopped),
            Some(Why::NotReadyAfterLaunch),
        );
    }

    // 여기서 **원래 요청이 재개된다.** 사람이 같은 말을 두 번 하지 않는다.
    ports.companion.focus(intent);
    settled(
        Outcome::StartedAndOpenedCompanion,
        agent,
        host,
        Some(CompanionState::Ready),
        None,
    )
}

/// 유한하게 기다린다. 매번 **새 challenge** 로 묻는다.
fn until_ready(ports: &Ports<'_>, descriptor: Option<&crate::companion::DescriptorV1>) -> bool {
    let Some(descriptor) = descriptor else {
        return false;
    };
    for _ in 0..ports.policy.ready_attempts {
        if ports.companion.probe_ready(descriptor) {
            return true;
        }
        ports.clock.sleep(ports.policy.ready_gap_ms);
    }
    false
}

fn settled(
    outcome: Outcome,
    agent: AgentCoreState,
    host: HostSurface,
    companion: Option<CompanionState>,
    why: Option<Why>,
) -> Settled {
    Settled {
        outcome,
        installation: installation_of(agent, host, companion),
        why,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::companion::DescriptorV1;
    use std::cell::{Cell, RefCell};

    /// 무엇이 실제로 불렸는지 세는 가짜 문. 시계는 잠들지 않고 **센다**.
    /// `capability.test.mjs` 의 `fakePorts` 와 같은 모양이다.
    struct Fake {
        log: RefCell<Vec<&'static str>>,
        slept: RefCell<Vec<u64>>,
        host: HostSurface,
        state: CompanionState,
        ready_after: u32,
        can_launch: bool,
        probes: Cell<u32>,
    }

    impl Fake {
        fn new(state: CompanionState) -> Self {
            Self {
                log: RefCell::new(Vec::new()),
                slept: RefCell::new(Vec::new()),
                host: HostSurface::Unverified,
                state,
                ready_after: 0,
                can_launch: true,
                probes: Cell::new(0),
            }
        }
        fn host(mut self, host: HostSurface) -> Self {
            self.host = host;
            self
        }
        fn ready_after(mut self, n: u32) -> Self {
            self.ready_after = n;
            self
        }
        fn can_launch(mut self, can: bool) -> Self {
            self.can_launch = can;
            self
        }
        fn log(&self) -> Vec<&'static str> {
            self.log.borrow().clone()
        }
        fn count(&self, what: &str) -> usize {
            self.log.borrow().iter().filter(|one| **one == what).count()
        }
        fn run(&self) -> Settled {
            self.run_with(Policy::default())
        }
        fn run_with(&self, policy: Policy) -> Settled {
            open_monitor(&Ports {
                agent: AgentCoreState::Ready,
                host: self,
                companion: self,
                clock: self,
                policy,
            })
        }
    }

    impl Host for Fake {
        fn surface(&self) -> HostSurface {
            self.log.borrow_mut().push("host.surface");
            self.host
        }
        fn open(&self, _: MonitorIntent) -> bool {
            self.log.borrow_mut().push("host.open");
            true
        }
    }

    impl Companion for Fake {
        fn state(&self) -> Seen {
            self.log.borrow_mut().push("companion.state");
            Seen {
                state: self.state,
                descriptor: match self.state {
                    CompanionState::Missing => None,
                    _ => Some(DescriptorV1::for_app("0.1.0")),
                },
            }
        }
        fn launch(&self, _: MonitorIntent) -> bool {
            self.log.borrow_mut().push("companion.launch");
            self.can_launch
        }
        fn focus(&self, _: MonitorIntent) -> bool {
            self.log.borrow_mut().push("companion.focus");
            true
        }
        fn probe_ready(&self, _: &DescriptorV1) -> bool {
            self.log.borrow_mut().push("companion.probeReady");
            self.probes.set(self.probes.get() + 1);
            self.ready_after > 0 && self.probes.get() >= self.ready_after
        }
    }

    impl Clock for Fake {
        fn sleep(&self, ms: u64) {
            self.slept.borrow_mut().push(ms);
        }
    }

    // ── ① 표면 선택 순서 (JS 1~7) ────────────────────────────────────

    #[test]
    fn js1_a_verified_host_surface_never_touches_the_companion() {
        let made = Fake::new(CompanionState::Ready).host(HostSurface::Verified);
        let settled = made.run();
        assert_eq!(settled.outcome, Outcome::OpenedPersistentHost);
        assert_eq!(settled.installation.monitor_surface, MonitorSurface::PersistentHost);
        assert_eq!(made.log(), vec!["host.surface", "host.open"]);
        assert!(!made.log().iter().any(|one| one.starts_with("companion.")), "Companion 을 건드렸다");
    }

    #[test]
    fn js2_a_ready_companion_is_focused_not_relaunched() {
        let made = Fake::new(CompanionState::Ready);
        let settled = made.run();
        assert_eq!(settled.outcome, Outcome::FocusedExistingCompanion);
        assert_eq!(settled.installation.monitor_surface, MonitorSurface::NativeCompanion);
        assert_eq!(made.log(), vec!["host.surface", "companion.state", "companion.focus"]);
        assert_eq!(made.count("companion.launch"), 0, "이미 떠 있는데 또 띄웠다");
    }

    #[test]
    fn js3_stopped_is_launched_reprobed_then_focused_in_that_order() {
        let made = Fake::new(CompanionState::Stopped).ready_after(3);
        let settled = made.run();
        assert_eq!(settled.outcome, Outcome::StartedAndOpenedCompanion);
        assert_eq!(
            made.log(),
            vec![
                "host.surface", "companion.state", "companion.launch",
                "companion.probeReady", "companion.probeReady", "companion.probeReady",
                "companion.focus",
            ]
        );
    }

    #[test]
    fn js4_a_successful_launch_is_not_completion_without_a_handshake() {
        let made = Fake::new(CompanionState::Stopped).ready_after(0);
        let settled = made.run();
        assert_ne!(settled.outcome, Outcome::StartedAndOpenedCompanion);
        assert_eq!(settled.outcome, Outcome::MonitorUnavailable);
        assert_eq!(settled.why, Some(Why::NotReadyAfterLaunch));
        assert_eq!(made.count("companion.launch"), 1, "실행은 했다");
        assert_eq!(made.count("companion.focus"), 0, "ready 가 아닌데 창을 앞으로 가져왔다");
    }

    #[test]
    fn js5_neither_a_failed_launch_nor_a_silent_one_is_reported_as_success() {
        let made = Fake::new(CompanionState::Stopped).ready_after(0);
        let settled = made.run();
        assert_eq!(settled.outcome, Outcome::MonitorUnavailable);
        assert_eq!(settled.installation.monitor_surface, MonitorSurface::NativeCompanion);

        let failed = Fake::new(CompanionState::Stopped).can_launch(false);
        let second = failed.run();
        assert_eq!(second.outcome, Outcome::MonitorUnavailable);
        assert_eq!(second.why, Some(Why::LaunchFailed));
        assert_eq!(failed.count("companion.probeReady"), 0, "실행도 못 했는데 handshake 를 물었다");
    }

    #[test]
    fn js6_outdated_is_never_flattened_into_missing_or_stopped() {
        let seen: Vec<Outcome> = [CompanionState::Missing, CompanionState::Outdated, CompanionState::Stopped]
            .into_iter()
            .map(|state| Fake::new(state).ready_after(1).run().outcome)
            .collect();
        assert_eq!(
            seen,
            vec![
                Outcome::NeedsCompanionInstall,
                Outcome::NeedsCompanionUpdate,
                Outcome::StartedAndOpenedCompanion,
            ]
        );
        let made = Fake::new(CompanionState::Outdated);
        made.run();
        assert_eq!(made.count("companion.launch"), 0);
        assert_eq!(made.count("companion.focus"), 0);
        assert_eq!(
            installation_of(AgentCoreState::Ready, HostSurface::Unverified, Some(CompanionState::Outdated))
                .monitor_surface,
            MonitorSurface::Unavailable
        );
    }

    #[test]
    fn js7_missing_never_starts_an_install_without_approval() {
        let made = Fake::new(CompanionState::Missing);
        let settled = made.run();
        assert_eq!(settled.outcome, Outcome::NeedsCompanionInstall);
        assert_eq!(made.log(), vec!["host.surface", "companion.state"]);
        assert!(crate::say::outcome(settled.outcome).contains("승인"));
    }

    // ── ② degraded mode (JS 8·8b) ─────────────────────────────────────

    #[test]
    fn js8_the_agent_surface_survives_without_any_monitor() {
        let settled = Fake::new(CompanionState::Missing).run();
        assert_eq!(settled.installation.agent_surface, AgentSurface::Ready);
        assert_eq!(settled.installation.monitor_surface, MonitorSurface::Unavailable);
        assert!(!is_complete(settled.installation), "Monitor 없이 설치 완료라고 했다");
        for outcome in [Outcome::NeedsCompanionInstall, Outcome::NeedsCompanionUpdate, Outcome::MonitorUnavailable] {
            assert!(crate::say::outcome(outcome).contains("계속할 수 있다"), "{outcome:?}");
        }
    }

    #[test]
    fn js8b_every_degraded_path_still_answers_with_a_live_agent_surface() {
        let paths = [
            ("missing", Fake::new(CompanionState::Missing), Outcome::NeedsCompanionInstall),
            ("stopped·실행 실패", Fake::new(CompanionState::Stopped).can_launch(false), Outcome::MonitorUnavailable),
            ("outdated", Fake::new(CompanionState::Outdated), Outcome::NeedsCompanionUpdate),
            ("handshake timeout", Fake::new(CompanionState::Stopped).ready_after(0), Outcome::MonitorUnavailable),
        ];
        for (name, made, expected) in paths {
            let settled = made.run();
            assert_eq!(settled.outcome, expected, "{name}");
            assert_eq!(settled.installation.agent_surface, AgentSurface::Ready, "{name}: text loop 가 죽었다");
        }
    }

    // ── ③ 유한함과 중복 없음 (JS 10·11·11b) ──────────────────────────

    #[test]
    fn js10_one_request_never_launches_twice_or_focuses_twice() {
        for made in [Fake::new(CompanionState::Ready), Fake::new(CompanionState::Stopped).ready_after(2)] {
            made.run();
            assert!(made.count("companion.launch") <= 1, "실행이 두 번이다");
            assert_eq!(made.count("companion.focus"), 1, "창을 두 번 앞으로 가져왔다");
        }
    }

    #[test]
    fn js11_retries_are_finite_and_measured_by_a_deterministic_clock() {
        let made = Fake::new(CompanionState::Stopped).ready_after(0);
        let settled = made.run_with(Policy { probe_timeout_ms: 2_000, ready_attempts: 5, ready_gap_ms: 40 });
        assert_eq!(settled.outcome, Outcome::MonitorUnavailable);
        assert_eq!(made.count("companion.probeReady"), 5, "시도 횟수가 정책과 다르다");
        assert_eq!(*made.slept.borrow(), vec![40, 40, 40, 40, 40], "기다린 시간이 정책과 다르다");
    }

    #[test]
    fn js11b_the_default_policy_is_finite_too() {
        let made = Fake::new(CompanionState::Stopped).ready_after(0);
        made.run();
        let tries = made.count("companion.probeReady");
        assert!(tries > 0 && tries <= 60, "시도가 {tries}회다");
        assert!(made.slept.borrow().iter().sum::<u64>() <= 10_000, "총 대기가 너무 길다");
    }

    // ── ⑥ 읽기 모델 ─────────────────────────────────────────────────

    #[test]
    fn completion_needs_both_an_agent_surface_and_a_persistent_monitor() {
        let table = [
            (HostSurface::Verified, CompanionState::Missing, MonitorSurface::PersistentHost, true),
            (HostSurface::Unverified, CompanionState::Ready, MonitorSurface::NativeCompanion, true),
            (HostSurface::Unverified, CompanionState::Stopped, MonitorSurface::NativeCompanion, true),
            (HostSurface::Unverified, CompanionState::Outdated, MonitorSurface::Unavailable, false),
            (HostSurface::Unverified, CompanionState::Missing, MonitorSurface::Unavailable, false),
        ];
        for (host, companion, surface, complete) in table {
            let installation = installation_of(AgentCoreState::Ready, host, Some(companion));
            assert_eq!(installation.monitor_surface, surface, "{host:?}+{companion:?}");
            assert_eq!(is_complete(installation), complete, "{host:?}+{companion:?}");
        }
        assert!(!is_complete(installation_of(
            AgentCoreState::Unavailable,
            HostSurface::Verified,
            Some(CompanionState::Ready)
        )));
    }

    // ── agent 상태는 자기 descriptor 에서 — spawn 없이 ───────────────

    #[test]
    fn the_agent_state_of_this_process_is_ready_and_never_spawns() {
        assert_eq!(AgentCoreState::of_this_process(), AgentCoreState::Ready);
    }

    #[test]
    fn a_foreign_product_is_unavailable_and_a_range_mismatch_is_outdated() {
        let mut other = crate::agent::DescriptorV1::current();
        other.product = "something_else".into();
        assert_eq!(AgentCoreState::of(&other), AgentCoreState::Unavailable);

        let mut old = crate::agent::DescriptorV1::current();
        old.action_surface = crate::agent::RangeV1 { min: 99, max: 99 };
        assert_eq!(AgentCoreState::of(&old), AgentCoreState::OutdatedAgent);
    }

    #[test]
    fn the_intent_cannot_be_written_anywhere() {
        // 컴파일 시점의 사실을 시험으로 적어 둔다: Serialize 가 없으므로 어디에도 적히지 않는다.
        fn must_not_serialize<T>(_: &T) {}
        must_not_serialize(&MonitorIntent::open_monitor());
    }
}
