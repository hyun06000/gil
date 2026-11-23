//! 닫힌 Experiment T 의 Exit 세계와 그 확정 정책 유도(§6.1 의 「Experiment 의 Exit → 그 실험을 열 때의 적용 정책」) 시험 — **합성 사건
//! 입력**과 **기존 Cycles 전이로 만든 체인**, 그리고 이름을 밝힌 **복원 구조**를 쓴다.
//!
//! 정책 판·사건 칸·시도 결과·등록부는 시험이 만든 값이다. 승인은 시험 자료의 `approved: yes` 이며 모니터 승인이 아니다. `state.yaml`·
//! `gil start`·파일 시스템을 쓰지 않는다.
//!
//! 기대값은 구현에서 옮기지 않고 계약에서 도출했다 — §6.1 의 확정 정책 표(「Experiment 의 Verify·Exit → 그 실험을 열 때의 적용 정책
//! (§6.2 의 P)」, 「Interview 의 Exit → 그 Entry 의 확정 정책을 물려받는다」), 「정책은 사이클 사이에서만 바뀌므로 한 실험의 생애에는
//! 정책이 하나다」, 「`exit_snapshot_ref` 는 한 번 쓰면 바꾸지 않는다. 사건은 그 값들을 덮어쓰지 않고 옆에 선다」, §6.2 의 P(N)
//! (「next_cycle_id ≤ N 인 사건 가운데 이름이 가장 큰 것의 policy_after, 없으면 정책 없음」), §5.1 의 판 이름 규칙, §5.5 의 같은 세계.
//!
//! 체인 두 가지:
//!
//! ```text
//! tree()           Cycles 층의 실제 전이 — open_child · close_current · revisit · open_after_revisit
//!   chain:C1  cycle:C1 Interview   (닫힘·open_child)     Entry A1 · Exit A1
//!             └ cycle:C2 Experiment (닫힘·success)       Entry A1 · Verify A2 → Exit A2
//!                 ├ cycle:C3 Experiment (닫힘·failure → revisit cycle:C2)   Entry A2 · Verify A3 → Exit A3
//!                 └ cycle:C4 Experiment (열림)  parent C2 · revisit_from C3   Entry A2
//! line(n)          Interview 만의 직선 — cycle:C1 ─▶ … ─▶ cycle:C<n>, C<n> 만 열림, 모든 Entry·Exit A1
//! restored(..)     위 체인 하나의 상태를 기존 `Cycles::restore` 로 다시 세운 것 — 한 칸만 바꾼다
//! ```
//!
//! `tree` 는 Cycles 층의 실제 Experiment 닫기(Verify 의 세계 확정 → Exit)와 되돌아가기 전이로 만든 **메모리 구조**다. 프로젝트 세션의
//! Verify 관측·보관·작업물 복원·저장·잠금을 검증하지 않는다. `restored` 는 `Cycles::restore` 의 그래프 불변식을 통과한 값이지만 현행
//! `Project` load 의 세계 검사(닫힌 사이클의 Exit 실재)는 거칠 수 없다 — 그런 시험은 그 사실을 적는다. 어떤 시험도 실제 승인·보관·관문 ②·
//! 제품 load 성공을 검증했다고 말하지 않는다.

use super::event_cycles_tests::{a, c, ch, close_interview, m, open_interview, registry, rules, world, x1};
use super::*;
use crate::artifact::gilignore::EngineVersion;
use crate::artifact::{SnapshotRegistry, digest_from_raw, digest_of};
use crate::cycle::CycleState;
use crate::cycles::{Cycles, CyclesState};
use crate::refs::{JourneyRef, SnapshotRef};
use crate::walk::{RestoreError, WalkState};
use crate::{Chain, Cycle, CycleKind, NodeKind, RuleSet};

use AttemptSeen::{Archived as A, NotArchived as F};
use BaselineSlot::{Unwritten, Written};
use EventKind::{RestoreResult, ScopeChange, Start};
use OpeningSelection::BeforeAdoptionInGivenHistory as Before;

#[path = "../../../tests/common/mod.rs"]
mod common;

// ── 체인 ───────────────────────────────────────────────────────────────────
//
// 아래의 `pub(super)` 도움 함수는 Exit 계보 검사(`exit_lineage_tests`)도 같은 체인을 쓰려고 연 것이다. 동작은 같다.

/// 지금 서 있는 열린 Experiment 를 Define → Hypothesis → Verify(세계 `A<verify>` 확정) → Analysis → Outcome 으로 걷고 닫는다.
/// `success` 는 `open_child`, `failure` 는 부모로의 `revisit` 을 적는다(공통 도움 함수 `cycle_report` 가 부모를 대상으로 고른다).
pub(super) fn close_experiment(cycles: &mut Cycles, verify: u32, verdict: &str) {
    let report = {
        let cycle = cycles.current_mut();
        for kind in [NodeKind::Define, NodeKind::Hypothesis, NodeKind::Verify, NodeKind::Analysis] {
            cycle.open_step(kind).unwrap_or_else(|err| panic!("{kind} 를 열지 못했다: {err}"));
            let report = common::full_report(cycle.rules(), CycleKind::Experiment, kind);
            match kind == NodeKind::Verify {
                true => cycle.close_verify_step(report, a(verify)),
                false => cycle.close_step(report),
            }
            .unwrap_or_else(|err| panic!("{kind} 를 닫지 못했다: {err}"));
        }
        cycle.open_step(NodeKind::Outcome).expect("Outcome 을 연다");
        let outcome = cycle.steps().current().expect("Outcome 에 서 있다");
        let report = common::full_report(cycle.rules(), CycleKind::Experiment, NodeKind::Outcome)
            .with("verdict", verdict)
            .with("lesson", "이 갈래가 남긴 것 — 시험 자료")
            .with(common::ACTION, "close_cycle")
            .with(common::REASON, "판정을 Cycle 경계로 넘긴다");
        cycle.close_step(report).expect("Outcome 을 닫는다");
        let mut report = common::cycle_report(cycle, verdict, outcome);
        match verdict {
            "success" => {
                report.insert(common::ACTION, "open_child");
            }
            _ => assert_eq!(report.get(common::ACTION), Some("revisit"), "실패는 부모로 되돌아간다고 적는다"),
        }
        report
    };
    cycles.close_current(report, JourneyRef::new(x1(), 0)).expect("Experiment 를 닫는다");
}

