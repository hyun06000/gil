//! 한 시점에 건넨 **정책 연결 입력**(3c)과 **사이클 입력**(3h)이 같은 관리 사건 목록을 나타내는지의 대응 검사 — 구현 순서 3번 안의
//! 작은 후속(편의상 「3j」).
//!
//! 「3j」는 이 PR 의 편의상 이름이다. **실제 원장에서 빠짐없이 뽑아 같은 입력을 건네는 연결 계층의 완료가 아니고, 3번 전체의 완료도
//! 아니다.** 정본은 [공유 저장·전이 계약](../../../spec/GIL_Storage_Transition_Contract_v0.1.md) §5.1 의 단일 `events` 기록과 §5.7 의
//! T1·T3·T4·T6·T7 이다. 범위 설명은 [구현 순서](../../../spec/GIL_Implementation_Order_2026-11-19.md)의 「3j」 기록에 있다.
//!
//! ```text
//! PolicyLedger + EventPolicyLinks ─── check_policy_links (3c, 그대로) ──────────▶ PolicyLinks 갈래
//! EventCycleFacts* + &[Chain] + 등록부 ─ check_cycle_refs (3h, 그대로) ────────────▶ CycleRefs 갈래
//! EventPolicyLinks.events() ↔ EventCycleFacts* — 자리마다 이름, 같은 이름이면 종류, 그리고 개수 ─▶ Correspondence 갈래
//!                                                    │
//!                                 check_event_inputs ─▶ EventInputsAgree | [EventInputsProblem]
//! ```
//!
//! # 왜 필요한가
//!
//! 3c 와 3h 는 같은 사건 원장의 **서로 다른 칸**을 따로 건네받는다. 각 검사는 자기 입력 안의 규칙만 본다. 그래서 정책 연결 목록의 `M2` 는
//! `scope_change` 인데 사이클 입력의 `M2` 는 `restore_result` 이거나, 정책 연결 목록은 `M1`–`M3` 인데 사이클 입력은 `M1`–`M2` 뿐이어도 두
//! 검사는 각각 통과할 수 있다. 정본에서 두 입력은 한 `events` 목록(§5.1)의 같은 기록에서 나온 값이므로, 함께 받아들이려면 사건 수·자리별
//! 이름·같은 사건의 종류가 맞아야 한다. 이 검사는 그 대응만 더한다.
//!
//! # 새 규칙을 만들지 않는다
//!
//! - 이름 꼴(`M1` 부터 자리와 같음)과 사건 발급기는 3c 의 [`EventPolicyLinks`] 생성자가, 사이클 입력의 이름 자리는 3h 가 이미 본다. 이
//!   검사는 「같은 자리의 두 이름이 같은가」만 견주며 이름 규칙을 다시 정의하지 않는다.
//! - `next_event_id`(사건 발급기)와 `next_cycle_id`(Cycle 발급기)는 서로 다른 발급기다. 견주지 않는다.
//! - 대응은 자리로만 본다. 정렬·중복 제거·이름 다시 매기기·짧은 쪽에 맞춘 비교로 입력을 보정하지 않는다. 이름이 다른 자리는 같은 사건이라고
//!   볼 근거가 없으므로 그 자리의 종류를 견주지 않는다. 길이가 다르면 그 사실과 짝이 없는 사건을 따로 돌려준다. 빠진 사건을 만들거나 한쪽
//!   종류로 다른 쪽을 덮지 않는다.
//! - 3c·3h 의 실패는 그대로 담고, 대응은 그 결과와 무관하게 평가한다. 어느 갈래의 성공도 다른 갈래의 실패를 덮지 않는다.
//!
//! # 「판단하지 않음」은 그대로 남는다
//!
//! 성공 값은 3c 의 [`PolicyLinksAgree`] 와 3h 의 [`CycleRefsAgree`] 를 그대로 지닌다 — 3h 의 `null`·칸 없음(`not_judged`)과 `chain_ref`
//! 미조회(`chains_not_looked_up`)가 숨지 않고, 이 검사가 그것을 승인으로 바꾸지도 않는다. 검사하지 않은 것은
//! [`EVENT_INPUTS_DO_NOT_SHOW`] 에 있다.

use std::fmt;

use super::super::SnapshotRegistry;
use super::PolicyLedger;
use super::event_cycles::{CycleRefProblem, CycleRefsAgree, EventCycleFacts, check_cycle_refs};
use super::event_links::{EventKind, EventName, EventPolicyLinks, LinkProblem, PolicyLinksAgree, check_policy_links};
use crate::Chain;

