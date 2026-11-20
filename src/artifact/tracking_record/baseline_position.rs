//! 건네받은 사건 목록의 **보관 대기·대체 위치 구분**과, **변경 전부터 대체된 사건의 기준점 추가 거절** — 구현 순서 3번 안의
//! 작은 후속(편의상 「3g」).
//!
//! 「3g」는 이 PR 의 편의상 이름이다. **계약 T5 전체의 완료가 아니고, 3번 전체의 완료도 아니다.** 정본은
//! [공유 저장·전이 계약](../../../spec/GIL_Storage_Transition_Contract_v0.1.md) §5.2 의 「보관 대기는 유도값이다」, §5.3 의 표
//! 「앞 사건은 기준점 없이 대체된다」, §5.7 의 T1·T5, §6.5 (나)다. 범위 설명은
//! [구현 순서](../../../spec/GIL_Implementation_Order_2026-11-19.md)의 「3g」 기록에 있다.
//!
//! ```text
//! BaselineFacts* + 등록부 ─ check_baselines (3e) + 이름 자리(3f 와 같은 규칙) ─▶ classify_baseline_positions
//!                                                                         ─▶ BaselinePositions | [PositionProblem]
//!
//! (앞 BaselineFacts*, 앞 등록부) ─┬─ check_baseline_history (3f, 그대로) ─────▶ History 갈래
//! (뒤 BaselineFacts*, 뒤 등록부) ─┴─ 앞 시점의 위치 구분 + 뒤 시점의 기준점 ─▶ ReplacedGainedBaseline 갈래
//!                                            │
//!                                 check_replaced_baselines ─▶ ReplacedBaselinesKept | [ReplacedBaselineProblem]
//! ```
//!
//! # 한 시점: 목록 위치의 구분
//!
//! §5.2: 마지막 사건에 기준점이 없으면 보관 대기, 마지막이 아닌 사건에 기준점이 없으면 뒤 사건으로 대체된 것이다. 보관 대기는
//! **유도값**이며 저장 플래그가 아니다. 그래서 이 구분은 새 상태·칸을 만들지 않고 건넨 기록에서 읽기 전용으로 유도한다.
//! 기준점이 있는 사건은 자리와 무관하게 「기준점 있음」이다 — 앞쪽에 있어도 대체 사건이 아니다. 「기준점 있음」은 보관 객체가
//! 온전하다거나 다음 실험을 열어도 된다는 뜻이 아니다.
//!
//! 순서가 서지 않으면 「마지막」을 말할 수 없으므로 3e 의 한 시점 검사와 3f 의 이름 자리 규칙(`M1` 부터 빈틈없이 자리와 같다)을
//! 통과한 목록만 구분한다. 그래도 **목록의 끝이 잘려 건네졌는지는 모른다**(발급기 값이 입력에 없다) — 잘린 목록에서는 대체된
//! 사건이 보관 대기로 보인다. 그래서 구분의 이름에 「목록상」을 붙인다.
//!
//! # 두 시점: 변경 전부터 대체된 사건의 기준점 추가
//!
//! 앞 시점에서 이미 「목록상 대체됨」인 사건은 뒤 사건이 지워지지 않으므로(T1 「추가만 한다」) 그 뒤 어느 시점에도 마지막이
//! 되지 못한다. 그 사건에 기준점을 적는 것은 §5.2 가 금지한 재보관이다. 그래서 뒤 시점에 그 사건의 기준점이 보이면 거절한다.
//!
//! 앞 시점의 **마지막 대기 사건**에 기준점이 생긴 것은, 뒤 시점에 새 사건이 붙어 그 사건이 마지막이 아니어도 거절하지 않는다.
//! 두 관측 사이에 보관 성공 뒤 새 승인이 있었을 수 있다(§5.3 표). 같은 두 입력이 「새 승인 뒤 재보관」에서 왔을 수도 있지만 두
//! 시점으로는 실행 순서를 알 수 없다 — 그 한계를 [`POSITIONS_DO_NOT_SHOW`] 에 적고, 뒤 시점의 위치만으로 기준점 추가를 막지 않는다.
//!
//! # 3f 와의 역할 구분
//!
//! 3f([`check_baseline_history`])는 그대로 부르고 범위를 바꾸지 않는다 — 그 성공은 계속 위치·재보관 자격을 말하지 않는다. 그
//! 실패는 [`ReplacedBaselineProblem::History`] 한 갈래에 그대로 담고, 이번 위치 조건의 실패는
//! [`ReplacedBaselineProblem::ReplacedGainedBaseline`] 으로 따로 돌려준다. 변경 전 입력이나 입력 대응이 서지 않으면 어느 사건이
//! 대체됐는지 정할 수 없으므로 위치 조건을 **평가하지 않는다** — 결과는 3f 의 실패 그대로이고 위치 검사의 성공으로 바뀌지 않는다.
//!
//! 기준점 추가의 거절은 대체된 사건의 모든 기록 변경 금지가 아니다. 대체된 사건에 실패 기록만 뒤늦게 덧붙은 경우는 거절하지도
//! 정당하다고 판단하지도 않으며, 성공 결과에 [`ReplacedBaselinesKept::unjudged_late_attempts`] 로 드러낸다.

