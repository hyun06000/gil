//! 기준점과 보관 시도 **결과 순서**의 두 시점 비교 — 구현 순서 3번 안의 작은 후속(편의상 「3f」).
//!
//! 「3f」는 이 PR 의 편의상 이름이다. **계약 T6 전체의 완료가 아니고, 3번 전체의 완료도 아니다.** 정본은
//! [공유 저장·전이 계약](../../../spec/GIL_Storage_Transition_Contract_v0.1.md) §5.1 의 `baseline` 칸 표와 「추가만
//! 한다」, §5.3 의 전이, §5.7 의 T1·T6, §6.3 의 재시도다. 범위 설명은
//! [구현 순서](../../../spec/GIL_Implementation_Order_2026-11-19.md)의 「3f」 기록에 있다.
//!
//! ```text
//! (앞 BaselineFacts*, 앞 등록부) ─┬─ check_baselines (3e) ─────▶ 앞 시점이 한 시점으로 맞는가
//! (뒤 BaselineFacts*, 뒤 등록부) ─┼─ check_baselines (3e) ─────▶ 뒤 시점이 한 시점으로 맞는가
//!                                 ├─ 입력 대응 ────────────────▶ 어느 앞 사건과 어느 뒤 사건을 견주는가
//!                                 └─ 기준점 보존 ──────────────▶ 앞선 사건의 기준점 칸·시도 결과 순서가 남았는가
//!                                            │
//!                                 check_baseline_history ─▶ BaselineHistoryKept | [BaselineHistoryProblem]
//! ```
//!
//! # 두 관측 시점의 비교다
//!
//! 보관을 실행하는 명령이 아니다. 두 시점 사이에 보관 재시도가 몇 번 실패해 기록됐어도 정당하므로(§6.3 재시도에
//! 횟수 제한이 없다) 덧붙은 시도의 수를 제한하지 않는다. 반대로 두 입력의 **차이**만 보므로 그 사이에 바꿨다가
//! 되돌린 일, 기록되지 않은 실패는 보이지 않는다.
//!
//! # 계약에 적힌 조건과 도출한 조건
//!
//! - 적힌 것: 기준점·보관 경계·발급기 값은 한 번만 적는다(T6, §5.1) — 적힌 값을 바꾸지 않는다. 시작 사건의 `null`
//!   경계도 적힌 값이다(§5.1 예시). 사건 원장은 추가만 한다(T1, §5.1). `archived` 는 마지막에 한 번(§5.1).
//! - 도출한 것: 정본에서 앞선 사건의 `baseline` 을 바꾸는 전이는 보관 대기 사건의 실패 시도 기록 추가(§5.3 표, §6.3)와
//!   저장 2(기준점·경계·발급기·시도 기록을 한 번에 적어 확정, §5.3·§5.8) 둘뿐이고, 어느 전이도 적힌 시도를 지우거나
//!   고치지 않으며 확정된 기준점을 대기로 되돌리지 않는다(T1 「기록은 추가만 한다」). 그래서 앞 시점의 시도 결과 순서는
//!   뒤 시점의 앞부분이고, 기준점이 있던 사건은 기준점·두 칸·시도 결과 순서가 모두 그대로다.
//!
//! # 입력 대응
//!
//! [`BaselineFacts`] 목록은 이름의 중복·순서·완전성을 스스로 보장하지 않는다. 그래서 배열의 같은 자리를 같은 사건으로
//! 가정하기 전에 **최소 대응 검사**를 한다: 각 시점의 이름이 `M1` 부터 빈틈없이 자리와 같고(T1 의 사건 이름 규칙 —
//! 3c 의 `EventPolicyLinks::from_links` 와 같은 규칙이지만 발급기 값은 이 입력에 없다), 앞 시점의 사건이 모두 뒤 시점의
//! 같은 자리에 같은 이름·같은 종류로 있다. 대응이 서지 않으면 보존 비교를 하지 않는다. **종류는 같은 사건임을 가리는
//! 표지로만 쓴다** — 종류·정책 연결 칸의 보존은 3d([`super::link_history`])의 규칙이며 여기서 다시 정의하지 않는다.
//! 이 입력이 3c·3d 에 건넨 것과 같은 사건 목록에서 뽑혔는지는 둘을 함께 부를 후속 연결 계층의 책임이다.
//!
//! # 보장은 「건네받은 결과 순서의 보존」까지다
//!
//! [`AttemptSeen`] 은 「`archived` 인가」뿐이다. 실패 사유 원문, 서로 다른 실패 결과 사이의 변경, 같은 결과를 가진
//! 시도들의 실제 정체성·재배열, 원본 시도 레코드 전체의 유효성은 이 비교로 말할 수 없다. 그것을 말하려고 새 결과 목록이나
//! 저장 꼴을 만들지 않았다. 검사하지 않은 것은 [`BASELINE_HISTORY_DOES_NOT_SHOW`] 에 있다.

