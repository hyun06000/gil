//! 관리 사건의 **기준점·보관 시도 기록의 한 시점 내부 일관성** 검사 — 구현 순서 3번 안의 작은 후속(편의상 「3e」).
//!
//! 「3e」는 이 PR 의 편의상 이름이다. **계약 T6 전체의 완료가 아니고, 3번 전체의 완료도 아니다.** 정본은
//! [공유 저장·전이 계약](../../../spec/GIL_Storage_Transition_Contract_v0.1.md) §5.1 의 `baseline` 칸 표, §5.3 의 전이,
//! §5.7 의 T6 이다. 범위 설명은 [구현 순서](../../../spec/GIL_Implementation_Order_2026-11-19.md)의 「3e」 기록에 있다.
//!
//! ```text
//! BaselineFacts*  ─┐
//!                  ├─ check_baselines ─▶ BaselinesAgree | [BaselineProblem]
//! SnapshotRegistry ┘   (resolve 만 — 읽기 전용)
//! ```
//!
//! # 입력은 「뽑아 건넨 값」이다
//!
//! [`BaselineFacts`] 는 미래의 전체 사건 기록에서 **이 검사에 필요한 칸만** 뽑아 건넨 값이다 — 사건 이름·종류,
//! `baseline.snapshot_ref`, `baseline.after_cycle_ref`·`baseline.next_cycle_id` 칸의 유무와 값, 그리고 시도마다
//! 「`archived` 인가 아닌가」의 순서. 전체 사건 저장 꼴도, 새 영구 기록 형식도 아니다. 직렬화 자료형이 없고
//! `state.yaml` 에 연결되지 않는다.
//!
//! 시도는 [`AttemptSeen`] 두 값으로만 받는다. **실패 사유(`cause`)의 목록을 만들지 않는다** — 그 값은 5번 작업이
//! 확정한다(§5.3). [`AttemptSeen::NotArchived`] 는 「원본 레코드가 `archived` 가 아니다」일 뿐이며, 원본이 올바른
//! `failed` 레코드라는 뜻이 아니다. 원본 시도 레코드 전체의 검증은 뽑아 건넬 계층의 일이다.
//!
//! # 계약에 적힌 조건과 도출한 조건
//!
//! - 적힌 것: `archived` 는 `snapshot_ref` 가 있을 때만(T6, §5.1), 보관 경계·발급기 값은 `snapshot_ref` 가 있을 때만
//!   그와 함께 적는다(T6, §5.1 「함께 한 번만 적는다」), `archived` 는 마지막에 한 번(§5.1), 기준점은 등록부에
//!   실재한다(T6), 보관 대기 상태의 `start` 는 없다(§5.3).
//! - 도출한 것: 기준점이 있으면 `archived` 가 있다 — 저장 2 가 세계 이름·`baseline`·시도 기록을 한 번에 적어 기준점을
//!   확정하고(§5.3, §5.8), 최선 노력은 **실패** 기록뿐이다(§5.3). 기준점이 있는데 경계·발급기 칸 중 하나라도 없으면
//!   「함께」를 어긴 부분 기록이다. `start` 의 보관 경계는 값이 `null` 로 **적힌** 것이고(§5.1 예시) 빠진 칸이 아니다.
//!
//! # 일부러 보지 않는 것
//!
//! - 시도가 없거나 실패가 여럿인 보관 대기는 정상이다(§5.3·§6.3). 실패 기록 저장도 실패할 수 있으므로 시도가 없다는
//!   사실로 「보관을 시도하지 않았다」고 말하지 않는다. 횟수 제한이 없다.
//! - 같은 보관본을 서로 다른 사건이 가리켜도 정상이다(§5.5). 보관본 재사용은 사건 중복이 아니다.
//! - 경계·발급기 **값의 의미**(T7)와 종류별 `null` 여부는 보지 않는다. 일반 사건에 non-null 경계를 요구하지 않는다.
//! - 「한 번만 적는다」의 시간 측면은 한 시점으로 증명할 수 없다(건넨 두 시점의 비교는 [`super::baseline_history`],
//!   「3f」). 검사하지 않은 것은 [`BASELINES_DO_NOT_SHOW`] 에 있다.

use std::fmt;

use super::super::SnapshotRegistry;
use super::event_links::{EventKind, EventName};
use crate::refs::{CycleRef, SnapshotRef};

