//! 정책 원장과 사건의 **정책 연결 칸**의 추가 전후 보존 검사 — 구현 순서 3번 안의 작은 후속(편의상 「3d」).
//!
//! 「3d」는 이 PR 의 편의상 이름이다. **3번 전체의 완료가 아니고, 사건 기록 전체의 추가 전후 비교도 아니다.**
//! 정본은 [공유 저장·전이 계약](../../../spec/GIL_Storage_Transition_Contract_v0.1.md) §5.1 의 「추가만 한다」와
//! §5.7 의 T1 이다. 범위 설명은 [구현 순서](../../../spec/GIL_Implementation_Order_2026-11-19.md)의 「3d」 기록에 있다.
//!
//! ```text
//! (앞 원장, 앞 사건 연결) ─┬─ check_policy_links (3c) ──────────▶ 앞 시점이 일관된가
//! (뒤 원장, 뒤 사건 연결) ─┼─ check_policy_links (3c) ──────────▶ 뒤 시점이 일관된가
//!                          ├─ check_append_only (3a) ───────────▶ 앞선 정책 판이 그대로인가
//!                          └─ check_link_append_only ───────────▶ 앞선 사건의 정책 연결 칸이 그대로인가
//!                                     │
//!                          check_policy_history ─▶ HistoryKept | [HistoryProblem]
//! ```
//!
//! # 「추가만 한다」를 어디에 적용하는가
//!
//! 사건에서 보는 칸은 3c 가 이미 건네받는 넷 — 이름·종류·`policy_before_ref`·`policy_after_ref` — 뿐이다. 정본에서
//! 이 칸들은 승인 저장(저장 1)에서 사건과 함께 생긴 뒤 어떤 전이도 고치지 않는다. 같은 사건의 `baseline` 은 다르다:
//! 보관 대기 사건의 기준점·보관 경계·발급기 값은 저장 2 에서 한 번 채워지고 보관 시도는 늘어난다(§5.1·§5.3·§6.3).
//! 그 칸들은 이 입력에 없으며, 이 모듈은 「앞선 사건의 모든 칸이 바뀌지 않는다」는 규칙을 두지 않는다.
//!
//! # 새 규칙을 만들지 않는다
//!
//! 한 시점의 일관성은 3c 의 [`check_policy_links`], 정책 판의 보존은 3a 의 [`check_append_only`] 를 그대로 부른다.
//! 이름의 꼴은 [`EventPolicyLinks`] 가 보장한다(`M<n>` 은 n번째 자리, 발급기 = 사건 수 + 1). 그래서 같은 자리의 이름은
//! 바뀔 수 없고, 앞 자리가 모두 남아 있으면 이름·순서·발급기도 보존된 것이다. 재배열이나 다른 유효한 연결로의
//! 교체는 앞 자리의 종류·참조 변경으로 걸린다. 한 번에 몇 개를 덧붙였는지는 제한하지 않는다 — 명령이 아니라 두
//! 값의 비교다.

use std::fmt;

use super::event_links::{EventName, EventPolicyLinks, LinkProblem, check_policy_links};
use super::{AppendViolation, PolicyLedger, check_append_only};

/// 이 검사가 성공해도 **보이지 않은** 것.
pub(crate) const HISTORY_DOES_NOT_SHOW: [&str; 8] = [
    "입력이 실제 전체 원장·사건 기록에서 빠짐없이 뽑혔는가 — 후속 연결 계층의 책임",
    "모니터 승인 응답과 proposal_digest 의 연결, 사용자 승인 사실과 정책의 실제 적용",
    "사건 기록 전체의 보존 — chain_ref·after_cycle_ref·review_cycle_ref·next_cycle_id·world_before_ref 는 입력에 없다",
    "기준점·보관 경계·보관 시도·실패 사유(cause)의 보존이나 허용된 갱신 — T5·T6, 계약 §5.3·§6.3",
    "세계·사이클 참조와 그 전이 — T7·T8·T9, 계약 §6",
    "두 시점 사이의 실제 상태 저장이 한 번에 일어났는가(원자성)",
    "덧붙은 사건을 실행할 권한이 있었는가, 그 사건이 성공했는가",
    "3c 의 한 시점 검사가 보지 않는 것 전부 — LINKS_DO_NOT_SHOW",
];

// ── 사건의 정책 연결 칸 보존 ───────────────────────────────────────────────

/// 앞선 사건의 정책 연결 칸 가운데 바뀐 것.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinkSlot {
    Kind,
    PolicyBefore,
    PolicyAfter,
}

impl LinkSlot {
    /// 계약 §5.1 의 칸 이름.
    pub(crate) fn name(self) -> &'static str {
        match self {
            LinkSlot::Kind => "kind",
            LinkSlot::PolicyBefore => "policy_before_ref",
            LinkSlot::PolicyAfter => "policy_after_ref",
        }
    }
}

/// 사건 연결의 추가 전후 비교가 찾은 과거 사건의 변경.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LinkAppendViolation {
    /// 앞선 사건이 뒤 목록에 없다 — 삭제·잘라내기.
    Removed { event: EventName },
    /// 앞선 사건의 자리에 다른 값이 있다 — 종류·참조 수정, 재배열, 다른 연결로의 교체가 여기 걸린다.
    Changed { event: EventName, slots: Vec<LinkSlot> },
}

impl fmt::Display for LinkAppendViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkAppendViolation::Removed { event } => {
                write!(f, "앞선 사건 {event} 이 사라졌다. 사건은 추가만 한다")
            }
            LinkAppendViolation::Changed { event, slots } => {
                let names: Vec<_> = slots.iter().map(|slot| slot.name()).collect();
                write!(f, "앞선 사건 {event} 의 정책 연결 칸이 바뀌었다({}). 사건은 추가만 한다", names.join(", "))
            }
        }
    }
}

