//! **보관 대기인 범위 변경 사건 하나의 명시 재시도** — 지금 서 있는 경계에서 입력을 다시 검증하고, 적용 정책으로 실제 폴더를 새로 관측해
//! 범위 변화 검사를 하고, 객체를 확정한 뒤, 한 저장 단위의 저장 결과를 다시 읽어 확인한 경우에만 기준점 확정을 보고하는 격리 실행·판정
//! 바탕. 제품 미연결.
//!
//! 정본은 [공유 저장·전이 계약](../../../spec/GIL_Storage_Transition_Contract_v0.1.md) §5.3(전이표의 「보관 대기 → 보관 시도」 두 줄,
//! 저장 2)·§5.4(기준점 보관 때의 범위 변화 검사)·§5.7 의 T5·T6·T8·§5.8·§6.3(재시도)이다. **구현 순서 5번 전체, 승인 저장(저장 1),
//! 정책 파일 동기화, 다음 Entry 선택·Experiment 개방 허가, 제품 `state.yaml` 저장, 복원의 완료가 아니다.**
//!
//! ```text
//! 읽기 (confirmed_world::read_scope_change)        관측·객체 쓰기·상태 저장으로 나가는 통로가 없다
//!
//! 명시 재시도 (RetryAttempt::begin / run_retry)    이 문만 한 시도를 시작한다
//!   begin   read_scope_change 를 이 호출의 입력으로 다시 실행 ── 대기가 아니면 시도를 만들지 않는다
//!           확정 정책 … 적용 정책의 원문을 창고에서 다시 읽어 주소·엔진 해석 확인(load_policy) → select_ledger_span(3o)
//!           마지막 확정 세계의 manifest 를 창고에서 읽는다
//!   ① Observe         observe_scoped(루트, 적용 정책)              두 번 훑어 같을 때만
//!        └ 범위 변화 검사: 기준 ∪ 지금 의 경로마다 LedgerSpan::classify(3n) — 계속 추적한 경로만 내용 비교
//!   ② Capture         capture_scoped(루트, 적용 정책, 창고)        추적 파일의 바이트를 창고에
//!        └ 같은 범위 변화 검사를 capture 의 결과에 다시
//!   ③ StoreManifest   ObjectStore::put_manifest
//!   ④ Save            BaselineState::save_baseline(한 저장 단위: 세계 이름 · 보관 경계 · 발급기 · archived 시도)
//!   ⑤ Reread          BaselineState::reread → 앞선 기록의 보존(3d·3i 와 그 안의 3f·3g·3h) + read_scope_change 로 이 저장 단위 확인
//!   ✗ RecordFailure   BaselineState::record_failure(최선 노력) — 어느 걸음이든 실패하면
//!                                                  ▼
//! RetryOutcome::Confirmed | Failed { cause, record } | Unknown { why }
//! ```
//!
//! # 호출자의 성공 보고를 믿지 않는다
//!
//! 관측·보관의 결과는 「성공했다」는 표식이 아니라 실제 [`ScopedWorld`] 로 받고, 범위 변화 검사는 이 모듈이 그 manifest 로 직접 한다.
//! 보고([`RetryReport`])의 내용을 만드는 문은 [`RetryAttempt::perform`] 하나이며 그것은 기존 [`observe_scoped`]·[`capture_scoped`]·
//! [`ObjectStore::put_manifest`] 와 호출자가 건넨 저장 경계를 실제로 부른다. 밖에서는 보고를 지어낼 수 없고, 낸 보고를 보류·복제·재주입할
//! 수 있을 뿐이다 — 그런 보고는 [`AttemptTicket`] 이 가른다.
//!
//! 저장 경계가 「저장했다」고 답해도 그것만으로 확정을 보고하지 않는다. 다시 읽은 기록에서 앞선 기록이 보존됐고 대상 사건에 **이 시도의**
//! 저장 단위(세계 이름과 그 주소·보관 경계·발급기·`archived` 하나)가 적혀 있을 때만 [`RetryOutcome::Confirmed`] 다.
//!
//! # 한 시도의 결속
//!
//! 시도는 시작할 때 읽은 사건·적용 정책·확정 정책·`world_before_ref`·지금 경계·발급기·그때의 시도 수([`AttemptBinding`])에 묶인다.
//! 요청마다 그 시도와 걸음을 가리키는 [`AttemptTicket`] 이 있고, 보고는 기다리는 요청의 표와 같아야 한다. 다른 시도의 보고, 같은 시도의
//! 지난 걸음·앞선 걸음의 보고, 끝난 시도로 온 보고는 거절하고 시도를 바꾸지 않는다([`RetryOutOfOrder`]). 표는 **이 프로세스의 메모리 값**
//! 이다 — 저장하지 않고, 영구 시도 ID 가 아니며, 사람의 동의를 인증하지도 않는다. 「명시적」이란 이 내부 문을 불렀다는 뜻일 뿐이고 실제
//! 제품의 요청 채널은 범위 밖이다.
//!
//! # 새로 관측한다
//!
//! 시도마다 `begin` 이 입력을 다시 검증하고 ① 부터 다시 한다. 앞선 시도의 manifest·보고·남은 객체를 받는 자리가 없다. 객체 일부가 창고에
//! 남았다는 사실은 성공이 아니다(§5.3). 조회([`read_scope_change`])와 같은 입력의 재전송은 시도를 만들지 않는다.
//!
//! # 실패와 결과 불명
//!
//! - 관측 불가·불안정은 「변경 없음」이 아니다. 계속 추적한 경로의 차이(내용 변경·삭제·새 파일)는 미확정 변경이다. 어느 것이든 보관하지
//!   않는다.
//! - 실패 뒤에는 저장 경계에 실패 시도 기록을 한 번 청한다. 그 기록이 저장되지 않아도 결과는 실패 그대로다.
//! - 저장 경계가 「저장하지 않았다」고 확인하면 실패다. 저장됐을 수도 있는데(불확실한 답, 또는 「저장했다」는 답) 다시 읽어 확인하지 못하면
//!   [`RetryOutcome::Unknown`] 이다 — 성공도, 확정된 대기도 아니다. 불확실한 답 뒤에 다시 읽은 기록이 앞선 기록 그대로의 대기이면 저장되지
//!   않은 것으로 확인된 실패다.
//!
//! 실패 사유([`FailureCause`])는 이 모듈 안의 분류다. 계약 §5.3 이 5번 작업에 남긴 `cause` 값 목록을 정하지 않으며 저장 꼴이 없다.
//! 보이지 않는 것은 [`BASELINE_RETRY_DOES_NOT_SHOW`] 에 있다.
#![cfg(all(feature = "tracking-policy", unix))]