/// 이 검사가 성공해도 **보이지 않은** 것.
pub(crate) const BASELINES_DO_NOT_SHOW: [&str; 12] = [
    "입력이 실제 사건 기록에서 빠짐없이 뽑혔는가, 사건 이름의 순서와 3c 입력과 같은 사건인가 — 후속 연결 계층의 책임",
    "원본 시도 레코드 전체의 유효성 — archived 밖의 result 값, 실패 사유(cause, 5번 작업이 확정)와 그 밖의 칸",
    "「한 번만 적는다」의 시간 측면 — 적힌 기준점·보관 경계·발급기가 그 뒤 바뀌지 않았는가, 앞선 시도가 지워지지 않았는가. 한 시점으로는 볼 수 없다 — 건넨 두 시점의 기준점 칸과 시도 결과 순서의 보존만 baseline_history 가 본다",
    "실제 보관 성공과 보관 실행 권한, 실패 기록이 없는 대기에서 실제 시도가 없었는가(실패 기록은 최선 노력, §5.3)",
    "기준점 객체의 실재와 내용 — 등록부에서 이름이 조회되는지만 본다. manifest·blob 의 실재·주소 일치는 아니다",
    "기준점이 올바른 세계를 담았는가 — 적용 정책으로의 관측과 범위 변화 검사(§5.4)",
    "보관 경계·발급기 값의 의미 — T7(같은 Chain 의 닫힌 사이클, 이름 < 발급기, 사건의 next_cycle_id 이상)과 종류별 null 여부",
    "world_before_ref 와 마지막 확정 세계(T8), 현재 세계와 다음 Entry(§6), 보관 대기 뒤의 실험(T9)",
    "기준점이 없는 사건의 위치와 대체 — T5",
    "사용자 승인과 정책의 실제 적용, 정책 변경 뒤 실험을 열어도 되는가",
    "실제 저장의 원자성 — 저장 2 가 기준점과 시도를 정말 한 번에 적었는가",
    "즉시 보관 중단 표식(T12), Chain 종료와 사건 이관(§12 O-3 미결)",
];

// ── 뽑아 건넨 칸 ───────────────────────────────────────────────────────────

/// 사건에 적힌 칸 하나 — **칸이 없음**과 **칸이 있고 값이 무엇인가**를 가른다.
///
/// 보관 경계는 값이 `null` 일 수 있다(`start`, §5.1 예시). 그래서 `Option` 하나로는 「칸 없음」과 「`null` 로 적힘」이
/// 섞인다. 발급기 칸의 값이 `null` 인 꼴은 정본에 없으므로 뽑는 계층이 거절할 일이며 이 입력에 담을 자리가 없다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BaselineSlot<T> {
    /// 칸이 없다.
    Unwritten,
    /// 칸이 있다.
    Written(T),
}

impl<T> BaselineSlot<T> {
    fn is_written(&self) -> bool {
        matches!(self, BaselineSlot::Written(_))
    }
}

/// 보관 시도 하나에서 뽑아 건넨 것 — `archived` 인가 아닌가뿐이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AttemptSeen {
    /// 원본 레코드의 `result` 가 `archived` 다.
    Archived,
    /// 원본 레코드가 `archived` 가 아니다. 그 레코드가 올바른 `failed` 레코드인지, 사유가 무엇인지는 말하지 않는다.
    NotArchived,
}

/// 사건 하나에서 뽑아 건넨 기준점·보관 시도 칸.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BaselineFacts {
    event: EventName,
    kind: EventKind,
    snapshot: Option<SnapshotRef>,
    boundary: BaselineSlot<Option<CycleRef>>,
    issuer: BaselineSlot<u32>,
    attempts: Vec<AttemptSeen>,
}

impl BaselineFacts {
    pub(crate) fn new(
        event: EventName,
        kind: EventKind,
        snapshot: Option<SnapshotRef>,
        boundary: BaselineSlot<Option<CycleRef>>,
        issuer: BaselineSlot<u32>,
        attempts: Vec<AttemptSeen>,
    ) -> BaselineFacts {
        BaselineFacts { event, kind, snapshot, boundary, issuer, attempts }
    }

    pub(crate) fn event(&self) -> EventName {
        self.event
    }

    pub(crate) fn kind(&self) -> EventKind {
        self.kind
    }

    /// `baseline.snapshot_ref`.
    pub(crate) fn snapshot(&self) -> Option<SnapshotRef> {
        self.snapshot
    }

    /// `baseline.after_cycle_ref` 칸.
    pub(crate) fn boundary(&self) -> BaselineSlot<Option<CycleRef>> {
        self.boundary
    }

