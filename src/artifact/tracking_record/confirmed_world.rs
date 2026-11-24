//! **이미 저장된 범위 변경 사건 하나에 대해, 적용 정책과 「현재 닫힌 경계의 마지막 확정 세계·그 확정 정책」을 출처까지 따로 읽는다** —
//! 계약 §6.1 의 확정 정책 표 세 줄(사건의 기준점 · Experiment 의 Exit · Interview 의 Exit)과 §6.3 의 보관 대기 읽기를 한 입력 묶음에서
//! 잇는 순수 읽기, 제품 미연결.
//!
//! 정본은 [공유 저장·전이 계약](../../../spec/GIL_Storage_Transition_Contract_v0.1.md) §5.1(사건 칸 표의 `review_cycle_ref`·
//! `world_before_ref`)·§5.2·§5.4(세 정책의 구분)·§5.7 의 T5·T8·T9·§6.1·§6.2·§6.3·§6.6 과
//! [Artifact Model](../../../spec/GIL_Artifact_Model_v0.1.md) §7.1·§7.2·§7.5 다. **§6.1 전체(열린 사이클의 Verify 계보), 실제 개방 허가,
//! 제품 load·status·Monitor 연결, 형식 10/11 의 범위 변경 기록 읽기의 완료가 아니다.**
//!
//! ```text
//! RecordInputs { PolicyLedger, EventPolicyLinks, EventCycleFacts*, EventWorldFacts*, &[Chain], 등록부, ScopeMarks } + 대상 사건 이름
//!   ├─ check_event_inputs (3j, 그대로)                           입력 사이의 대응
//!   ├─ EventWorldFacts 의 사건 수·자리별 이름·종류 대응           보충 입력(review_cycle_ref·world_before_ref)
//!   ├─ classify_baseline_positions (3g, 그대로)                  대기 / 이미 확정 / 대체
//!   ├─ 범위: 종류 scope_change · 뿌리가 start 인 열린 Chain · 닫힌 경계에 서 있음
//!   ├─ start 기준점 = Chain Entry = 뿌리 Entry
//!   ├─ select_boundary_world(지금 경계 T)                        B(T)
//!   │     사건의 기준점 ─────────────────────────────────────▶ 그 사건의 policy_after
//!   │     T 의 Exit ─┬ Experiment: select_experiment_exit(P(T)) + check_experiment_exit_lineage(일치)
//!   │                │             + 검증 ①(Entry = S(T) 의 세계) + 검증 ②(S(T) 의 확정 정책 = P(T))
//!   │                └ Interview:  저장 Exit = Entry, select_opening_world(S(N), 보관 발급기 필터) + 검증 ①
//!   │                              → S(N) 이 사건의 기준점이면 그 policy_after, 부모 Exit 이면 부모로 같은 유도를 잇는다
//!   │                              → 뿌리면 start 기준점
//!   └─ world_before_ref ─ 승인 경계의 앞선 기록(대상 사건보다 앞의 사건만)으로 유도한 세계
//!                       ─ 대기: 지금 경계의 B(T) / 이미 확정: 보관 경계의 앞선 기록으로 유도한 세계
//!                                                  ▼
//! ScopeChangeReading { 적용 정책과 출처 · 마지막 확정 세계와 확정 정책·유도 경로 · 사건의 대기/이미 확정/대체 · 판단하지 않은 것 }
//! ```
//!
//! # 세 값을 섞지 않는다
//!
//! - **적용 정책**은 건넨 원장의 마지막 판이고(§5.2), 그 판을 도입한 사건과 마지막 사건을 출처로 함께 지닌다.
//! - **마지막 확정 세계와 확정 정책**은 지금 서 있는 닫힌 경계 T 의 B(T) 와 §6.1 의 유도다. 현재 적용 정책이나 P(N) 으로 대신하지 않는다.
//!   보관 대기 중에 연 Interview 는 P(N) 이 새 판이어도 그 세계는 앞 판으로 확정됐다 — Interview 는 Entry 의 출처를 따라 계승한다.
//! - **사건의 기준점**은 대상 사건 자신의 `baseline` 이다. 적용 정책 P2 · 마지막 확정 세계 A5(P1) · 기준점 없음은 **유효한 대기**이며
//!   (§6.3) 이 읽기는 P2 를 미승인 후보로 되돌리지도, A5 를 P2 로 보관한 세계라고 말하지도 않는다.
//!
//! 같은 세계 이름을 다른 사건·다른 정책이 가리켜도 출처는 고른 자리 하나다(§5.5). 유도 경로([`PolicyStep`])는 걸음마다 사건·판·세계를
//! 따로 지닌다.
//!
//! # 호출자가 지정하지 못하는 것
//!
//! 호출자가 고르는 것은 대상 사건 이름 하나다. 지금 경계, 확정 정책, 성공 표식을 받는 자리가 없다. 지금 경계는 대상 사건의
//! `chain_ref` 가 가리키는 Chain 의 서 있는 자리에서 읽고, 모든 기존 선택(3j·B(T)·S(N)·P(T)·Exit 계보)은 **이 호출의 입력으로** 다시
//! 실행한다. 다른 입력에서 얻은 성공 값을 받아 결합하는 문이 없다.
//!
//! # world_before_ref 와 순환
//!
//! T8 의 두 비교 모두 **대상 사건보다 앞의 사건만 남긴 입력**에서, 지금 경계와 **같은 유도**(B(T) 와 그 확정 정책 — Interview 의 Entry
//! 계승, Experiment 의 Exit 계보와 검증 ①·②, 보관 발급기 필터, start 기준점)로 얻은 세계와 견준다. 승인 경계·보관 경계에서 고른 Exit 의
//! 근거가 서지 않으면 세계 이름이 같아도 일치로 치지 않는다. 대상 사건 자신과 그 뒤의 사건은 그 입력에 없으므로 「검사 대상 사건의 새
//! 기준점으로 그 사건의 보관 전 근거를 채우는」 순환도, 뒤 사건의 판·기준점을 과거의 근거로 대입하는 일도 생기지 않는다. 지금 경계가 대상
//! 사건의 기준점을 고른 경우에도 승인 경계와 보관 경계의 근거는 따로 유도한다. 앞선 사건의 기준점이 견주는 발급 번호(승인 때는 사건의
//! `next_cycle_id`, 보관 때는 `baseline.next_cycle_id`)보다 늦게 보관됐다고 적혀 있으면 선후를 정할 수 없으므로 성공으로 채우지 않는다.
//! 승인 경계의 근거와 두 번째 근거(대기면 지금 경계, 이미 확정이면 보관 경계)의 확정 정책이 다른 판이면 한 비교 근거가 아니므로 거절한다.
//! 대기 사건은 마지막 사건이라 지금 경계의 유도가 곧 앞선 기록의 유도다.
//!
//! # 읽기 전용
//!
//! 이 모듈에는 폴더 관측·객체 창고·상태 저장으로 나가는 통로가 없다. 받은 값을 바꾸지 않고(등록부는 `resolve` 만 불린다) 재시도·보관을
//! 시작하지 않는다. 같은 입력으로 몇 번을 불러도, 값을 새로 만들어 불러도 같은 읽기일 뿐이다. 보이지 않는 것은
//! [`CONFIRMED_WORLD_DOES_NOT_SHOW`] 에, 이번 호출이 판단하지 않은 칸은 결과의 [`ScopeChangeReading::not_judged`] 에 있다.

use std::fmt;

use super::super::SnapshotRegistry;
use super::baseline_position::{BaselinePosition, PositionProblem, classify_baseline_positions};
use super::baseline_record::{BaselineFacts, BaselineSlot};
use super::boundary_world::{BoundarySource, BoundaryWorldProblem, select_boundary_world};
use super::event_cycles::{CycleField, EventCycleFacts, RefReading};
use super::event_inputs::{EventInputsProblem, check_event_inputs};
use super::event_links::{EventKind, EventName, EventPolicyLinks};
use super::exit_lineage::{ExitLineageProblem, ExitLineageVerdict, ExpectedSource, check_experiment_exit_lineage};
use super::experiment_exit::{ExperimentExitProblem, owned_cycle, select_experiment_exit};
use super::opening_policy::{OpeningSelection, TargetProblem, select_opening_policy};
use super::opening_world::{EntryVerdict, OpeningWorldProblem, WorldSource, select_opening_world};
use super::{PolicyLedger, PolicyName};
use crate::refs::{ChainRef, CycleRef, SnapshotRef};
use crate::{Chain, Cycle, CycleKind};