use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use super::super::gilignore::{ApprovedPolicy, PathKind};
use super::super::{
    EntryPath, Manifest, ManifestAddress, ObjectError, ObjectStore, ObserveError, ScopedWorld, SnapshotRegistry,
    capture_scoped, load_policy, observe_scoped,
};
use super::confirmed_world::{
    EventStanding, EventWorldFacts, ReadingProblem, RecordInputs, ScopeChangeReading, ScopeMarks, read_scope_change,
};
use super::event_cycle_history::check_event_cycle_history;
use super::event_cycles::EventCycleFacts;
use super::event_links::{EventName, EventPolicyLinks};
use super::ledger_span::{BoardPolicies, LedgerSpan, LedgerSpanProblem, LedgerStart, select_ledger_span};
use super::link_history::check_policy_history;
use super::path_continuity::PathContinuity;
use super::{PolicyLedger, PolicyName};
use crate::Chain;
use crate::refs::{ChainRef, CycleRef, SnapshotRef};

/// 재시도가 [`RetryOutcome::Confirmed`] 로 끝나도 **보이지 않는** 것.
pub(crate) const BASELINE_RETRY_DOES_NOT_SHOW: [&str; 11] = [
    "실제 사용자 요청 — 이 문을 불렀다는 사실은 사람의 재시도 요청·동의의 인증이 아니다. 실제 제품의 요청 채널(Monitor·도구)은 없다",
    "입력과 승인 — read_scope_change 의 CONFIRMED_WORLD_DOES_NOT_SHOW 그대로다. 입력의 완전성·동일 원본 추출·실제 승인(저장 1 의 성공)을 증명하지 않는다",
    "실제 저장의 원자성과 내구성 — 저장 경계는 호출자가 건넨 값이다. 한 저장 단위가 실제로 한 번에 적혔는지, 프로세스 강제 종료·전원 장애 뒤에도 남는지는 그 경계의 일이며 이 모듈의 시험용 경계는 그 증거가 아니다",
    "다시 읽은 기록의 진위 — 다시 읽기는 저장 경계가 돌려준 값이다. 그것이 실제 저장소에서 새로 읽은 기록인지는 보지 않는다. 그 값 안에서 앞선 기록의 보존과 이 시도의 저장 단위만 확인한다",
    "새로 추적하는 경로의 과거 — 처음 포함되거나 제외됐다가 다시 포함된 경로는 지금 내용이 최초 보관이다. 승인 뒤·보관 전에 그 내용이 바뀌었는지 탐지한다고 주장하지 않는다(§5.4)",
    "적용 범위 밖의 경로 — 비교하지도 보관하지도 않고 파일을 건드리지 않는다. 완전히 제외된 폴더 안은 들여다보지 않는다",
    "마지막 확정 세계의 manifest 가 확정 정책의 범위로 관측됐는가 — 등록부가 가리키는 manifest 를 창고에서 읽어 견줄 뿐이다. 그 blob 의 실재는 보지 않는다",
    "외부 쓰기의 차단 — 두 번 훑어 견주고 capture 의 결과를 다시 검사할 뿐이다. capture 뒤·저장 전의 변경, 관측 사이에 바뀌었다가 돌아온 편집을 감지한다고 주장하지 않는다",
    "정책 파일 — 디스크의 .gilignore 와 승인 대기 제안을 읽지 않는다. 관측과 구간은 원장의 판이 가리키는 창고의 원문으로만 한다",
    "보관 중단·Chain 종료·미완 복원·다음 실험 개방 — 읽기가 범위 밖으로 돌려준 기록에서는 시도를 만들지 않는다. O-2·O-3 을 정하지 않고, 확정 뒤 다음 Entry 선택·Experiment 개방을 허가하지 않는다",
    "제품 연결 — 이 모듈을 부르는 제품 경로(CLI·MCP·Monitor·ProjectSession·load/save)가 없고 state.yaml 을 읽거나 쓰지 않는다. 형식 10/11 의 범위 변경 기록은 활성화되지 않았다",
];

// ── 실행 환경과 저장 경계 ──────────────────────────────────────────────────

/// 격리된 실행 환경 — 관측할 프로젝트 루트와 객체 창고. 제품의 `ProjectSession` 이 아니며 잠금을 잡지 않는다.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RetryEnv<'a> {
    pub(crate) root: &'a Path,
    pub(crate) store: &'a ObjectStore,
}

/// 저장 경계가 돌려주는 한 시점의 기록 — [`RecordInputs`] 로 빌려 읽는다.
#[derive(Debug, Clone)]
pub(crate) struct OwnedRecords {
    pub(crate) ledger: PolicyLedger,
    pub(crate) links: EventPolicyLinks,
    pub(crate) facts: Vec<EventCycleFacts>,
    pub(crate) worlds: Vec<EventWorldFacts>,
    pub(crate) chains: Vec<Chain>,
    pub(crate) registry: SnapshotRegistry,
    pub(crate) marks: ScopeMarks,
}

impl OwnedRecords {
    pub(crate) fn inputs(&self) -> RecordInputs<'_> {
        RecordInputs {
            ledger: &self.ledger,
            links: &self.links,
            facts: &self.facts,
            worlds: &self.worlds,
            chains: &self.chains,
            registry: &self.registry,
            marks: self.marks,
        }
    }
}

/// 저장 요청에 대한 저장 경계의 답. **이 답만으로 확정을 보고하지 않는다.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SaveAnswer {
    /// 저장했다고 답했다. 다시 읽어 확인한다.
    Stored,
    /// 저장하지 않았고 앞 상태 그대로라고 확인했다.
    NotStored { said: String },
    /// 저장됐을 수도 있다 — 오류가 났지만 닿았는지 모른다. 다시 읽어 확인한다.
    Uncertain { said: String },
}