    /// `baseline.next_cycle_id` 칸.
    pub(crate) fn issuer(&self) -> BaselineSlot<u32> {
        self.issuer
    }

    /// `baseline.attempts` 의 순서대로 본 결과.
    pub(crate) fn attempts(&self) -> &[AttemptSeen] {
        &self.attempts
    }
}

// ── 모순 ───────────────────────────────────────────────────────────────────

/// 기준점과 함께 적는 칸.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BaselineField {
    AfterCycleRef,
    NextCycleId,
}

impl BaselineField {
    /// 계약 §5.1 의 칸 이름.
    pub(crate) fn name(self) -> &'static str {
        match self {
            BaselineField::AfterCycleRef => "baseline.after_cycle_ref",
            BaselineField::NextCycleId => "baseline.next_cycle_id",
        }
    }
}

fn joined(fields: &[BaselineField]) -> String {
    fields.iter().map(|field| field.name()).collect::<Vec<_>>().join(", ")
}

fn ordinals(positions: &[usize]) -> String {
    positions.iter().map(|at| format!("{}번째", at + 1)).collect::<Vec<_>>().join(", ")
}

/// 건넨 기준점·보관 시도 칸이 서로 맞지 않는 이유. 시도의 자리는 0 부터 센다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BaselineProblem {
    /// 기준점이 없는데 `archived` 시도가 있다(T6, §5.1).
    ArchivedWithoutSnapshot { event: EventName, positions: Vec<usize> },
    /// 기준점이 없는데 보관 경계·발급기 칸이 있다(T6, §5.1).
    BoundaryWithoutSnapshot { event: EventName, fields: Vec<BaselineField> },
    /// 기준점이 있는데 함께 적어야 할 칸이 빠졌다(§5.1 「함께」에서 도출).
    PartialBaseline { event: EventName, missing: Vec<BaselineField> },
    /// 기준점이 있는데 `archived` 시도가 없다(저장 2 의 한 번 저장, §5.3 에서 도출).
    SnapshotWithoutArchived { event: EventName },
    /// `archived` 가 두 번 이상 나온다(§5.1 「한 번」).
    ArchivedMoreThanOnce { event: EventName, positions: Vec<usize> },
    /// `archived` 뒤에 시도가 더 있다(§5.1 「마지막에」).
    ArchivedNotLast { event: EventName, position: usize, attempts: usize },
    /// 기준점 이름이 건넨 등록부에 없다(T6).
    UnknownSnapshot { event: EventName, snapshot: SnapshotRef },
    /// `start` 에 기준점이 없다 — 보관 대기 상태의 `start` 는 없다(§5.3).
    StartWithoutSnapshot { event: EventName },
}

impl fmt::Display for BaselineProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BaselineProblem::ArchivedWithoutSnapshot { event, positions } => write!(
                f,
                "{event} 에 baseline.snapshot_ref 가 없는데 {} 시도가 archived 다",
                ordinals(positions)
            ),
            BaselineProblem::BoundaryWithoutSnapshot { event, fields } => write!(
                f,
                "{event} 에 baseline.snapshot_ref 가 없는데 {} 가 적혀 있다. 보관할 때 함께 적는 칸이다",
                joined(fields)
            ),
            BaselineProblem::PartialBaseline { event, missing } => write!(
                f,
                "{event} 의 baseline.snapshot_ref 는 있는데 {} 가 없다. 함께 적는 칸이다",
                joined(missing)
            ),
            BaselineProblem::SnapshotWithoutArchived { event } => write!(
                f,
                "{event} 의 baseline.snapshot_ref 는 있는데 archived 시도가 없다. 기준점과 그 시도는 한 번에 적는다"
            ),
            BaselineProblem::ArchivedMoreThanOnce { event, positions } => write!(
                f,
                "{event} 의 시도 가운데 {} 가 archived 다. archived 는 한 번만 나온다",
                ordinals(positions)
            ),
            BaselineProblem::ArchivedNotLast { event, position, attempts } => write!(
                f,
                "{event} 의 {}번째 시도가 archived 인데 시도가 {attempts}개다. archived 는 마지막에 나온다",
                position + 1
            ),
            BaselineProblem::UnknownSnapshot { event, snapshot } => {
                write!(f, "{event} 의 기준점 {snapshot} 이 건넨 등록부에 없다")
            }
            BaselineProblem::StartWithoutSnapshot { event } => {
                write!(f, "start 사건 {event} 에 baseline.snapshot_ref 가 없다. 보관 대기 상태의 start 는 없다")
            }
        }
    }
}