impl std::error::Error for LinkAppendViolation {}

/// 앞선 사건 연결 뒤에 사건이 **덧붙기만 했는가** — 3a 의 [`check_append_only`] 와 같은 꼴의 순수 비교다.
///
/// 두 값 모두 [`EventPolicyLinks`] 이므로 이름은 자리에 묶여 있다. 앞선 사건마다 같은 자리의 종류·앞뒤 정책이
/// 그대로이고 사라진 사건이 없으면 이름·순서·발급기도 보존된 것이다. 덧붙은 사건의 수를 돌려준다. 덧붙은 사건이
/// 일관되는지는 이 함수가 보지 않는다([`check_policy_history`] 가 3c 로 본다).
pub(crate) fn check_link_append_only(
    before: &EventPolicyLinks,
    after: &EventPolicyLinks,
) -> Result<usize, LinkAppendViolation> {
    for (at, earlier) in before.events().iter().enumerate() {
        let Some(later) = after.events().get(at) else {
            return Err(LinkAppendViolation::Removed { event: earlier.name() });
        };
        let mut slots = Vec::new();
        if later.kind() != earlier.kind() {
            slots.push(LinkSlot::Kind);
        }
        if later.before() != earlier.before() {
            slots.push(LinkSlot::PolicyBefore);
        }
        if later.after() != earlier.after() {
            slots.push(LinkSlot::PolicyAfter);
        }
        if !slots.is_empty() {
            return Err(LinkAppendViolation::Changed { event: earlier.name(), slots });
        }
    }
    Ok(after.events().len() - before.events().len())
}

// ── 정책과 사건을 함께 보는 두 시점 비교 ───────────────────────────────────

/// 두 시점 비교가 실패한 갈래. 갈래마다 기존 검사의 결과를 그대로 담는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HistoryProblem {
    /// 변경 전 입력 자체의 정책 연결이 맞지 않는다(3c).
    BeforeLinks(Vec<LinkProblem>),
    /// 변경 후 입력 자체의 정책 연결이 맞지 않는다(3c).
    AfterLinks(Vec<LinkProblem>),
    /// 앞선 정책 판이 사라졌거나 바뀌었다(3a).
    PastPolicy(AppendViolation),
    /// 앞선 사건이 사라졌거나 그 정책 연결 칸이 바뀌었다.
    PastEventLinks(LinkAppendViolation),
}

impl fmt::Display for HistoryProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let listed = |problems: &[LinkProblem]| problems.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ");
        match self {
            HistoryProblem::BeforeLinks(problems) => write!(f, "변경 전 입력의 정책 연결이 맞지 않는다: {}", listed(problems)),
            HistoryProblem::AfterLinks(problems) => write!(f, "변경 후 입력의 정책 연결이 맞지 않는다: {}", listed(problems)),
            HistoryProblem::PastPolicy(violation) => write!(f, "과거 정책 기록이 바뀌었다: {violation}"),
            HistoryProblem::PastEventLinks(violation) => write!(f, "과거 사건의 정책 연결이 바뀌었다: {violation}"),
        }
    }
}

/// 건넨 두 시점에서 정책 연결이 각각 일관되고, 앞선 정책 판과 사건의 정책 연결 칸이 보존되며, 새 기록이 끝에만
/// 붙었다 — **그것뿐이다.** 보이지 않은 것은 [`HISTORY_DOES_NOT_SHOW`] 에 있다. 이 값을 만드는 문은
/// [`check_policy_history`] 하나뿐이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HistoryKept {
    policies_added: usize,
    events_added: usize,
}

impl HistoryKept {
    /// 끝에 덧붙은 정책 판의 수.
    pub(crate) fn policies_added(&self) -> usize {
        self.policies_added
    }

    /// 끝에 덧붙은 사건의 수.
    pub(crate) fn events_added(&self) -> usize {
        self.events_added
    }
}

/// 변경 전후의 (정책 원장, 사건 연결) 두 쌍을 비교한다. **순수 함수**다.
///
/// 네 갈래를 모두 돌려보고, 실패한 갈래를 변경 전 입력, 변경 후 입력, 과거 정책, 과거 사건 연결 순으로 돌려준다.
/// 각 갈래는 기존 검사를 그대로 부를 뿐 따로 규칙을 적지 않는다. 입력을 바꾸지 않고 파일·상태를 읽거나 쓰지 않는다.
pub(crate) fn check_policy_history(
    before: (&PolicyLedger, &EventPolicyLinks),
    after: (&PolicyLedger, &EventPolicyLinks),
) -> Result<HistoryKept, Vec<HistoryProblem>> {
    let (before_ledger, before_links) = before;
    let (after_ledger, after_links) = after;
    let mut problems = Vec::new();
    if let Err(found) = check_policy_links(before_ledger, before_links) {
        problems.push(HistoryProblem::BeforeLinks(found));
    }
    if let Err(found) = check_policy_links(after_ledger, after_links) {
        problems.push(HistoryProblem::AfterLinks(found));
    }
    let policies = check_append_only(before_ledger, after_ledger).map_err(HistoryProblem::PastPolicy);
    let events = check_link_append_only(before_links, after_links).map_err(HistoryProblem::PastEventLinks);
    match (policies, events) {
        (Ok(policies_added), Ok(events_added)) if problems.is_empty() => Ok(HistoryKept { policies_added, events_added }),
        (policies, events) => {
            problems.extend(policies.err());
            problems.extend(events.err());
            Err(problems)
        }
    }
}