/// 실패 시도 기록 요청에 대한 저장 경계의 답 — 최선 노력이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FailureRecord {
    Recorded,
    /// 적지 못했다(또는 적혔는지 모른다). 기록이 없다는 사실은 성공을 뜻하지 않는다(§5.3).
    NotRecorded { said: String },
}

/// 내부 저장 경계 — 제품의 `state.yaml` 저장이 아니다. 이 모듈은 이 세 문만 부른다.
///
/// 저장 경계의 일: 저장 단위나 실패 시도의 결속([`AttemptBinding`] — 대상 사건이 아직 기준점 없이 그때의 시도 수 그대로인가)이 **지금
/// 기록**과 맞을 때만 적고, 적는다면 한 번의 저장으로 전부 적으며, 앞선 정책·사건·기준점·시도를 지우거나 고치지 않는다. 이 모듈은 그
/// 일을 대신하지 않고, 다시 읽은 기록에서 그 결과만 확인한다.
pub(crate) trait BaselineState {
    /// 한 저장 단위(세계 이름·보관 경계·발급기·`archived` 시도)를 **한 번의 저장**으로 적는다.
    fn save_baseline(&mut self, commit: &BaselineCommit) -> SaveAnswer;
    /// 저장된 기록을 새로 읽어 돌려준다.
    fn reread(&mut self) -> Result<OwnedRecords, String>;
    /// 실패 시도 하나를 적는다. 정책·사건·기준점을 바꾸지 않는다.
    fn record_failure(&mut self, failure: &FailedAttempt) -> FailureRecord;
}

// ── 결속 ───────────────────────────────────────────────────────────────────

static NEXT_ATTEMPT: AtomicU64 = AtomicU64::new(1);

/// 한 시도의 한 걸음을 가리키는 표 — **이 프로세스의 메모리 값**이다. 저장하지 않으며 영구 시도 ID 가 아니다.
///
/// 만드는 문이 이 모듈 밖에 없다. 요청에서 읽고 보고와 견줄 수만 있다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AttemptTicket {
    attempt: u64,
    step: u32,
}

/// 한 시도가 시작할 때 읽어 묶인 값 — 다시 읽은 기록과 견주는 기준이다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AttemptBinding {
    event: EventName,
    chain: ChainRef,
    applied: PolicyName,
    confirmed_policy: PolicyName,
    world_before: SnapshotRef,
    boundary: CycleRef,
    issuer: u32,
    attempts_before: usize,
}

impl AttemptBinding {
    /// 대상 사건.
    pub(crate) fn event(&self) -> EventName {
        self.event
    }

    pub(crate) fn chain(&self) -> ChainRef {
        self.chain
    }

    /// 적용 정책 — 이 판으로 관측·보관한다.
    pub(crate) fn applied(&self) -> PolicyName {
        self.applied
    }

    /// 마지막 확정 세계의 확정 정책 — 구간의 시작이다. 읽기가 유도한 값이며 호출자가 건넨 값이 아니다.
    pub(crate) fn confirmed_policy(&self) -> PolicyName {
        self.confirmed_policy
    }

    /// 비교 근거 — 사건의 `world_before_ref`(= 지금 경계의 마지막 확정 세계).
    pub(crate) fn world_before(&self) -> SnapshotRef {
        self.world_before
    }

    /// 지금 서 있는 경계 — 확정하면 `baseline.after_cycle_ref` 가 된다.
    pub(crate) fn boundary(&self) -> CycleRef {
        self.boundary
    }

    /// 지금의 Cycle 발급기 값 — 확정하면 `baseline.next_cycle_id` 가 된다.
    pub(crate) fn issuer(&self) -> u32 {
        self.issuer
    }

    /// 시작할 때 기록에 있던 시도 수 — 새 시도 기록은 이 자리에 붙는다.
    pub(crate) fn attempts_before(&self) -> usize {
        self.attempts_before
    }
}

/// 기준점 확정의 **한 저장 단위**(§5.3 저장 2) — 세계 이름 발급, `baseline` 의 경계·발급기·`snapshot_ref`, `archived` 시도 하나.
///
/// 저장 경계는 이 값 전체를 한 번의 저장으로 적거나 아무것도 적지 않는다. 이 값이 만들어졌다는 것은 확정이 아니다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BaselineCommit {
    ticket: AttemptTicket,
    binding: AttemptBinding,
    snapshot: SnapshotRef,
    manifest: ManifestAddress,
    issues_name: bool,
}

impl BaselineCommit {
    pub(crate) fn ticket(&self) -> AttemptTicket {
        self.ticket
    }

    pub(crate) fn binding(&self) -> &AttemptBinding {
        &self.binding
    }

    /// `baseline.snapshot_ref` 가 될 세계 이름.
    pub(crate) fn snapshot(&self) -> SnapshotRef {
        self.snapshot
    }

    /// 그 이름이 가리킬 manifest 의 주소 — 창고에 이미 놓였다.
    pub(crate) fn manifest(&self) -> &ManifestAddress {
        &self.manifest
    }

    /// 등록부에 새 이름을 발급해야 하는가. 거짓이면 같은 세계의 기존 이름이다(§5.5).
    pub(crate) fn issues_name(&self) -> bool {
        self.issues_name
    }
}

/// 실패한 시도 하나 — 저장 경계가 `attempts` 의 끝에 실패로 적을 값.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FailedAttempt {
    ticket: AttemptTicket,
    binding: AttemptBinding,
    cause: FailureCause,
}

impl FailedAttempt {
    pub(crate) fn ticket(&self) -> AttemptTicket {
        self.ticket
    }

    pub(crate) fn binding(&self) -> &AttemptBinding {
        &self.binding
    }

    pub(crate) fn cause(&self) -> &FailureCause {
        &self.cause
    }
}

// ── 범위 변화 검사 ─────────────────────────────────────────────────────────