/// 위 그림의 `tree` — 모두 기존 전이 함수로 걸어 만든다.
pub(super) fn tree() -> Vec<Chain> {
    let mut cycles = Cycles::start_at(rules(), x1(), a(1), 1);
    close_interview(&mut cycles, "open_child");
    cycles.open_child(CycleKind::Experiment, x1()).expect("승인된 인터뷰 뒤 C2 를 연다");
    close_experiment(&mut cycles, 2, "success");
    cycles.open_child(CycleKind::Experiment, x1()).expect("성공한 C2 뒤 C3 을 연다");
    close_experiment(&mut cycles, 3, "failure");
    let moved = cycles.revisit().expect("적어 둔 대로 C2 로 되돌아간다");
    assert_eq!((moved.target, moved.target_world), (c(2), a(2)));
    cycles.open_after_revisit(CycleKind::Experiment, x1()).expect("되돌아간 C2 에서 새 갈래를 연다");
    vec![Chain::restore(ch(1), x1(), a(1), None, cycles)]
}

/// 열린 체인 하나에 Interview `count` 개 — 마지막만 열려 있고, 모든 Entry·Exit 은 A1 이다.
fn line(count: u32) -> Vec<Chain> {
    let mut cycles = Cycles::start_at(rules(), x1(), a(1), 1);
    for _ in 1..count {
        close_interview(&mut cycles, "open_child");
        open_interview(&mut cycles);
    }
    vec![Chain::restore(ch(1), x1(), a(1), None, cycles)]
}

/// 사이클 하나를 기존 복원 입구의 상태로 옮긴다 — 접근자로 읽은 값 그대로이고, 한 칸만 채운다.
///
/// `Project` 의 단계 닫기는 닫은 Journey 판을 `Walk::record_journey` 로 적는다. 이 시험의 Cycles 층 메모리 전이는 그 칸을 비워 두므로,
/// 복원 입구(`Walk::restore`)가 닫힌 단계에 요구하는 그 칸에만 사이클을 닫을 때 쓴 것과 같은 합성 판 `X1` 의 0번 판을 적는다.
fn state_of(cycle: &Cycle) -> CycleState {
    let walk = cycle.steps();
    let mut nodes = walk.nodes().to_vec();
    for node in nodes.iter_mut().filter(|node| node.is_closed() && node.journey.is_none()) {
        node.journey = Some(JourneyRef::new(x1(), 0));
    }
    CycleState {
        id: cycle.id(),
        parent: cycle.parent(),
        revisit_from: cycle.revisit_from(),
        kind: cycle.kind(),
        status: cycle.status(),
        existence: cycle.existence(),
        journey: cycle.journey(),
        report: cycle.report().cloned(),
        entry: cycle.entry_snapshot(),
        exit: cycle.exit_snapshot(),
        steps: WalkState {
            nodes,
            current: walk.current(),
            next_id: walk.next_id(),
            pending_revisit: walk.pending_revisit(),
        },
    }
}

/// 체인의 사이클 상태를 `edit` 로 바꾼 뒤 기존 [`Cycles::restore`] 로 다시 세운다. 그 그래프 불변식이 거절하면 그 오류 그대로다.
pub(super) fn restored(chain: &Chain, edit: impl FnOnce(&mut [CycleState])) -> Result<Chain, RestoreError> {
    restored_under(rules(), chain, edit)
}

/// [`restored`] 와 같되 체인을 걸어 만든 문법 `rules` 로 다시 세운다.
pub(super) fn restored_under(
    rules: RuleSet,
    chain: &Chain,
    edit: impl FnOnce(&mut [CycleState]),
) -> Result<Chain, RestoreError> {
    let cycles = chain.cycles();
    let mut nodes: Vec<CycleState> = cycles.nodes().iter().map(state_of).collect();
    edit(&mut nodes);
    let state = CyclesState {
        next_id: cycles.next_name(),
        current: cycles.current_id(),
        pending_revisit: cycles.pending_revisit(),
        nodes,
    };
    Cycles::restore(rules, state)
        .map(|cycles| Chain::restore(chain.id(), chain.existence(), chain.entry_snapshot(), chain.parent(), cycles))
}

pub(super) fn at(number: u32) -> usize {
    usize::try_from(number - 1).unwrap()
}

/// 사이클마다 (번호, 종류, 닫힘, 부모, revisit_from, Entry, Exit, 판정).
type Shape = (u32, CycleKind, bool, Option<u32>, Option<u32>, u32, Option<u32>, Option<String>);

fn shape(chain: &Chain) -> Vec<Shape> {
    chain
        .cycles()
        .nodes()
        .iter()
        .map(|cycle| {
            (
                cycle.id().to_ref().number(),
                cycle.kind(),
                cycle.is_closed(),
                cycle.parent().map(|id| id.to_ref().number()),
                cycle.revisit_from().map(|id| id.to_ref().number()),
                cycle.entry_snapshot().number(),
                cycle.exit_snapshot().map(SnapshotRef::number),
                cycle.report().and_then(|report| report.get("verdict")).map(str::to_string),
            )
        })
        .collect()
}