/// 이 읽기가 성공해도 **보이지 않는** 것 — 결과를 들고 있어도 이것들은 성립하지 않는다.
pub(crate) const CONFIRMED_WORLD_DOES_NOT_SHOW: [&str; 12] = [
    "입력의 출처와 완전성 — 건넨 원장·사건 입력·보충 입력·Chain 목록·등록부가 실제 프로젝트의 같은 시점·같은 원본에서 빠짐없이 뽑혔는지 보지 않는다. 함께 잘라낸 입력은 알아내지 못하며 잘린 이력 안에서 읽는다. 구조적으로 맞는 입력이 동일 원본 추출의 증명은 아니다",
    "실제 승인 — 대상 사건과 유도 경로의 사건들이 모니터 승인 응답(proposal_digest)에 묶여 있는지, 승인 저장(저장 1)이 실제로 성공했는지 확인하지 않는다. 승인 저장 실패·결과 불명인 기록을 적용 완료 입력으로 건네면 알아내지 못한다",
    "실제 보관 — 유도 경로의 기준점·Verify 가 실제로 그 정책으로 관측·보관됐는지, manifest·blob 이 실재하는지 보지 않는다. 세계 이름이 건넨 등록부에서 조회되는지만 본다",
    "열린 사이클의 마지막 확정 세계 — 지금 서 있는 자리가 열린 사이클이면 범위 밖 결과다. Verify 계보에서 가장 가까운 세계를 고르는 유도는 하지 않는다",
    "실제 개방 허가 — 검증 ①·②는 유도 경로 위 사이클의 기록 일관성 검사다. 다음 Entry 선택, Experiment 를 열어도 되는지의 판정, 실제 개방 전이가 아니다. 유도 경로 밖 사이클(버려진 가지 등)의 ①·②는 보지 않는다",
    "대상이 아닌 사건의 T8·review_cycle_ref — world_before_ref 와 review_cycle_ref 는 대상 사건 하나에 대해서만 견준다",
    "review_cycle_ref 와 승인 경계의 관계 — 대상 사건의 실제 참조가 같은 Chain 의 닫힌 Interview 이고 승인 발급 번호보다 앞인지만 본다. 그 인터뷰가 변경안을 실제로 검토했는지, 승인 경계와 어떤 관계여야 하는지는 정하지 않는다",
    "보관 중단 표식·미완 복원 흔적 — 호출자가 있다고 건네면 범위 밖 결과이고, 건네지 않았으면 판단하지 않은 것으로 남긴다(not_judged). 요청·해제의 의미(O-2)를 정하지 않는다",
    "Chain 종료 경계·후속 Chain·정책 미채택 출처 — 종료 인터뷰를 닫고 보고서를 기다리는 경계, 닫힌 Chain, 부모가 있는 Chain, 첫 사건이 start 가 아닌 기록은 범위 밖 결과다. 미완료 사건의 이관(O-3)을 정하지 않는다",
    "restore_result 와 과거 경계로의 되돌아가기 — 대상이 될 수 없고, 대기 중에 승인 경계보다 앞선 경계에 서 있는 기록은 범위 밖 결과다(§6.4 는 6번 작업)",
    "두 시점의 보존 — 한 시점의 읽기다. 앞선 기록이 지워지거나 고쳐지지 않았는지는 3d·3f·3g·3i 의 두 시점 비교가 본다",
    "제품 연결 — 이 읽기를 부르는 제품 경로(CLI·MCP·Monitor·ProjectSession·load/save)가 없고 파일·state.yaml·객체 창고를 읽거나 쓰지 않는다. 형식 10/11 의 범위 변경 기록은 활성화되지 않았다",
];

// ── 입력 ───────────────────────────────────────────────────────────────────

/// 사건 하나에서 뽑아 건넨, 기존 입력(3c·3e·3h)에 없는 계약 칸 둘 — `review_cycle_ref` 와 `world_before_ref`(§5.1 사건 칸 표).
///
/// 새 저장 꼴이 아니다. 직렬화 자료형이 없고 `state.yaml` 에 연결되지 않는다. 칸 없음과 `null` 을 가르려고 3e 의 [`BaselineSlot`] 을 쓴다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EventWorldFacts {
    event: EventName,
    kind: EventKind,
    review: BaselineSlot<Option<CycleRef>>,
    world_before: BaselineSlot<Option<SnapshotRef>>,
}

impl EventWorldFacts {
    pub(crate) fn new(
        event: EventName,
        kind: EventKind,
        review: BaselineSlot<Option<CycleRef>>,
        world_before: BaselineSlot<Option<SnapshotRef>>,
    ) -> EventWorldFacts {
        EventWorldFacts { event, kind, review, world_before }
    }

    pub(crate) fn event(&self) -> EventName {
        self.event
    }

    pub(crate) fn kind(&self) -> EventKind {
        self.kind
    }

    /// `review_cycle_ref` 칸.
    pub(crate) fn review(&self) -> BaselineSlot<Option<CycleRef>> {
        self.review
    }

    /// `world_before_ref` 칸.
    pub(crate) fn world_before(&self) -> BaselineSlot<Option<SnapshotRef>> {
        self.world_before
    }
}

/// 범위 밖 표식 하나를 호출자가 기록에서 어떻게 읽어 건넸는가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MarkSeen {
    /// 건네지 않았다 — 이 읽기는 판단하지 않은 것으로 남긴다.
    NotProvided,
    /// 기록에 표식이 없다고 건넸다. 그 말의 진위는 보지 않는다.
    Absent,
    /// 기록에 표식이 있다고 건넸다 — 범위 밖 결과다.
    Present,
}

/// 이 읽기가 다루지 않는 두 상태의 표식 — 즉시 보관 중단(`archive_halt`, §5.6)과 미완 복원 흔적.
///
/// 있다고 건네면 범위 밖 결과로 돌려줄 뿐이며, 요청·해제의 의미(§12 O-2)를 정하지 않는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScopeMarks {
    pub(crate) archive_halt: MarkSeen,
    pub(crate) unfinished_restore: MarkSeen,
}

impl ScopeMarks {
    /// 두 표식 모두 건네지 않았다.
    pub(crate) fn not_provided() -> ScopeMarks {
        ScopeMarks { archive_halt: MarkSeen::NotProvided, unfinished_restore: MarkSeen::NotProvided }
    }
}

/// 한 시점의 입력 묶음 — 모든 검사와 선택이 이 값 하나에서 읽는다.
///
/// 빌린 값의 묶음일 뿐이며 그 자체로는 아무것도 검사되지 않았다. 같은 시점·같은 원본에서 뽑았다는 것은 호출자의 계약이다.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RecordInputs<'a> {
    pub(crate) ledger: &'a PolicyLedger,
    pub(crate) links: &'a EventPolicyLinks,
    pub(crate) facts: &'a [EventCycleFacts],
    pub(crate) worlds: &'a [EventWorldFacts],
    pub(crate) chains: &'a [Chain],
    pub(crate) registry: &'a SnapshotRegistry,
    pub(crate) marks: ScopeMarks,
}

// ── 결과 ───────────────────────────────────────────────────────────────────

/// 적용 정책과 그 출처 — 건넨 원장의 마지막 판(§5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AppliedPolicy {
    /// 원장의 마지막 판 이름.
    pub(crate) policy: PolicyName,
    /// 그 판을 도입한 `start`·`scope_change` 사건(3c).
    pub(crate) introduced_by: EventName,
    /// 건넨 목록의 마지막 사건 — 그 `policy_after` 가 이 판이다(T4).
    pub(crate) last_event: EventName,
}

/// 확정 정책 유도의 한 걸음 — 걸음마다 사건·판·세계를 따로 지닌다. 같은 세계 이름이어도 걸음을 합치지 않는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PolicyStep {
    /// 사건의 기준점 — 확정 정책은 그 사건의 `policy_after`(§6.1).
    EventBaseline {
        event: EventName,
        kind: EventKind,
        world: SnapshotRef,
        policy: PolicyName,
        /// 보관 경계 `baseline.after_cycle_ref`.
        archived_at: CycleRef,
        /// 보관 발급기 `baseline.next_cycle_id`.
        archive_issuer: u32,
    },
    /// 닫힌 Experiment 의 저장 Exit — 확정 정책은 P(T)(§6.1). Exit 계보가 일치했고, 뒤따르는 걸음이 그 Entry 의 출처(검증 ①·②)다.
    ExperimentExit {
        cycle: CycleRef,
        world: SnapshotRef,
        policy: PolicyName,
        /// P(T) 로 고른 사건.
        opened_under: EventName,
        /// 저장 Exit 과 일치한 계보 기대 세계의 출처(Artifact Model §7.5).
        lineage: ExpectedSource,
    },
    /// 닫힌 Interview 의 저장 Exit = Entry — 확정 정책은 뒤따르는 걸음(그 Entry 의 출처)에서 물려받는다.
    InterviewInherits { cycle: CycleRef, world: SnapshotRef },
    /// 뿌리 사이클의 Entry — `start` 사건의 기준점 = Chain Entry = 뿌리 Entry.
    StartRoot { cycle: CycleRef, chain: ChainRef, event: EventName, world: SnapshotRef, policy: PolicyName },
}

/// 지금 서 있는 닫힌 경계의 마지막 확정 세계와 그 확정 정책 — 건넨 기록 안의 유도다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConfirmedWorld {
    boundary: CycleRef,
    world: SnapshotRef,
    policy: PolicyName,
    path: Vec<PolicyStep>,
}

impl ConfirmedWorld {
    /// 지금 서 있는 닫힌 사이클 T.
    pub(crate) fn boundary(&self) -> CycleRef {
        self.boundary
    }

    /// B(T) 의 세계 이름.
    pub(crate) fn world(&self) -> SnapshotRef {
        self.world
    }

    /// 그 세계의 확정 정책 — 판 이름이다. 적용 정책과 다를 수 있다(보관 대기).
    pub(crate) fn policy(&self) -> PolicyName {
        self.policy
    }

    /// 유도 경로 — 첫 걸음이 경계의 자리이고, 뒤따르는 걸음이 그 출처다. 비어 있지 않다.
    pub(crate) fn path(&self) -> &[PolicyStep] {
        &self.path
    }
}

/// 대상 사건의 기준점 유무와 목록상 위치(3g) — 건넨 한 시점의 기록에서 읽은 값이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EventStanding {
    /// 기준점이 없는 마지막 사건 — 「기준점 보관 필요 — 다음 실험 보류」(§5.2). `attempts` 는 건넨 기록의 시도 수다.
    Waiting { attempts: usize },
    /// 기준점이 이미 적혀 있다. 다시 보관하지 않는다.
    AlreadyConfirmed { snapshot: SnapshotRef, archived_at: CycleRef, archive_issuer: u32, attempts: usize },
    /// 기준점 없이 뒤 사건으로 대체됐다. 다시 보관하지 않는다(§5.2).
    Replaced { by: EventName },
}

/// `world_before_ref` 의 두 번째 비교가 무엇이었는가(T8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SecondBasis {
    /// 대기 사건 — 지금 서 있는 경계의 마지막 확정 세계와 같다. 그 근거는 읽기의 [`ScopeChangeReading::confirmed`] 다.
    CurrentBoundary { boundary: CycleRef },
    /// 이미 확정된 사건 — 보관 경계에서 **대상 사건보다 앞의 기록만으로** 유도한 세계·확정 정책·유도 경로. 그 세계와 같다.
    ArchiveBoundary { basis: ConfirmedWorld },
    /// 대체된 사건 — 보관하지 않았으므로 두 번째 비교가 없다.
    NoneForReplaced,
}

