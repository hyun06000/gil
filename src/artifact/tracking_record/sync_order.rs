//! 정책 파일 동기화의 **쓰기 순서와 중단 처리** — 계약 §5.2 「정책 파일 동기화의 기준」의 세 걸음과
//! 그 상황 표, §5.7 T13. 구현 순서 3번 안의 작은 후속(편의상 「3b」): 동기화 절차의 독립 제어 로직이다.
//! 3번 전체의 완료가 아니며 정본의 공식 완료 단위도 아니다.
//!
//! ```text
//! begin(적용 원문 주소, 저장된 sync_base, 잠금 안 재확인)        ← 명시적 동기화만
//!   ├─ 디스크 = 원문, 기준 = present + 원문 ──────────────▶ Stop  (파일도 기준도 쓰지 않는다)
//!   ├─ 디스크 = 원문, 기준 그 밖 ─▶ StoreBase(present) ─▶ Stop (기준 갱신만, 파일은 쓰지 않는다)
//!   ├─ 판정이 갱신 필요(구판 잔류)가 아니다 ──────────────▶ Stop  (쓰지 않는다)
//!   └─ 쓰기 후보
//!        ▼ ① StoreBase(unconfirmed) ── Stored 가 아니면 ─────▶ Stop  (파일 쓰기를 요청하지 않는다)
//!        ▼ ② ReplaceFile ── Replaced + 재조회 = 원문 ─────────▶ ③
//!        │                 NotReplaced + 같은 잠금 재조회 = 이전 기준 ─▶ StoreBase(이전 기준) ─▶ Stop
//!        │                 그 밖 ───────────────────────────▶ Stop  (복귀하지 않는다. 기준 unconfirmed)
//!        ▼ ③ StoreBase(present + 원문) ── Stored ─▶ Stop(InSync), 아니면 Stop(파일 = 원문, 기준 미확인)
//!
//! begin_cleanup(같은 입력)                                       ← 그 밖의 변경 명령
//!   ├─ 디스크 = 원문, 기준 unconfirmed ─▶ StoreBase(present) ─▶ Stop (기준 정리만)
//!   └─ 그 밖 ──────────────────────────────────────────────▶ Stop  (파일도 기준도 쓰지 않는다)
//! ```
//!
//! # 누가 어느 입구를 부르나
//!
//! - **읽기 전용 조회**는 [`super::classify_policy_file`] 만 쓴다. 디스크가 원문과 같으면 동기화됨이고, 그 결과에는
//!   저장 요청이 들어갈 자리가 없다 — 파일도 상태도 저장하지 않는다.
//! - **명시적 동기화**(사용자가 동기화를 실행한 변경 명령)는 [`PolicyFileSync::begin`] 을 부른다. 디스크가 이미
//!   원문 B와 같으면 파일은 그대로 두고, 기준이 `present`(B)가 아니면(`present`(A)·`absent`·`unconfirmed`)
//!   `present`(B) 저장만 요청한다(계약 §5.2, 2026-11-20 사용자 결정).
//! - **그 밖의 변경 명령**은 [`PolicyFileSync::begin_cleanup`] 을 부른다. 계약 §5.2 상황표의 「다음 변경 명령이
//!   디스크를 확인하고 기준을 적용 원문으로 저장한다」 — 기준이 `unconfirmed` 일 때만이다. 확인된 다른 기준을
//!   바꾸지 않으며 파일 쓰기를 요청하지 않는다. 그런 명령이 파일 쓰기까지 시작해도 되는지는 계약이 정하지
//!   않았고 이 로직도 정하지 않는다.
//!
//! # 이 로직이 하지 않는 것
//!
//! - **파일을 읽거나 쓰지 않는다. 상태를 저장하지 않는다. 잠그지 않는다. 경로를 검사하지 않는다.**
//!   이것들과 동시 외부 편집 대응은 후속 연결 계층의 책임이다([`CALLER_DUTIES`]).
//! - 입력은 호출자가 건넨 관측([`DiskReading`])과 작업 결과([`StoreResult`]·[`ReplaceResult`])뿐이다.
//!   `Stored`·`Replaced`·`SameLock` 은 호출자의 **보고**다. 이 로직은 그것을 실제 디스크 저장·교체·잠금의
//!   증명으로 다루지 않으며, 보고가 거짓이면 결과도 틀린다.
//! - [`PolicyFileSync`] 는 한 번의 시도 동안만 메모리에 있는 **일시적 제어 값**이다. 저장하지 않으며 정책·승인
//!   기록이 아니다. 새 영구 상태나 저장 칸을 만들지 않는다. 재시작 뒤에 남는 것은 저장된 `sync_base` 와
//!   디스크뿐이므로 [`PolicyFileSync::begin`] 부터 다시 판정한다.
//! - 두 입구 모두 프로젝트 잠금을 쥔 **변경 명령**이 부른다고 가정한다. 그 보장과, 호출이 정말 명시적
//!   동기화인지의 구분도 연결 계층의 몫이다.
//! - 승인 확인·승인 처리가 아니다. [`base_for_new_policy`] 는 승인 저장이 함께 적을 `sync_base` 값을 고르는
//!   규칙일 뿐이다.