use std::fmt;

use super::super::SnapshotRegistry;
use super::baseline_history::{
    BaselineHistoryKept, BaselineHistoryProblem, BaselineUpdate, check_baseline_history, first_name_out_of_place,
};
use super::baseline_record::{BaselineFacts, BaselineProblem, check_baselines};
use super::event_links::EventName;
use crate::refs::SnapshotRef;

/// 이 구분과 검사가 성공해도 **보이지 않은** 것.
pub(crate) const POSITIONS_DO_NOT_SHOW: [&str; 13] = [
    "입력이 실제 사건 원장에서 빠짐없이 뽑혔는가 — 목록의 끝이 잘려 건네지면 대체된 사건이 보관 대기로 보인다(발급기 값이 입력에 없다). 그래서 구분은 「목록상」이다. 후속 연결 계층의 책임",
    "두 목록이 같은 프로젝트의 앞뒤 상태에서 뽑혔는가 — 서로 다른 상태에서 뽑은 목록을 잘못 연결해도 이름·종류가 맞으면 구별하지 못한다. 3c·3d 입력과 같은 사건 목록인가도 보지 않는다",
    "두 관측 사이의 실제 실행 순서 — 앞 시점의 마지막 대기 사건에 기준점이 생기고 뒤에 새 사건이 붙었으면, 성공 뒤 새 승인이었는지 새 승인 뒤 재보관이었는지 구별하지 못하고 받아들인다",
    "한 시점의 구분만으로는 재보관 위반을 말하지 않는다 — 앞쪽 사건의 기준점이 대체 전에 생겼는지 뒤에 생겼는지 한 시점으로는 모른다",
    "「기준점 있음」은 보관 객체의 실재·내용·올바른 세계(§5.4)가 아니고, 다음 실험을 열어도 된다는 뜻(§6.2 ②)도 아니다",
    "이미 대체된 사건에 실패 기록만 뒤늦게 덧붙은 경우의 의미 — 거절하지도 정당하다고 판단하지도 않는다. 원본 실패 사유·시도 정체성·지연 기록 구조도 보지 않는다",
    "실제 보관 실행 권한과 재시도 실행 — 보관 대기 사건을 지금 보관해도 되는가(사전 조건, world_before_ref·T8)",
    "보관 경계·발급기 값의 의미(T7), 현재 세계 계산과 다음 실험 허용(T8·T9·§6)",
    "등록부의 같은 이름 → 같은 객체 매핑 보존과 객체 내용 — 각 시점의 등록부에서 이름이 조회되는지만 본다(3e)",
    "승인 때의 수동 편집·삭제와 BaseUndecided, 사용자 승인과 사건을 만들 권한",
    "전환 동의 채널(O-1), 보관 중단 해제 채널(O-2)과 T12, Chain 종료 때 미완료 사건의 이관(O-3) — chain_ref 를 읽지 않으며 체인 종료·이관이 얽힌 기록을 승인하지 않는다",
    "사건 종류·정책 연결 칸의 보존 — 3d(link_history)의 규칙이다. 종류는 3f 의 대응 표지로만 쓰인다",
    "형식 전환·활성화·제품 연결 — 이 검사를 부르는 제품 경로가 없다",
];

