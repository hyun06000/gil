//! 정책 원장과 관리 사건의 **정책 연결** 검사 — 구현 순서 3번 안의 작은 후속(편의상 「3c」).
//!
//! 「3c」는 이 PR 의 편의상 이름이다. **3번 전체의 완료가 아니고, 계약 T1 전체나 T3–T13 전체의 완료도 아니다.**
//! 정본은 [공유 저장·전이 계약](../../../spec/GIL_Storage_Transition_Contract_v0.1.md) §5.1 의 사건 종류표와
//! §5.7 의 T1(사건 이름 부분)·T3·T4 다. 범위 설명은 [구현 순서](../../../spec/GIL_Implementation_Order_2026-11-19.md)의
//! 「3c」 진행 기록에 있다.
//!
//! ```text
//! EventPolicyLink* ─from_links─▶ EventPolicyLinks        T1 의 사건 이름·사건 발급기 부분
//!                                     │
//! PolicyLedger (3a) ──────────────────┴ check_policy_links ─▶ PolicyLinksAgree | [LinkProblem]
//!                                                            T3, T4, §5.1 종류표의 정책 열과 그 도출 조건
//! ```
//!
//! # 입력은 「뽑아 건넨 값」이다
//!
//! [`EventPolicyLink`] 는 미래의 전체 사건 기록에서 **정책 연결에 필요한 칸만** 뽑아 건넨 값이다 —
//! 이름·종류·`policy_before_ref`·`policy_after_ref`. 전체 사건 저장 꼴이 아니고 새로운 영구 기록 형식도 아니다.
//! 그래서 직렬화 자료형이 없고 `state.yaml` 에 연결되지 않는다. 세계·사이클·기준점·보관 시도·`chain_ref`·
//! `next_cycle_id` 는 받지 않는다. 사건 종류는 닫힌 세 값이며 임의 종류·payload 를 담을 자리가 없다.
//!
//! # 성공이 말하는 것과 말하지 않는 것
//!
//! [`PolicyLinksAgree`] 는 「**건넨** 원장과 사건 사이의 정책 연결이 일관된다」뿐이다. 사용자 승인, 정책의
//! 실제 적용, 사건 실행의 성공, 기록 전체의 유효성을 말하지 않는다. 입력이 실제 원장에서 빠짐없이 뽑혔는지는
//! 이 검사를 부를 후속 연결 계층의 책임이다. 검사하지 않은 조건은 [`LINKS_DO_NOT_SHOW`] 에 있다.
//!
//! # 계약에 적힌 조건과 도출한 조건
//!
//! - 적힌 것: T1 사건 이름(`M1` 부터 빈틈없이 오름차순, 발급기보다 작다), T3(첫 사건의 before 는 없음, 다음 사건의
//!   before 는 앞 사건의 after), T4(원장의 각 판은 정확히 한 `start`·`scope_change` 의 after, 마지막 정책 = 마지막
//!   사건의 after), §5.1 종류표(`start` 없음 → 새 판, `scope_change` 앞 판 → 새 판·첫 채택은 없음 → P1,
//!   `restore_result` 같은 판 → 같은 판), 초기 상태 `policies: []`·`events: []`.
//! - 도출한 것: 사건 발급기 = 사건 수 + 1(3a 의 정책 발급기와 같은 기존 규칙), 모든 사건의 after 에 판이 있다(종류표),
//!   정책 참조는 건넨 원장의 판이다, `start` 는 첫 사건에서만 나온다(종류표 + T3), k번째로 새 판을 도입하는 사건의
//!   after 는 `Pk` 다(추가만 하는 두 원장 + 새 판과 그 사건의 한 번 저장(§5.2) + 저장된 모든 상태의 T4).
//!
//! 같은 원문을 가진 서로 다른 판은 이름으로만 가른다. 이 검사는 판의 원문 주소를 보지 않는다.

use std::fmt;

use super::{PolicyLedger, PolicyName};
use crate::refs::RefSyntaxError;