use std::fmt;

use super::sync::{DiskReading, PolicyFileReading, SyncBase, classify_policy_file};
use crate::artifact::ContentDigest;

/// 이 제어 로직이 **맡지 않는** 일 — 후속 연결 계층이 진다.
pub(crate) const CALLER_DUTIES: [&str; 7] = [
    "프로젝트 잠금을 쥐고 begin·begin_cleanup 의 재확인부터 마지막 저장까지 같은 잠금 범위에 머무는 것",
    "루트 .gilignore 경로의 안전 검사(연결 파일·디렉터리·경로 밖)",
    "임시 파일 배타 생성 → fsync → 교체 → 다시 읽기의 실제 실행",
    "sync_base 의 실제 상태 저장과 그 결과를 사실대로 보고하는 것",
    "교체가 일어났는지 알 수 없는 오류를 교체 전 실패로 보고하지 않는 것",
    "동시 외부 편집의 탐지 — 이 로직은 건네받은 관측만 비교한다",
    "사용자가 명시적으로 동기화를 실행했을 때만 begin 을, 그 밖의 변경 명령은 begin_cleanup 을 부르는 것",
];

/// 상태 저장 한 번의 결과 — 호출자의 보고다. **알 수 없음은 성공이 아니다.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StoreResult {
    /// 저장했다고 보고됐다.
    Stored,
    /// 저장하지 못했음이 분명하다 — 앞 상태가 그대로다.
    Failed { reason: String },
    /// 저장됐는지 알 수 없다.
    Unknown { reason: String },
}

/// 교체 전 실패 뒤의 재조회가 어디서 일어났는가.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Reread {
    /// 재확인과 **같은 잠금 범위** 안에서 다시 읽었다.
    SameLock(DiskReading),
    /// 같은 잠금 범위에서 다시 읽지 않았다(잠금을 놓쳤거나 읽지 않았다).
    NotInSameLock,
}

/// 파일 쓰기(②) 한 번의 결과 — 호출자의 보고다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReplaceResult {
    /// 교체가 끝났고 그 뒤 디스크를 다시 읽었다. 원문과 같은지는 이 로직이 견준다.
    Replaced { reread: DiskReading },
    /// 교체 **전에** 오류로 끝났음이 분명하다(권한, 임시 파일 생성 실패 등).
    NotReplaced { reason: String, reread: Reread },
    /// 교체가 일어났는지 알 수 없다.
    Unknown { reason: String },
}

/// 호출자가 요청된 작업을 하고 돌려주는 보고.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SyncReport {
    Store(StoreResult),
    Replace(ReplaceResult),
}