/// 대상 사건의 `world_before_ref` 와 그것을 견준 근거. 이 값이 있다면 적힌 비교는 모두 일치했고, 근거마다 지금 경계와 같은 유도가 섰다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorldBefore {
    /// 사건에 적힌 `world_before_ref`.
    pub(crate) world: SnapshotRef,
    /// 승인 경계 `after_cycle_ref` 에서 **대상 사건보다 앞의 기록만으로** 유도한 세계·확정 정책·유도 경로. 그 세계와 같다.
    pub(crate) approval: ConfirmedWorld,
    pub(crate) second: SecondBasis,
}

/// 이번 호출이 **판단하지 않은** 칸 — 거절하지도 유효하다고 승인하지도 않았다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unjudged {
    /// 기준점이 없는 사건의 `baseline.after_cycle_ref` 칸이 없다 — 3h 는 판단하지 않았지만, 같은 호출의 3e 가 「기준점이 없으면 보관
    /// 경계·발급기 칸도 없다」를 확인했다. 보관 대기·대체된 사건의 **정상 부재**이며 확인이 빠진 것이 아니다.
    BaselineSlotsNotWritten { event: EventName },
    /// 3h 가 판단하지 않은 그 밖의 `null`·칸 없음의 사이클 참조 — 그 사건의 경계를 확인하지 못했다(시작 사건의 `null` 경계는 이 읽기가
    /// 따로 확인하므로 여기 없다).
    CycleRefAbsent { event: EventName, field: CycleField, reading: RefReading },
    /// 실제 사이클 참조가 없어 3h 가 `chain_ref` 의 실재를 조회하지 않은 사건(시작 사건은 이 읽기가 따로 확인한다).
    ChainNotLookedUp { event: EventName },
    /// 대상 사건의 `review_cycle_ref` 가 `null`(`true`)이거나 칸이 없다(`false`).
    ReviewCycleAbsent { event: EventName, written_null: bool },
    /// 보관 중단 표식을 건네받지 않았다(T12).
    ArchiveHaltNotProvided,
    /// 미완 복원 흔적을 건네받지 않았다.
    UnfinishedRestoreNotProvided,
}

/// 건넨 기록 안에서 읽은 범위 변경 사건 하나의 세 값 — **그것뿐이다.** 보이지 않는 것은 [`CONFIRMED_WORLD_DOES_NOT_SHOW`] 에 있다.
/// 이 값을 만드는 문은 [`read_scope_change`] 하나뿐이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScopeChangeReading {
    event: EventName,
    chain: ChainRef,
    issuer: u32,
    policy_before: Option<PolicyName>,
    policy_after: PolicyName,
    applied: AppliedPolicy,
    confirmed: ConfirmedWorld,
    standing: EventStanding,
    world_before: WorldBefore,
    not_judged: Vec<Unjudged>,
}

impl ScopeChangeReading {
    /// 대상 사건.
    pub(crate) fn event(&self) -> EventName {
        self.event
    }

    /// 대상 사건의 `chain_ref` — 지금 경계를 읽은 Chain 이다.
    pub(crate) fn chain(&self) -> ChainRef {
        self.chain
    }

    /// 그 Chain 의 지금 Cycle 발급기 값 — 지금 보관한다면 `baseline.next_cycle_id` 가 될 값이다.
    pub(crate) fn issuer(&self) -> u32 {
        self.issuer
    }

    /// 대상 사건의 `policy_before_ref`.
    pub(crate) fn policy_before(&self) -> Option<PolicyName> {
        self.policy_before
    }

    /// 대상 사건의 `policy_after_ref`. 대기 사건이면 적용 정책과 같다.
    pub(crate) fn policy_after(&self) -> PolicyName {
        self.policy_after
    }

    /// 적용 정책과 그 원장·사건 출처.
    pub(crate) fn applied(&self) -> AppliedPolicy {
        self.applied
    }

    /// 지금 경계의 마지막 확정 세계와 확정 정책·유도 경로.
    pub(crate) fn confirmed(&self) -> &ConfirmedWorld {
        &self.confirmed
    }

    /// 대상 사건의 대기 / 이미 확정 / 대체.
    pub(crate) fn standing(&self) -> EventStanding {
        self.standing
    }

    /// 대상 사건의 `world_before_ref` 와 견준 근거.
    pub(crate) fn world_before(&self) -> &WorldBefore {
        &self.world_before
    }

    /// 기준 일치(§6.1) — 마지막 확정 세계의 확정 정책이 적용 정책과 같은 판인가. 다르다고 손상이 아니다.
    pub(crate) fn basis_agrees(&self) -> bool {
        self.confirmed.policy == self.applied.policy
    }

    /// 이번 호출이 판단하지 않은 칸.
    pub(crate) fn not_judged(&self) -> &[Unjudged] {
        &self.not_judged
    }
}

// ── 실패 ───────────────────────────────────────────────────────────────────

/// 이 읽기가 **판정하지 않는** 기록. 손상 판정이 아니고, 제품이 그 상태를 허용·금지한다는 규칙도 아니다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScopeLimit {
    /// 대상이 `scope_change` 가 아니다.
    EventKind { event: EventName, kind: EventKind },
    /// 첫 사건이 `start` 가 아니다 — 최초 start 기준점에서 이어지지 않는 출처.
    NoStartOrigin { first: EventName, kind: EventKind },
    /// `start` 사건이 다른 Chain 의 것이다 — 후속 Chain 의 근거가 필요하다.
    StartInOtherChain { chain: ChainRef, start_chain: ChainRef },
    /// 대상 사건의 Chain 에 부모 Chain 이 있다 — 후속 Chain.
    ChainHasParent { chain: ChainRef, parent: ChainRef },
    /// 대상 사건의 Chain 이 닫혔다.
    ChainClosed { chain: ChainRef },
    /// 종료 인터뷰를 닫고 Chain 보고서를 기다리는 경계다(§6.6, O-3).
    ChainClosingBoundary { chain: ChainRef },
    /// 열린 사이클 안에 서 있다 — 닫힌 경계가 아니다.
    OpenCycle { chain: ChainRef, cycle: CycleRef },
    /// 대기 사건인데 되돌아온 뒤 새 사이클을 열지 않은 자리다.
    RevisitWhileWaiting { event: EventName, boundary: CycleRef },
    /// 대기 사건인데 지금 경계가 승인 경계도, 승인 뒤에 연 사이클도 아니다.
    BoundaryBeforeApproval { event: EventName, boundary: CycleRef, approval: CycleRef },
    /// 보관 중단 표식이 있다고 건네받았다.
    ArchiveHaltPresent,
    /// 미완 복원 흔적이 있다고 건네받았다.
    UnfinishedRestorePresent,
}

/// 보충 입력이 사건 목록과 대응하지 않는 자리. 자리는 0 부터 센다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorldFactsMismatch {
    CountDiffers { events: usize, worlds: usize },
    NameDiffers { position: usize, event: EventName, found: EventName },
    KindDiffers { event: EventName, kind: EventKind, found: EventKind },
}

/// `start` 기준점·Chain Entry·뿌리 Entry 의 연결이 서지 않는 이유.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartProblem {
    /// 시작 사건의 승인 경계나 보관 경계가 `null` 로 적혀 있지 않다(§5.1 예시).
    BoundaryNotNull { event: EventName },
    /// 시작 사건의 승인 발급기와 보관 발급기가 다르다 — 한 번의 저장이 아니다(§5.3).
    IssuersDiffer { event: EventName },
    /// 시작 사건의 기준점이 Chain Entry 와 다르다.
    NotChainEntry { event: EventName, baseline: SnapshotRef, chain_entry: SnapshotRef },
    /// Chain 의 뿌리 사이클이 하나가 아니다.
    RootNotSingle { chain: ChainRef, roots: usize },
    /// 시작 사건의 기준점이 뿌리 사이클의 Entry 와 다르다.
    NotRootEntry { event: EventName, baseline: SnapshotRef, root: CycleRef, root_entry: SnapshotRef },
    /// 시작 사건의 발급기가 뿌리 사이클의 번호보다 크다 — 뿌리보다 늦게 저장됐다고 적혀 있다.
    IssuerAfterRoot { event: EventName, issuer: u32, root: CycleRef },
}

/// `world_before_ref` 를 견준 자리.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BasisAt {
    /// 승인 경계(사건의 `after_cycle_ref`)의 앞선 기록.
    Approval,
    /// 지금 서 있는 경계(대기 사건).
    CurrentBoundary,
    /// 보관 경계(`baseline.after_cycle_ref`)의 앞선 기록(이미 확정된 사건).
    Archive,
}

/// `review_cycle_ref` 의 실제 참조가 맞지 않는 이유.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReviewProblem {
    /// 건넨 목록에서 하나로 정할 수 없다.
    Target(TargetProblem),
    /// 사건의 `chain_ref` 와 다른 Chain 의 사이클이다.
    OtherChain { owner: ChainRef },
    /// Interview 가 아니다.
    NotInterview,
    /// 닫히지 않았다.
    NotClosed,
    /// 번호가 사건의 승인 발급 번호보다 작지 않다 — 승인보다 늦게 연 사이클이다.
    NotBeforeApproval { issuer: u32 },
}