/// 계속 추적한 경로 하나가 마지막 확정 세계와 어떻게 다른가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DifferenceKind {
    /// 두 세계에 모두 있고 내용이 다르다.
    Modified,
    /// 마지막 확정 세계에 있었는데 지금 없다.
    Deleted,
    /// 마지막 확정 세계에 없었는데 지금 있다.
    Added,
    /// 지금 관측에 있는데 적용 정책이 추적하지 않는다고 판정한다 — 같은 정책의 관측이라면 설 수 없는 값이며 받아들이지 않는다.
    OutsideAppliedScope,
}

/// 범위 변화 검사가 찾은 차이 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PathDifference {
    pub(crate) path: EntryPath,
    pub(crate) kind: DifferenceKind,
}

/// 마지막 확정 세계와 지금 관측 사이의 범위 변화 검사 결과(§5.4 의 표).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScopeDelta {
    differences: Vec<PathDifference>,
    compared: usize,
    newly_tracked: Vec<EntryPath>,
    not_compared: Vec<EntryPath>,
}

impl ScopeDelta {
    /// 계속 추적한 경로의 차이 — 비어 있지 않으면 미확정 변경이다.
    pub(crate) fn differences(&self) -> &[PathDifference] {
        &self.differences
    }

    /// 내용을 견준 계속 추적 경로의 수.
    pub(crate) fn compared(&self) -> usize {
        self.compared
    }

    /// 지금 관측에 있고 새로 추적하는 경로 — 지금 내용이 최초 보관이다.
    pub(crate) fn newly_tracked(&self) -> &[EntryPath] {
        &self.newly_tracked
    }

    /// 마지막 확정 세계에만 있고 계속 추적한 경로가 아니라 견주지 않은 경로 — 범위 변화의 효과다.
    pub(crate) fn not_compared(&self) -> &[EntryPath] {
        &self.not_compared
    }

    fn is_clean(&self) -> bool {
        self.differences.is_empty()
    }
}

/// 기준 manifest 와 지금 manifest 의 **합집합**에서 경로마다 3n 의 분류를 묻고, 계속 추적한 경로만 내용을 견준다.
fn scope_delta(span: &LedgerSpan<'_>, base: &Manifest, now: &Manifest) -> ScopeDelta {
    let mut paths: BTreeSet<&EntryPath> = base.paths().collect();
    paths.extend(now.paths());
    let mut delta = ScopeDelta { differences: Vec::new(), compared: 0, newly_tracked: Vec::new(), not_compared: Vec::new() };
    for path in paths {
        let (was, seen) = (base.get(path), now.get(path));
        match span.classify(path, PathKind::File) {
            PathContinuity::ContinuouslyTracked { .. } => {
                delta.compared += 1;
                let kind = match (was, seen) {
                    (Some(was), Some(seen)) if was == seen => continue,
                    (Some(_), Some(_)) => DifferenceKind::Modified,
                    (Some(_), None) => DifferenceKind::Deleted,
                    (None, _) => DifferenceKind::Added,
                };
                delta.differences.push(PathDifference { path: path.clone(), kind });
            }
            PathContinuity::NewlyTracked { .. } => match seen {
                Some(_) => delta.newly_tracked.push(path.clone()),
                None => delta.not_compared.push(path.clone()),
            },
            PathContinuity::OutsideCurrentScope { .. } => match seen {
                Some(_) => {
                    delta.differences.push(PathDifference { path: path.clone(), kind: DifferenceKind::OutsideAppliedScope })
                }
                None => delta.not_compared.push(path.clone()),
            },
        }
    }
    delta
}

// ── 결과 ───────────────────────────────────────────────────────────────────

/// 어느 걸음의 관측이었는가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SeenAt {
    /// ① 보관 전의 안정 관측.
    Observation,
    /// ② 객체를 확정하는 관측.
    Capture,
}

/// 시도가 실패한 까닭 — 이 모듈 안의 분류이며 저장 꼴이 아니다(§5.3 의 `cause` 값 목록은 5번 작업이 확정한다).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FailureCause {
    /// 계속 추적한 경로가 마지막 확정 세계와 다르다 — 미확정 변경.
    UnconfirmedChange { seen_at: SeenAt, differences: Vec<PathDifference> },
    /// 두 번 훑는 사이에 세계가 바뀌었다 — 불안정한 관측.
    UnstableObservation { seen_at: SeenAt },
    /// 관측할 수 없었다 — 변경 없음이 아니다.
    ObservationRefused { seen_at: SeenAt, said: String },
    /// 객체 창고가 답하지 못했다.
    ObjectStore { said: String },
    /// 저장 경계가 저장하지 않았다고 확인했다(또는 불확실한 답 뒤 다시 읽어 확인했다).
    StateNotStored { said: String },
}

/// 저장됐을 수도 있는데 확인하지 못한 까닭.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UnknownWhy {
    /// 다시 읽지 못했다.
    RereadUnavailable { said: String },
    /// 다시 읽은 기록이 앞선 기록(정책·사건·기준점·시도·등록부·보충 칸)을 보존하지 않는다.
    HistoryNotKept { said: String },
    /// 다시 읽은 기록을 읽기가 받아들이지 않는다.
    RereadUnreadable { problems: Vec<ReadingProblem> },
    /// 다시 읽은 기록의 대상 사건이 이 시도의 저장 단위와 다르다 — 다른 시도의 결과이거나 다른 값이다.
    RereadDiffers { found: EventStanding },
    /// 저장했다고 답했는데 다시 읽은 기록은 앞선 대기 그대로다.
    StoredButStillWaiting,
}

/// 한 시도의 끝.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RetryOutcome {
    /// 기준점 확정 — 다시 읽은 기록에서 이 시도의 저장 단위를 확인했다. `reading` 은 다시 읽은 기록의 읽기다.
    Confirmed { commit: BaselineCommit, delta: ScopeDelta, reading: ScopeChangeReading },
    /// 실패 — 적용 정책·마지막 확정 세계·보관 대기는 시작할 때 그대로다. `record` 가 무엇이든 실패다.
    Failed { cause: FailureCause, record: FailureRecord },
    /// 결과 불명 — 저장됐을 수도 있다. **성공도, 확정된 대기도 아니다.** 조회로 확인한다.
    Unknown { commit: BaselineCommit, answer: SaveAnswer, why: UnknownWhy },
}