/// 호출자에게 요청하는 다음 작업. **요청이지 실행 결과가 아니다.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SyncRequest {
    /// 상태 저장: `sync_base` 를 이 값으로 적는다.
    StoreBase(SyncBase),
    /// 파일 쓰기: 이 주소의 원문으로 루트 `.gilignore` 를 교체하고 같은 잠금 안에서 다시 읽는다.
    ReplaceFile { rules: ContentDigest },
}

/// 한 걸음의 결과 — 다음 작업 요청이거나 멈춤.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SyncStep {
    Request(SyncRequest),
    Stop(SyncStop),
}

/// 시도가 멈춘 이유. 어느 것도 승인·적용·보관이 아니다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SyncStop {
    /// 재확인한 디스크와 저장된 기준이 이미 모두 원문이다(`present` + 원문). 파일도 기준도 쓰지 않는다.
    AlreadyMatches,
    /// 재확인한 디스크가 쓰기 후보(갱신 필요 — 구판 잔류)가 아니다. 판정을 그대로 돌려주며 쓰지 않는다.
    NotWritable(PolicyFileReading),
    /// [`PolicyFileSync::begin_cleanup`] 의 정리 대상(디스크 = 원문, 기준 `unconfirmed`)이 아니다. 판정을 그대로
    /// 돌려주며 파일도 기준도 쓰지 않는다.
    NoCleanup(PolicyFileReading),
    /// ① `unconfirmed` 저장이 확인되지 않았다. **파일 쓰기를 요청하지 않았다.** `Failed` 면 기준·파일이
    /// 그대로이고 나중에 처음부터 다시 할 수 있다. `Unknown` 이면 저장된 기준을 다시 읽어 판정한다.
    MarkNotStored(StoreResult),
    /// 파일이 원문과 같고 그 원문을 기준으로 저장했다고 **보고됐다**. 기준 확정을 말할 수 있는 유일한 멈춤이다.
    InSync,
    /// **파일 내용은 원문과 같지만 기준 확정은 확인되지 않았다** — 마지막 기준 저장이 실패했거나 알 수 없다.
    /// 저장된 기준은 앞 값(`present`(이전)·`absent`·`unconfirmed`)이거나 알 수 없다. 표시는 동기화됨이다. 파일을
    /// 다시 쓰지 않는다. 다음 명시적 동기화가 저장된 기준과 디스크를 다시 읽어 판정한다(기준이 `unconfirmed` 면
    /// 다른 변경 명령의 기준 정리도 한다). 기준이 이전 값으로 남은 채 사용자가 파일을 그 이전 내용으로
    /// 되돌리면 구판 잔류와 구별할 수 없다 — 이 멈춤 뒤의 편집 보호는 보장하지 않는다.
    FileMatchesBaseUnsettled(StoreResult),
    /// 교체 전 실패가 분명했고 같은 잠금에서 다시 읽은 디스크가 이전 기준과 같아, 이전 기준으로 되돌려
    /// 저장했다고 보고됐다. 표시: 갱신 실패. 처음부터 다시 할 수 있다.
    WriteFailedBaseRestored,
    /// 쓰기를 끝내지 못했고 기준은 `unconfirmed`(또는 알 수 없음)로 남는다. **이전 기준으로 되돌리지 않았다.**
    /// 이후 판정은 원인 미확인이며 자동으로 다시 쓰지 않는다.
    WriteUnsettled(Unsettled),
}

/// [`SyncStop::WriteUnsettled`] 의 원인.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Unsettled {
    /// 교체가 일어났는지 알 수 없다.
    ReplacementUnknown { reason: String },
    /// 교체했다고 보고됐으나 다시 읽은 디스크가 원문과 다르거나 읽지 못했다.
    RereadDiffers(DiskReading),
    /// 교체 전 실패였지만 같은 잠금 범위에서 다시 읽지 않았다.
    NotRereadInSameLock { reason: String },
    /// 교체 전 실패였지만 다시 읽은 디스크가 이전 기준과 다르거나 읽지 못했다.
    DiskNotAtPrevious { reason: String, reread: DiskReading },
    /// 이전 기준으로 되돌리는 저장이 확인되지 않았다.
    RestoreNotStored(StoreResult),
}