// ── 한 시점의 위치 구분 ─────────────────────────────────────────────────────

/// 건넨 목록 안에서 사건 하나의 위치. 건넨 기록에서 읽기 전용으로 유도한 값이며 저장하지 않는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BaselinePosition {
    /// 건넨 기록에 `baseline.snapshot_ref` 가 적혀 있다. 자리와 무관하다. 보관 객체의 온전함이나 다음 실험 허용을 뜻하지 않는다.
    HasBaseline,
    /// 기준점이 없는 **마지막** 사건 — 목록상 보관 대기(§5.2). 목록의 끝이 잘렸으면 실제로는 대체된 사건일 수 있다.
    WaitingInList,
    /// 기준점이 없는 **앞쪽** 사건 — 목록상 뒤 사건으로 대체됨(§5.2). 다시 보관하지 않는다.
    ReplacedInList,
}

/// 건넨 목록의 사건마다 유도한 위치. 빈 목록이면 사건이 없다. 이 값을 만드는 문은 [`classify_baseline_positions`] 와
/// [`check_replaced_baselines`] 의 성공뿐이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BaselinePositions {
    entries: Vec<(EventName, BaselinePosition)>,
}

impl BaselinePositions {
    /// 사건 순서대로 (이름, 위치).
    pub(crate) fn entries(&self) -> &[(EventName, BaselinePosition)] {
        &self.entries
    }

    /// 사건이 없다(§5.1 `events: []`).
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 목록상 보관 대기인 사건 — 있으면 마지막 사건 하나다.
    pub(crate) fn waiting(&self) -> Option<EventName> {
        self.entries.iter().find(|(_, at)| *at == BaselinePosition::WaitingInList).map(|(event, _)| *event)
    }

    /// 목록상 대체된 사건, 사건 순서대로. 여럿이어도 정상이다.
    pub(crate) fn replaced(&self) -> Vec<EventName> {
        self.of(BaselinePosition::ReplacedInList)
    }

    /// 기준점이 적힌 사건, 사건 순서대로.
    pub(crate) fn with_baseline(&self) -> Vec<EventName> {
        self.of(BaselinePosition::HasBaseline)
    }

    fn of(&self, wanted: BaselinePosition) -> Vec<EventName> {
        self.entries.iter().filter(|(_, at)| *at == wanted).map(|(event, _)| *event).collect()
    }

    fn position_of(&self, event: EventName) -> Option<BaselinePosition> {
        self.entries.iter().find(|(name, _)| *name == event).map(|(_, at)| *at)
    }
}

/// 검사를 이미 통과한 목록의 위치를 유도한다. 기준점의 유무와 자리만 본다.
fn positions_of(events: &[BaselineFacts]) -> BaselinePositions {
    let last = events.len().checked_sub(1);
    let entries = events
        .iter()
        .enumerate()
        .map(|(at, facts)| {
            let position = match (facts.snapshot(), Some(at) == last) {
                (Some(_), _) => BaselinePosition::HasBaseline,
                (None, true) => BaselinePosition::WaitingInList,
                (None, false) => BaselinePosition::ReplacedInList,
            };
            (facts.event(), position)
        })
        .collect();
    BaselinePositions { entries }
}

/// 한 시점의 목록을 구분할 수 없는 이유. 자리는 0 부터 센다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PositionProblem {
    /// 자리의 이름이 `M<자리>` 가 아니다 — 빈틈·재배열(`repeated: false`) 또는 중복(`true`). 순서가 서지 않아 「마지막」을 말할
    /// 수 없다. 첫 자리 하나만 돌려준다.
    NameOutOfPlace { position: usize, found: EventName, repeated: bool },
    /// 건넨 기준점 기록 자체가 한 시점 검사(3e)를 통과하지 못했다.
    Point(Vec<BaselineProblem>),
}