use std::fmt;

use super::super::SnapshotRegistry;
use super::baseline_record::{AttemptSeen, BaselineFacts, BaselineProblem, BaselineSlot, check_baselines};
use super::event_links::{EventKind, EventName};
use crate::refs::{CycleRef, SnapshotRef};

/// 이 비교가 성공해도 **보이지 않은** 것.
pub(crate) const BASELINE_HISTORY_DOES_NOT_SHOW: [&str; 14] = [
    "실패 사유(cause) 원문의 보존과 서로 다른 실패 결과 사이의 변경 — 시도는 「archived 인가」로만 건네받는다",
    "같은 결과를 가진 시도들의 실제 정체성과 재배열, 원본 시도 레코드 전체의 유효성 — 건네받은 결과 순서의 보존만 본다",
    "두 시점 사이에 바꿨다가 되돌린 일, 기록되지 않은 실패 — 두 입력의 차이만 본다",
    "입력이 실제 두 상태의 사건 기록에서 빠짐없이 뽑혔는가 — 특히 앞 시점 목록의 끝이 잘려 건네졌는가(발급기 값이 없다), 3c·3d 입력과 같은 사건 목록인가. 후속 연결 계층의 책임",
    "같은 이름·같은 종류의 다른 사건으로 바꿔 넣었는가 — 기준점 칸의 차이로만 보이고, 허용된 갱신의 꼴이면 구별하지 못한다",
    "사건 종류·정책 연결 칸의 보존 — 3d(link_history)의 규칙이다. 여기서는 종류를 대응의 표지로만 쓴다",
    "등록부 자체의 과거 매핑 보존 — 두 등록부에서 같은 이름이 같은 manifest 를 가리키는가는 보지 않는다",
    "기준점 객체의 실재와 내용 — 각 시점의 등록부에서 이름이 조회되는지만 본다(3e)",
    "대체된 사건의 재보관 금지(§5.2)와 기준점이 없는 사건의 위치(T5) — 앞 사건에 기준점이 새로 적혀도 그 자리가 보관 대기인 마지막 사건이었는지 판단하지 않는다. 변경 전부터 목록상 대체된 사건의 기준점 추가는 baseline_position 의 check_replaced_baselines 가 따로 거절한다",
    "실패 시도를 덧붙일 자격 — 보관 대기인 마지막 사건이었는가, 실제로 보관을 시도했는가",
    "보관 경계·발급기 값의 의미(T7), world_before_ref 와 현재 세계 계산(T8, §6), 보관 대기 뒤의 실험(T9)",
    "사용자 승인, 사건을 만들 권한, 실제 보관 성공",
    "두 시점 사이의 실제 상태 저장이 한 번에 일어났는가(원자성)",
    "즉시 보관 중단 표식(T12), Chain 종료와 사건 이관(§12 O-3 미결)",
];

// ── 입력 대응 ──────────────────────────────────────────────────────────────

/// 두 시점 가운데 어느 쪽인가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    Before,
    After,
}

impl Side {
    fn name(self) -> &'static str {
        match self {
            Side::Before => "변경 전",
            Side::After => "변경 후",
        }
    }
}