/// 순서에 맞지 않는 보고. 거절해도 제어 값은 바뀌지 않는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SyncOutOfOrder {
    /// 기다리던 작업과 다른 종류의 보고다.
    UnexpectedReport { awaiting: SyncRequest, got: SyncReport },
    /// 이미 멈춘 시도에 보고가 왔다.
    AlreadyStopped { stop: SyncStop, got: SyncReport },
}

impl fmt::Display for SyncOutOfOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncOutOfOrder::UnexpectedReport { awaiting, got } => {
                write!(f, "정책 파일 동기화가 {awaiting:?} 의 결과를 기다리는데 {got:?} 가 왔다")
            }
            SyncOutOfOrder::AlreadyStopped { stop, got } => {
                write!(f, "정책 파일 동기화는 이미 멈췄다({stop:?}). {got:?} 를 받지 않는다")
            }
        }
    }
}

impl std::error::Error for SyncOutOfOrder {}

/// 기다리는 보고. 각 값은 바로 앞에서 낸 요청 하나에 대응한다.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Awaiting {
    /// 디스크가 이미 원문일 때 기준만 `present` + 원문으로 저장(파일 쓰기 없음)한 결과 — 명시적 동기화의 기준
    /// 갱신과 다른 변경 명령의 `unconfirmed` 정리가 함께 쓴다.
    Rebase,
    /// ① `unconfirmed` 저장의 결과.
    Mark,
    /// ② 파일 교체·재조회의 결과.
    Replace,
    /// ③ 새 `present` 기준 저장의 결과.
    Settle,
    /// 교체 전 실패 뒤 이전 기준 복귀 저장의 결과.
    Restore,
}

/// 한 번의 동기화 시도를 이끄는 **일시적** 제어 값 — 저장하지 않는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PolicyFileSync {
    rules: ContentDigest,
    /// 시작할 때 저장돼 있던 기준. 쓰기 후보였다면 재확인 때 디스크와 같았던 확인된 기준(`present`·`absent`)이고,
    /// 교체 전 실패 뒤 복귀할 수 있는 유일한 값이다.
    previous: SyncBase,
    state: Result<(Awaiting, SyncRequest), SyncStop>,
}

impl PolicyFileSync {
    /// **명시적 동기화**가 잠금 안에서 디스크를 다시 읽은 뒤 부른다(쓰기 0단계). 요청 하나 또는 멈춤으로 시작한다.
    ///
    /// `rules` 는 적용 원문의 주소다 — 어느 판이 적용 정책인지 이 함수는 정하지 않는다. 디스크가 이미 원문과
    /// 같으면 파일 쓰기도 그를 위한 `unconfirmed` 저장도 요청하지 않고, 기준이 `present` + 원문이 아닐 때만
    /// 그 값의 저장을 요청한다.
    pub(crate) fn begin(rules: &ContentDigest, base: &SyncBase, recheck: &DiskReading) -> (PolicyFileSync, SyncStep) {
        let state = match classify_policy_file(rules, base, recheck) {
            PolicyFileReading::StaleMatchesBase => {
                Ok((Awaiting::Mark, SyncRequest::StoreBase(SyncBase::Unconfirmed)))
            }
            PolicyFileReading::MatchesApplied => match base {
                SyncBase::Present(kept) if kept == rules => Err(SyncStop::AlreadyMatches),
                SyncBase::Present(_) | SyncBase::Absent | SyncBase::Unconfirmed => Ok(Self::rebase(rules)),
            },
            other => Err(SyncStop::NotWritable(other)),
        };
        Self::started(rules, base, state)
    }