#[test]
fn the_fixtures_are_what_the_picture_says() {
    use CycleKind::{Experiment, Interview};
    let verdict = |text: &str| Some(text.to_string());
    let tree = tree();
    let wanted: Vec<Shape> = vec![
        (1, Interview, true, None, None, 1, Some(1), verdict("success")),
        (2, Experiment, true, Some(1), None, 1, Some(2), verdict("success")),
        (3, Experiment, true, Some(2), None, 2, Some(3), verdict("failure")),
        (4, Experiment, false, Some(2), Some(3), 2, None, None),
    ];
    assert_eq!(shape(&tree[0]), wanted);
    // 바꾸지 않고 다시 세우면 같은 구조다 — 복원 도움 함수가 아무것도 지어내지 않는다.
    assert_eq!(shape(&restored(&tree[0], |_| {}).expect("그대로 복원된다")), wanted);
    let line = line(2);
    assert_eq!(
        shape(&line[0]),
        [(1, Interview, true, None, None, 1, Some(1), verdict("success")), (2, Interview, false, Some(1), None, 1, None, None)]
    );
}

// ── 사건: 한 행에서 두 부분 입력을 함께 뽑는다 ─────────────────────────────

fn p(number: u32) -> PolicyName {
    PolicyName::new(number).expect("0 이 아니다")
}

/// 판마다 원문 글자를 받는다 — 같은 글자를 두 번 주면 같은 원문의 다른 판이 된다(§5.1).
fn ledger_of(texts: &[&str]) -> PolicyLedger {
    let records = texts
        .iter()
        .enumerate()
        .map(|(at, text)| {
            let n = u32::try_from(at).unwrap() + 1;
            let rules = digest_of(text.as_bytes()).expect("메모리의 바이트는 언제나 읽힌다");
            let proposal = ProposalDigest(digest_from_raw([u8::try_from(n).unwrap(); 32]));
            PolicyRecord::new(p(n), EngineVersion::V1, rules, proposal)
        })
        .collect::<Vec<_>>();
    PolicyLedger::from_records(u32::try_from(texts.len()).unwrap() + 1, records).expect("시험 원장은 꼴이 맞다")
}

#[derive(Clone, Copy)]
enum Base {
    /// 기준점 없음, 경계·발급기 칸 없음, 실패 시도 하나 — 보관 대기이거나 뒤 사건으로 대체됐다.
    Waiting,
    /// 기준점·`baseline` 경계(`None` = `null`)·`baseline.next_cycle_id` 를 함께 적고 `archived` 하나.
    Fixed { snapshot: u32, boundary: Option<u32>, issuer: u32 },
}

/// 사건 원장의 한 기록 — 3c 입력과 3h 입력이 이 한 행에서 함께 나온다. 모든 사건의 `chain_ref` 는 `chain:C1` 이다.
#[derive(Clone, Copy)]
struct Row {
    kind: EventKind,
    before: Option<u32>,
    after: u32,
    /// 승인 경계 `after_cycle_ref` (`None` = `null`).
    at: Option<u32>,
    /// 승인 당시의 `next_cycle_id`.
    issuer: u32,
    base: Base,
}

/// §5.1 예시의 `start` — `chain:C1`, 경계 `null`, `next_cycle_id: 1`, 기준점 A1(보관 경계 `null`, 발급기 1).
fn start() -> Row {
    Row { kind: Start, before: None, after: 1, at: None, issuer: 1, base: Base::Fixed { snapshot: 1, boundary: None, issuer: 1 } }
}

/// `P<after - 1> → P<after>` 범위 변경 — 승인 경계 `at`, 승인 당시 발급기 `issuer`. `after = 1` 이면 기존 프로젝트의 첫 채택(없음 → P1).
fn change(after: u32, at: u32, issuer: u32, base: Base) -> Row {
    Row { kind: ScopeChange, before: (after > 1).then(|| after - 1), after, at: Some(at), issuer, base }
}

fn restore(policy: u32, at: u32, issuer: u32, base: Base) -> Row {
    Row { kind: RestoreResult, before: Some(policy), after: policy, at: Some(at), issuer, base }
}

/// 경계 `boundary` 에서 발급기 `issuer` 일 때 보관한 기준점 `A<snapshot>`.
fn kept(snapshot: u32, boundary: u32, issuer: u32) -> Base {
    Base::Fixed { snapshot, boundary: Some(boundary), issuer }
}

/// 한 기록 목록과 그것이 도입한 판의 원장 — 셋은 같은 한 시점에서 뽑은 값이다.
struct Records {
    ledger: PolicyLedger,
    links: EventPolicyLinks,
    facts: Vec<EventCycleFacts>,
}

fn records_with(texts: &[&str], rows: &[Row]) -> Records {
    let links = rows
        .iter()
        .enumerate()
        .map(|(at, row)| EventPolicyLink::new(m(u32::try_from(at).unwrap() + 1), row.kind, row.before.map(p), Some(p(row.after))))
        .collect::<Vec<_>>();
    let facts = rows
        .iter()
        .enumerate()
        .map(|(at, row)| {
            let number = u32::try_from(at).unwrap() + 1;
            let baseline = match row.base {
                Base::Waiting => BaselineFacts::new(m(number), row.kind, None, Unwritten, Unwritten, vec![F]),
                Base::Fixed { snapshot, boundary, issuer } => BaselineFacts::new(
                    m(number),
                    row.kind,
                    Some(a(snapshot)),
                    Written(boundary.map(c)),
                    Written(issuer),
                    vec![A],
                ),
            };
            EventCycleFacts::new(baseline, ch(1), Written(row.at.map(c)), row.issuer)
        })
        .collect();
    Records {
        ledger: ledger_of(texts),
        links: EventPolicyLinks::from_links(u32::try_from(rows.len()).unwrap() + 1, links).expect("시험 사건 이름은 꼴이 맞다"),
        facts,
    }
}

