//! **JS 정본과의 byte 동치** — 이전 기간 동안 두 문이 같은 말을 하는지.
//!
//! 정본은 `plugins/gil-companion-prototype/{say,capability}.mjs` 다. 그 출력을
//! `tests/fixtures/companion-say.json` 에 찍어 두었고, 이 시험은 Rust 의 문장·값·읽기 모델을
//! 그 snapshot 과 **byte 로** 견준다. Node 가 없는 기계에서도 돈다.
//!
//! Node 가 있으면 한 걸음 더 간다 — JS 를 지금 다시 실행해 snapshot 이 낡지 않았는지 본다.
//! 그래서 JS 쪽 문장이 바뀌면 개발 기계에서 먼저 걸리고, snapshot 을 다시 찍기 전에는
//! Rust 도 따라갈 수 없다. 정본이 하나라는 뜻이다.

use std::collections::BTreeMap;

use gil::capability::{AgentCoreState, CompanionState, HostSurface, Outcome, Policy, installation_of, is_complete};

#[derive(serde::Deserialize)]
struct Fixture {
    values: Values,
    policy: JsPolicy,
    say: Say,
    installation: Vec<Row>,
}

#[derive(serde::Deserialize)]
struct Values {
    #[serde(rename = "OUTCOME")]
    outcome: Vec<String>,
    #[serde(rename = "COMPANION_STATE")]
    companion_state: Vec<String>,
    #[serde(rename = "MONITOR_SURFACE")]
    monitor_surface: Vec<String>,
    #[serde(rename = "AGENT_SURFACE")]
    agent_surface: Vec<String>,
    #[serde(rename = "AGENT_CORE_STATE")]
    agent_core_state: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct JsPolicy {
    probe_timeout_ms: u64,
    ready_attempts: u32,
    ready_gap_ms: u64,
}

#[derive(serde::Deserialize)]
struct Say {
    outcomes: BTreeMap<String, String>,
    states: BTreeMap<String, StateSaid>,
}

#[derive(serde::Deserialize)]
struct StateSaid {
    with_version: String,
    without_version: String,
}

#[derive(serde::Deserialize)]
struct Row {
    host: String,
    companion: String,
    agent: String,
    agent_surface: String,
    monitor_surface: String,
    complete: bool,
}

const FIXTURE: &str = include_str!("fixtures/companion-say.json");

fn fixture() -> Fixture {
    serde_json::from_str(FIXTURE).expect("fixture JSON")
}

fn wire<T: serde::Serialize>(one: &T) -> String {
    serde_json::to_value(one)
        .expect("wire")
        .as_str()
        .expect("string enum")
        .to_string()
}

fn outcome_named(name: &str) -> Outcome {
    Outcome::ALL
        .into_iter()
        .find(|one| wire(one) == name)
        .unwrap_or_else(|| panic!("모르는 outcome: {name}"))
}

const STATES: [CompanionState; 4] = [
    CompanionState::Missing,
    CompanionState::Stopped,
    CompanionState::Outdated,
    CompanionState::Ready,
];

fn state_named(name: &str) -> CompanionState {
    STATES
        .into_iter()
        .find(|one| wire(one) == name)
        .unwrap_or_else(|| panic!("모르는 state: {name}"))
}

fn agent_named(name: &str) -> AgentCoreState {
    [AgentCoreState::Ready, AgentCoreState::OutdatedAgent, AgentCoreState::Unavailable]
        .into_iter()
        .find(|one| wire(one) == name)
        .unwrap_or_else(|| panic!("모르는 agent state: {name}"))
}

fn host_named(name: &str) -> HostSurface {
    match name {
        "verified" => HostSurface::Verified,
        "unverified" => HostSurface::Unverified,
        other => panic!("모르는 host surface: {other}"),
    }
}

// ── 값의 이름 ─────────────────────────────────────────────────────────

#[test]
fn every_wire_value_is_spelled_exactly_as_js_spells_it() {
    let seen = fixture().values;
    let outcomes: Vec<String> = Outcome::ALL.iter().map(wire).collect();
    assert_eq!(outcomes, seen.outcome);
    let states: Vec<String> = STATES.iter().map(wire).collect();
    assert_eq!(states, seen.companion_state);
    assert_eq!(
        vec![
            wire(&gil::capability::MonitorSurface::PersistentHost),
            wire(&gil::capability::MonitorSurface::NativeCompanion),
            wire(&gil::capability::MonitorSurface::Unavailable),
        ],
        seen.monitor_surface
    );
    assert_eq!(
        vec![wire(&gil::capability::AgentSurface::Ready), wire(&gil::capability::AgentSurface::Unavailable)],
        seen.agent_surface
    );
    assert_eq!(
        vec![
            wire(&AgentCoreState::Ready),
            wire(&AgentCoreState::OutdatedAgent),
            wire(&AgentCoreState::Unavailable),
        ],
        seen.agent_core_state
    );
}

#[test]
fn the_default_policy_is_the_js_default_policy() {
    let js = fixture().policy;
    let rust = Policy::default();
    assert_eq!(rust.probe_timeout_ms, js.probe_timeout_ms);
    assert_eq!(rust.ready_attempts, js.ready_attempts);
    assert_eq!(rust.ready_gap_ms, js.ready_gap_ms);
}

// ── 문장 ──────────────────────────────────────────────────────────────

#[test]
fn every_outcome_sentence_is_byte_for_byte_the_js_sentence() {
    let seen = fixture().say.outcomes;
    assert_eq!(seen.len(), Outcome::ALL.len(), "outcome 수가 다르다");
    for (name, expected) in seen {
        let said = gil::say::outcome(outcome_named(&name));
        assert_eq!(said.as_bytes(), expected.as_bytes(), "{name}\nRust: {said}\nJS:   {expected}");
    }
}

#[test]
fn every_state_sentence_is_byte_for_byte_the_js_sentence_with_and_without_a_version() {
    let seen = fixture().say.states;
    assert_eq!(seen.len(), STATES.len(), "state 수가 다르다");
    for (name, expected) in seen {
        let state = state_named(&name);
        let with = gil::say::state(state, Some("0.1.0"));
        let without = gil::say::state(state, None);
        assert_eq!(with.as_bytes(), expected.with_version.as_bytes(), "{name} (판 있음)");
        assert_eq!(without.as_bytes(), expected.without_version.as_bytes(), "{name} (판 없음)");
    }
}

// ── 읽기 모델 ─────────────────────────────────────────────────────────

#[test]
fn the_installation_read_model_agrees_with_js_on_all_24_combinations() {
    let rows = fixture().installation;
    assert_eq!(rows.len(), 24);
    for row in rows {
        let made = installation_of(agent_named(&row.agent), host_named(&row.host), Some(state_named(&row.companion)));
        let label = format!("{}+{}+{}", row.host, row.companion, row.agent);
        assert_eq!(wire(&made.agent_surface), row.agent_surface, "{label}");
        assert_eq!(wire(&made.monitor_surface), row.monitor_surface, "{label}");
        assert_eq!(is_complete(made), row.complete, "{label}");
    }
}

// ── snapshot 이 낡지 않았는가 — Node 가 있을 때만 ─────────────────────

#[test]
fn the_snapshot_is_still_what_js_says_today_when_node_is_here() {
    let script = r#"
import { OUTCOME, COMPANION_STATE } from "./plugins/gil-companion-prototype/capability.mjs";
import { sayOutcome, sayState } from "./plugins/gil-companion-prototype/say.mjs";
const outcomes = {}; for (const v of Object.values(OUTCOME)) outcomes[v] = sayOutcome(v);
const states = {}; for (const v of Object.values(COMPANION_STATE)) states[v] = { with_version: sayState(v, "0.1.0"), without_version: sayState(v, null) };
process.stdout.write(JSON.stringify({ outcomes, states }));
"#;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let ran = std::process::Command::new("node")
        .args(["--input-type=module", "-e", script])
        .current_dir(root)
        .output();
    let Ok(done) = ran else {
        // Node 가 없는 기계다. 위의 snapshot 시험들이 여전히 지킨다 — 여기서는 그 사실만 적는다.
        eprintln!("node 가 없어 snapshot 신선도는 확인하지 않았다 (snapshot 대조는 위에서 했다)");
        return;
    };
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let live: Say = serde_json::from_slice(&done.stdout).expect("JS 출력");
    let snap = fixture().say;
    assert_eq!(live.outcomes, snap.outcomes, "snapshot 의 outcome 문장이 낡았다 — fixture 를 다시 찍어라");
    for (name, one) in live.states {
        let kept = &snap.states[&name];
        assert_eq!(one.with_version, kept.with_version, "{name}: snapshot 이 낡았다");
        assert_eq!(one.without_version, kept.without_version, "{name}: snapshot 이 낡았다");
    }
}