impl fmt::Display for PositionProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PositionProblem::NameOutOfPlace { position, found, repeated } => {
                let place = position + 1;
                match repeated {
                    true => write!(f, "목록의 {place}번째 사건이 앞의 이름 {found} 을 다시 쓴다. 사건 순서를 정할 수 없다"),
                    false => write!(
                        f,
                        "목록의 {place}번째 사건 이름이 {found} 이다. 빈틈없는 순서라면 event:M{place} 이어야 하며, 순서가 서지 않으면 마지막 사건을 정할 수 없다"
                    ),
                }
            }
            PositionProblem::Point(found) => write!(
                f,
                "건넨 기준점 기록이 한 시점으로 맞지 않는다: {}",
                found.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
            ),
        }
    }
}

/// 건넨 한 시점의 목록에서 사건마다 「기준점 있음 / 목록상 보관 대기 / 목록상 대체됨」을 유도한다. **순수 함수**다.
///
/// 3e 의 [`check_baselines`] 와 이름 자리 규칙을 통과한 목록만 구분한다. 이 구분만으로 재보관 위반을 말하지 않는다 — 앞쪽 사건의
/// 기준점이 대체 전에 생겼는지 뒤에 생겼는지 한 시점으로는 모른다. 입력과 등록부를 바꾸지 않는다(등록부는 3e 를 거쳐 `resolve`
/// 만 불린다).
pub(crate) fn classify_baseline_positions(
    events: &[BaselineFacts],
    registry: &SnapshotRegistry,
) -> Result<BaselinePositions, Vec<PositionProblem>> {
    let mut problems = Vec::new();
    if let Some((position, found, repeated)) = first_name_out_of_place(events) {
        problems.push(PositionProblem::NameOutOfPlace { position, found, repeated });
    }
    if let Err(found) = check_baselines(events, registry) {
        problems.push(PositionProblem::Point(found));
    }
    match problems.is_empty() {
        true => Ok(positions_of(events)),
        false => Err(problems),
    }
}

// ── 두 시점: 변경 전부터 대체된 사건의 기준점 추가 ──────────────────────────

/// 두 시점 검사가 실패한 갈래.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReplacedBaselineProblem {
    /// 기존 두 시점 비교(3f)의 실패 — 한 시점 검사, 입력 대응, 기준점·시도 결과 순서의 보존. 그대로 담는다.
    History(Vec<BaselineHistoryProblem>),
    /// 변경 전 목록에서 이미 대체된 사건(`last_before` 가 그 시점의 마지막 사건이었다)에 변경 후 기준점이 적혔다 —
    /// §5.2 「다시 보관하지 않는다」.
    ReplacedGainedBaseline { event: EventName, last_before: EventName, snapshot: SnapshotRef },
}

impl fmt::Display for ReplacedBaselineProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReplacedBaselineProblem::History(found) => write!(
                f,
                "기존 두 시점 기준점 비교가 맞지 않는다: {}",
                found.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
            ),
            ReplacedBaselineProblem::ReplacedGainedBaseline { event, last_before, snapshot } => write!(
                f,
                "{event} 은 변경 전 목록에서 이미 대체됐는데(그 뒤에 {last_before} 까지 사건이 있었다) 변경 후 기준점 {snapshot} 이 적혔다. 대체된 사건은 다시 보관하지 않는다"
            ),
        }
    }
}

/// 건넨 두 시점이 3f 비교를 통과하고, 변경 전부터 목록상 대체된 사건에 기준점이 새로 적히지 않았다 — **그것뿐이다.** 보이지
/// 않은 것은 [`POSITIONS_DO_NOT_SHOW`] 에 있다. 이 값을 만드는 문은 [`check_replaced_baselines`] 하나뿐이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReplacedBaselinesKept {
    history: BaselineHistoryKept,
    before: BaselinePositions,
    after: BaselinePositions,
}