/// 정책 연결 검사가 성공해도 **보이지 않은** 것.
pub(crate) const LINKS_DO_NOT_SHOW: [&str; 11] = [
    "입력이 실제 전체 원장·사건 기록에서 빠짐없이 뽑혔는가 — 후속 연결 계층의 책임",
    "모니터 승인 응답과 proposal_digest 의 연결, 사용자 승인 사실",
    "정책의 실제 적용과 사건 실행의 성공",
    "사건의 세계·사이클 참조와 경계 — T7 의 after_cycle_ref·next_cycle_id, T8 의 world_before_ref, T9",
    "기준점 보관·보관 시도·대체·재시도 — T5·T6",
    "현재 세계와 다음 실험의 Entry — 계약 §6",
    "즉시 보관 중단 표식 — T12",
    "Chain 종료와 다음 Chain 으로의 사건 이관 — 계약 §12 O-3 미결",
    "정책 파일 동기화 실행 — T13",
    "restore_result 가 정책이 달라진 뒤 과거 경계에서만 나오는가 — 계약 §6.4, 세계·사이클 필요",
    "사건 기록의 추가 전후 비교(과거 사건의 보존) — 한 시점만 보므로 볼 수 없다. 정책 연결 칸의 보존만 link_history 가 두 시점으로 본다",
];

// ── 사건의 이름과 종류 ─────────────────────────────────────────────────────

/// 관리 사건의 **이름** — 기록 자신의 칸에는 bare `M1`, 남이 가리킬 때는 `event:M1`.
///
/// 수의 규칙(선행 0 없음, 0 없음)은 [`crate::refs`] 의 것을 그대로 쓴다. `memory:M11` 과 글자가 같으므로
/// 종류 없이 섞어 쓰지 않는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct EventName(u32);

impl EventName {
    pub(crate) const KIND: &'static str = "event";
    pub(crate) const LETTER: &'static str = "M";

    /// 수 하나로 만든다. `M0` 은 없다.
    pub(crate) fn new(number: u32) -> Option<EventName> {
        (number != 0).then_some(EventName(number))
    }

    /// bare 이름(`M1`)을 읽는다 — 뽑아 건넬 계층이 같은 수 규칙을 다시 적지 않게.
    pub(crate) fn parse_bare(text: &str) -> Result<EventName, RefSyntaxError> {
        if text.chars().any(char::is_whitespace) {
            return Err(RefSyntaxError::Whitespace { text: text.to_string() });
        }
        crate::refs::id_of(text, Self::LETTER, false).map(EventName)
    }

    pub(crate) fn number(self) -> u32 {
        self.0
    }

    pub(crate) fn id(self) -> String {
        format!("{}{}", Self::LETTER, self.0)
    }
}

impl fmt::Display for EventName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}{}", Self::KIND, Self::LETTER, self.0)
    }
}

/// 정본에 있는 관리 사건 종류 — **닫힌 집합**(계약 §5.1). 모르는 종류를 담을 자리가 없다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum EventKind {
    /// 새 프로젝트의 최초 정책: 없음 → 새 판.
    Start,
    /// 사이클 경계의 범위 변경: 앞 판 → 새 판. 기존 프로젝트의 첫 채택(없음 → P1)도 이 종류다.
    ScopeChange,
    /// 정책이 달라진 뒤 과거 경계로 되돌아간 결과 세계: 같은 판 → 같은 판.
    RestoreResult,
}

impl EventKind {
    /// 계약 §5.1 의 종류 이름.
    pub(crate) fn name(self) -> &'static str {
        match self {
            EventKind::Start => "start",
            EventKind::ScopeChange => "scope_change",
            EventKind::RestoreResult => "restore_result",
        }
    }

    /// 종류 이름을 읽는다. 세 이름 말고는 `None` — 임의 종류를 받지 않는다.
    pub(crate) fn from_name(text: &str) -> Option<EventKind> {
        [EventKind::Start, EventKind::ScopeChange, EventKind::RestoreResult]
            .into_iter()
            .find(|kind| kind.name() == text)
    }

    /// 이 종류의 사건이 판을 **도입**하는가(T4 의 `start`·`scope_change`).
    fn introduces(self) -> bool {
        matches!(self, EventKind::Start | EventKind::ScopeChange)
    }
}

/// 사건 하나에서 뽑아 건넨 정책 연결 칸 — 이름·종류·앞뒤 정책뿐이다.
///
/// `after` 가 `Option` 인 것은 뽑아 건넨 값에 판이 빠진 경우를 검사가 **거절하기 위해서**다. 정본의 세 종류는 모두
/// after 에 판이 있다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EventPolicyLink {
    name: EventName,
    kind: EventKind,
    before: Option<PolicyName>,
    after: Option<PolicyName>,
}

impl EventPolicyLink {
    pub(crate) fn new(
        name: EventName,
        kind: EventKind,
        before: Option<PolicyName>,
        after: Option<PolicyName>,
    ) -> EventPolicyLink {
        EventPolicyLink { name, kind, before, after }
    }

    pub(crate) fn name(&self) -> EventName {
        self.name
    }

    pub(crate) fn kind(&self) -> EventKind {
        self.kind
    }

    pub(crate) fn before(&self) -> Option<PolicyName> {
        self.before
    }