/// 시도를 만들지 않은 까닭. **어느 것도 저장 경계나 폴더 관측을 부르지 않았다.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NotStarted {
    /// 기록을 읽지 못했다(범위 밖 포함) — 읽기의 갈래 그대로.
    Reading(Vec<ReadingProblem>),
    /// 기준점이 이미 확정돼 있다. 새 시도·중복 확정을 만들지 않는다.
    AlreadyConfirmed { reading: ScopeChangeReading },
    /// 뒤 사건으로 대체됐다. 다시 보관하지 않는다.
    Replaced { reading: ScopeChangeReading },
    /// 구간에 든 판의 원문을 창고에서 읽어 확인하지 못했다.
    PolicySource { policy: PolicyName, said: String },
    /// 확정 정책부터 적용 정책까지의 구간을 세우지 못했다(3o).
    Span(LedgerSpanProblem),
    /// 마지막 확정 세계의 manifest 를 창고에서 읽지 못했다.
    BaseWorld { world: SnapshotRef, said: String },
}

impl fmt::Display for NotStarted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotStarted::Reading(found) => write!(
                f,
                "기록을 읽지 못해 재시도를 시작하지 않았다: {}",
                found.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
            ),
            NotStarted::AlreadyConfirmed { reading } => {
                write!(f, "{} 의 기준점은 이미 확정돼 있다. 새 시도를 만들지 않았다", reading.event())
            }
            NotStarted::Replaced { reading } => {
                write!(f, "{} 은 뒤 사건으로 대체됐다. 다시 보관하지 않는다", reading.event())
            }
            NotStarted::PolicySource { policy, said } => write!(f, "{policy} 의 원문을 창고에서 확인하지 못했다: {said}"),
            NotStarted::Span(problem) => write!(f, "확정 정책부터 적용 정책까지의 구간을 세우지 못했다: {problem}"),
            NotStarted::BaseWorld { world, said } => write!(f, "마지막 확정 세계 {world} 의 manifest 를 읽지 못했다: {said}"),
        }
    }
}

// ── 요청과 보고 ────────────────────────────────────────────────────────────

/// 시도가 기다리는 작업 하나 — [`RetryAttempt::perform`] 이 실제로 실행한다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RetryRequest {
    /// ① 적용 정책으로 지금 폴더를 안정 관측한다. 아무것도 쓰지 않는다.
    Observe { ticket: AttemptTicket },
    /// ② 같은 정책으로 관측하며 추적 파일의 바이트를 창고에 둔다.
    Capture { ticket: AttemptTicket },
    /// ③ 보관한 세계의 manifest 를 창고에 둔다.
    StoreManifest { ticket: AttemptTicket },
    /// ④ 한 저장 단위를 저장한다.
    Save { ticket: AttemptTicket, commit: BaselineCommit },
    /// ⑤ 저장된 기록을 새로 읽는다.
    Reread { ticket: AttemptTicket },
    /// 실패 시도를 적는다(최선 노력).
    RecordFailure { ticket: AttemptTicket, failure: FailedAttempt },
}

impl RetryRequest {
    pub(crate) fn ticket(&self) -> AttemptTicket {
        match self {
            RetryRequest::Observe { ticket }
            | RetryRequest::Capture { ticket }
            | RetryRequest::StoreManifest { ticket }
            | RetryRequest::Save { ticket, .. }
            | RetryRequest::Reread { ticket }
            | RetryRequest::RecordFailure { ticket, .. } => *ticket,
        }
    }
}

/// 요청 하나를 실제로 실행한 결과. 내용을 만드는 문은 [`RetryAttempt::perform`] 하나뿐이다 — 밖에서는 지어낼 수 없다.
#[derive(Debug, Clone)]
pub(crate) struct RetryReport {
    ticket: AttemptTicket,
    body: ReportBody,
}

impl RetryReport {
    /// 이 보고가 답하는 요청의 표.
    pub(crate) fn ticket(&self) -> AttemptTicket {
        self.ticket
    }
}

#[derive(Debug, Clone)]
enum ReportBody {
    Observed(Result<ScopedWorld, ObserveError>),
    Captured(Result<ScopedWorld, ObserveError>),
    ManifestStored(Result<ManifestAddress, ObjectError>),
    Saved(SaveAnswer),
    Reread(Result<OwnedRecords, String>),
    FailureRecorded(FailureRecord),
}

/// 보고를 받은 뒤의 걸음.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RetryStep {
    /// 다음에 실행할 요청.
    Next(RetryRequest),
    /// 시도가 끝났다.
    Done(RetryOutcome),
}

/// 받지 않은 보고. 거절해도 시도는 바뀌지 않는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RetryOutOfOrder {
    /// 이미 끝난 시도다 — 보고를 받지 않는다.
    AlreadyClosed { got: AttemptTicket },
    /// 다른 시도의 보고다.
    OtherAttempt { awaiting: AttemptTicket, got: AttemptTicket },
    /// 같은 시도의 다른 걸음의 보고다 — 이미 받은 보고의 재주입이거나 순서를 앞지른 보고다.
    OtherStep { awaiting: AttemptTicket, got: AttemptTicket },
    /// 표는 같은데 기다리던 종류의 보고가 아니다.
    UnexpectedKind { awaiting: AttemptTicket },
}

impl fmt::Display for RetryOutOfOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RetryOutOfOrder::AlreadyClosed { got } => write!(f, "이미 끝난 시도다. {got:?} 의 보고를 받지 않는다"),
            RetryOutOfOrder::OtherAttempt { awaiting, got } => {
                write!(f, "{awaiting:?} 의 결과를 기다리는데 다른 시도의 보고 {got:?} 가 왔다")
            }
            RetryOutOfOrder::OtherStep { awaiting, got } => {
                write!(f, "{awaiting:?} 의 결과를 기다리는데 같은 시도의 다른 걸음 {got:?} 의 보고가 왔다")
            }
            RetryOutOfOrder::UnexpectedKind { awaiting } => write!(f, "{awaiting:?} 가 기다리던 종류의 보고가 아니다"),
        }
    }
}