/// 어느 앞 사건과 어느 뒤 사건을 견줄지 정할 수 없는 이유.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MatchProblem {
    /// 한 시점의 목록에서 자리의 이름이 `M<자리>` 가 아니다 — 빈틈·재배열(`repeated: false`) 또는 중복(`true`).
    /// 시점마다 첫 자리 하나만 돌려준다. 뒤 자리는 그 결과일 수 있다.
    NameOutOfPlace { side: Side, position: usize, found: EventName, repeated: bool },
    /// 앞 시점의 사건이 뒤 시점에 없다 — 삭제·잘라내기.
    Missing { event: EventName },
    /// 같은 이름의 사건 종류가 다르다 — 같은 사건이라고 볼 수 없다.
    KindDiffers { event: EventName, before: EventKind, after: EventKind },
}

impl fmt::Display for MatchProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MatchProblem::NameOutOfPlace { side, position, found, repeated } => {
                let place = position + 1;
                match repeated {
                    true => write!(f, "{} 목록의 {place}번째 사건이 앞의 이름 {found} 을 다시 쓴다", side.name()),
                    false => write!(
                        f,
                        "{} 목록의 {place}번째 사건 이름이 {found} 이다. 빈틈없는 순서라면 event:M{place} 이어야 한다",
                        side.name()
                    ),
                }
            }
            MatchProblem::Missing { event } => write!(f, "앞선 사건 {event} 이 변경 후 목록에 없다. 사건은 추가만 한다"),
            MatchProblem::KindDiffers { event, before, after } => write!(
                f,
                "{event} 의 종류가 변경 전 {} 이고 변경 후 {} 이다. 같은 사건으로 견줄 수 없다",
                before.name(),
                after.name()
            ),
        }
    }
}

/// 이름이 자리에서 어긋난 첫 사건 — (자리, 찾은 이름, 앞의 이름을 다시 쓰는가). 자리는 0 부터 센다.
///
/// 한 목록의 이름이 `M1` 부터 빈틈없이 자리와 같은가의 규칙 하나를 3f 의 입력 대응과 3g(`baseline_position`)의 한 시점
/// 구분이 함께 쓴다.
pub(super) fn first_name_out_of_place(events: &[BaselineFacts]) -> Option<(usize, EventName, bool)> {
    events.iter().enumerate().find_map(|(position, facts)| {
        let found = facts.event();
        let in_place = usize::try_from(found.number()).is_ok_and(|number| number == position + 1);
        (!in_place).then(|| (position, found, events[..position].iter().any(|seen| seen.event() == found)))
    })
}

/// 한 시점의 이름이 `M1` 부터 자리와 같은가. 어긋난 첫 자리를 돌려준다.
fn names_in_place(side: Side, events: &[BaselineFacts]) -> Option<MatchProblem> {
    first_name_out_of_place(events)
        .map(|(position, found, repeated)| MatchProblem::NameOutOfPlace { side, position, found, repeated })
}

/// 대응이 서면 견줄 짝(앞 사건, 뒤 사건)을, 아니면 그 이유를 모두 돌려준다.
fn match_events<'a>(
    before: &'a [BaselineFacts],
    after: &'a [BaselineFacts],
) -> Result<Vec<(&'a BaselineFacts, &'a BaselineFacts)>, Vec<MatchProblem>> {
    let mut problems: Vec<_> = [names_in_place(Side::Before, before), names_in_place(Side::After, after)]
        .into_iter()
        .flatten()
        .collect();
    if !problems.is_empty() {
        // 이름이 자리에 묶이지 않으면 같은 자리를 같은 사건으로 볼 근거가 없다.
        return Err(problems);
    }
    let mut pairs = Vec::with_capacity(before.len());
    for (at, earlier) in before.iter().enumerate() {
        match after.get(at) {
            None => problems.push(MatchProblem::Missing { event: earlier.event() }),
            Some(later) if later.kind() != earlier.kind() => problems.push(MatchProblem::KindDiffers {
                event: earlier.event(),
                before: earlier.kind(),
                after: later.kind(),
            }),
            Some(later) => pairs.push((earlier, later)),
        }
    }
    match problems.is_empty() {
        true => Ok(pairs),
        false => Err(problems),
    }
}

// ── 기준점 보존 ────────────────────────────────────────────────────────────