    pub(crate) fn after(&self) -> Option<PolicyName> {
        self.after
    }
}

// ── 사건 이름의 꼴 (T1 의 사건 부분) ──────────────────────────────────────

/// **이름 꼴이 맞는** 사건 연결 목록과 사건 발급기.
///
/// 만들어졌다면 T1 의 사건 이름 부분이 참이다: `M1` 부터 빈틈없이 오름차순이고 발급기는 마지막 이름 다음이다.
/// 발급기 = 사건 수 + 1 은 [`PolicyLedger`] 의 정책 발급기와 같은 규칙이다. 정책 연결은 아직 보지 않았다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EventPolicyLinks {
    next_event_id: u32,
    events: Vec<EventPolicyLink>,
}

impl EventPolicyLinks {
    /// 사건이 하나도 없다 — `next_event_id: 1`, `events: []`.
    pub(crate) fn empty() -> EventPolicyLinks {
        EventPolicyLinks { next_event_id: 1, events: Vec::new() }
    }

    pub(crate) fn from_links(
        next_event_id: u32,
        events: Vec<EventPolicyLink>,
    ) -> Result<EventPolicyLinks, EventNamesError> {
        for (position, link) in events.iter().enumerate() {
            let expected = u32::try_from(position)
                .ok()
                .and_then(|at| at.checked_add(1))
                .ok_or(EventNamesError::Exhausted)?;
            if link.name.number() != expected {
                let repeated = events[..position].iter().any(|seen| seen.name == link.name);
                return Err(match repeated {
                    true => EventNamesError::DuplicateName { name: link.name, position },
                    false => EventNamesError::OutOfSequence {
                        found: link.name,
                        expected: EventName(expected),
                        position,
                    },
                });
            }
        }
        let wanted = u32::try_from(events.len())
            .ok()
            .and_then(|count| count.checked_add(1))
            .ok_or(EventNamesError::Exhausted)?;
        if next_event_id != wanted {
            return Err(EventNamesError::WrongIssuer { found: next_event_id, wanted });
        }
        Ok(EventPolicyLinks { next_event_id, events })
    }

    pub(crate) fn next_event_id(&self) -> u32 {
        self.next_event_id
    }

    pub(crate) fn events(&self) -> &[EventPolicyLink] {
        &self.events
    }
}

/// 사건 이름의 꼴이 맞지 않는 이유. **하나라도 있으면 목록을 세우지 않는다.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EventNamesError {
    /// 같은 이름이 두 번 실렸다.
    DuplicateName { name: EventName, position: usize },
    /// 빈틈이 있거나 순서가 바뀌었다.
    OutOfSequence { found: EventName, expected: EventName, position: usize },
    /// 발급기가 마지막 이름 다음이 아니다.
    WrongIssuer { found: u32, wanted: u32 },
    /// 이름을 더 발급할 수 없다. 감싸 돌지 않는다.
    Exhausted,
}

impl fmt::Display for EventNamesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventNamesError::DuplicateName { name, position } => {
                write!(f, "사건 목록의 {}번째 사건이 앞의 이름 {name} 을 다시 쓴다", position + 1)
            }
            EventNamesError::OutOfSequence { found, expected, position } => write!(
                f,
                "사건 목록의 {}번째 이름이 {found} 이다. 빈틈없는 순서라면 {expected} 이어야 한다",
                position + 1
            ),
            EventNamesError::WrongIssuer { found, wanted } => {
                write!(f, "next_event_id 가 {found} 이다. 마지막 사건 다음인 {wanted} 이어야 한다")
            }
            EventNamesError::Exhausted => f.write_str("사건 이름을 더 발급할 수 없다"),
        }
    }
}

impl std::error::Error for EventNamesError {}

// ── 정책 연결 ──────────────────────────────────────────────────────────────

/// 정책 참조가 적힌 칸.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinkField {
    Before,
    After,
}

impl LinkField {
    fn name(self) -> &'static str {
        match self {
            LinkField::Before => "policy_before_ref",
            LinkField::After => "policy_after_ref",
        }
    }
}