/// 판마다 다른 원문 — 판 수는 판을 도입한 사건(`restore_result` 밖) 수다.
fn records(rows: &[Row]) -> Records {
    let introduced = rows.iter().filter(|row| row.kind != RestoreResult).count();
    let texts: Vec<String> = (1..=introduced).map(|n| format!("rule-{n}/\n")).collect();
    records_with(&texts.iter().map(String::as_str).collect::<Vec<_>>(), rows)
}

fn no_records() -> Records {
    Records { ledger: PolicyLedger::empty(), links: EventPolicyLinks::empty(), facts: Vec::new() }
}

fn select_with(
    target: u32,
    records: &Records,
    chains: &[Chain],
    registry: &SnapshotRegistry,
) -> Result<ExperimentExit, Vec<ExperimentExitProblem>> {
    select_experiment_exit(c(target), &records.ledger, &records.links, &records.facts, chains, registry)
}

fn derived(target: u32, records: &Records, chains: &[Chain]) -> ExperimentExit {
    select_with(target, records, chains, &registry(12)).unwrap_or_else(|problems| panic!("유도해야 한다: {problems:#?}"))
}

fn refused(target: u32, records: &Records, chains: &[Chain]) -> Vec<ExperimentExitProblem> {
    select_with(target, records, chains, &registry(12)).expect_err("거절해야 한다")
}

/// (Exit 세계 번호, 확정 정책).
fn exit_and_policy(target: u32, records: &Records, chains: &[Chain]) -> (u32, OpeningSelection) {
    let found = derived(target, records, chains);
    (found.world().number(), found.policy())
}

/// 기대값 — P(T) 의 사건 번호·종류·판 번호·승인 당시 발급기.
fn applied(event: u32, kind: EventKind, policy: u32, next_cycle_id: u32) -> OpeningSelection {
    OpeningSelection::Applied { event: m(event), kind, policy: p(policy), next_cycle_id }
}

/// 같은 입력의 P(T) — 비교용. 제품 코드가 아니라 시험이 따로 부른다.
fn opening_of(target: u32, records: &Records, chains: &[Chain]) -> OpeningPolicy {
    select_opening_policy(c(target), &records.ledger, &records.links, &records.facts, chains, &registry(12)).expect("P(T) 를 고른다")
}

/// 같은 입력의 B(T) — 비교용.
fn boundary_of(target: u32, records: &Records, chains: &[Chain]) -> BoundaryWorld {
    select_boundary_world(c(target), &records.ledger, &records.links, &records.facts, chains, &registry(12)).expect("B(T) 를 고른다")
}

// ── 사건이 없을 때: 각 실험 자신의 Exit, 건넨 이력 안의 정책 채택 전 ─────────

#[test]
fn without_events_each_closed_experiment_gives_its_own_exit_before_adoption() {
    // 성공한 C2 의 Exit A2, 실패한 C3 의 Exit A3. 뿌리의 Entry A1·부모 Exit·되돌아간 뒤 연 C4 의 것이 아니다. 판정으로 거르지 않는다.
    let tree = tree();
    assert_eq!(exit_and_policy(2, &no_records(), &tree), (2, Before));
    assert_eq!(exit_and_policy(3, &no_records(), &tree), (3, Before));
    let found = derived(3, &no_records(), &tree);
    assert_eq!((found.target(), found.chain(), found.world()), (c(3), ch(1), a(3)));
    assert_ne!(Some(found.world()), tree[0].cycles().nodes()[at(2)].exit_snapshot(), "부모 C2 의 Exit 으로 대신했다");
    assert!(found.inputs().events().is_empty());
}

// ── 개방 뒤 정책이 바뀌어도 Exit 정책은 과거 P(T) ─────────────────────────

/// | 사건 | 종류 | 정책 | 승인 경계·next_cycle_id | 기준점 (보관 경계·발급기) |
/// |---|---|---|---|---|
/// | M1 | start | 없음 → P1 | `null`·1 | A1 (`null`·1) |
/// | M2 | scope_change | P1 → P2 | C2·3 | A5 (C2·3) |
/// | M3 | scope_change | P2 → P3 | C3·4 | A6 (C3·4) |
/// | M4 | restore_result | P3 → P3 | C2·5 | A7 (C2·5) |
fn changed_after_opening() -> Vec<Row> {
    vec![start(), change(2, 2, 3, kept(5, 2, 3)), change(3, 3, 4, kept(6, 3, 4)), restore(3, 2, 5, kept(7, 2, 5))]
}

#[test]
fn the_policy_applied_when_the_experiment_opened_survives_later_changes() {
    let tree = tree();
    let records = records(&changed_after_opening());
    // C2 은 M1 뒤(발급기 1 ≤ 2)에 열렸다 — P1. M2·M3·M4 는 C2 가 열린 뒤에 저장됐다.
    assert_eq!(exit_and_policy(2, &records, &tree), (2, applied(1, Start, 1, 1)));
    // C3 은 M2 뒤(발급기 3 ≤ 3)에 열렸다 — P2. C3 을 닫은 경계의 M3(P3)·원장의 마지막 판 P3 이 아니다.
    assert_eq!(exit_and_policy(3, &records, &tree), (3, applied(2, ScopeChange, 2, 3)));
    assert_eq!(records.ledger.policies().last().map(PolicyRecord::name), Some(p(3)), "원장의 마지막 판은 P3 이다");
}

#[test]
fn the_result_carries_the_same_call_opening_policy_unchanged() {
    // 확정 정책은 같은 입력의 P(T) 선택 그대로이고, 그 사건·종류·판·승인 발급기와 3j 성공 값을 함께 읽는다.
    let tree = tree();
    let records = records(&changed_after_opening());
    for target in [2, 3] {
        let found = derived(target, &records, &tree);
        let opening = opening_of(target, &records, &tree);
        assert_eq!(found.opening(), &opening);
        assert_eq!(found.policy(), opening.selection());
        assert_eq!((found.target(), found.chain()), (opening.target(), opening.chain()));
        assert_eq!(found.inputs(), opening.inputs());
    }
}