/// 이 검사가 성공해도 **보이지 않은** 것.
pub(crate) const EVENT_INPUTS_DO_NOT_SHOW: [&str; 10] = [
    "입력이 실제 사건 원장에서 빠짐없이 추출됐는가 — 실제 상태 추출과 제품 연결은 구현하지 않았다. 후속 연결 계층의 책임",
    "두 목록과 Chain 목록·등록부가 실제로 같은 상태에서 나왔는가 — 건넨 값끼리의 대응만 본다",
    "두 목록을 함께 잘라냈거나 함께 바꿔 만든 입력 — 두 목록이 서로 맞으면 구별하지 못한다",
    "같은 이름·같은 종류를 가진 서로 다른 원본 기록 — 사건 수·이름·종류만 견주므로 구별하지 못한다",
    "정책 원문 객체의 실재(T2)와 모니터의 실제 승인 응답 — 3c 의 한계 그대로다",
    "두 시점 사이의 보존 — 정책 연결 칸은 3d, 기준점은 3f·3g, 승인 당시 세 칸은 3i 의 몫이며 사건 기록 전체의 보존은 아무 검사도 보지 않는다",
    "null·칸 없음의 종류별 허용, review_cycle_ref 의 필수 여부와 world_before_ref 의 관계 — 3h 의 판단하지 않음은 성공 값의 cycle_refs() 에 그대로 남는다",
    "현재 세계 계산(§6), 다음 실험을 열 자격(T9), 실제 승인·보관·복원과 그 권한",
    "BaseUndecided 와 사용자 편집, 저장 전환 동의(O-1), 즉시 보관 중단 해제(O-2), Chain 종료와 미완료 사건의 이관(O-3), 등록부 매핑의 보존 의미",
    "형식 10/11 의 제품 읽기·쓰기와 활성화 — 이 검사를 부르는 제품 경로가 없다",
];

// ── 입력 대응 ──────────────────────────────────────────────────────────────

/// 대응을 견주는 두 입력.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputSide {
    /// 3c 의 정책 연결 입력([`EventPolicyLinks`]).
    PolicyLinks,
    /// 3h 의 사이클 입력([`EventCycleFacts`] 목록).
    CycleFacts,
}

impl InputSide {
    fn name(self) -> &'static str {
        match self {
            InputSide::PolicyLinks => "정책 연결 입력",
            InputSide::CycleFacts => "사이클 입력",
        }
    }
}

/// 두 입력이 같은 사건 목록을 나타내지 않는 이유. 자리는 0 부터 센다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InputMismatch {
    /// 두 목록의 사건 수가 다르다.
    CountDiffers { policy_links: usize, cycle_facts: usize },
    /// 긴 쪽에만 있는 자리의 사건 — 다른 쪽에 같은 자리가 없다. 짧은 쪽에 맞춰 버리지 않고 드러낸다.
    Unmatched { side: InputSide, position: usize, event: EventName },
    /// 같은 자리의 사건 이름이 다르다. 그 자리의 종류는 견주지 않았다.
    NameDiffers { position: usize, policy_links: EventName, cycle_facts: EventName },
    /// 같은 사건의 종류가 다르다. 사이클 입력의 종류는 그 안의 3e `BaselineFacts` 에서 읽었다.
    KindDiffers { event: EventName, policy_links: EventKind, cycle_facts: EventKind },
}

impl fmt::Display for InputMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InputMismatch::CountDiffers { policy_links, cycle_facts } => write!(
                f,
                "정책 연결 입력의 사건은 {policy_links}개, 사이클 입력의 사건은 {cycle_facts}개다. 같은 사건 목록이라면 수가 같아야 한다"
            ),
            InputMismatch::Unmatched { side, position, event } => write!(
                f,
                "{}의 {}번째 사건 {event} 와 짝이 되는 사건이 다른 입력에 없다",
                side.name(),
                position + 1
            ),
            InputMismatch::NameDiffers { position, policy_links, cycle_facts } => write!(
                f,
                "{}번째 사건이 정책 연결 입력에서는 {policy_links}, 사이클 입력에서는 {cycle_facts} 다. 같은 자리의 사건 이름이 같아야 한다",
                position + 1
            ),
            InputMismatch::KindDiffers { event, policy_links, cycle_facts } => write!(
                f,
                "{event} 의 종류가 정책 연결 입력에서는 {}, 사이클 입력에서는 {} 다. 같은 사건의 종류는 하나다",
                policy_links.name(),
                cycle_facts.name()
            ),
        }
    }
}

/// 두 입력의 사건 수·자리별 이름·같은 사건의 종류를 견준다. 찾은 불일치를 모두 돌려준다 — 개수, 짝이 없는 사건, 자리 순서의 이름·종류.
fn correspondence(links: &EventPolicyLinks, facts: &[EventCycleFacts]) -> Vec<InputMismatch> {
    let links = links.events();
    let mut found = Vec::new();
    if links.len() != facts.len() {
        found.push(InputMismatch::CountDiffers { policy_links: links.len(), cycle_facts: facts.len() });
        let unmatched = links
            .iter()
            .enumerate()
            .skip(facts.len())
            .map(|(position, link)| (InputSide::PolicyLinks, position, link.name()))
            .chain(
                facts
                    .iter()
                    .enumerate()
                    .skip(links.len())
                    .map(|(position, fact)| (InputSide::CycleFacts, position, fact.event())),
            );
        found.extend(unmatched.map(|(side, position, event)| InputMismatch::Unmatched { side, position, event }));
    }
    for (position, (link, fact)) in links.iter().zip(facts).enumerate() {
        let (policy_name, cycle_name) = (link.name(), fact.event());
        if policy_name != cycle_name {
            found.push(InputMismatch::NameDiffers { position, policy_links: policy_name, cycle_facts: cycle_name });
            continue;
        }
        let (policy_kind, cycle_kind) = (link.kind(), fact.baseline().kind());
        if policy_kind != cycle_kind {
            found.push(InputMismatch::KindDiffers { event: policy_name, policy_links: policy_kind, cycle_facts: cycle_kind });
        }
    }
    found
}