    /// 명시적 동기화가 **아닌** 변경 명령이 잠금 안에서 디스크를 다시 읽은 뒤 부른다. 계약 §5.2 상황표의 기준
    /// 정리 — 디스크가 원문과 같고 기준이 `unconfirmed` 일 때만 `present` + 원문 저장을 요청한다. 파일 쓰기는
    /// 어떤 경우에도 요청하지 않는다.
    pub(crate) fn begin_cleanup(rules: &ContentDigest, base: &SyncBase, recheck: &DiskReading) -> (PolicyFileSync, SyncStep) {
        let state = match (classify_policy_file(rules, base, recheck), base) {
            (PolicyFileReading::MatchesApplied, SyncBase::Unconfirmed) => Ok(Self::rebase(rules)),
            (reading, _) => Err(SyncStop::NoCleanup(reading)),
        };
        Self::started(rules, base, state)
    }

    fn rebase(rules: &ContentDigest) -> (Awaiting, SyncRequest) {
        (Awaiting::Rebase, SyncRequest::StoreBase(SyncBase::Present(rules.clone())))
    }

    fn started(rules: &ContentDigest, base: &SyncBase, state: Result<(Awaiting, SyncRequest), SyncStop>) -> (PolicyFileSync, SyncStep) {
        let sync = PolicyFileSync { rules: rules.clone(), previous: base.clone(), state };
        let step = sync.step();
        (sync, step)
    }

    /// 지금 기다리는 요청, 또는 멈춘 이유.
    pub(crate) fn step(&self) -> SyncStep {
        match &self.state {
            Ok((_, request)) => SyncStep::Request(request.clone()),
            Err(stop) => SyncStep::Stop(stop.clone()),
        }
    }

    /// 앞서 낸 요청의 결과를 받아 다음 걸음을 정한다. 종류가 맞지 않거나 이미 멈췄으면 거절하고 바뀌지 않는다.
    pub(crate) fn advance(&mut self, report: SyncReport) -> Result<SyncStep, SyncOutOfOrder> {
        let (awaiting, request) = match &self.state {
            Ok(pending) => pending.clone(),
            Err(stop) => return Err(SyncOutOfOrder::AlreadyStopped { stop: stop.clone(), got: report }),
        };
        let next = match (awaiting, report) {
            (Awaiting::Rebase, SyncReport::Store(result)) => Err(match result {
                StoreResult::Stored => SyncStop::InSync,
                unsettled => SyncStop::FileMatchesBaseUnsettled(unsettled),
            }),
            (Awaiting::Mark, SyncReport::Store(result)) => match result {
                StoreResult::Stored => Ok((Awaiting::Replace, SyncRequest::ReplaceFile { rules: self.rules.clone() })),
                unsettled => Err(SyncStop::MarkNotStored(unsettled)),
            },
            (Awaiting::Replace, SyncReport::Replace(result)) => self.after_replace(result),
            (Awaiting::Settle, SyncReport::Store(result)) => Err(match result {
                StoreResult::Stored => SyncStop::InSync,
                unsettled => SyncStop::FileMatchesBaseUnsettled(unsettled),
            }),
            (Awaiting::Restore, SyncReport::Store(result)) => Err(match result {
                StoreResult::Stored => SyncStop::WriteFailedBaseRestored,
                unsettled => SyncStop::WriteUnsettled(Unsettled::RestoreNotStored(unsettled)),
            }),
            (_, got) => return Err(SyncOutOfOrder::UnexpectedReport { awaiting: request, got }),
        };
        self.state = next;
        Ok(self.step())
    }