/// 앞선 사건의 기준점 칸이나 시도 결과 순서에서 정본의 전이로 설명되지 않는 변경. 시도의 자리는 0 부터 센다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BaselineChange {
    /// 적힌 기준점이 다른 이름으로 바뀌었다(T6 「한 번만 적는다」).
    SnapshotReplaced { event: EventName, before: SnapshotRef, after: SnapshotRef },
    /// 적힌 기준점이 지워졌다 — 확정된 사건이 보관 대기로 되돌아갔다(T1, §5.3 에서 도출).
    SnapshotCleared { event: EventName, before: SnapshotRef },
    /// 적힌 보관 경계 칸이 바뀌거나 지워졌다(T6). `null` 로 적힌 경계도 적힌 값이다.
    BoundaryChanged { event: EventName, before: Option<CycleRef>, after: BaselineSlot<Option<CycleRef>> },
    /// 적힌 발급기 칸이 바뀌거나 지워졌다(T6).
    IssuerChanged { event: EventName, before: u32, after: BaselineSlot<u32> },
    /// 앞서 건네받은 시도 결과가 같은 자리에서 다르다 — 실패를 성공으로, 성공을 실패로 바꿔치기.
    AttemptChanged { event: EventName, position: usize, before: AttemptSeen, after: AttemptSeen },
    /// 앞서 건네받은 시도가 줄었다 — 삭제·잘라내기.
    AttemptsRemoved { event: EventName, before: usize, after: usize },
    /// 기준점이 있던 사건에 시도가 덧붙었다(§5.1 「마지막에 한 번」, 저장 2 의 확정에서 도출).
    AttemptsAfterBaseline { event: EventName, added: usize },
}

fn boundary_text(slot: &BaselineSlot<Option<CycleRef>>) -> String {
    match slot {
        BaselineSlot::Unwritten => "칸 없음".to_string(),
        BaselineSlot::Written(None) => "null".to_string(),
        BaselineSlot::Written(Some(cycle)) => cycle.to_string(),
    }
}

fn attempt_text(attempt: AttemptSeen) -> &'static str {
    match attempt {
        AttemptSeen::Archived => "archived",
        AttemptSeen::NotArchived => "archived 아님",
    }
}

impl fmt::Display for BaselineChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BaselineChange::SnapshotReplaced { event, before, after } => write!(
                f,
                "{event} 의 baseline.snapshot_ref 가 {before} 에서 {after} 로 바뀌었다. 기준점은 한 번만 적는다"
            ),
            BaselineChange::SnapshotCleared { event, before } => write!(
                f,
                "{event} 의 baseline.snapshot_ref {before} 가 지워졌다. 확정된 기준점은 보관 대기로 돌아가지 않는다"
            ),
            BaselineChange::BoundaryChanged { event, before, after } => write!(
                f,
                "{event} 의 baseline.after_cycle_ref 가 {} 에서 {} 로 바뀌었다. 보관 경계는 한 번만 적는다",
                boundary_text(&BaselineSlot::Written(*before)),
                boundary_text(after)
            ),
            BaselineChange::IssuerChanged { event, before, after } => {
                let after = match after {
                    BaselineSlot::Unwritten => "칸 없음".to_string(),
                    BaselineSlot::Written(value) => value.to_string(),
                };
                write!(f, "{event} 의 baseline.next_cycle_id 가 {before} 에서 {after} 로 바뀌었다. 발급기 값은 한 번만 적는다")
            }
            BaselineChange::AttemptChanged { event, position, before, after } => write!(
                f,
                "{event} 의 {}번째 시도 결과가 {} 에서 {} 로 바뀌었다. 적힌 시도는 고치지 않는다",
                position + 1,
                attempt_text(*before),
                attempt_text(*after)
            ),
            BaselineChange::AttemptsRemoved { event, before, after } => write!(
                f,
                "{event} 의 시도가 {before}개에서 {after}개로 줄었다. 적힌 시도는 지우지 않는다"
            ),
            BaselineChange::AttemptsAfterBaseline { event, added } => write!(
                f,
                "기준점이 확정된 {event} 에 시도가 {added}개 덧붙었다. archived 는 마지막에 한 번 나온다"
            ),
        }
    }
}