// ── 검사 ───────────────────────────────────────────────────────────────────

/// 대응 검사가 실패한 갈래. 앞의 두 갈래는 기존 검사의 결과를 그대로 담는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EventInputsProblem {
    /// 정책 연결 입력 자체가 3c 를 통과하지 못했다.
    PolicyLinks(Vec<LinkProblem>),
    /// 사이클 입력 자체가 3h 를 통과하지 못했다.
    CycleRefs(Vec<CycleRefProblem>),
    /// 두 입력이 같은 사건 목록을 나타내지 않는다.
    Correspondence(Vec<InputMismatch>),
}

impl fmt::Display for EventInputsProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn listed<T: ToString>(items: &[T]) -> String {
            items.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
        }
        match self {
            EventInputsProblem::PolicyLinks(found) => write!(f, "정책 연결 입력이 맞지 않는다: {}", listed(found)),
            EventInputsProblem::CycleRefs(found) => write!(f, "사이클 입력이 맞지 않는다: {}", listed(found)),
            EventInputsProblem::Correspondence(found) => {
                write!(f, "두 입력이 같은 사건 목록을 나타내지 않는다: {}", listed(found))
            }
        }
    }
}

/// 건넨 정책 연결 입력이 3c 를, 사이클 입력이 3h 를 통과하고, 두 입력의 사건 수·자리별 이름·같은 사건의 종류가 대응한다 — **그것뿐이다.**
/// 보이지 않은 것은 [`EVENT_INPUTS_DO_NOT_SHOW`] 에 있다. 이 값을 만드는 문은 [`check_event_inputs`] 하나뿐이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EventInputsAgree {
    policy_links: PolicyLinksAgree,
    cycle_refs: CycleRefsAgree,
    events: Vec<(EventName, EventKind)>,
}

impl EventInputsAgree {
    /// 3c 의 성공 값 그대로.
    pub(crate) fn policy_links(&self) -> &PolicyLinksAgree {
        &self.policy_links
    }

    /// 3h 의 성공 값 그대로 — 그 판단하지 않음(`not_judged`·`chains_not_looked_up`)을 지닌다.
    pub(crate) fn cycle_refs(&self) -> &CycleRefsAgree {
        &self.cycle_refs
    }

    /// 두 입력에서 대응을 확인한 사건의 이름과 종류, 원장 순서대로. 같은 원본 기록이라는 증명이 아니다.
    pub(crate) fn events(&self) -> &[(EventName, EventKind)] {
        &self.events
    }
}

/// 한 시점에 건넨 (정책 원장, 정책 연결 입력)과 (사이클 입력, Chain 목록, 등록부)가 같은 사건 목록의 두 부분인지 본다. **순수 함수**다.
///
/// 3c 의 [`check_policy_links`] 와 3h 의 [`check_cycle_refs`] 를 그대로 부르고 그 실패를 갈래마다 담는다. 두 입력의 대응은 그 결과와 무관하게
/// 평가해 [`EventInputsProblem::Correspondence`] 로 돌려준다. 갈래 순서는 정책 연결, 사이클 입력, 대응이다. 셋이 모두 성립할 때만 성공한다.
/// 입력·체인·등록부를 바꾸지 않고 파일·상태를 읽거나 쓰지 않는다.
pub(crate) fn check_event_inputs(
    ledger: &PolicyLedger,
    links: &EventPolicyLinks,
    facts: &[EventCycleFacts],
    chains: &[Chain],
    registry: &SnapshotRegistry,
) -> Result<EventInputsAgree, Vec<EventInputsProblem>> {
    let mut problems = Vec::new();
    let policy = check_policy_links(ledger, links);
    if let Err(found) = &policy {
        problems.push(EventInputsProblem::PolicyLinks(found.clone()));
    }
    let cycles = check_cycle_refs(facts, chains, registry);
    if let Err(found) = &cycles {
        problems.push(EventInputsProblem::CycleRefs(found.clone()));
    }
    let mismatches = correspondence(links, facts);
    if !mismatches.is_empty() {
        problems.push(EventInputsProblem::Correspondence(mismatches));
    }

    match (problems.is_empty(), policy, cycles) {
        (true, Ok(policy_links), Ok(cycle_refs)) => {
            let events = links.events().iter().map(|link| (link.name(), link.kind())).collect();
            Ok(EventInputsAgree { policy_links, cycle_refs, events })
        }
        _ => Err(problems),
    }
}