// ── T 경계의 나중 기준점은 Exit 세계·정책을 대체하지 않는다 ────────────────

#[test]
fn a_later_baseline_at_the_experiment_boundary_replaces_neither_its_exit_nor_its_policy() {
    let tree = tree();
    let records = records(&changed_after_opening());
    // 지금 건넨 기록의 B(C2) 는 C2 에서 보관한 M2·M4 가운데 M4 의 A7(P3) 이다. Exit 은 A2·P1 그대로다.
    let boundary = boundary_of(2, &records, &tree);
    assert_eq!((boundary.world(), boundary.policy()), (a(7), BoundaryPolicy::PolicyAfter { event: m(4), policy: p(3) }));
    let found = derived(2, &records, &tree);
    assert_eq!((found.world(), found.policy()), (a(2), applied(1, Start, 1, 1)));
    // C3 도 같다 — B(C3) 는 M3 의 A6(P3), Exit 은 A3·P2.
    let boundary = boundary_of(3, &records, &tree);
    assert_eq!((boundary.world(), boundary.policy()), (a(6), BoundaryPolicy::PolicyAfter { event: m(3), policy: p(3) }));
    assert_eq!(exit_and_policy(3, &records, &tree), (3, applied(2, ScopeChange, 2, 3)));
    // 사건을 덧붙이기 전의 기록에서도 Exit 은 같다 — 사건은 과거의 Exit 을 고치지 않고 옆에 선다(§6.1).
    let rows = changed_after_opening();
    let earlier = records_with(&["rule-1/\n"], &rows[..1]);
    assert_eq!(derived(2, &earlier, &tree).world(), found.world());
}

// ── 같은 세계·같은 원문의 다른 판을 합치지 않는다 ──────────────────────────

#[test]
fn the_same_world_and_the_same_text_under_other_boards_are_not_merged() {
    // P1·P3 은 같은 원문이다. M3 은 C2 경계에서 C2 의 Exit 과 같은 세계 A2 를 P3 으로 보관했다(§5.5 의 같은 세계).
    let tree = tree();
    let rows = [start(), change(2, 1, 2, kept(5, 1, 2)), change(3, 2, 3, kept(2, 2, 3))];
    let records = records_with(&["same/\n", "other/\n", "same/\n"], &rows);
    assert_eq!(records.ledger.policies()[0].rules(), records.ledger.policies()[2].rules(), "P1 과 P3 은 같은 원문이다");
    // C2 의 Exit A2 는 P2 로 확정됐다. 같은 이름 A2 를 P3 의 사건이 가리켜도 그 판으로 바꾸지 않는다.
    assert_eq!(exit_and_policy(2, &records, &tree), (2, applied(2, ScopeChange, 2, 2)));
    let boundary = boundary_of(2, &records, &tree);
    assert_eq!((boundary.world(), boundary.policy()), (a(2), BoundaryPolicy::PolicyAfter { event: m(3), policy: p(3) }));
    // C3 은 P3 으로 열렸다 — 같은 원문의 P1 로 합치지 않는다.
    let (world, policy) = exit_and_policy(3, &records, &tree);
    assert_eq!((world, policy), (3, applied(3, ScopeChange, 3, 3)));
    assert_ne!(policy, applied(1, Start, 1, 1));
}

// ── 건넨 이력 안의 정책 채택 전 ────────────────────────────────────────────

#[test]
fn before_adoption_in_the_given_history_is_neither_not_derived_nor_a_failure() {
    // 기존 프로젝트의 첫 채택(없음 → P1)을 C2 경계에서 저장했다(발급기 3). C2 를 열 때는 사건이 없었다.
    let tree = tree();
    let records = records(&[change(1, 2, 3, kept(5, 2, 3))]);
    assert_eq!(exit_and_policy(2, &records, &tree), (2, Before));
    assert_eq!(exit_and_policy(3, &records, &tree), (3, applied(1, ScopeChange, 1, 3)));
    // 사건이 없으면 B(C2)·S(C3) 은 Exit 의 확정 정책을 유도하지 않는다. 그 값은 이 유도와 다르고, 이 유도가 바꾸지 않는다.
    let none = no_records();
    assert_eq!(boundary_of(2, &none, &tree).policy(), BoundaryPolicy::ExitPolicyNotDerived);
    let opening_world = select_opening_world(c(3), &none.ledger, &none.links, &none.facts, &tree, &registry(12)).expect("S(C3) 를 고른다");
    assert_eq!(opening_world.policy(), WorldSourcePolicy::ExitPolicyNotDerived);
    assert_eq!(exit_and_policy(2, &none, &tree), (2, Before));
}

// ── 증명하지 않는 것: T 의 개방 ────────────────────────────────────────────

#[test]
fn the_opening_of_t_is_not_checked() {
    // C3 은 실제 전이로 C2 의 Exit A2 에서 열렸지만, 건넨 기록에서는 C2 에서 보관한 M2(P2, A5)가 C3 이전의 기준점이다.
    // 이 사례가 직접 보이는 것은 검증 ① 의 Entry 불일치(실제 A2, S(C3) 의 세계 A5)다. S(C3) 의 정책과 P(C3) 은 모두 P2 이므로 이 사례에서
    // 검증 ② 는 어긋나지 않는다. 이 유도는 ① 의 불일치를 보고도 거절하지 않고 §6.1 의 정의대로 A3·P(C3) = P2 를 낸다 — 같은 이유로
    // 검증 ②·T9 도 이 유도가 검사하지 않는다(그것을 어기는 사례는 이 시험이 만들지 않는다).
    let tree = tree();
    let records = records(&[start(), change(2, 2, 3, kept(5, 2, 3))]);
    let opening_world =
        select_opening_world(c(3), &records.ledger, &records.links, &records.facts, &tree, &registry(12)).expect("S(C3) 를 고른다");
    let check = opening_world.check_entry();
    assert_eq!((check.verdict(), check.entry(), check.expected()), (EntryVerdict::Differs, a(2), a(5)));
    assert_eq!(opening_world.policy(), WorldSourcePolicy::PolicyAfter { event: m(2), policy: p(2) });
    assert_eq!(opening_of(3, &records, &tree).selection(), applied(2, ScopeChange, 2, 3), "P(C3) 도 P2 다");
    assert_eq!(exit_and_policy(3, &records, &tree), (3, applied(2, ScopeChange, 2, 3)));
}