impl std::error::Error for RetryOutOfOrder {}

/// 요청을 실행하지 않은 까닭. 어느 것이든 아무것도 부르지 않았다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RetryClosed {
    /// 끝난 시도에는 실행할 요청이 없다.
    Finished,
    /// 이 걸음의 요청은 이미 한 번 실행했다 — 같은 관측·저장 요청을 두 번 내보내지 않는다. 그 보고를 건네야 다음 걸음으로 간다.
    AlreadyPerformed { ticket: AttemptTicket },
}

// ── 시도 ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
enum Phase {
    Observing,
    Capturing,
    StoringManifest { captured: Manifest, delta: ScopeDelta },
    Saving { commit: BaselineCommit, delta: ScopeDelta },
    Rereading { commit: BaselineCommit, delta: ScopeDelta, answer: SaveAnswer },
    RecordingFailure { cause: FailureCause },
    Done(RetryOutcome),
}

/// 명시적으로 시작한 **한 번의** 보관 재시도. 만드는 문은 [`RetryAttempt::begin`] 하나뿐이다.
#[derive(Debug)]
pub(crate) struct RetryAttempt<'a> {
    serial: u64,
    step: u32,
    /// 지금 걸음의 요청을 이미 실행했는가 — 한 걸음은 한 번만 실행한다.
    performed: bool,
    inputs: RecordInputs<'a>,
    before: ScopeChangeReading,
    binding: AttemptBinding,
    /// 확정 정책부터 적용 정책까지 — 원장 순서. 창고의 원문에서 다시 해석한 값이다.
    policies: Vec<(PolicyName, ApprovedPolicy)>,
    /// 마지막 확정 세계의 manifest.
    base: Manifest,
    phase: Phase,
}

fn span_of<'p>(
    ledger: &PolicyLedger,
    start: PolicyName,
    policies: &'p [(PolicyName, ApprovedPolicy)],
) -> Result<LedgerSpan<'p>, LedgerSpanProblem> {
    let boards = policies.iter().fold(BoardPolicies::new(), |boards, (name, policy)| boards.with(*name, policy));
    select_ledger_span(ledger, LedgerStart::Board(start), &boards)
}

impl<'a> RetryAttempt<'a> {
    /// 재시도 하나를 시작한다 — **이 문만 시도를 만든다.**
    ///
    /// 건넨 입력으로 [`read_scope_change`] 를 다시 실행하고, 대상 사건이 보관 대기일 때만 시도를 만든다. 확정 정책은 그 읽기가 유도한
    /// 값만 쓴다. 구간에 든 판의 원문과 마지막 확정 세계의 manifest 를 창고에서 **읽는다.** 폴더를 관측하거나 무엇을 쓰지 않는다 —
    /// 첫 관측은 첫 요청([`RetryRequest::Observe`])이다.
    pub(crate) fn begin(event: EventName, inputs: RecordInputs<'a>, env: &RetryEnv<'_>) -> Result<RetryAttempt<'a>, NotStarted> {
        let before = read_scope_change(event, &inputs).map_err(NotStarted::Reading)?;
        let attempts_before = match before.standing() {
            EventStanding::Waiting { attempts } => attempts,
            EventStanding::AlreadyConfirmed { .. } => return Err(NotStarted::AlreadyConfirmed { reading: before }),
            EventStanding::Replaced { .. } => return Err(NotStarted::Replaced { reading: before }),
        };
        let confirmed_policy = before.confirmed().policy();
        let world_before = before.confirmed().world();

        let mut policies = Vec::new();
        for record in inputs.ledger.policies().iter().filter(|record| record.name() >= confirmed_policy) {
            let policy = load_policy(env.store, record.engine(), record.rules())
                .map_err(|problem| NotStarted::PolicySource { policy: record.name(), said: problem.to_string() })?;
            policies.push((record.name(), policy));
        }
        span_of(inputs.ledger, confirmed_policy, &policies).map_err(NotStarted::Span)?;

        let base_problem = |said: String| NotStarted::BaseWorld { world: world_before, said };
        let address = inputs
            .registry
            .resolve(world_before)
            .ok_or_else(|| base_problem("건넨 등록부에 그 이름이 없다".to_string()))?;
        let base = env.store.read_manifest(address).map_err(|problem| base_problem(problem.to_string()))?;