/// 읽지 못한 갈래. **어느 것도 「정책 채택 전」·「기준점 없음」·성공이 아니다.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReadingProblem {
    /// 이 읽기의 범위 밖이다 — 판정하지 않았다.
    OutOfScope(ScopeLimit),
    /// 같은 호출에 건넨 입력이 3j 를 통과하지 못했다 — 그 실패 그대로.
    Inputs(Vec<EventInputsProblem>),
    /// 보충 입력이 사건 목록과 사건 수·자리별 이름·종류로 대응하지 않는다.
    WorldFacts(Vec<WorldFactsMismatch>),
    /// 3g 의 위치 구분이 서지 않았다.
    Positions(Vec<PositionProblem>),
    /// 대상 사건이 건넨 목록에 없다.
    UnknownEvent { event: EventName, events: usize },
    /// 대상 사건의 `chain_ref` 가 건넨 목록에 `count` 번 있다(0 또는 둘 이상).
    EventChain { event: EventName, chain: ChainRef, count: usize },
    /// `start` 기준점·Chain Entry·뿌리 Entry 의 연결이 서지 않는다.
    Start(StartProblem),
    /// 지금 경계의 B(T) 를 고르지 못했다 — 그 실패 그대로.
    Boundary(Vec<BoundaryWorldProblem>),
    /// 유도 경로의 사이클을 건넨 목록에서 하나로 정할 수 없다.
    Cycle(TargetProblem),
    /// 유도 경로의 사이클이 대상 사건의 Chain 밖에 있다.
    CycleOutsideChain { cycle: CycleRef, owner: ChainRef },
    /// 유도 경로의 Interview 가 닫히지 않았거나 Exit 이 없다.
    InterviewNotClosed { cycle: CycleRef },
    /// 닫힌 Experiment 의 Exit·P(T) 를 유도하지 못했다 — 그 실패 그대로.
    ExperimentExit { cycle: CycleRef, found: Vec<ExperimentExitProblem> },
    /// 닫힌 Experiment 의 Exit 계보를 검사하지 못했다 — 그 실패 그대로.
    ExitLineage { cycle: CycleRef, found: Vec<ExitLineageProblem> },
    /// 닫힌 Experiment 의 저장 Exit 이 계보 기대 세계와 다르다.
    ExitLineageDiffers { cycle: CycleRef, stored: SnapshotRef, expected: SnapshotRef, source: ExpectedSource },
    /// 유도 경로의 Experiment 를 열 때 건넨 이력 안에 적용된 정책이 없었다 — start 에서 이어진 기록에서 설 수 없는 값이다.
    ExperimentBeforeAdoption { cycle: CycleRef },
    /// 검증 ② — Experiment 의 Entry 출처의 확정 정책이 그 실험을 열 때의 적용 정책 P(T) 와 다르다.
    ExperimentPolicyDiffers { cycle: CycleRef, opened_under: PolicyName, entry_confirmed: PolicyName },
    /// Interview 의 저장 Exit 이 Entry 와 다르다(§6.1, Artifact Model §7.5).
    InterviewExitNotEntry { cycle: CycleRef, entry: SnapshotRef, exit: SnapshotRef },
    /// 비뿌리 사이클의 S(N) 을 고르지 못했다 — 그 실패 그대로.
    OpeningWorld { cycle: CycleRef, found: Vec<OpeningWorldProblem> },
    /// 검증 ① — 사이클의 Entry 가 S(N) 의 세계와 다르다.
    EntryDiffers { cycle: CycleRef, entry: SnapshotRef, expected: SnapshotRef },
    /// 뿌리 사이클의 Entry 가 시작 사건의 기준점과 다르다.
    RootEntryDiffers { cycle: CycleRef, entry: SnapshotRef, start: SnapshotRef },
    /// 구조적 부모를 따라간 걸음이 Chain 의 사이클 수를 넘었다 — 순환이다.
    LineageLoops { cycle: CycleRef },
    /// T9 — 대상 사건의 승인 저장과 기준점 보관(또는 대체) 사이에 연 Experiment 가 있다.
    ExperimentWhileWaiting { event: EventName, cycle: CycleRef },
    /// 대상 사건의 승인 경계 `after_cycle_ref` 가 실제 사이클 참조가 아니다.
    ApprovalBoundaryMissing { event: EventName },
    /// 기준점이 있는 대상 사건의 보관 경계·발급기가 실제 값이 아니다.
    ArchiveBoundaryMissing { event: EventName },
    /// 대상 사건의 `world_before_ref` 가 `null`(`true`)이거나 칸이 없다(`false`).
    WorldBeforeMissing { event: EventName, written_null: bool },
    /// 대상 사건의 `world_before_ref` 가 건넨 등록부에 없다.
    WorldBeforeNotInRegistry { event: EventName, world: SnapshotRef },
    /// 대상 사건보다 앞의 기록만 남긴 입력을 세우지 못했다.
    PriorRecords { event: EventName },
    /// 앞선 기록에서 그 경계의 세계와 확정 정책을 지금 경계와 같은 유도로 얻지 못했다 — 그 유도의 실패 그대로.
    PriorBasis { event: EventName, at: BasisAt, boundary: CycleRef, found: Vec<ReadingProblem> },
    /// 승인 경계의 근거와 두 번째 근거의 확정 정책이 다른 판이다 — 세계 이름이 같아도 한 비교 근거가 아니다.
    BasisPolicyDiffers { event: EventName, at: BasisAt, approval: PolicyName, second: PolicyName },
    /// 앞선 사건의 기준점이 견주는 발급 번호보다 늦게 보관됐다고 적혀 있다 — 선후를 정할 수 없다.
    PriorBaselineNotBefore { event: EventName, at: BasisAt, basis: EventName, archive_issuer: u32, bound: u32 },
    /// T8 — `world_before_ref` 가 유도한 세계와 다르다.
    WorldBeforeDiffers { event: EventName, at: BasisAt, boundary: CycleRef, recorded: SnapshotRef, derived: SnapshotRef },
    /// 대상 사건의 `review_cycle_ref` 실제 참조가 같은 Chain 의 닫힌 Interview 가 아니다.
    ReviewCycle { event: EventName, cycle: CycleRef, why: ReviewProblem },
}

fn listed<T: ToString>(items: &[T]) -> String {
    items.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
}

impl fmt::Display for ReadingProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadingProblem::OutOfScope(limit) => write!(f, "이 읽기의 범위 밖이라 판정하지 않았다: {limit:?}"),
            ReadingProblem::Inputs(found) => write!(f, "건넨 사건 입력이 맞지 않는다: {}", listed(found)),
            ReadingProblem::WorldFacts(found) => {
                write!(f, "review_cycle_ref·world_before_ref 입력이 사건 목록과 대응하지 않는다: {found:?}")
            }
            ReadingProblem::Positions(found) => write!(f, "기준점 위치를 구분하지 못했다: {}", listed(found)),
            ReadingProblem::UnknownEvent { event, events } => {
                write!(f, "{event} 은 건넨 사건 {events}개 안에 없다")
            }
            ReadingProblem::EventChain { event, chain, count } => {
                write!(f, "{event} 의 chain_ref {chain} 이 건넨 목록에 {count}번 있다")
            }
            ReadingProblem::Start(problem) => {
                write!(f, "start 기준점·Chain Entry·뿌리 Entry 의 연결이 서지 않는다: {problem:?}")
            }
            ReadingProblem::Boundary(found) => write!(f, "지금 경계의 세계를 고르지 못했다: {}", listed(found)),
            ReadingProblem::Cycle(problem) => write!(f, "유도 경로의 사이클을 정하지 못했다: {problem}"),
            ReadingProblem::CycleOutsideChain { cycle, owner } => {
                write!(f, "유도 경로의 {cycle} 이 대상 사건의 Chain 이 아닌 {owner} 에 있다")
            }
            ReadingProblem::InterviewNotClosed { cycle } => {
                write!(f, "유도 경로의 Interview {cycle} 이 닫히지 않았거나 Exit 이 없다")
            }
            ReadingProblem::ExperimentExit { cycle, found } => {
                write!(f, "Experiment {cycle} 의 Exit 정책을 유도하지 못했다: {}", listed(found))
            }
            ReadingProblem::ExitLineage { cycle, found } => {
                write!(f, "Experiment {cycle} 의 Exit 계보를 검사하지 못했다: {}", listed(found))
            }
            ReadingProblem::ExitLineageDiffers { cycle, stored, expected, source } => write!(
                f,
                "Experiment {cycle} 의 저장 Exit {stored} 이 계보 기대 세계 {expected}({source}) 와 다르다"
            ),
            ReadingProblem::ExperimentBeforeAdoption { cycle } => {
                write!(f, "Experiment {cycle} 을 열 때 건넨 이력 안에 적용된 정책이 없다")
            }
            ReadingProblem::ExperimentPolicyDiffers { cycle, opened_under, entry_confirmed } => write!(
                f,
                "Experiment {cycle} 을 열 때의 적용 정책은 {opened_under} 인데 그 Entry 출처의 확정 정책은 {entry_confirmed} 다(검증 ②)"
            ),
            ReadingProblem::InterviewExitNotEntry { cycle, entry, exit } => {
                write!(f, "Interview {cycle} 의 저장 Exit {exit} 이 Entry {entry} 와 다르다")
            }
            ReadingProblem::OpeningWorld { cycle, found } => {
                write!(f, "{cycle} 의 세계 출처를 고르지 못했다: {}", listed(found))
            }
            ReadingProblem::EntryDiffers { cycle, entry, expected } => {
                write!(f, "{cycle} 의 Entry {entry} 가 그 세계 출처 {expected} 와 다르다(검증 ①)")
            }
            ReadingProblem::RootEntryDiffers { cycle, entry, start } => {
                write!(f, "뿌리 {cycle} 의 Entry {entry} 가 시작 사건의 기준점 {start} 과 다르다")
            }
            ReadingProblem::LineageLoops { cycle } => {
                write!(f, "{cycle} 에서 구조적 부모를 따라간 걸음이 Chain 의 사이클 수를 넘었다")
            }
            ReadingProblem::ExperimentWhileWaiting { event, cycle } => {
                write!(f, "{event} 의 승인 저장과 기준점 보관 사이에 연 Experiment {cycle} 이 있다(T9)")
            }
            ReadingProblem::ApprovalBoundaryMissing { event } => {
                write!(f, "{event} 의 after_cycle_ref 가 실제 사이클 참조가 아니다")
            }
            ReadingProblem::ArchiveBoundaryMissing { event } => {
                write!(f, "{event} 의 baseline.after_cycle_ref·next_cycle_id 가 실제 값이 아니다")
            }
            ReadingProblem::WorldBeforeMissing { event, written_null } => match written_null {
                true => write!(f, "{event} 의 world_before_ref 가 null 이다"),
                false => write!(f, "{event} 의 world_before_ref 칸이 없다"),
            },
            ReadingProblem::WorldBeforeNotInRegistry { event, world } => {
                write!(f, "{event} 의 world_before_ref {world} 이 건넨 등록부에 없다")
            }
            ReadingProblem::PriorRecords { event } => {
                write!(f, "{event} 보다 앞의 기록만 남긴 입력을 세우지 못했다")
            }
            ReadingProblem::PriorBasis { event, at, boundary, found } => write!(
                f,
                "{event} 의 앞선 기록에서 {boundary} 의 세계와 확정 정책({at:?})을 유도하지 못했다: {}",
                listed(found)
            ),
            ReadingProblem::BasisPolicyDiffers { event, at, approval, second } => write!(
                f,
                "{event} 의 승인 경계 근거의 확정 정책 {approval} 과 두 번째 근거({at:?})의 확정 정책 {second} 이 다르다"
            ),
            ReadingProblem::PriorBaselineNotBefore { event, at, basis, archive_issuer, bound } => write!(
                f,
                "{event} 의 근거({at:?})로 고른 {basis} 의 기준점은 발급기 {archive_issuer} 에서 보관됐다고 적혀 있어 견주는 발급 번호 {bound} 보다 늦다"
            ),
            ReadingProblem::WorldBeforeDiffers { event, at, boundary, recorded, derived } => write!(
                f,
                "{event} 의 world_before_ref {recorded} 이 {boundary} 에서 유도한 세계 {derived}({at:?}) 와 다르다(T8)"
            ),
            ReadingProblem::ReviewCycle { event, cycle, why } => {
                write!(f, "{event} 의 review_cycle_ref {cycle} 이 같은 Chain 의 닫힌 Interview 가 아니다: {why:?}")
            }
        }
    }
}