/// 건넨 원장과 사건 사이의 정책 연결이 맞지 않는 이유.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LinkProblem {
    /// 판은 있는데 사건이 없다 — 어느 판도 도입되지 않았다(T4).
    PoliciesWithoutEvents { policies: usize },
    /// 사건은 있는데 판이 없다 — 어느 사건의 after 도 원장의 판이 아니다(T4).
    EventsWithoutPolicies { events: usize },
    /// 사건의 after 에 판이 없다(§5.1 종류표: 세 종류 모두 after 가 판이다).
    MissingAfter { event: EventName },
    /// 정책 참조가 건넨 원장에 없는 판을 가리킨다.
    UnknownPolicy { event: EventName, field: LinkField, policy: PolicyName },
    /// 첫 사건의 before 가 없음이 아니다(T3).
    FirstBeforePresent { event: EventName, before: PolicyName },
    /// 사건의 before 가 앞 사건의 after 와 다르다(T3).
    BrokenChain { event: EventName, before: Option<PolicyName>, previous_after: Option<PolicyName> },
    /// `start` 가 첫 사건이 아니다(§5.1 종류표의 「없음 →」과 T3).
    StartNotFirst { event: EventName },
    /// `restore_result` 의 before 와 after 가 다르다(§5.1 종류표: 같은 판 → 같은 판).
    RestoreChangesPolicy { event: EventName, before: Option<PolicyName>, after: PolicyName },
    /// 이미 도입된 판을 `start`·`scope_change` 가 다시 도입한다(T4: 정확히 한 사건).
    IntroducedAgain { policy: PolicyName, first: EventName, again: EventName },
    /// 새 판의 도입 순서가 원장의 순서와 다르다(도출 조건: k번째로 도입되는 판은 `Pk`).
    IntroducedOutOfOrder { event: EventName, found: PolicyName, expected: PolicyName },
    /// 원장의 판을 도입한 `start`·`scope_change` 가 없다(T4).
    NotIntroduced { policy: PolicyName },
    /// 원장의 마지막 판과 마지막 사건의 after 가 다르다(T4).
    LastMismatch { last_policy: PolicyName, last_after: PolicyName },
}

impl fmt::Display for LinkProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let shown = |policy: &Option<PolicyName>| match policy {
            Some(policy) => policy.to_string(),
            None => "없음".to_string(),
        };
        match self {
            LinkProblem::PoliciesWithoutEvents { policies } => {
                write!(f, "정책 판이 {policies}개 있는데 관리 사건이 없다. 어느 판도 도입되지 않았다")
            }
            LinkProblem::EventsWithoutPolicies { events } => {
                write!(f, "관리 사건이 {events}개 있는데 정책 판이 없다")
            }
            LinkProblem::MissingAfter { event } => write!(f, "{event} 의 policy_after_ref 에 판이 없다"),
            LinkProblem::UnknownPolicy { event, field, policy } => {
                write!(f, "{event} 의 {} 가 원장에 없는 {policy} 을 가리킨다", field.name())
            }
            LinkProblem::FirstBeforePresent { event, before } => {
                write!(f, "첫 사건 {event} 의 policy_before_ref 가 {before} 이다. 없음이어야 한다")
            }
            LinkProblem::BrokenChain { event, before, previous_after } => write!(
                f,
                "{event} 의 policy_before_ref 가 {} 이다. 앞 사건의 policy_after_ref 인 {} 이어야 한다",
                shown(before),
                shown(previous_after)
            ),
            LinkProblem::StartNotFirst { event } => write!(f, "start 사건 {event} 이 첫 사건이 아니다"),
            LinkProblem::RestoreChangesPolicy { event, before, after } => write!(
                f,
                "restore_result 사건 {event} 이 정책을 {} 에서 {after} 로 바꾼다. 같은 판이어야 한다",
                shown(before)
            ),
            LinkProblem::IntroducedAgain { policy, first, again } => {
                write!(f, "{policy} 은 {first} 이 이미 도입했는데 {again} 이 다시 도입한다")
            }
            LinkProblem::IntroducedOutOfOrder { event, found, expected } => write!(
                f,
                "{event} 이 새로 도입하는 판이 {found} 이다. 원장의 순서라면 {expected} 이어야 한다"
            ),
            LinkProblem::NotIntroduced { policy } => {
                write!(f, "{policy} 을 도입한 start·scope_change 사건이 없다")
            }
            LinkProblem::LastMismatch { last_policy, last_after } => write!(
                f,
                "원장의 마지막 판은 {last_policy} 인데 마지막 사건의 policy_after_ref 는 {last_after} 이다"
            ),
        }
    }
}

/// 건넨 원장과 사건 사이의 정책 연결이 일관된다 — **그것뿐이다.**
///
/// 승인·적용·사건 실행·기록 전체의 유효성을 말하지 않는다([`LINKS_DO_NOT_SHOW`]). 이 값을 만드는 문은
/// [`check_policy_links`] 하나뿐이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PolicyLinksAgree {
    /// `introduced_by[i]` 는 `P(i+1)` 을 도입한 사건.
    introduced_by: Vec<EventName>,
    last_after: Option<PolicyName>,
}