/// 견줄 수 있었던 앞선 사건이 두 시점 사이에 겪은 것. 덧붙은 시도는 모두 「`archived` 가 아님」이며, 그것이 올바른
/// 실패 기록인지·사유가 무엇인지는 말하지 않는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BaselineUpdate {
    /// 기준점 칸과 시도 결과 순서가 그대로다.
    Unchanged,
    /// 기준점 없이 남았고, 앞선 시도 뒤에 `archived` 가 아닌 시도가 덧붙었다(§5.3 보관 실패, §6.3 재시도).
    AttemptsAdded { added: usize },
    /// 기준점이 처음 적혔다. 앞선 시도 뒤에 `archived` 가 아닌 시도 `added_before_archived` 개와 마지막 `archived` 하나가
    /// 붙었고 경계·발급기가 함께 적혔다(§5.3 저장 2). 기준점이 적힐 **자격**(T5, §5.2 대체)은 판단하지 않는다.
    Fixed { added_before_archived: usize },
}

/// 앞선 사건 하나의 보존을 본다. 짝은 대응 검사가 정했다.
fn compare(earlier: &BaselineFacts, later: &BaselineFacts, changes: &mut Vec<BaselineChange>) -> BaselineUpdate {
    let event = earlier.event();
    let (old, new) = (earlier.attempts(), later.attempts());

    // 적힌 시도 결과는 같은 자리에 그대로 — 뒤 시점의 앞부분이어야 한다.
    if let Some(position) = old.iter().zip(new).position(|(was, now)| was != now) {
        changes.push(BaselineChange::AttemptChanged { event, position, before: old[position], after: new[position] });
    }
    if new.len() < old.len() {
        changes.push(BaselineChange::AttemptsRemoved { event, before: old.len(), after: new.len() });
    }

    // 적힌 칸은 다시 쓰지 않는다 — 기준점, `null` 을 포함한 보관 경계, 발급기.
    if let Some(snapshot) = earlier.snapshot() {
        match later.snapshot() {
            None => changes.push(BaselineChange::SnapshotCleared { event, before: snapshot }),
            Some(now) if now != snapshot => {
                changes.push(BaselineChange::SnapshotReplaced { event, before: snapshot, after: now })
            }
            Some(_) => {}
        }
        if new.len() > old.len() {
            changes.push(BaselineChange::AttemptsAfterBaseline { event, added: new.len() - old.len() });
        }
    }
    if let BaselineSlot::Written(before) = earlier.boundary() {
        if later.boundary() != BaselineSlot::Written(before) {
            changes.push(BaselineChange::BoundaryChanged { event, before, after: later.boundary() });
        }
    }
    if let BaselineSlot::Written(before) = earlier.issuer() {
        if later.issuer() != BaselineSlot::Written(before) {
            changes.push(BaselineChange::IssuerChanged { event, before, after: later.issuer() });
        }
    }

    let added = new.len().saturating_sub(old.len());
    match (earlier.snapshot(), later.snapshot()) {
        (None, Some(_)) => BaselineUpdate::Fixed { added_before_archived: added.saturating_sub(1) },
        _ if added == 0 => BaselineUpdate::Unchanged,
        _ => BaselineUpdate::AttemptsAdded { added },
    }
}

// ── 두 시점 비교 ───────────────────────────────────────────────────────────

/// 두 시점 비교가 실패한 갈래.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BaselineHistoryProblem {
    /// 변경 전 입력 자체가 한 시점 검사(3e)를 통과하지 못했다.
    BeforePoint(Vec<BaselineProblem>),
    /// 변경 후 입력 자체가 한 시점 검사(3e)를 통과하지 못했다.
    AfterPoint(Vec<BaselineProblem>),
    /// 어느 앞 사건과 어느 뒤 사건을 견줄지 정할 수 없다. 이때는 보존을 비교하지 않는다.
    Unmatched(Vec<MatchProblem>),
    /// 견준 앞선 사건의 기준점 칸이나 시도 결과 순서가 정본의 전이로 설명되지 않게 바뀌었다.
    PastBaselines(Vec<BaselineChange>),
}