type Problems = Vec<ReadingProblem>;

fn one<T>(problem: ReadingProblem) -> Result<T, Problems> {
    Err(vec![problem])
}

// ── start 기준점 ───────────────────────────────────────────────────────────

/// 확인된 최초 출처 — `start` 사건의 기준점이 Chain Entry 이고 뿌리 Entry 다.
#[derive(Debug, Clone, Copy)]
struct StartLink {
    event: EventName,
    world: SnapshotRef,
    policy: PolicyName,
}

fn start_link(chain: &Chain, links: &EventPolicyLinks, facts: &[EventCycleFacts]) -> Result<StartLink, ReadingProblem> {
    let (link, fact) = (&links.events()[0], &facts[0]);
    let event = link.name();
    if link.kind() != EventKind::Start {
        return Err(ReadingProblem::OutOfScope(ScopeLimit::NoStartOrigin { first: event, kind: link.kind() }));
    }
    if fact.chain() != chain.id() {
        return Err(ReadingProblem::OutOfScope(ScopeLimit::StartInOtherChain { chain: chain.id(), start_chain: fact.chain() }));
    }
    let baseline = fact.baseline();
    let null = BaselineSlot::Written(None);
    let (Some(world), true) = (baseline.snapshot(), fact.boundary() == null && baseline.boundary() == null) else {
        return Err(ReadingProblem::Start(StartProblem::BoundaryNotNull { event }));
    };
    if baseline.issuer() != BaselineSlot::Written(fact.issuer()) {
        return Err(ReadingProblem::Start(StartProblem::IssuersDiffer { event }));
    }
    if chain.entry_snapshot() != world {
        let chain_entry = chain.entry_snapshot();
        return Err(ReadingProblem::Start(StartProblem::NotChainEntry { event, baseline: world, chain_entry }));
    }
    let roots: Vec<&Cycle> = chain.cycles().nodes().iter().filter(|cycle| cycle.parent().is_none()).collect();
    let [root] = roots.as_slice() else {
        return Err(ReadingProblem::Start(StartProblem::RootNotSingle { chain: chain.id(), roots: roots.len() }));
    };
    let root_ref = root.id().to_ref();
    if root.entry_snapshot() != world {
        let root_entry = root.entry_snapshot();
        return Err(ReadingProblem::Start(StartProblem::NotRootEntry { event, baseline: world, root: root_ref, root_entry }));
    }
    if fact.issuer() > root_ref.number() {
        return Err(ReadingProblem::Start(StartProblem::IssuerAfterRoot { event, issuer: fact.issuer(), root: root_ref }));
    }
    let policy = link.after().expect("3c 의 성공은 모든 사건의 after 에 판이 있음을 보장한다");
    Ok(StartLink { event, world, policy })
}

// ── 확정 정책의 유도 ───────────────────────────────────────────────────────

/// 한 호출의 입력과 확인된 최초 출처 — 유도의 모든 걸음이 이 값에서 읽는다.
struct Derivation<'a, 'b> {
    inputs: &'b RecordInputs<'a>,
    chain: &'a Chain,
    start: StartLink,
}

impl Derivation<'_, '_> {
    /// 닫힌 경계 `target` 의 B(T) 와 그 확정 정책.
    fn boundary(&self, target: CycleRef) -> Result<ConfirmedWorld, Problems> {
        let i = self.inputs;
        let chosen = select_boundary_world(target, i.ledger, i.links, i.facts, i.chains, i.registry)
            .map_err(|found| vec![ReadingProblem::Boundary(found)])?;
        let mut path = Vec::new();
        let (world, policy) = match chosen.source() {
            BoundarySource::EventBaseline { event, kind, world, policy, archived_at, archive_issuer } => {
                path.push(PolicyStep::EventBaseline { event, kind, world, policy, archived_at, archive_issuer });
                (world, policy)
            }
            BoundarySource::ExitOfTarget { world } => {
                let (exit, policy) = self.exit(target, &mut path, self.chain.cycles().nodes().len())?;
                debug_assert_eq!(exit, world, "B(T) 가 고른 Exit 과 유도가 읽은 Exit 은 같은 칸이다");
                (exit, policy)
            }
        };
        Ok(ConfirmedWorld { boundary: target, world, policy, path })
    }

    /// 닫힌 사이클 `target` 의 **저장 Exit** 과 그 확정 정책 — Experiment 는 P(T), Interview 는 Entry 의 출처에서 계승.
    fn exit(&self, target: CycleRef, path: &mut Vec<PolicyStep>, left: usize) -> Result<(SnapshotRef, PolicyName), Problems> {
        let Some(left) = left.checked_sub(1) else {
            return one(ReadingProblem::LineageLoops { cycle: target });
        };
        let i = self.inputs;
        let (owner, cycle) = owned_cycle(target, i.chains).map_err(|problem| vec![ReadingProblem::Cycle(problem)])?;
        if owner != self.chain.id() {
            return one(ReadingProblem::CycleOutsideChain { cycle: target, owner });
        }
        match cycle.kind() {
            CycleKind::Experiment => {
                let exit = select_experiment_exit(target, i.ledger, i.links, i.facts, i.chains, i.registry)
                    .map_err(|found| vec![ReadingProblem::ExperimentExit { cycle: target, found }])?;
                let lineage = check_experiment_exit_lineage(target, i.chains, i.registry)
                    .map_err(|found| vec![ReadingProblem::ExitLineage { cycle: target, found }])?;
                if lineage.verdict() == ExitLineageVerdict::Differs {
                    return one(ReadingProblem::ExitLineageDiffers {
                        cycle: target,
                        stored: lineage.stored(),
                        expected: lineage.expected(),
                        source: lineage.source(),
                    });
                }
                let OpeningSelection::Applied { event: opened_under, policy, .. } = exit.policy() else {
                    return one(ReadingProblem::ExperimentBeforeAdoption { cycle: target });
                };
                let world = exit.world();
                path.push(PolicyStep::ExperimentExit { cycle: target, world, policy, opened_under, lineage: lineage.source() });
                // 검증 ①·② — Entry 의 출처와 그 확정 정책. 실제 개방 허가가 아니라 기록의 일관성이다.
                let (_, entry_confirmed) = self.entry(target, cycle, path, left)?;
                if entry_confirmed != policy {
                    return one(ReadingProblem::ExperimentPolicyDiffers { cycle: target, opened_under: policy, entry_confirmed });
                }
                Ok((world, policy))
            }
            CycleKind::Interview => {
                let (Some(exit), true) = (cycle.exit_snapshot(), cycle.is_closed()) else {
                    return one(ReadingProblem::InterviewNotClosed { cycle: target });
                };
                let entry = cycle.entry_snapshot();
                if exit != entry {
                    return one(ReadingProblem::InterviewExitNotEntry { cycle: target, entry, exit });
                }
                path.push(PolicyStep::InterviewInherits { cycle: target, world: exit });
                let (source, policy) = self.entry(target, cycle, path, left)?;
                debug_assert_eq!(source, entry, "검증 ① 을 통과한 Entry 는 그 출처의 세계다");
                Ok((exit, policy))
            }
        }
    }

    /// 사이클 `target` 의 **Entry 의 출처**와 그 확정 정책 — 검증 ① 을 함께 본다. 뿌리는 start 기준점이다.
    fn entry(
        &self,
        target: CycleRef,
        cycle: &Cycle,
        path: &mut Vec<PolicyStep>,
        left: usize,
    ) -> Result<(SnapshotRef, PolicyName), Problems> {
        let i = self.inputs;
        if cycle.parent().is_none() {
            let (entry, start) = (cycle.entry_snapshot(), self.start);
            if entry != start.world {
                return one(ReadingProblem::RootEntryDiffers { cycle: target, entry, start: start.world });
            }
            path.push(PolicyStep::StartRoot {
                cycle: target,
                chain: self.chain.id(),
                event: start.event,
                world: start.world,
                policy: start.policy,
            });
            return Ok((start.world, start.policy));
        }
        let opening = select_opening_world(target, i.ledger, i.links, i.facts, i.chains, i.registry)
            .map_err(|found| vec![ReadingProblem::OpeningWorld { cycle: target, found }])?;
        if opening.check_entry().verdict() == EntryVerdict::Differs {
            return one(ReadingProblem::EntryDiffers { cycle: target, entry: opening.entry(), expected: opening.expected() });
        }
        match opening.source() {
            WorldSource::EventBaseline { event, kind, world, policy, archived_at, archive_issuer } => {
                path.push(PolicyStep::EventBaseline { event, kind, world, policy, archived_at, archive_issuer });
                Ok((world, policy))
            }
            WorldSource::ParentExit { world } => {
                let (exit, policy) = self.exit(opening.parent(), path, left)?;
                debug_assert_eq!(exit, world, "S(N) 이 고른 부모 Exit 과 유도가 읽은 Exit 은 같은 칸이다");
                Ok((exit, policy))
            }
        }
    }
}