// ── 대상: Interview·열린 Experiment·없는/모호한 대상·Exit 누락 ──────────────

#[test]
fn an_interview_is_out_of_scope_open_or_closed() {
    let out = |cycle| [ExperimentExitProblem::InterviewOutOfScope { cycle: c(cycle), chain: ch(1) }];
    // 닫힌 Interview C1 — P(C1) 은 고를 수 있지만 그것을 Interview 의 확정 정책·성공으로 쓰지 않는다.
    let tree = tree();
    assert!(select_opening_policy(c(1), &PolicyLedger::empty(), &EventPolicyLinks::empty(), &[], &tree, &registry(12)).is_ok());
    assert_eq!(refused(1, &no_records(), &tree), out(1));
    assert_eq!(refused(1, &records(&[start()]), &tree), out(1));
    // 열린 Interview 와 다른 체인의 Interview 도 같다.
    assert_eq!(refused(2, &no_records(), &line(2)), out(2));
    assert_eq!(
        refused(5, &no_records(), &world()),
        [ExperimentExitProblem::InterviewOutOfScope { cycle: c(5), chain: ch(2) }]
    );
}

#[test]
fn an_open_experiment_has_no_exit_and_is_not_given_its_parents() {
    // C4 는 열린 Experiment 다 — 부모 C2 의 Exit A2 나 Entry A2 로 대신하지 않는다.
    assert_eq!(
        refused(4, &records(&[start()]), &tree()),
        [ExperimentExitProblem::Experiment(ExperimentTargetProblem::NotClosed { cycle: c(4), chain: ch(1) })]
    );
}

#[test]
fn a_missing_target_is_reported_once() {
    // P(T) 와 이 선택의 조회가 같은 소유 규칙으로 같은 사실을 찾는다 — 한 번만 담는다.
    let not_found = |cycle| [ExperimentExitProblem::Target(TargetProblem::NotFound { cycle: c(cycle) })];
    assert_eq!(refused(9, &no_records(), &tree()), not_found(9));
    assert_eq!(refused(9, &records(&[start()]), &tree()), not_found(9));
    assert_eq!(refused(2, &no_records(), &[]), not_found(2));
}

fn ambiguous_lookup(problem: &ExperimentExitProblem) -> bool {
    matches!(problem, ExperimentExitProblem::Inputs(found)
        if matches!(found.as_slice(), [EventInputsProblem::CycleRefs(refs)]
            if refs.iter().any(|problem| matches!(problem, CycleRefProblem::AmbiguousLookup(_)))))
}

#[test]
fn a_target_owned_by_two_chain_entries_is_ambiguous_and_3j_is_reported_too() {
    let mut chains = tree();
    chains.push(chains[0].clone());
    let problems = refused(2, &no_records(), &chains);
    assert_eq!(problems.len(), 2, "{problems:#?}");
    assert!(ambiguous_lookup(&problems[0]), "{problems:#?}");
    assert_eq!(problems[1], ExperimentExitProblem::Target(TargetProblem::SeveralOwners { cycle: c(2), chains: vec![ch(1), ch(1)] }));
}

#[test]
fn a_target_whose_chain_name_is_repeated_is_ambiguous() {
    // chain:C1 이라는 이름의 다른 항목이 C7 을 지닌다. C2 를 소유한 항목은 하나지만 그 이름의 기록이 둘이다.
    let mut chains = tree();
    chains.push(Chain::restore(ch(1), x1(), a(1), None, Cycles::start_at(rules(), x1(), a(1), 7)));
    let problems = refused(2, &no_records(), &chains);
    assert_eq!(problems.len(), 2, "{problems:#?}");
    assert!(ambiguous_lookup(&problems[0]), "{problems:#?}");
    assert_eq!(problems[1], ExperimentExitProblem::Target(TargetProblem::OwnerChainRepeated { cycle: c(2), chain: ch(1), count: 2 }));
}

#[test]
fn a_closed_experiment_without_an_exit_is_refused_without_a_substitute() {
    // 복원 구조: 닫힌 실패 실험 C3 의 Exit 칸을 비운 상태를 Cycles::restore 가 받아들인다(되돌아간 대상 C2 의 Exit 을 비우면 C4 의
    // 갈래 검사가 거절한다). 현행 Project load 의 ②(닫힌 사이클의 Exit 실재)는 이 구조를 거절한다.
    let chains = vec![restored(&tree()[0], |nodes| nodes[at(3)].exit = None).expect("Cycles::restore 가 받아들인다")];
    let verified: Vec<SnapshotRef> =
        chains[0].cycles().nodes()[at(3)].steps().nodes().iter().filter_map(|node| node.snapshot).collect();
    assert_eq!(verified, [a(3)], "Verify 의 세계는 그대로 있다 — 그것으로 Exit 을 대신하지 않는다");
    assert_eq!(
        refused(3, &no_records(), &chains),
        [ExperimentExitProblem::Experiment(ExperimentTargetProblem::ClosedWithoutExit { cycle: c(3), chain: ch(1) })]
    );
    // 같은 구조의 C2 는 Exit 이 있으므로 그대로 유도한다.
    assert_eq!(exit_and_policy(2, &no_records(), &chains), (2, Before));
}