impl fmt::Display for BaselineHistoryProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn listed<T: ToString>(items: &[T]) -> String {
            items.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
        }
        match self {
            BaselineHistoryProblem::BeforePoint(found) => write!(f, "변경 전 입력의 기준점 기록이 맞지 않는다: {}", listed(found)),
            BaselineHistoryProblem::AfterPoint(found) => write!(f, "변경 후 입력의 기준점 기록이 맞지 않는다: {}", listed(found)),
            BaselineHistoryProblem::Unmatched(found) => write!(f, "두 시점의 사건을 짝지을 수 없다: {}", listed(found)),
            BaselineHistoryProblem::PastBaselines(found) => write!(f, "앞선 사건의 기준점 기록이 바뀌었다: {}", listed(found)),
        }
    }
}

/// 건넨 두 시점이 각각 한 시점 검사를 통과하고, 앞선 사건이 모두 같은 자리에 있으며, 그 기준점 칸과 시도 결과 순서가
/// 정본의 전이(실패 시도 추가, 최초 확정)로만 달라졌다 — **그것뿐이다.** 보이지 않은 것은
/// [`BASELINE_HISTORY_DOES_NOT_SHOW`] 에 있다. 이 값을 만드는 문은 [`check_baseline_history`] 하나뿐이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BaselineHistoryKept {
    compared: Vec<(EventName, BaselineUpdate)>,
    added: Vec<EventName>,
}

impl BaselineHistoryKept {
    /// 앞 시점에 있던 사건마다 두 시점 사이에 겪은 것, 사건 순서대로.
    pub(crate) fn compared(&self) -> &[(EventName, BaselineUpdate)] {
        &self.compared
    }

    /// 뒤 시점의 끝에 덧붙은 사건. 뒤 시점의 한 시점 검사만 받았다 — 만들 권한이나 실행 성공은 판단하지 않는다.
    pub(crate) fn added(&self) -> &[EventName] {
        &self.added
    }
}

/// 변경 전후의 (기준점·시도 칸 목록, 그 시점의 등록부) 두 쌍을 비교한다. **순수 함수**다.
///
/// 각 시점에 3e 의 [`check_baselines`] 를 그대로 부르고, 입력 대응을 세운 뒤, 대응이 서면 앞선 사건마다 보존을 본다.
/// 실패한 갈래를 변경 전 입력, 변경 후 입력, 입력 대응, 앞선 기준점 순으로 모두 돌려준다. 입력과 등록부를 바꾸지 않고
/// (등록부는 3e 를 거쳐 `resolve` 만 불린다) 파일·상태를 읽거나 쓰지 않는다.
pub(crate) fn check_baseline_history(
    before: (&[BaselineFacts], &SnapshotRegistry),
    after: (&[BaselineFacts], &SnapshotRegistry),
) -> Result<BaselineHistoryKept, Vec<BaselineHistoryProblem>> {
    let (before_events, before_registry) = before;
    let (after_events, after_registry) = after;
    let mut problems = Vec::new();
    if let Err(found) = check_baselines(before_events, before_registry) {
        problems.push(BaselineHistoryProblem::BeforePoint(found));
    }
    if let Err(found) = check_baselines(after_events, after_registry) {
        problems.push(BaselineHistoryProblem::AfterPoint(found));
    }
    let pairs = match match_events(before_events, after_events) {
        Ok(pairs) => pairs,
        Err(found) => {
            problems.push(BaselineHistoryProblem::Unmatched(found));
            return Err(problems);
        }
    };

    let mut changes = Vec::new();
    let compared: Vec<_> =
        pairs.into_iter().map(|(earlier, later)| (earlier.event(), compare(earlier, later, &mut changes))).collect();
    if !changes.is_empty() {
        problems.push(BaselineHistoryProblem::PastBaselines(changes));
    }
    match problems.is_empty() {
        true => Ok(BaselineHistoryKept {
            compared,
            added: after_events[before_events.len()..].iter().map(BaselineFacts::event).collect(),
        }),
        false => Err(problems),
    }
}