// ── world_before_ref 의 근거 ───────────────────────────────────────────────

/// 대상 사건(자리 `index`)보다 **앞의 사건만** 남긴 입력에서 `boundary` 의 세계와 확정 정책을 **지금 경계와 같은 유도**로 얻는다.
///
/// 앞의 사건만 남긴 원장·정책 연결·사이클 칸·보충 칸으로 한 입력 묶음을 세우고 [`Derivation::boundary`] 를 그대로 부른다 — B(T) 선택,
/// Interview 의 Entry 계승, Experiment 의 Exit 계보와 검증 ①·②, 보관 발급기 필터, start 기준점이 모두 같은 코드다. 대상 사건 자신과 그
/// 뒤의 사건은 입력에 없으므로 그 판·기준점이 근거가 되지 않는다. 고른 것이 앞선 사건의 기준점이면 그 보관 발급기가 `bound` 보다 크지
/// 않아야 한다.
fn prior_basis(
    inputs: &RecordInputs<'_>,
    chain: &Chain,
    start: StartLink,
    index: usize,
    event: EventName,
    boundary: CycleRef,
    bound: u32,
    at: BasisAt,
) -> Result<ConfirmedWorld, Problems> {
    let unbuildable = || vec![ReadingProblem::PriorRecords { event }];
    // 3c 가 성공했으므로 앞의 사건들이 도입한 판은 P1 … 대상 사건의 policy_before 다(T3·도입 순서).
    let boards = inputs.links.events()[index].before().map_or(0, |policy| policy.number());
    let kept = usize::try_from(boards).ok().and_then(|count| inputs.ledger.policies().get(..count)).ok_or_else(unbuildable)?;
    let ledger = PolicyLedger::from_records(boards.checked_add(1).ok_or_else(unbuildable)?, kept.to_vec())
        .map_err(|_| unbuildable())?;
    let links = EventPolicyLinks::from_links(event.number(), inputs.links.events()[..index].to_vec())
        .map_err(|_| unbuildable())?;
    let earlier = RecordInputs {
        ledger: &ledger,
        links: &links,
        facts: &inputs.facts[..index],
        worlds: &inputs.worlds[..index],
        chains: inputs.chains,
        registry: inputs.registry,
        marks: inputs.marks,
    };
    let basis = Derivation { inputs: &earlier, chain, start }
        .boundary(boundary)
        .map_err(|found| vec![ReadingProblem::PriorBasis { event, at, boundary, found }])?;
    if let Some(PolicyStep::EventBaseline { event: basis_event, archive_issuer, .. }) = basis.path.first()
        && *archive_issuer > bound
    {
        return one(ReadingProblem::PriorBaselineNotBefore { event, at, basis: *basis_event, archive_issuer: *archive_issuer, bound });
    }
    Ok(basis)
}

/// 대상 사건의 `review_cycle_ref` — 실제 참조면 같은 Chain 의 닫힌 Interview 이고 승인 발급 번호보다 앞인지 본다.
fn review_cycle(
    world_facts: &EventWorldFacts,
    fact: &EventCycleFacts,
    chains: &[Chain],
) -> Result<Option<Unjudged>, ReadingProblem> {
    let event = fact.event();
    let cycle = match world_facts.review() {
        BaselineSlot::Written(Some(cycle)) => cycle,
        BaselineSlot::Written(None) => return Ok(Some(Unjudged::ReviewCycleAbsent { event, written_null: true })),
        BaselineSlot::Unwritten => return Ok(Some(Unjudged::ReviewCycleAbsent { event, written_null: false })),
    };
    let refuse = |why| Err(ReadingProblem::ReviewCycle { event, cycle, why });
    let (owner, node) = match owned_cycle(cycle, chains) {
        Ok(found) => found,
        Err(problem) => return refuse(ReviewProblem::Target(problem)),
    };
    if owner != fact.chain() {
        return refuse(ReviewProblem::OtherChain { owner });
    }
    if node.kind() != CycleKind::Interview {
        return refuse(ReviewProblem::NotInterview);
    }
    if !node.is_closed() {
        return refuse(ReviewProblem::NotClosed);
    }
    if cycle.number() >= fact.issuer() {
        return refuse(ReviewProblem::NotBeforeApproval { issuer: fact.issuer() });
    }
    Ok(None)
}

fn world_facts_mismatches(links: &EventPolicyLinks, worlds: &[EventWorldFacts]) -> Vec<WorldFactsMismatch> {
    let events = links.events();
    let mut found = Vec::new();
    if events.len() != worlds.len() {
        found.push(WorldFactsMismatch::CountDiffers { events: events.len(), worlds: worlds.len() });
    }
    for (position, (link, world)) in events.iter().zip(worlds).enumerate() {
        if link.name() != world.event {
            found.push(WorldFactsMismatch::NameDiffers { position, event: link.name(), found: world.event });
        } else if link.kind() != world.kind {
            found.push(WorldFactsMismatch::KindDiffers { event: link.name(), kind: link.kind(), found: world.kind });
        }
    }
    found
}

// ── 읽기 ───────────────────────────────────────────────────────────────────