#[test]
fn an_exit_missing_from_the_registry_is_refused() {
    // 등록부는 A1 만 안다. C2 의 Exit A2 가 조회되지 않는다.
    assert_eq!(
        select_with(2, &no_records(), &tree(), &registry(1)).expect_err("거절해야 한다"),
        [ExperimentExitProblem::ExitNotInRegistry { cycle: c(2), chain: ch(1), world: a(2) }]
    );
    // A1·A2 를 아는 등록부 — C2 는 유도하고 C3 의 A3 는 거절한다.
    assert_eq!(select_with(2, &no_records(), &tree(), &registry(2)).expect("유도한다").world(), a(2));
    assert_eq!(
        select_with(3, &no_records(), &tree(), &registry(2)).expect_err("거절해야 한다"),
        [ExperimentExitProblem::ExitNotInRegistry { cycle: c(3), chain: ch(1), world: a(3) }]
    );
}

// ── 입력 오류와 대상 오류의 보존 ───────────────────────────────────────────

fn only_inputs(problems: &[ExperimentExitProblem]) -> &[EventInputsProblem] {
    match problems {
        [ExperimentExitProblem::Inputs(found)] => found,
        other => panic!("입력 갈래 하나만 와야 한다: {other:#?}"),
    }
}

#[test]
fn input_problems_are_kept_verbatim_and_with_every_target_problem() {
    let tree = tree();
    let mut cut = records(&[start(), change(2, 2, 3, kept(5, 2, 3))]);
    cut.facts.truncate(1);
    // 대상이 정상이면 입력 갈래 하나뿐이며 P(T) 의 입력 실패 그대로다.
    let problems = refused(2, &cut, &tree);
    assert!(matches!(only_inputs(&problems), [EventInputsProblem::Correspondence(_)]), "{problems:#?}");
    let from_opening = select_opening_policy(c(2), &cut.ledger, &cut.links, &cut.facts, &tree, &registry(12)).expect_err("P(T) 도 거절한다");
    assert_eq!(from_opening, [OpeningPolicyProblem::Inputs(only_inputs(&problems).to_vec())]);
    // Interview·열린 Experiment·없는 대상·Exit 없는 대상과 함께 오면 둘 다 순서대로 온다. 대상 문제는 한 번이다.
    let second = |target, chains: &[Chain]| {
        let problems = refused(target, &cut, chains);
        assert_eq!(problems.len(), 2, "{problems:#?}");
        assert!(matches!(&problems[0], ExperimentExitProblem::Inputs(_)), "{problems:#?}");
        problems[1].clone()
    };
    assert_eq!(second(1, &tree), ExperimentExitProblem::InterviewOutOfScope { cycle: c(1), chain: ch(1) });
    assert_eq!(second(4, &tree), ExperimentExitProblem::Experiment(ExperimentTargetProblem::NotClosed { cycle: c(4), chain: ch(1) }));
    assert_eq!(second(9, &tree), ExperimentExitProblem::Target(TargetProblem::NotFound { cycle: c(9) }));
    let without_exit = vec![restored(&tree[0], |nodes| nodes[at(3)].exit = None).expect("Cycles::restore 가 받아들인다")];
    assert_eq!(
        second(3, &without_exit),
        ExperimentExitProblem::Experiment(ExperimentTargetProblem::ClosedWithoutExit { cycle: c(3), chain: ch(1) })
    );
    // 대상 Exit 의 등록부 누락도 입력 검사와 무관하게 평가해 함께 돌려준다 — 입력 오류 전체가 먼저, 그 뒤에 Exit 누락이다.
    // 등록부는 A1 만 알고 cut 의 기준점은 A1 뿐이므로 입력 실패는 대응(Correspondence) 하나 그대로다.
    let problems = select_with(2, &cut, &tree, &registry(1)).expect_err("거절해야 한다");
    let from_opening = select_opening_policy(c(2), &cut.ledger, &cut.links, &cut.facts, &tree, &registry(1)).expect_err("P(T) 도 거절한다");
    let [OpeningPolicyProblem::Inputs(inputs)] = from_opening.as_slice() else { panic!("P(T) 는 입력 갈래 하나: {from_opening:#?}") };
    assert!(matches!(inputs.as_slice(), [EventInputsProblem::Correspondence(_)]), "{inputs:#?}");
    assert_eq!(
        problems,
        [
            ExperimentExitProblem::Inputs(inputs.clone()),
            ExperimentExitProblem::ExitNotInRegistry { cycle: c(2), chain: ch(1), world: a(2) },
        ]
    );
}

#[test]
fn a_baseline_and_the_target_exit_both_missing_from_the_registry_are_both_kept() {
    // 등록부는 A1 만 안다. M2 의 기준점 A5 누락은 P(T) 안의 3j(3e)가, C3 의 Exit A3 누락은 이 선택의 대상 조회가 찾는다.
    // 입력 오류를 Exit 누락으로 바꾸지 않고, Exit 누락을 입력 오류 뒤에 숨기지 않는다.
    let tree = tree();
    let records = records(&[start(), change(2, 2, 3, kept(5, 2, 3))]);
    let problems = select_with(3, &records, &tree, &registry(1)).expect_err("거절해야 한다");
    let from_opening =
        select_opening_policy(c(3), &records.ledger, &records.links, &records.facts, &tree, &registry(1)).expect_err("P(T) 도 거절한다");
    let [OpeningPolicyProblem::Inputs(inputs)] = from_opening.as_slice() else { panic!("P(T) 는 입력 갈래 하나: {from_opening:#?}") };
    match inputs.as_slice() {
        [EventInputsProblem::CycleRefs(found)] => {
            assert!(found.iter().any(|problem| matches!(problem, CycleRefProblem::Point(_))), "{found:#?}")
        }
        other => panic!("{other:#?}"),
    }
    assert_eq!(
        problems,
        [
            ExperimentExitProblem::Inputs(inputs.clone()),
            ExperimentExitProblem::ExitNotInRegistry { cycle: c(3), chain: ch(1), world: a(3) },
        ]
    );
}