    fn after_replace(&self, result: ReplaceResult) -> Result<(Awaiting, SyncRequest), SyncStop> {
        match result {
            ReplaceResult::Replaced { reread: DiskReading::Present(found) } if found == self.rules => {
                Ok((Awaiting::Settle, SyncRequest::StoreBase(SyncBase::Present(self.rules.clone()))))
            }
            ReplaceResult::Replaced { reread } => Err(SyncStop::WriteUnsettled(Unsettled::RereadDiffers(reread))),
            ReplaceResult::Unknown { reason } => Err(SyncStop::WriteUnsettled(Unsettled::ReplacementUnknown { reason })),
            ReplaceResult::NotReplaced { reason, reread: Reread::NotInSameLock } => {
                Err(SyncStop::WriteUnsettled(Unsettled::NotRereadInSameLock { reason }))
            }
            ReplaceResult::NotReplaced { reason, reread: Reread::SameLock(reread) } => {
                let unchanged = match (&self.previous, &reread) {
                    (SyncBase::Present(base), DiskReading::Present(found)) => base == found,
                    (SyncBase::Absent, DiskReading::Absent) => true,
                    _ => false,
                };
                match unchanged {
                    true => Ok((Awaiting::Restore, SyncRequest::StoreBase(self.previous.clone()))),
                    false => Err(SyncStop::WriteUnsettled(Unsettled::DiskNotAtPrevious { reason, reread })),
                }
            }
        }
    }
}

// ── 승인 저장 때의 기준 ─────────────────────────────────────────────────────

/// 계약이 정하지 않은 조합 — 이 규칙은 값을 고르지 않는다.
///
/// 확인된 기준(`present`·`absent`)이 있는데 재확인한 디스크가 그 기준과도 새 원문과도 다를 때다(사용자
/// 편집·삭제). 계약은 「재확인에서 읽은 디스크 상태로 기준을 정한다」고만 적는다. 그 글자대로 디스크를
/// 기준으로 삼으면 다음 동기화가 사용자의 편집을 덮어쓸 수 있는 후보가 되고, 이전 기준을 남기면 글자와
/// 다르다. 어느 쪽인지는 승인 저장을 구현하는 작업이 계약을 먼저 고쳐 정한다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BaseUndecided {
    pub(crate) base: SyncBase,
    pub(crate) disk: DiskReading,
}

impl fmt::Display for BaseUndecided {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "기준 {:?} 과 재확인한 디스크 {:?} 의 조합은 계약 §5.2 가 새 기준을 정하지 않았다",
            self.base, self.disk
        )
    }
}

impl std::error::Error for BaseUndecided {}

/// 승인 저장(계약 §5.3 의 저장 1)이 함께 적을 `sync_base` — 계약 §5.2 의 「승인을 저장할 때」 규칙.
///
/// **승인 확인이나 승인 처리가 아니다.** 저장 1 을 해도 되는지(①②③)는 이 함수가 모른다. 입력은 새 적용
/// 원문의 주소, 지금 저장된 기준, 재확인에서 읽은 디스크다.
///
/// - 디스크 = 새 원문 → `present` + 새 원문. 디스크를 확인한 결과다.
/// - 기준 `unconfirmed`, 디스크 ≠ 새 원문 → `unconfirmed` 유지. 승인이 미확인 상태를 확인된 상태로 바꾸지 않는다.
/// - 확인된 기준과 디스크가 같다 → 그 기준 그대로(= 디스크 상태).
/// - 디스크를 읽지 못했다 → 기준 그대로(상황표의 「디스크를 읽을 수 없다」).
/// - 확인된 기준과도 새 원문과도 다르다 → [`BaseUndecided`].
pub(crate) fn base_for_new_policy(
    new_rules: &ContentDigest,
    base: &SyncBase,
    recheck: &DiskReading,
) -> Result<SyncBase, BaseUndecided> {
    match (base, recheck) {
        (_, DiskReading::Present(found)) if found == new_rules => Ok(SyncBase::Present(new_rules.clone())),
        (_, DiskReading::Unreadable { .. }) => Ok(base.clone()),
        (SyncBase::Unconfirmed, _) => Ok(SyncBase::Unconfirmed),
        (SyncBase::Present(kept), DiskReading::Present(found)) if kept == found => Ok(base.clone()),
        (SyncBase::Absent, DiskReading::Absent) => Ok(SyncBase::Absent),
        (SyncBase::Present(_) | SyncBase::Absent, disk) => Err(BaseUndecided { base: base.clone(), disk: disk.clone() }),
    }
}