/// 건넨 한 시점의 기록에서 범위 변경 사건 `event` 의 적용 정책·마지막 확정 세계와 확정 정책·기준점 유무를 따로 읽는다. **순수 함수**다.
///
/// 입력·체인·등록부를 바꾸지 않고 파일·상태·창고를 읽거나 쓰지 않는다. 관측·보관·재시도를 시작하지 않는다. 읽지 못하면 그 갈래를
/// 돌려주며, 어느 갈래도 성공값·정책 채택 전·기준점 없음으로 바꾸지 않는다.
pub(crate) fn read_scope_change(event: EventName, inputs: &RecordInputs<'_>) -> Result<ScopeChangeReading, Vec<ReadingProblem>> {
    let RecordInputs { ledger, links, facts, worlds, chains, registry, marks } = *inputs;

    // 범위 밖 표식 — 있다고 건네받았으면 아무것도 판정하지 않는다.
    let mut limits = Vec::new();
    if marks.archive_halt == MarkSeen::Present {
        limits.push(ReadingProblem::OutOfScope(ScopeLimit::ArchiveHaltPresent));
    }
    if marks.unfinished_restore == MarkSeen::Present {
        limits.push(ReadingProblem::OutOfScope(ScopeLimit::UnfinishedRestorePresent));
    }
    if !limits.is_empty() {
        return Err(limits);
    }

    // 입력 사이의 대응 — 3j 와 보충 입력.
    let mut problems = Vec::new();
    let agreed = check_event_inputs(ledger, links, facts, chains, registry);
    if let Err(found) = &agreed {
        problems.push(ReadingProblem::Inputs(found.clone()));
    }
    let mismatches = world_facts_mismatches(links, worlds);
    if !mismatches.is_empty() {
        problems.push(ReadingProblem::WorldFacts(mismatches));
    }
    let agreed = match agreed {
        Ok(agreed) if problems.is_empty() => agreed,
        _ => return Err(problems),
    };
    let baselines: Vec<BaselineFacts> = facts.iter().map(|fact| fact.baseline().clone()).collect();
    let positions =
        classify_baseline_positions(&baselines, registry).map_err(|found| vec![ReadingProblem::Positions(found)])?;

    // 대상 사건과 그 Chain.
    let index = usize::try_from(event.number()).ok().and_then(|number| number.checked_sub(1)).filter(|at| *at < facts.len());
    let Some(index) = index else {
        return one(ReadingProblem::UnknownEvent { event, events: facts.len() });
    };
    let (link, fact, world_facts) = (&links.events()[index], &facts[index], &worlds[index]);
    if link.kind() != EventKind::ScopeChange {
        return one(ReadingProblem::OutOfScope(ScopeLimit::EventKind { event, kind: link.kind() }));
    }
    let owners: Vec<&Chain> = chains.iter().filter(|chain| chain.id() == fact.chain()).collect();
    let [chain] = owners.as_slice() else {
        return one(ReadingProblem::EventChain { event, chain: fact.chain(), count: owners.len() });
    };
    let chain: &Chain = *chain;

    // 범위 — 최초 start 에서 이어진 열린 Chain 의 닫힌 경계.
    let start = start_link(chain, links, facts).map_err(|problem| vec![problem])?;
    if let Some(parent) = chain.parent() {
        return one(ReadingProblem::OutOfScope(ScopeLimit::ChainHasParent { chain: chain.id(), parent }));
    }
    if chain.is_closed() {
        return one(ReadingProblem::OutOfScope(ScopeLimit::ChainClosed { chain: chain.id() }));
    }
    if chain.awaiting_report() {
        return one(ReadingProblem::OutOfScope(ScopeLimit::ChainClosingBoundary { chain: chain.id() }));
    }
    let standing_on = chain.cycles().current();
    let boundary = standing_on.id().to_ref();
    if !standing_on.is_closed() {
        return one(ReadingProblem::OutOfScope(ScopeLimit::OpenCycle { chain: chain.id(), cycle: boundary }));
    }
    let issuer = chain.cycles().next_name();

    // 적용 정책 — 원장의 마지막 판과 그 출처.
    let last = links.events().last().expect("대상 사건이 있으므로 목록이 비어 있지 않다");
    let applied_policy = last.after().expect("3c 의 성공은 모든 사건의 after 에 판이 있음을 보장한다");
    let applied = AppliedPolicy {
        policy: applied_policy,
        introduced_by: agreed.policy_links().introduced_by(applied_policy).expect("3c 의 성공은 원장의 모든 판에 도입 사건이 있음을 보장한다"),
        last_event: last.name(),
    };

    // 마지막 확정 세계와 확정 정책 — 지금 경계의 B(T) 와 §6.1 의 유도.
    let confirmed = Derivation { inputs, chain, start }.boundary(boundary)?;

    // 대상 사건의 위치와, 그 승인 뒤에 연 사이클(T9).
    let position = positions
        .entries()
        .iter()
        .find(|(name, _)| *name == event)
        .map(|(_, position)| *position)
        .expect("위치 목록은 건넨 사건마다 하나씩 있다");
    let baseline = fact.baseline();
    let attempts = baseline.attempts().len();
    let (standing, opened_until) = match position {
        BaselinePosition::WaitingInList => (EventStanding::Waiting { attempts }, u32::MAX),
        BaselinePosition::ReplacedInList => {
            let next = &facts[index + 1];
            (EventStanding::Replaced { by: next.event() }, next.issuer())
        }
        BaselinePosition::HasBaseline => {
            let (Some(snapshot), BaselineSlot::Written(Some(archived_at)), BaselineSlot::Written(archive_issuer)) =
                (baseline.snapshot(), baseline.boundary(), baseline.issuer())
            else {
                return one(ReadingProblem::ArchiveBoundaryMissing { event });
            };
            (EventStanding::AlreadyConfirmed { snapshot, archived_at, archive_issuer, attempts }, archive_issuer)
        }
    };
    let mut problems = Vec::new();
    for cycle in chains.iter().flat_map(|chain| chain.cycles().nodes()) {
        let number = cycle.id().to_ref().number();
        if cycle.kind() == CycleKind::Experiment && number >= fact.issuer() && number < opened_until {
            problems.push(ReadingProblem::ExperimentWhileWaiting { event, cycle: cycle.id().to_ref() });
        }
    }

    // 승인 경계와 world_before_ref(T8).
    let BaselineSlot::Written(Some(approval)) = fact.boundary() else {
        problems.push(ReadingProblem::ApprovalBoundaryMissing { event });
        return Err(problems);
    };
    let recorded = match world_facts.world_before() {
        BaselineSlot::Written(Some(world)) => world,
        slot => {
            problems.push(ReadingProblem::WorldBeforeMissing { event, written_null: slot == BaselineSlot::Written(None) });
            return Err(problems);
        }
    };
    if registry.resolve(recorded).is_none() {
        problems.push(ReadingProblem::WorldBeforeNotInRegistry { event, world: recorded });
    }
    // 근거마다 지금 경계와 같은 유도를 하고, 그 세계를 world_before_ref 와 견준다.
    let differs = |at: BasisAt, boundary: CycleRef, derived: SnapshotRef| {
        (derived != recorded).then_some(ReadingProblem::WorldBeforeDiffers { event, at, boundary, recorded, derived })
    };
    let approval_basis = match prior_basis(inputs, chain, start, index, event, approval, fact.issuer(), BasisAt::Approval) {
        Ok(basis) => {
            problems.extend(differs(BasisAt::Approval, approval, basis.world));
            Some(basis)
        }
        Err(found) => {
            problems.extend(found);
            None
        }
    };
    // 두 번째 근거 — (비교한 자리, 그 확정 정책, 결과에 담을 값). 유도하지 못했으면 정책이 없다.
    let (second_at, second_policy, second) = match standing {
        EventStanding::Waiting { .. } => {
            // 대기 사건은 마지막 사건이고 기준점이 없으므로, 전체 입력의 지금 경계 유도가 곧 앞선 기록의 유도다.
            problems.extend(differs(BasisAt::CurrentBoundary, boundary, confirmed.world));
            (BasisAt::CurrentBoundary, Some(confirmed.policy), Some(SecondBasis::CurrentBoundary { boundary }))
        }
        EventStanding::AlreadyConfirmed { archived_at, archive_issuer, .. } => {
            match prior_basis(inputs, chain, start, index, event, archived_at, archive_issuer, BasisAt::Archive) {
                Ok(basis) => {
                    problems.extend(differs(BasisAt::Archive, archived_at, basis.world));
                    (BasisAt::Archive, Some(basis.policy), Some(SecondBasis::ArchiveBoundary { basis }))
                }
                Err(found) => {
                    problems.extend(found);
                    (BasisAt::Archive, None, None)
                }
            }
        }
        EventStanding::Replaced { .. } => (BasisAt::Approval, None, Some(SecondBasis::NoneForReplaced)),
    };
    if let (Some(approval_basis), Some(second_policy)) = (&approval_basis, second_policy)
        && approval_basis.policy != second_policy
    {
        problems.push(ReadingProblem::BasisPolicyDiffers {
            event,
            at: second_at,
            approval: approval_basis.policy,
            second: second_policy,
        });
    }

    // 대기 사건이 서 있을 수 있는 경계 — 승인 경계이거나 그 뒤에 연 사이클(T9 가 Interview 임을 본다).
    if let EventStanding::Waiting { .. } = standing {
        if chain.cycles().pending_revisit().is_some() {
            problems.push(ReadingProblem::OutOfScope(ScopeLimit::RevisitWhileWaiting { event, boundary }));
        } else if boundary != approval && boundary.number() < fact.issuer() {
            problems.push(ReadingProblem::OutOfScope(ScopeLimit::BoundaryBeforeApproval { event, boundary, approval }));
        }
    }

    // 판단하지 않은 칸.
    let mut not_judged = Vec::new();
    match review_cycle(world_facts, fact, chains) {
        Ok(Some(unjudged)) => not_judged.push(unjudged),
        Ok(None) => {}
        Err(problem) => problems.push(problem),
    }
    let (Some(approval_basis), Some(second), true) = (approval_basis, second, problems.is_empty()) else {
        debug_assert!(!problems.is_empty(), "근거를 유도하지 못했다면 그 갈래가 담겨 있다");
        return Err(problems);
    };
    let world_before = WorldBefore { world: recorded, approval: approval_basis, second };
    for (name, field, reading) in agreed.cycle_refs().not_judged() {
        if name == start.event {
            continue;
        }
        // 3e 의 성공: 기준점이 있으면 보관 경계 칸이 있다. 그래서 그 칸이 없다는 것은 기준점이 없는 사건의 정상 부재다.
        not_judged.push(match (field, reading) {
            (CycleField::BaselineAfterCycleRef, RefReading::UnwrittenNotJudged) => Unjudged::BaselineSlotsNotWritten { event: name },
            _ => Unjudged::CycleRefAbsent { event: name, field, reading },
        });
    }
    for name in agreed.cycle_refs().chains_not_looked_up() {
        if name != start.event {
            not_judged.push(Unjudged::ChainNotLookedUp { event: name });
        }
    }
    if marks.archive_halt == MarkSeen::NotProvided {
        not_judged.push(Unjudged::ArchiveHaltNotProvided);
    }
    if marks.unfinished_restore == MarkSeen::NotProvided {
        not_judged.push(Unjudged::UnfinishedRestoreNotProvided);
    }

    Ok(ScopeChangeReading {
        event,
        chain: chain.id(),
        issuer,
        policy_before: link.before(),
        policy_after: link.after().expect("3c 의 성공은 모든 사건의 after 에 판이 있음을 보장한다"),
        applied,
        confirmed,
        standing,
        world_before,
        not_judged,
    })
}

// ── 기록 전체의 읽기 — 제품의 load·열기·조회가 함께 쓰는 한 규칙 ───────────

/// 지금 서 있는 자리 — 닫힌 경계이거나 열린 사이클 안이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StandingAt {
    /// 닫힌 사이클 T 의 경계(방금 닫았거나 되돌아와 서 있다).
    ClosedBoundary { cycle: CycleRef },
    /// 열린 Interview 안. Interview 는 세계를 확정하지 않는다 — 마지막 확정 세계는 그 Entry 다(§6.1·§6.3).
    OpenInterview { cycle: CycleRef },
    /// 열린 Experiment 안. 마지막 확정 세계는 계보의 가장 가까운 닫힌 Verify, 없으면 Entry 다(§6.1, 현행).
    OpenExperiment { cycle: CycleRef },
}

/// 지금 자리의 마지막 확정 세계와 그 확정 정책 — 건넨 기록 안의 유도다. 열린 사이클이면 `path` 는 그 Entry 의 출처다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LastConfirmed {
    pub(crate) world: SnapshotRef,
    pub(crate) policy: PolicyName,
    pub(crate) path: Vec<PolicyStep>,
}

/// [`check_record`] 가 건넨 기록 전체를 검사한 뒤 돌려주는, 지금 자리의 세 값(§6.1).
///
/// `confirmed` 는 **최초 `start` 의 Chain 안에 서 있을 때만** 유도한다. 후속 Chain 안의 자리는 이 읽기가 확정 정책을 유도하지
/// 않으며(`None`), 그것을 적용 정책과 같다고 채우지 않는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecordStanding {
    /// 지금 Chain(건넨 목록의 마지막).
    pub(crate) chain: ChainRef,
    /// 최초 `start` 사건의 Chain — 이 읽기가 사건을 다루는 유일한 Chain 이다.
    pub(crate) start_chain: ChainRef,
    /// 지금 Chain 의 Cycle 발급기 값.
    pub(crate) issuer: u32,
    pub(crate) applied: AppliedPolicy,
    pub(crate) at: StandingAt,
    pub(crate) confirmed: Option<LastConfirmed>,
    /// 기준점이 없는 마지막 사건 — 「기준점 보관 필요 — 다음 실험 보류」(§5.2).
    pub(crate) waiting: Option<EventName>,
    /// 지금 Chain 이 종료 인터뷰를 닫고 Chain 보고서를 기다리는 경계다(§6.6).
    pub(crate) closing_boundary: bool,
    /// 지금 Chain 이 닫혔다.
    pub(crate) chain_closed: bool,
}

impl RecordStanding {
    /// 기준 일치(§6.1) — 유도한 확정 정책이 적용 정책과 같은 판인가. 유도하지 않았으면 `None` 이다.
    pub(crate) fn basis_agrees(&self) -> Option<bool> {
        self.confirmed.as_ref().map(|confirmed| confirmed.policy == self.applied.policy)
    }
}