        let binding = AttemptBinding {
            event,
            chain: before.chain(),
            applied: before.applied().policy,
            confirmed_policy,
            world_before,
            boundary: before.confirmed().boundary(),
            issuer: before.issuer(),
            attempts_before,
        };
        Ok(RetryAttempt {
            serial: NEXT_ATTEMPT.fetch_add(1, Ordering::Relaxed),
            step: 0,
            performed: false,
            inputs,
            before,
            binding,
            policies,
            base,
            phase: Phase::Observing,
        })
    }

    /// 이 시도가 묶인 값.
    pub(crate) fn binding(&self) -> &AttemptBinding {
        &self.binding
    }

    /// 시작할 때의 읽기 — 실패로 끝나면 이 값(적용 정책·마지막 확정 세계와 확정 정책·대기)이 그대로다.
    pub(crate) fn before(&self) -> &ScopeChangeReading {
        &self.before
    }

    /// 끝났으면 그 결과.
    pub(crate) fn outcome(&self) -> Option<&RetryOutcome> {
        match &self.phase {
            Phase::Done(outcome) => Some(outcome),
            _ => None,
        }
    }

    fn ticket(&self) -> AttemptTicket {
        AttemptTicket { attempt: self.serial, step: self.step }
    }

    /// 지금 기다리는 요청. 끝났으면 `None`.
    pub(crate) fn pending(&self) -> Option<RetryRequest> {
        let ticket = self.ticket();
        Some(match &self.phase {
            Phase::Observing => RetryRequest::Observe { ticket },
            Phase::Capturing => RetryRequest::Capture { ticket },
            Phase::StoringManifest { .. } => RetryRequest::StoreManifest { ticket },
            Phase::Saving { commit, .. } => RetryRequest::Save { ticket, commit: commit.clone() },
            Phase::Rereading { .. } => RetryRequest::Reread { ticket },
            Phase::RecordingFailure { cause } => RetryRequest::RecordFailure { ticket, failure: self.failed(cause) },
            Phase::Done(_) => return None,
        })
    }

    fn failed(&self, cause: &FailureCause) -> FailedAttempt {
        FailedAttempt { ticket: self.ticket(), binding: self.binding.clone(), cause: cause.clone() }
    }

    /// 적용 정책 — 구간의 마지막 판. `begin` 이 구간을 세웠으므로 언제나 있다.
    fn applied_policy(&self) -> &ApprovedPolicy {
        &self.policies.last().expect("begin 이 확정 정책부터 적용 정책까지의 구간을 세웠다").1
    }

    /// 기다리는 요청을 **실제로 실행**하고 그 결과를 보고로 돌려준다. 걸음은 그대로다 — 보고를 [`RetryAttempt::advance`] 에 건네야 다음
    /// 걸음으로 간다. 한 걸음의 요청은 한 번만 실행한다.
    ///
    /// 관측·보관은 기존 [`observe_scoped`]·[`capture_scoped`] 를 이 호출에서 부른다. 앞선 호출의 결과를 재사용하지 않는다.
    pub(crate) fn perform(&mut self, env: &RetryEnv<'_>, state: &mut dyn BaselineState) -> Result<RetryReport, RetryClosed> {
        if let Phase::Done(_) = self.phase {
            return Err(RetryClosed::Finished);
        }
        if self.performed {
            return Err(RetryClosed::AlreadyPerformed { ticket: self.ticket() });
        }
        self.performed = true;
        let body = match &self.phase {
            Phase::Observing => ReportBody::Observed(observe_scoped(env.root, self.applied_policy())),
            Phase::Capturing => ReportBody::Captured(capture_scoped(env.root, self.applied_policy(), env.store)),
            Phase::StoringManifest { captured, .. } => ReportBody::ManifestStored(env.store.put_manifest(captured)),
            Phase::Saving { commit, .. } => ReportBody::Saved(state.save_baseline(commit)),
            Phase::Rereading { .. } => ReportBody::Reread(state.reread()),
            Phase::RecordingFailure { cause } => ReportBody::FailureRecorded(state.record_failure(&self.failed(cause))),
            Phase::Done(_) => unreachable!("끝난 시도는 위에서 돌려보냈다"),
        };
        Ok(RetryReport { ticket: self.ticket(), body })
    }

    /// 기다리던 요청의 보고를 받아 다음 걸음을 정한다. 다른 시도·다른 걸음·끝난 시도로 온 보고는 거절하고 바뀌지 않는다.
    pub(crate) fn advance(&mut self, report: RetryReport) -> Result<RetryStep, RetryOutOfOrder> {
        let awaiting = self.ticket();
        if let Phase::Done(_) = self.phase {
            return Err(RetryOutOfOrder::AlreadyClosed { got: report.ticket });
        }
        if report.ticket.attempt != awaiting.attempt {
            return Err(RetryOutOfOrder::OtherAttempt { awaiting, got: report.ticket });
        }
        if report.ticket.step != awaiting.step {
            return Err(RetryOutOfOrder::OtherStep { awaiting, got: report.ticket });
        }
        let next = match (&self.phase, report.body) {
            (Phase::Observing, ReportBody::Observed(seen)) => match self.checked(seen, SeenAt::Observation) {
                Ok(_) => Phase::Capturing,
                Err(cause) => Phase::RecordingFailure { cause },
            },
            (Phase::Capturing, ReportBody::Captured(seen)) => match self.checked(seen, SeenAt::Capture) {
                Ok((world, delta)) => Phase::StoringManifest { captured: world.manifest, delta },
                Err(cause) => Phase::RecordingFailure { cause },
            },
            (Phase::StoringManifest { delta, .. }, ReportBody::ManifestStored(stored)) => match stored {
                Ok(manifest) => match self.commit(manifest) {
                    Some(commit) => Phase::Saving { commit, delta: delta.clone() },
                    None => Phase::RecordingFailure {
                        cause: FailureCause::ObjectStore { said: "세계 이름을 더 발급할 수 없다".to_string() },
                    },
                },
                Err(problem) => Phase::RecordingFailure { cause: FailureCause::ObjectStore { said: problem.to_string() } },
            },
            (Phase::Saving { commit, delta }, ReportBody::Saved(answer)) => match answer {
                SaveAnswer::NotStored { said } => Phase::RecordingFailure { cause: FailureCause::StateNotStored { said } },
                answer => Phase::Rereading { commit: commit.clone(), delta: delta.clone(), answer },
            },
            (Phase::Rereading { commit, delta, answer }, ReportBody::Reread(read)) => self.settle(commit, delta, answer, read),
            (Phase::RecordingFailure { cause }, ReportBody::FailureRecorded(record)) => {
                Phase::Done(RetryOutcome::Failed { cause: cause.clone(), record })
            }
            _ => return Err(RetryOutOfOrder::UnexpectedKind { awaiting }),
        };
        self.phase = next;
        self.step += 1;
        self.performed = false;
        Ok(match (&self.phase, self.pending()) {
            (Phase::Done(outcome), _) => RetryStep::Done(outcome.clone()),
            (_, Some(request)) => RetryStep::Next(request),
            (_, None) => unreachable!("끝나지 않은 시도에는 기다리는 요청이 있다"),
        })
    }

    /// 관측 결과에 범위 변화 검사를 적용한다 — 관측 불가·불안정·계속 추적한 경로의 차이는 모두 실패 사유다.
    fn checked(&self, seen: Result<ScopedWorld, ObserveError>, seen_at: SeenAt) -> Result<(ScopedWorld, ScopeDelta), FailureCause> {
        let world = seen.map_err(|problem| match problem {
            ObserveError::ChangedDuringObservation => FailureCause::UnstableObservation { seen_at },
            ObserveError::ObjectStore(problem) => FailureCause::ObjectStore { said: problem.to_string() },
            other => FailureCause::ObservationRefused { seen_at, said: other.to_string() },
        })?;
        let span = span_of(self.inputs.ledger, self.binding.confirmed_policy, &self.policies)
            .expect("begin 이 같은 원장·같은 자료로 구간을 세웠다");
        let delta = scope_delta(&span, &self.base, &world.manifest);
        match delta.is_clean() {
            true => Ok((world, delta)),
            false => Err(FailureCause::UnconfirmedChange { seen_at, differences: delta.differences }),
        }
    }

    /// 한 저장 단위 — 세계 이름은 시작할 때의 등록부에서 정한다: 같은 세계면 기존 이름(§5.5), 아니면 다음 이름.
    fn commit(&self, manifest: ManifestAddress) -> Option<BaselineCommit> {
        let registry = self.inputs.registry;
        let (snapshot, issues_name) = match registry.find_by_manifest(&manifest) {
            Some(known) => (known, false),
            None => (SnapshotRef::new(registry.next_id())?, true),
        };
        // 이 저장 단위를 싣는 것은 다음 걸음(Save)의 요청이다 — 그 걸음의 표를 적는다.
        let ticket = AttemptTicket { attempt: self.serial, step: self.step + 1 };
        Some(BaselineCommit { ticket, binding: self.binding.clone(), snapshot, manifest, issues_name })
    }

    /// 저장 뒤 다시 읽은 기록으로 결과를 정한다.
    fn settle(&self, commit: &BaselineCommit, delta: &ScopeDelta, answer: &SaveAnswer, read: Result<OwnedRecords, String>) -> Phase {
        let unknown = |why| Phase::Done(RetryOutcome::Unknown { commit: commit.clone(), answer: answer.clone(), why });
        let records = match read {
            Ok(records) => records,
            Err(said) => return unknown(UnknownWhy::RereadUnavailable { said }),
        };
        let (was, now) = (&self.inputs, records.inputs());
        if let Err(said) = history_kept(was, &now) {
            return unknown(UnknownWhy::HistoryNotKept { said });
        }
        let reading = match read_scope_change(self.binding.event, &now) {
            Ok(reading) => reading,
            Err(problems) => return unknown(UnknownWhy::RereadUnreadable { problems }),
        };
        let binding = &self.binding;
        match reading.standing() {
            EventStanding::AlreadyConfirmed { snapshot, archived_at, archive_issuer, attempts }
                if snapshot == commit.snapshot
                    && now.registry.resolve(snapshot) == Some(&commit.manifest)
                    && archived_at == binding.boundary
                    && archive_issuer == binding.issuer
                    && attempts == binding.attempts_before + 1
                    && reading.policy_after() == binding.applied =>
            {
                Phase::Done(RetryOutcome::Confirmed { commit: commit.clone(), delta: delta.clone(), reading })
            }
            EventStanding::Waiting { attempts } if attempts == binding.attempts_before => match answer {
                // 불확실한 답 뒤에 앞선 기록 그대로의 대기를 다시 읽었다 — 저장되지 않은 것으로 확인된 실패다.
                SaveAnswer::Uncertain { said } => Phase::RecordingFailure {
                    cause: FailureCause::StateNotStored { said: format!("다시 읽은 기록에 저장되지 않았다: {said}") },
                },
                _ => unknown(UnknownWhy::StoredButStillWaiting),
            },
            found => unknown(UnknownWhy::RereadDiffers { found }),
        }
    }
}