#[test]
fn a_baseline_missing_from_the_registry_is_refused_by_3j() {
    let records = records(&[start(), change(2, 2, 3, kept(5, 2, 3))]);
    let problems = select_with(3, &records, &tree(), &registry(4)).expect_err("3e 가 거절한다");
    match only_inputs(&problems) {
        [EventInputsProblem::CycleRefs(found)] => {
            assert!(found.iter().any(|problem| matches!(problem, CycleRefProblem::Point(_))), "{found:#?}")
        }
        other => panic!("{other:#?}"),
    }
}

// ── 3j 의 판단하지 않음과 잘린 이력의 한계 ─────────────────────────────────

#[test]
fn the_unjudged_parts_of_3j_stay_visible() {
    let tree = tree();
    let records = records(&[start(), change(2, 2, 3, kept(5, 2, 3))]);
    let agreed = check_event_inputs(&records.ledger, &records.links, &records.facts, &tree, &registry(12)).expect("3j 가 받아들인다");
    let found = derived(3, &records, &tree);
    assert_eq!(found.inputs(), &agreed);
    assert_eq!(
        found.inputs().cycle_refs().not_judged(),
        [(m(1), CycleField::AfterCycleRef, RefReading::NullNotJudged), (m(1), CycleField::BaselineAfterCycleRef, RefReading::NullNotJudged)]
    );
    assert_eq!(found.inputs().cycle_refs().chains_not_looked_up(), [m(1)]);
}

#[test]
fn inputs_cut_together_are_not_detected_and_are_chosen_within() {
    // 원장·정책 연결·사이클 입력을 함께 잘라낸 입력은 3j 가 알아내지 못한다 — 잘린 이력 안에서 P(T) 를 고르며 완전하다고 주장하지 않는다.
    let tree = tree();
    let rows = [start(), change(2, 1, 2, kept(5, 1, 2))];
    assert_eq!(exit_and_policy(2, &records(&rows), &tree), (2, applied(2, ScopeChange, 2, 2)));
    assert_eq!(exit_and_policy(2, &records(&rows[..1]), &tree), (2, applied(1, Start, 1, 1)));
    assert_eq!(exit_and_policy(2, &no_records(), &tree), (2, Before), "잘린 이력 안의 정책 채택 전이다");
}

#[test]
fn a_waiting_event_still_gives_the_policy_it_applied() {
    // C1 경계의 M2 는 보관 대기다(기준점 없음). 승인 저장으로 이미 적용됐으므로 그 뒤에 연 C2 의 P 는 P2 다(§5.2, §6.2 의 P).
    // 보관 대기 뒤에 실험을 열었다는 사실(T9 위반)은 이 유도가 판단하지 않는다.
    let tree = tree();
    let records = records(&[start(), change(2, 1, 2, Base::Waiting)]);
    assert_eq!(exit_and_policy(2, &records, &tree), (2, applied(2, ScopeChange, 2, 2)));
}

// ── 순수성과 한계 ──────────────────────────────────────────────────────────

#[test]
fn inputs_are_left_unchanged_and_the_answer_is_repeatable() {
    let tree = tree();
    let records = records(&changed_after_opening());
    let registry = registry(12);
    let snapshot = || format!("{:?}{:?}{:?}{:?}{:?}", records.ledger, records.links, records.facts, tree, registry);
    let before = snapshot();
    let first = select_with(3, &records, &tree, &registry);
    let second = select_with(3, &records, &tree, &registry);
    assert_eq!(snapshot(), before, "입력을 바꿨다");
    assert_eq!(first, second);
    let first = first.expect("유도한다");
    assert_eq!((first.world(), first.policy()), (a(3), applied(2, ScopeChange, 2, 3)));
}

#[test]
fn every_problem_reads_and_the_list_names_what_it_does_not_show() {
    let problems = [
        ExperimentExitProblem::Inputs(Vec::new()),
        ExperimentExitProblem::Target(TargetProblem::NotFound { cycle: c(9) }),
        ExperimentExitProblem::Target(TargetProblem::SeveralOwners { cycle: c(2), chains: vec![ch(1), ch(1)] }),
        ExperimentExitProblem::Target(TargetProblem::OwnerChainRepeated { cycle: c(2), chain: ch(1), count: 2 }),
        ExperimentExitProblem::InterviewOutOfScope { cycle: c(1), chain: ch(1) },
        ExperimentExitProblem::Experiment(ExperimentTargetProblem::NotClosed { cycle: c(4), chain: ch(1) }),
        ExperimentExitProblem::Experiment(ExperimentTargetProblem::ClosedWithoutExit { cycle: c(3), chain: ch(1) }),
        ExperimentExitProblem::ExitNotInRegistry { cycle: c(2), chain: ch(1), world: a(2) },
    ];
    assert!(problems.iter().all(|problem| !problem.to_string().is_empty()));
    for needle in [
        "함께 잘라낸",
        "not_judged",
        "chains_not_looked_up",
        "§7.5",
        "검증 ①",
        "②",
        "T9",
        "B(T)",
        "ExitPolicyNotDerived",
        "Interview",
        "manifest",
        "T8",
        "LedgerStart",
        "O-2",
        "O-3",
    ] {
        assert!(EXPERIMENT_EXIT_DOES_NOT_SHOW.iter().any(|line| line.contains(needle)), "{needle}");
    }
}