/// 건넨 한 시점의 기록 **전체**를 검사하고 지금 자리의 세 값을 읽는다 — load 와 사이클 열기가 같은 이 함수를 쓴다(§6.2 「한 함수」).
///
/// - 3j·보충 입력의 대응·3g 위치 구분(그대로).
/// - 최초 `start` 기준점 = Chain Entry = 뿌리 Entry, 모든 사건이 그 Chain 의 것.
/// - 그 Chain 의 **모든 사이클**에 검증 ①(Entry = S(N) 의 세계)과, Experiment 에 검증 ②(S(N) 의 확정 정책 = P(N))를 적용한다. 대기 사건
///   뒤에 연 Experiment 는 ② 위반으로 걸린다(T9).
/// - 모든 `scope_change` 사건에 T8(승인 경계의 앞선 기록, 그리고 이미 확정이면 보관 경계의 앞선 기록·대기면 지금 자리의 마지막 확정
///   세계)과 `review_cycle_ref` 를 [`read_scope_change`] 와 같은 유도([`prior_basis`]·[`review_cycle`])로 본다.
///
/// [`read_scope_change`] 와 달리 지금 자리가 열린 사이클이어도, Chain 이 종료 경계·닫힘이어도 **기록의 일관성**은 읽는다 — 그 자리에서
/// 무엇을 실행해도 되는지는 이 함수가 정하지 않는다. 표식(`marks`)은 읽지 않는다. 순수 함수다.
pub(crate) fn check_record(inputs: &RecordInputs<'_>) -> Result<RecordStanding, Vec<ReadingProblem>> {
    let RecordInputs { ledger, links, facts, worlds, chains, registry, .. } = *inputs;

    let mut problems = Vec::new();
    let agreed = check_event_inputs(ledger, links, facts, chains, registry);
    if let Err(found) = &agreed {
        problems.push(ReadingProblem::Inputs(found.clone()));
    }
    let mismatches = world_facts_mismatches(links, worlds);
    if !mismatches.is_empty() {
        problems.push(ReadingProblem::WorldFacts(mismatches));
    }
    let agreed = match agreed {
        Ok(agreed) if problems.is_empty() => agreed,
        _ => return Err(problems),
    };
    let baselines: Vec<BaselineFacts> = facts.iter().map(|fact| fact.baseline().clone()).collect();
    let positions =
        classify_baseline_positions(&baselines, registry).map_err(|found| vec![ReadingProblem::Positions(found)])?;

    let (Some(first), Some(last)) = (facts.first(), links.events().last()) else {
        let event = EventName::new(1).expect("M1 은 유효한 이름이다");
        return one(ReadingProblem::UnknownEvent { event, events: 0 });
    };
    let owners: Vec<&Chain> = chains.iter().filter(|chain| chain.id() == first.chain()).collect();
    let [chain] = owners.as_slice() else {
        return one(ReadingProblem::EventChain { event: first.event(), chain: first.chain(), count: owners.len() });
    };
    let chain: &Chain = *chain;
    let start = start_link(chain, links, facts).map_err(|problem| vec![problem])?;
    if let Some(parent) = chain.parent() {
        return one(ReadingProblem::OutOfScope(ScopeLimit::ChainHasParent { chain: chain.id(), parent }));
    }
    for fact in facts {
        if fact.chain() != chain.id() {
            problems.push(ReadingProblem::OutOfScope(ScopeLimit::StartInOtherChain { chain: fact.chain(), start_chain: chain.id() }));
        }
    }
    if !problems.is_empty() {
        return Err(problems);
    }

    let derivation = Derivation { inputs, chain, start };
    let budget = chain.cycles().nodes().len();

    // §6.2 검증 ①(모든 사이클)·②(Experiment) — 열 때와 load 때 같은 규칙.
    for cycle in chain.cycles().nodes() {
        let target = cycle.id().to_ref();
        let mut path = Vec::new();
        let entry_confirmed = match derivation.entry(target, cycle, &mut path, budget) {
            Ok((_, policy)) => policy,
            Err(found) => {
                problems.extend(found);
                continue;
            }
        };
        if cycle.kind() != CycleKind::Experiment {
            continue;
        }
        match select_opening_policy(target, ledger, links, facts, chains, registry).map(|opening| opening.selection()) {
            Ok(OpeningSelection::Applied { policy, .. }) if policy == entry_confirmed => {}
            Ok(OpeningSelection::Applied { policy, .. }) => {
                problems.push(ReadingProblem::ExperimentPolicyDiffers { cycle: target, opened_under: policy, entry_confirmed });
            }
            Ok(OpeningSelection::BeforeAdoptionInGivenHistory) => {
                problems.push(ReadingProblem::ExperimentBeforeAdoption { cycle: target });
            }
            Err(_) => problems.push(ReadingProblem::Cycle(TargetProblem::NotFound { cycle: target })),
        }
    }

    // 지금 자리와 그 마지막 확정 세계.
    let current_chain = chains.last().expect("대상 Chain 을 찾았으므로 목록이 비어 있지 않다");
    let standing_on = current_chain.cycles().current();
    let here = standing_on.id().to_ref();
    let at = match (standing_on.is_closed(), standing_on.kind()) {
        (true, _) => StandingAt::ClosedBoundary { cycle: here },
        (false, CycleKind::Interview) => StandingAt::OpenInterview { cycle: here },
        (false, CycleKind::Experiment) => StandingAt::OpenExperiment { cycle: here },
    };
    let confirmed = if current_chain.id() == chain.id() {
        let derived = match at {
            StandingAt::ClosedBoundary { cycle } => derivation
                .boundary(cycle)
                .map(|basis| LastConfirmed { world: basis.world, policy: basis.policy, path: basis.path }),
            StandingAt::OpenInterview { cycle } => {
                let mut path = Vec::new();
                derivation.entry(cycle, standing_on, &mut path, budget).map(|(world, policy)| LastConfirmed { world, policy, path })
            }
            StandingAt::OpenExperiment { cycle } => {
                let mut path = Vec::new();
                // 검증 ② 가 위에서 Entry 출처의 확정 정책 = P(N) 을 봤다. 한 실험의 생애에는 정책이 하나다(§6.1).
                derivation
                    .entry(cycle, standing_on, &mut path, budget)
                    .map(|(_, policy)| LastConfirmed { world: standing_on.world_snapshot(), policy, path })
            }
        };
        match derived {
            Ok(confirmed) => Some(confirmed),
            Err(found) => {
                problems.extend(found);
                None
            }
        }
    } else {
        None
    };

    // 모든 scope_change 사건의 T8 과 review_cycle_ref.
    for (index, ((link, fact), world_facts)) in links.events().iter().zip(facts).zip(worlds).enumerate() {
        if link.kind() != EventKind::ScopeChange {
            continue;
        }
        let event = link.name();
        let BaselineSlot::Written(Some(approval)) = fact.boundary() else {
            problems.push(ReadingProblem::ApprovalBoundaryMissing { event });
            continue;
        };
        let recorded = match world_facts.world_before() {
            BaselineSlot::Written(Some(world)) => world,
            slot => {
                problems.push(ReadingProblem::WorldBeforeMissing { event, written_null: slot == BaselineSlot::Written(None) });
                continue;
            }
        };
        if registry.resolve(recorded).is_none() {
            problems.push(ReadingProblem::WorldBeforeNotInRegistry { event, world: recorded });
        }
        let approval_policy = match prior_basis(inputs, chain, start, index, event, approval, fact.issuer(), BasisAt::Approval) {
            Ok(basis) => {
                if basis.world != recorded {
                    problems.push(ReadingProblem::WorldBeforeDiffers {
                        event,
                        at: BasisAt::Approval,
                        boundary: approval,
                        recorded,
                        derived: basis.world,
                    });
                }
                Some(basis.policy)
            }
            Err(found) => {
                problems.extend(found);
                None
            }
        };
        let position = positions
            .entries()
            .iter()
            .find(|(name, _)| *name == event)
            .map(|(_, position)| *position)
            .expect("위치 목록은 건넨 사건마다 하나씩 있다");
        let second = match position {
            BaselinePosition::HasBaseline => {
                let baseline = fact.baseline();
                let (BaselineSlot::Written(Some(archived_at)), BaselineSlot::Written(archive_issuer)) =
                    (baseline.boundary(), baseline.issuer())
                else {
                    problems.push(ReadingProblem::ArchiveBoundaryMissing { event });
                    continue;
                };
                match prior_basis(inputs, chain, start, index, event, archived_at, archive_issuer, BasisAt::Archive) {
                    Ok(basis) => Some((BasisAt::Archive, archived_at, basis.world, basis.policy)),
                    Err(found) => {
                        problems.extend(found);
                        None
                    }
                }
            }
            // 대기 사건은 마지막 사건이라 지금 자리의 유도가 곧 앞선 기록의 유도다. 열린 Interview 안이면 그 Entry 의 계승이다(§6.3).
            BaselinePosition::WaitingInList => {
                confirmed.as_ref().map(|now| (BasisAt::CurrentBoundary, here, now.world, now.policy))
            }
            BaselinePosition::ReplacedInList => None,
        };
        if let Some((at, boundary, derived, policy)) = second {
            if derived != recorded {
                problems.push(ReadingProblem::WorldBeforeDiffers { event, at, boundary, recorded, derived });
            }
            if let Some(approval) = approval_policy
                && approval != policy
            {
                problems.push(ReadingProblem::BasisPolicyDiffers { event, at, approval, second: policy });
            }
        }
        if let Err(problem) = review_cycle(world_facts, fact, chains) {
            problems.push(problem);
        }
    }
    if !problems.is_empty() {
        return Err(problems);
    }

    let applied_policy = last.after().expect("3c 의 성공은 모든 사건의 after 에 판이 있음을 보장한다");
    let applied = AppliedPolicy {
        policy: applied_policy,
        introduced_by: agreed
            .policy_links()
            .introduced_by(applied_policy)
            .expect("3c 의 성공은 원장의 모든 판에 도입 사건이 있음을 보장한다"),
        last_event: last.name(),
    };
    let waiting = positions
        .entries()
        .last()
        .and_then(|(name, position)| matches!(position, BaselinePosition::WaitingInList).then_some(*name));
    Ok(RecordStanding {
        chain: current_chain.id(),
        start_chain: chain.id(),
        issuer: current_chain.cycles().next_name(),
        applied,
        at,
        confirmed,
        waiting,
        closing_boundary: current_chain.awaiting_report(),
        chain_closed: current_chain.is_closed(),
    })
}