// ── 검사 ───────────────────────────────────────────────────────────────────

/// 건넨 기준점·보관 시도 칸이 한 시점 안에서 서로 모순되지 않는다 — **그것뿐이다.**
///
/// 실제 보관 성공·권한·객체 실재·세계의 올바름·시간에 걸친 보존을 말하지 않는다([`BASELINES_DO_NOT_SHOW`]).
/// 이 값을 만드는 문은 [`check_baselines`] 하나뿐이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BaselinesAgree {
    with_snapshot: Vec<(EventName, SnapshotRef)>,
    without_snapshot: Vec<EventName>,
}

impl BaselinesAgree {
    /// 건넨 입력에서 `snapshot_ref` 가 적힌 사건과 그 이름. 등록부에서 이름이 조회됐을 뿐이다.
    pub(crate) fn with_snapshot(&self) -> &[(EventName, SnapshotRef)] {
        &self.with_snapshot
    }

    /// 건넨 입력에서 `snapshot_ref` 가 없는 사건. 보관 대기인지 대체됐는지는 판단하지 않는다(T5).
    pub(crate) fn without_snapshot(&self) -> &[EventName] {
        &self.without_snapshot
    }
}

/// 사건마다 건넨 기준점·보관 시도 칸을 한 시점에서 검사한다. **순수 함수**다.
///
/// 입력과 등록부를 바꾸지 않고(등록부는 `resolve` 만 부른다) 파일·상태를 읽거나 쓰지 않는다. 사건 사이의 관계는
/// 보지 않는다 — 같은 보관본을 여러 사건이 가리켜도 된다(§5.5). 찾은 문제는 사건 순서대로 모두 돌려준다.
pub(crate) fn check_baselines(
    events: &[BaselineFacts],
    registry: &SnapshotRegistry,
) -> Result<BaselinesAgree, Vec<BaselineProblem>> {
    let mut problems = Vec::new();
    let mut with_snapshot = Vec::new();
    let mut without_snapshot = Vec::new();

    for facts in events {
        let event = facts.event;
        let archived: Vec<usize> = facts
            .attempts
            .iter()
            .enumerate()
            .filter(|(_, attempt)| **attempt == AttemptSeen::Archived)
            .map(|(at, _)| at)
            .collect();
        let written = [
            (BaselineField::AfterCycleRef, facts.boundary.is_written()),
            (BaselineField::NextCycleId, facts.issuer.is_written()),
        ];

        match facts.snapshot {
            None => {
                without_snapshot.push(event);
                if !archived.is_empty() {
                    problems.push(BaselineProblem::ArchivedWithoutSnapshot { event, positions: archived.clone() });
                }
                let fields: Vec<_> = written.iter().filter(|(_, is)| *is).map(|(field, _)| *field).collect();
                if !fields.is_empty() {
                    problems.push(BaselineProblem::BoundaryWithoutSnapshot { event, fields });
                }
                if facts.kind == EventKind::Start {
                    problems.push(BaselineProblem::StartWithoutSnapshot { event });
                }
            }
            Some(snapshot) => {
                with_snapshot.push((event, snapshot));
                let missing: Vec<_> = written.iter().filter(|(_, is)| !*is).map(|(field, _)| *field).collect();
                if !missing.is_empty() {
                    problems.push(BaselineProblem::PartialBaseline { event, missing });
                }
                if archived.is_empty() {
                    problems.push(BaselineProblem::SnapshotWithoutArchived { event });
                }
                if registry.resolve(snapshot).is_none() {
                    problems.push(BaselineProblem::UnknownSnapshot { event, snapshot });
                }
            }
        }

        // §5.1 「마지막에 한 번」 — 기준점의 유무와 따로 본다.
        if archived.len() > 1 {
            problems.push(BaselineProblem::ArchivedMoreThanOnce { event, positions: archived.clone() });
        }
        let last = facts.attempts.len().checked_sub(1);
        if let Some(position) = archived.first().copied().filter(|at| Some(*at) != last) {
            problems.push(BaselineProblem::ArchivedNotLast { event, position, attempts: facts.attempts.len() });
        }
    }

    match problems.is_empty() {
        true => Ok(BaselinesAgree { with_snapshot, without_snapshot }),
        false => Err(problems),
    }
}