/// 다시 읽은 기록이 시작할 때의 기록을 보존하는가 — 정책 판과 사건의 정책 연결(3d), 사건의 승인 당시 세 칸·기준점·시도 결과 순서
/// (3i 와 그 안의 3f·3g·3h), 등록부의 이름, 보충 칸.
fn history_kept(was: &RecordInputs<'_>, now: &RecordInputs<'_>) -> Result<(), String> {
    fn listed<T: ToString>(items: &[T]) -> String {
        items.iter().map(ToString::to_string).collect::<Vec<_>>().join("; ")
    }
    check_policy_history((was.ledger, was.links), (now.ledger, now.links)).map_err(|found| listed(&found))?;
    check_event_cycle_history((was.facts, was.chains, was.registry), (now.facts, now.chains, now.registry))
        .map_err(|found| listed(&found))?;
    for record in was.registry.records() {
        if now.registry.resolve(record.id()) != Some(record.manifest()) {
            return Err(format!("{} 이 가리키는 세계가 바뀌었거나 사라졌다", record.id()));
        }
    }
    if now.worlds.get(..was.worlds.len()) != Some(was.worlds) {
        return Err("앞선 사건의 review_cycle_ref·world_before_ref 가 그대로가 아니다".to_string());
    }
    Ok(())
}

// ── 한 번에 끝까지 ─────────────────────────────────────────────────────────

/// 끝까지 실행한 한 시도.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RetryRun {
    /// 시도가 묶인 값.
    pub(crate) binding: AttemptBinding,
    /// 시작할 때의 읽기.
    pub(crate) before: ScopeChangeReading,
    pub(crate) outcome: RetryOutcome,
}

/// 명시 재시도 하나를 시작해 끝까지 실행한다 — [`RetryAttempt::begin`] 뒤 요청마다 [`RetryAttempt::perform`] 과
/// [`RetryAttempt::advance`] 를 차례로 부른다. 부를 때마다 새 시도이며 새로 관측한다.
pub(crate) fn run_retry(
    event: EventName,
    inputs: RecordInputs<'_>,
    env: &RetryEnv<'_>,
    state: &mut dyn BaselineState,
) -> Result<RetryRun, NotStarted> {
    let mut attempt = RetryAttempt::begin(event, inputs, env)?;
    loop {
        if let Some(outcome) = attempt.outcome() {
            return Ok(RetryRun { binding: attempt.binding.clone(), before: attempt.before.clone(), outcome: outcome.clone() });
        }
        let report = attempt.perform(env, state).expect("끝나지 않은 시도에는 실행할 요청이 있다");
        attempt.advance(report).expect("이 시도가 방금 낸 보고는 기다리던 보고다");
    }
}