impl ReplacedBaselinesKept {
    /// 3f 비교의 성공 값 그대로.
    pub(crate) fn history(&self) -> &BaselineHistoryKept {
        &self.history
    }

    /// 변경 전 목록의 위치.
    pub(crate) fn before(&self) -> &BaselinePositions {
        &self.before
    }

    /// 변경 후 목록의 위치. 변경 후에 마지막이 아니게 된 사건의 기준점은 이 값만으로 판단하지 않는다.
    pub(crate) fn after(&self) -> &BaselinePositions {
        &self.after
    }

    /// 변경 전부터 목록상 대체된 사건 가운데 실패 기록이 덧붙은 것과 그 수. **판단하지 않은 것**이다 — 거절하지 않았지만 정당하다고
    /// 판단한 것도 아니다(이번 범위에서 결정하지 않았다).
    pub(crate) fn unjudged_late_attempts(&self) -> Vec<(EventName, usize)> {
        self.history
            .compared()
            .iter()
            .filter(|(event, _)| self.before.position_of(*event) == Some(BaselinePosition::ReplacedInList))
            .filter_map(|(event, update)| match update {
                BaselineUpdate::AttemptsAdded { added } => Some((*event, *added)),
                _ => None,
            })
            .collect()
    }
}

/// 변경 전후의 (기준점·시도 칸 목록, 그 시점의 등록부) 두 쌍에서 3f 비교와, 변경 전부터 대체된 사건의 기준점 추가 거절을 함께
/// 본다. **순수 함수**다.
///
/// 3f 의 [`check_baseline_history`] 를 그대로 부르고 그 실패를 [`ReplacedBaselineProblem::History`] 에 담는다. 변경 전 입력이 한
/// 시점 검사를 통과하고 입력 대응이 섰을 때만(3f 실패에 변경 전 입력·대응 갈래가 없을 때만) 위치 조건을 평가하고, 그 실패를
/// 사건마다 [`ReplacedBaselineProblem::ReplacedGainedBaseline`] 으로 덧붙인다. 평가하지 못하면 결과는 3f 의 실패 그대로다.
/// 입력과 등록부를 바꾸지 않고 파일·상태를 읽거나 쓰지 않는다.
pub(crate) fn check_replaced_baselines(
    before: (&[BaselineFacts], &SnapshotRegistry),
    after: (&[BaselineFacts], &SnapshotRegistry),
) -> Result<ReplacedBaselinesKept, Vec<ReplacedBaselineProblem>> {
    let (before_events, _) = before;
    let (after_events, _) = after;
    let history = check_baseline_history(before, after);

    // 어느 사건이 변경 전에 대체됐는지는 변경 전 목록이 검사를 통과하고 두 목록의 짝이 섰을 때만 정할 수 있다.
    let positioned = match &history {
        Ok(_) => true,
        Err(found) => !found.iter().any(|problem| {
            matches!(problem, BaselineHistoryProblem::BeforePoint(_) | BaselineHistoryProblem::Unmatched(_))
        }),
    };
    let mut rearchived = Vec::new();
    let before_positions = positions_of(before_events);
    if positioned {
        let last_before = before_events.last().map(BaselineFacts::event);
        for (earlier, later) in before_events.iter().zip(after_events) {
            let replaced = before_positions.position_of(earlier.event()) == Some(BaselinePosition::ReplacedInList);
            if let (true, Some(snapshot), Some(last_before)) = (replaced, later.snapshot(), last_before) {
                rearchived.push(ReplacedBaselineProblem::ReplacedGainedBaseline {
                    event: earlier.event(),
                    last_before,
                    snapshot,
                });
            }
        }
    }

    match history {
        Ok(kept) if rearchived.is_empty() => {
            Ok(ReplacedBaselinesKept { history: kept, before: before_positions, after: positions_of(after_events) })
        }
        Ok(_) => Err(rearchived),
        Err(found) => {
            let mut problems = vec![ReplacedBaselineProblem::History(found)];
            problems.extend(rearchived);
            Err(problems)
        }
    }
}