impl PolicyLinksAgree {
    /// 그 판을 도입한 `start`·`scope_change` 사건. 원장에 없는 판이면 `None`.
    pub(crate) fn introduced_by(&self, policy: PolicyName) -> Option<EventName> {
        let at = usize::try_from(policy.number()).ok()?.checked_sub(1)?;
        self.introduced_by.get(at).copied()
    }

    /// 마지막 사건의 after(= 원장의 마지막 판). 건넨 입력 안의 값이며 적용 여부의 판단이 아니다.
    pub(crate) fn last_after(&self) -> Option<PolicyName> {
        self.last_after
    }
}

/// 건넨 정책 원장과 사건 연결 사이의 T3·T4 와 §5.1 종류표의 정책 열을 검사한다. **순수 함수**다.
///
/// 입력을 바꾸지 않고 파일·상태를 읽거나 쓰지 않는다. 찾은 문제는 사건 순서, 원장 순서, 마지막 비교 순으로 모두
/// 돌려준다.
pub(crate) fn check_policy_links(
    ledger: &PolicyLedger,
    links: &EventPolicyLinks,
) -> Result<PolicyLinksAgree, Vec<LinkProblem>> {
    let policies = ledger.policies();
    let events = links.events();
    match (policies.is_empty(), events.is_empty()) {
        (true, true) => return Ok(PolicyLinksAgree { introduced_by: Vec::new(), last_after: None }),
        (false, true) => return Err(vec![LinkProblem::PoliciesWithoutEvents { policies: policies.len() }]),
        (true, false) => return Err(vec![LinkProblem::EventsWithoutPolicies { events: events.len() }]),
        (false, false) => {}
    }
    // 원장의 이름은 자리에 묶여 있다(`P<n>` 은 n번째) — 3a 의 `PolicyLedger` 가 보장한다.
    let known = |policy: PolicyName| usize::try_from(policy.number()).is_ok_and(|n| n >= 1 && n <= policies.len());

    let mut problems = Vec::new();
    let mut introduced: Vec<Option<EventName>> = vec![None; policies.len()];
    let mut introductions = 0usize;
    let mut previous_after: Option<Option<PolicyName>> = None;

    for link in events {
        let event = link.name;
        // T3 — 사건의 정책 사슬.
        match previous_after {
            None => {
                if let Some(before) = link.before {
                    problems.push(LinkProblem::FirstBeforePresent { event, before });
                }
            }
            Some(previous) => {
                if link.before != previous {
                    problems.push(LinkProblem::BrokenChain { event, before: link.before, previous_after: previous });
                }
                if link.kind == EventKind::Start {
                    problems.push(LinkProblem::StartNotFirst { event });
                }
            }
        }
        previous_after = Some(link.after);

        // 참조가 건넨 원장의 판인가.
        for (field, policy) in [(LinkField::Before, link.before), (LinkField::After, link.after)] {
            if let Some(policy) = policy.filter(|policy| !known(*policy)) {
                problems.push(LinkProblem::UnknownPolicy { event, field, policy });
            }
        }

        let Some(after) = link.after else {
            problems.push(LinkProblem::MissingAfter { event });
            continue;
        };
        if link.kind == EventKind::RestoreResult && link.before != Some(after) {
            problems.push(LinkProblem::RestoreChangesPolicy { event, before: link.before, after });
        }
        // T4 — 도입은 정확히 한 번, 원장의 순서대로.
        if link.kind.introduces() && known(after) {
            let slot = &mut introduced[after.number() as usize - 1];
            match *slot {
                Some(first) => problems.push(LinkProblem::IntroducedAgain { policy: after, first, again: event }),
                None => {
                    *slot = Some(event);
                    introductions += 1;
                    let expected = PolicyName(introductions as u32);
                    if after != expected {
                        problems.push(LinkProblem::IntroducedOutOfOrder { event, found: after, expected });
                    }
                }
            }
        }
    }

    for (record, by) in policies.iter().zip(&introduced) {
        if by.is_none() {
            problems.push(LinkProblem::NotIntroduced { policy: record.name() });
        }
    }
    let last_policy = policies.last().map(|record| record.name()).expect("비어 있지 않다");
    let last_after = events.last().and_then(|link| link.after);
    if let Some(last_after) = last_after.filter(|after| *after != last_policy) {
        problems.push(LinkProblem::LastMismatch { last_policy, last_after });
    }

    match problems.is_empty() {
        true => Ok(PolicyLinksAgree { introduced_by: introduced.into_iter().flatten().collect(), last_after }),
        false => Err(problems),
    }
}
