//! **인터뷰 연계 범위 변경 사건** — 구현 순서 5번의 제품 연결(`tracking-policy` 개발 구성, Unix).
//!
//! 정본은 [공유 저장·전이 계약](../../spec/GIL_Storage_Transition_Contract_v0.1.md) §5.2·§5.3·§5.4·§5.6·§5.7·§6.1–§6.3·§6.6 과
//! §12 의 O-2·O-3(2026-11-24 사용자 결정)이다. **구현 순서 5번 전체·6번 복원·형식 전환·기본 활성화의 완료가 아니다.**
//!
//! ```text
//! model  propose(root, rules)      닫힌 검토 Interview 의 경계에서 정확한 제안 판 n — 원문·효과·경계·비교 근거를 한 digest 로 묶는다
//!        status(root)              저장된 기록의 조회. 아무것도 쓰지 않고 시도·판·사건을 만들지 않는다
//! app    review(root)              최신 제안과 지금 자리(세 값·대기·중단) — capability 는 MCP 계층이 준다
//!        decide(root, n, digest)   Monitor 버튼의 응답 저장 → 승인이면 적용:
//!            잠금 안에서 ① 사전 조건 ② 계속 추적한 경로의 변경 검사(적용 정책으로 — 제안 정책을 쓰지 않는다)
//!            ③ 제안과 지금의 규칙·실제 추적 목록·정책 파일 재확인 → 정책 원문 객체 확정 →
//!            [저장 1] 새 판 + scope_change 사건(기준점 없음) + sync_base  ← 승인 기록 저장 = 정책 적용
//!            → 다시 읽어 확인 → [파일 동기화] §5.2 세 걸음 → [기준점 보관] 한 시도 → [저장 2]
//!        retry(root, event)        명시 재시도 — 지금 서 있는 닫힌 경계에서 새 검사·안정 관측부터
//!        halt(root)                즉시 보관 중단 표식. 현재 상태를 그대로 둔다
//!        release(root, at, how)    사용자가 현재 상태와 정리 방법을 확인한 뒤의 해제. 아무것도 실행하지 않는다
//! ```
//!
//! # 세 가지 성공은 따로 선다(§5.2)
//!
//! 승인 저장이 성공해 다시 읽힌 순간부터 새 판이 적용 정책이다. 정책 파일 쓰기가 실패해도, 기준점 보관이 실패해도 앞 판으로 되돌리지
//! 않는다. 승인 저장이 실패했거나 결과를 확인하지 못했으면 승인·적용 완료를 말하지 않는다. 결과는 `approval_response`·`policy_applied`·
//! `policy_file`·`baseline` 을 따로 싣는다.
//!
//! # 기준점 보관과 재시도는 기존 바탕을 그대로 부른다
//!
//! 관측·범위 변화 검사·객체 확정·저장 결과의 재확인은 `baseline_retry::run_retry` 가 한다. 이 모듈이 더하는 것은 **실제 저장 경계**
//! ([`StateAdapter`])뿐이다: 쓰는 그 시점에 디스크의 기록을 새로 읽어(load 의 검사 전체) 같은 상태에서 빠짐없이 입력을 뽑고, 결속 전체
//! ([`AttemptBinding::holds_in`])가 그대로일 때만 한 저장 단위(세계 이름 등록·`baseline` 경계/발급기/참조·`archived` 시도)를 한 번의
//! `state.yaml` 교체로 적는다. 다시 읽기도 같은 디스크 읽기다. 호출자가 고른 정책·성공값·「없다」는 표식을 받는 자리가 없다 — 미완 복원
//! 흔적은 `.gil/restore/` 를 실제로 본 값이다.
//!
//! # 자동으로 하지 않는 것
//!
//! 조회·재연결·재시작·같은 승인의 재전송은 판·사건·시도를 만들지 않는다. 승인 흐름 안의 첫 보관 시도 하나 말고는, 보관은 Monitor 의
//! 명시 재시도 요청이 있을 때만 시작한다. 중단 표식의 해제는 재시도·보관·복원·종료 승인이 아니다.
//!
//! # 미결 조합은 고르지 않는다
//!
//! 승인 저장 때의 `sync_base` 는 `base_for_new_policy` 그대로다. 세 내용이 모두 다르면 저장 1 을 보류하고(2026-11-22 결정), 계약이
//! 새 기준을 정하지 않은 조합(`BaseUndecided` — 삭제, 기준 없음 위의 새 원문이 아닌 파일, A = B 인데 C 만 다른 경우)은 **저장하지 않고
//! 미결로 돌려준다.** 성공으로 처리하지도, 영구 금지 규칙으로 바꾸지도 않는다. 최초 시작의 좁은 결합 예외는 여기서 쓰지 않는다.
mod ledger;

use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::artifact::start_policy as sp;
use crate::artifact::tracking::{
    AfterFailure, BaselineCommit, BaselineState, EventKind, EventName, FailedAttempt, FailureCause, FailureRecord, HaltAt,
    MarkSeen, OwnedRecords, PolicyName, RecordStanding, RetryOutcome, SaveAnswer, StandingAt, TrackedEvent, run_retry,
};
use crate::artifact::{ContentDigest, TrackingState};
use crate::{CycleKind, Project, ProjectSession};
use ledger::{Disk, Event, Ledger, Proposal, Response};

pub(crate) use ledger::SCOPE_DIR;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
/// 원문 상한 — 시작 준비와 같다.
const MAX_RULES: usize = 256 * 1024;
/// 응답의 채널 값. 개인 신원의 인증이 아니다.
const MONITOR: &str = "monitor_button";
/// 해제할 때 사용자가 확인하는 정리 방법(§5.6 「계속, 미달성 종료와 실패 경로, 범위 변경 제안」). 기록일 뿐 실행이 아니다.
pub(crate) const CLEANUPS: [&str; 3] = ["continue", "end_unachieved", "propose_scope_change"];

/// 이 계층의 거절 — 무엇이 막혔는지 코드와 사람의 말.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Refusal {
    pub(crate) code: &'static str,
    pub(crate) said: String,
    pub(crate) detail: Value,
}

fn refuse(code: &'static str, said: impl Into<String>) -> Refusal {
    Refusal { code, said: said.into(), detail: Value::Null }
}

fn refuse_with(code: &'static str, said: impl Into<String>, detail: Value) -> Refusal {
    Refusal { code, said: said.into(), detail }
}

fn tr(key: &str) -> String {
    crate::locale::text(key).to_string()
}

/// App 전용 capability 의 쓰임 — 서로 바꿔 쓸 수 없다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Purpose {
    Decide,
    Retry,
    Halt,
    Release,
}

// ── 입구 ───────────────────────────────────────────────────────────────────

fn root_of(root: &str) -> Result<PathBuf, Refusal> {
    let path = Path::new(root);
    let invalid = || refuse("invalid_root", tr("scope.invalid_root"));
    if !path.is_absolute() {
        return Err(invalid());
    }
    let real = path.canonicalize().map_err(|_| invalid())?;
    if !real.is_dir() {
        return Err(invalid());
    }
    Ok(real)
}

fn rules() -> Result<crate::RuleSet, Refusal> {
    crate::RuleSet::builtin().map_err(|e| refuse("unavailable", e.to_string()))
}

/// 프로젝트를 **잠그고** 연다 — 이 뒤의 모든 확인·저장이 같은 잠금 안이다.
fn open(root: &Path) -> Result<ProjectSession, Refusal> {
    let state = root.join(crate::STATE_PATH);
    if !state.exists() {
        return Err(refuse("not_started", tr("scope.not_started")));
    }
    let session = ProjectSession::open(rules()?, &state).map_err(|e| match e.is_busy() {
        true => refuse("busy", tr("scope.busy")),
        false => refuse("state_unreadable", e.to_string()),
    })?;
    if session.project().tracking().is_none() {
        return Err(refuse("not_tracking_project", tr("scope.not_tracking_project")));
    }
    Ok(session)
}

fn tracking(project: &Project) -> &TrackingState {
    project.tracking().expect("open 이 추적 기록을 확인했다")
}

fn standing(project: &Project) -> Result<RecordStanding, Refusal> {
    project
        .tracking_standing()
        .expect("open 이 추적 기록을 확인했다")
        .map_err(|e| refuse("record_unreadable", e.to_string()))
}

fn gil_of(session: &ProjectSession) -> Result<PathBuf, Refusal> {
    session.gil().map_err(|e| refuse("unavailable", e.to_string()))
}

fn read_ledger(gil: &Path) -> Result<Option<Ledger>, Refusal> {
    let Some(found) = ledger::read(gil).map_err(|e| refuse("scope_record_unreadable", e))? else { return Ok(None) };
    check_ledger(&found).map_err(|e| refuse("scope_record_unreadable", e))?;
    Ok(Some(found))
}

fn save_ledger(session: &ProjectSession, gil: &Path, before: Option<&Ledger>, after: &Ledger) -> Result<(), Refusal> {
    ledger::write(session.project_lock(), gil, before, after).map_err(|e| refuse("scope_record_not_saved", e))
}

/// 원장의 모든 칸을 다시 잰다 — 판 번호, 원문 바이트의 주소, 제안 digest 의 재계산, 응답이 가리키는 판과 그 유일함.
fn check_ledger(found: &Ledger) -> Result<(), String> {
    for (position, proposal) in found.proposals.iter().enumerate() {
        if u32::try_from(position).ok().and_then(|at| at.checked_add(1)) != Some(proposal.revision) {
            return Err(format!("제안 판 번호가 자리와 다르다: {}", proposal.revision));
        }
        let bytes = B64.decode(&proposal.rules_b64).map_err(|_| format!("판 {} 의 원문을 읽지 못했다", proposal.revision))?;
        if crate::artifact::digest_of(&bytes[..]).map(|found| found.hex()).ok().as_deref() != Some(proposal.rules_digest.as_str()) {
            return Err(format!("판 {} 의 원문과 주소가 다르다", proposal.revision));
        }
        if digest_of(proposal) != proposal.digest {
            return Err(format!("판 {} 의 제안 digest 가 내용과 다르다", proposal.revision));
        }
    }
    if u32::try_from(found.proposals.len()).ok().and_then(|count| count.checked_add(1)) != Some(found.next_revision) {
        return Err("next_revision 이 제안 수와 맞지 않는다".into());
    }
    for response in &found.responses {
        let Some(proposal) = found.proposal(&response.proposal_digest) else {
            return Err("응답이 가리키는 제안이 없다".into());
        };
        if proposal.revision != response.revision
            || response.source != MONITOR
            || !matches!(response.decision.as_str(), "approve" | "request_changes")
            || found.responses_to(&response.proposal_digest).count() != 1
        {
            return Err(format!("판 {} 의 응답 기록이 맞지 않는다", response.revision));
        }
    }
    Ok(())
}

/// 제안의 digest — `digest` 칸을 비운 제안 전체의 sha256. 원문·판·효과·검토 Interview·승인 경계·비교 근거가 모두 들어간다.
fn digest_of(proposal: &Proposal) -> String {
    let mut bare = proposal.clone();
    bare.digest = String::new();
    let bytes = serde_json::to_vec(&bare).expect("제안은 직렬화된다");
    Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

fn disk_now(root: &Path) -> Result<Option<(ContentDigest, Vec<u8>)>, String> {
    sp::read_root_policy_source(root)
}

fn reading(seen: &Result<Option<(ContentDigest, Vec<u8>)>, String>) -> sp::DiskReading {
    match seen {
        Ok(Some((digest, _))) => sp::DiskReading::Present(digest.clone()),
        Ok(None) => sp::DiskReading::Absent,
        Err(reason) => sp::DiskReading::Unreadable { reason: reason.clone() },
    }
}

fn disk_value(reading: &sp::DiskReading) -> Option<Disk> {
    match reading {
        sp::DiskReading::Present(digest) => Some(Disk::Present { digest: digest.hex() }),
        sp::DiskReading::Absent => Some(Disk::Absent),
        sp::DiskReading::Unreadable { .. } => None,
    }
}

/// `.gil/restore/` 를 실제로 본 값 — 미완 복원 흔적(§5.3 사전 조건 ①). 보지 못했으면 「없다」로 채우지 않는다.
fn restore_mark(gil: &Path) -> MarkSeen {
    match crate::restore::left_behind(gil) {
        Ok(None) => MarkSeen::Absent,
        Ok(Some(_)) => MarkSeen::Present,
        Err(_) => MarkSeen::NotProvided,
    }
}

/// 실제 Project 기록에서 **같은 원본의** 정책·사건·사이클·세계·시도·표식을 빠짐없이 뽑는다.
fn owned(project: &Project, restore: MarkSeen) -> Result<OwnedRecords, String> {
    let tracking = project.tracking().ok_or("추적 정책 기록이 없다")?;
    let extracted = tracking.extract(restore).map_err(|e| e.to_string())?;
    Ok(OwnedRecords {
        ledger: extracted.ledger,
        links: extracted.links,
        facts: extracted.facts,
        worlds: extracted.worlds,
        chains: project.chains().to_vec(),
        registry: project.artifacts().clone(),
        marks: extracted.marks,
    })
}

// ── 보이는 값 ──────────────────────────────────────────────────────────────

fn at_word(at: StandingAt) -> Value {
    match at {
        StandingAt::ClosedBoundary { cycle } => json!({"cycle": cycle.to_string(), "state": "closed_boundary"}),
        StandingAt::OpenInterview { cycle } => json!({"cycle": cycle.to_string(), "state": "open_interview"}),
        StandingAt::OpenExperiment { cycle } => json!({"cycle": cycle.to_string(), "state": "open_experiment"}),
    }
}

/// 지금 자리의 **세 값**(§6.1·§6.3)과 대기·중단 — 적용 정책과 확정 정책을 섞지 않는다.
pub(crate) fn standing_view(project: &Project) -> Result<Value, String> {
    let Some(tracking) = project.tracking() else { return Err("추적 정책 기록이 없다".into()) };
    let standing = tracking.standing(project.chains(), project.artifacts()).map_err(|e| e.to_string())?;
    let waiting = tracking.waiting().map(|(name, event)| {
        json!({
            "event": name.to_string(),
            "policy": event.after.to_string(),
            "world_before": event.world_before.map(|world| world.to_string()),
            "approved_at": event.after_cycle.map(|cycle| cycle.to_string()),
            "failed_attempts": event.failed,
            "holds": "next_experiment",
        })
    });
    Ok(json!({
        "chain": standing.chain.to_string(),
        "standing": at_word(standing.at),
        "applied_policy": standing.applied.policy.to_string(),
        "applied_by": standing.applied.introduced_by.to_string(),
        "last_confirmed_world": standing.confirmed.as_ref().map(|confirmed| confirmed.world.to_string()),
        // 후속 Chain 안에서는 확정 정책을 유도하지 않는다 — 적용 정책과 같다고 채우지 않는다.
        "confirmed_policy": standing.confirmed.as_ref().map(|confirmed| confirmed.policy.to_string()),
        "basis_agrees": standing.basis_agrees(),
        "waiting": waiting,
        "archive_halt": tracking.halt().map(|at| at.to_string()),
        "policy_file_sync_base": match tracking.sync_base() {
            sp::SyncBase::Absent => "absent",
            sp::SyncBase::Present(_) => "present",
            sp::SyncBase::Unconfirmed => "unconfirmed",
        },
    }))
}

fn policy_file_word(root: &Path, tracking: &TrackingState) -> &'static str {
    use crate::artifact::tracking::{PolicyFileReading, classify_policy_file};
    match classify_policy_file(tracking.applied().rules(), tracking.sync_base(), &reading(&disk_now(root))) {
        PolicyFileReading::MatchesApplied => "synced",
        PolicyFileReading::StaleMatchesBase => "update_needed",
        PolicyFileReading::UserChanged { deleted: true } => "user_deleted",
        PolicyFileReading::UserChanged { deleted: false } => "user_changed",
        PolicyFileReading::BaseUnconfirmed => "update_needed_cause_unconfirmed",
        PolicyFileReading::Unreadable { .. } => "unreadable",
    }
}

fn proposal_view(proposal: &Proposal, response: Option<&Response>) -> Value {
    let text = B64.decode(&proposal.rules_b64).ok().map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
    json!({
        "revision": proposal.revision,
        "digest": proposal.digest,
        "engine": format!("gil-gilignore/{}", proposal.engine),
        "rules": text,
        "rules_digest": proposal.rules_digest,
        "review_interview": proposal.boundary,
        "approval_boundary": proposal.boundary,
        "policy_before": proposal.policy_before,
        "world_before": proposal.world_before,
        "confirmed_policy": proposal.confirmed_policy,
        "tracked_count": proposal.tracked_after.len(),
        "newly_tracked": proposal.newly_tracked,
        "newly_excluded": proposal.newly_excluded,
        "effects": [tr("scope.effect.newly_tracked"), tr("scope.effect.newly_excluded"), tr("scope.effect.baseline")],
        "response": response.map(|response| response.decision.clone()),
    })
}

/// 이 제안이 지금 자리에서 여전히 승인할 수 있는 판인가 — 경계·발급기·적용 정책·비교 근거가 제안할 때 그대로다.
fn still_current(proposal: &Proposal, standing: &RecordStanding) -> bool {
    let (StandingAt::ClosedBoundary { cycle }, Some(confirmed)) = (standing.at, standing.confirmed.as_ref()) else { return false };
    cycle.to_string() == proposal.boundary
        && standing.issuer == proposal.issuer
        && standing.chain.to_string() == proposal.chain
        && standing.chain == standing.start_chain
        && !standing.chain_closed
        && !standing.closing_boundary
        && standing.applied.policy.to_string() == proposal.policy_before
        && confirmed.world.to_string() == proposal.world_before
        && confirmed.policy.to_string() == proposal.confirmed_policy
}

/// 저장된 정책 판 가운데 이 제안 digest 로 적용된 판.
fn applied_as(tracking: &TrackingState, digest: &str) -> Option<(PolicyName, EventName)> {
    let record = tracking.ledger().policies().iter().find(|record| record.proposal().hex() == digest)?;
    let (event, _) = tracking.events().find(|(_, event)| event.after == record.name())?;
    Some((record.name(), event))
}

fn status_of(root: &Path, project: &Project, found: Option<&Ledger>) -> Result<Value, Refusal> {
    let tracking = tracking(project);
    let now = standing_view(project).map_err(|e| refuse("record_unreadable", e))?;
    let standing = standing(project)?;
    let latest = found.and_then(Ledger::latest);
    let response = latest.and_then(|proposal| found.and_then(|l| l.responses_to(&proposal.digest).next()));
    let applied = latest.and_then(|proposal| applied_as(tracking, &proposal.digest));
    let phase = match (latest, response.map(|r| r.decision.as_str()), applied) {
        (None, _, _) => "no_proposal",
        (Some(_), _, Some(_)) => "applied",
        (Some(_), Some("approve"), None) => "approved_not_applied",
        (Some(_), Some(_), None) => "changes_requested",
        (Some(proposal), None, None) if still_current(proposal, &standing) => "awaiting_approval",
        (Some(_), None, None) => "superseded_by_progress",
    };
    let halted = tracking.halt().is_some();
    let next = match (halted, tracking.waiting().is_some(), phase) {
        (true, _, _) => tr("scope.next.halted"),
        (false, true, _) => tr("scope.next.waiting"),
        (false, false, "awaiting_approval") => tr("scope.next.awaiting_approval"),
        (false, false, "approved_not_applied") => tr("scope.next.approved_not_applied"),
        _ => tr("scope.next.none"),
    };
    Ok(json!({
        "said": tr("scope.status"),
        "next": next,
        "phase": phase,
        "tracking": now,
        "policy_file": policy_file_word(root, tracking),
        "proposal": latest.map(|proposal| proposal_view(proposal, response)),
        "applied": applied.map(|(policy, event)| json!({"policy": policy.to_string(), "event": event.to_string()})),
    }))
}

// ── 모델 도구: 제안과 조회 ─────────────────────────────────────────────────

/// 정확한 범위 변경 제안 판 하나를 원장에 남긴다. 프로젝트의 파일·정책·사건을 바꾸지 않는다.
///
/// 닫힌 **Interview** 의 경계에서만 받는다 — 그 인터뷰가 변경안을 검토한 인터뷰이자 승인을 저장할 경계다(§5.1·§5.3). 제안은 승인이
/// 아니고, 승인 전에는 어떤 판정에도 쓰이지 않는다(§5.4 「제안 정책」).
pub(crate) fn propose(root: &str, text: String) -> Result<Value, Refusal> {
    let root = root_of(root)?;
    let session = open(&root)?;
    let project = session.project();
    let tracking = tracking(project);
    project.refuse_while_halted().map_err(|said| refuse("halted", said))?;
    let standing = standing(project)?;
    let (StandingAt::ClosedBoundary { cycle }, Some(confirmed)) = (standing.at, standing.confirmed.as_ref()) else {
        return Err(refuse("not_at_boundary", tr("scope.not_at_boundary")));
    };
    if standing.chain_closed || standing.closing_boundary || standing.chain != standing.start_chain {
        return Err(refuse("chain_boundary", tr("scope.chain_boundary")));
    }
    if project.cycles().pending_revisit().is_some() {
        return Err(refuse("revisit_pending", tr("scope.revisit_pending")));
    }
    if project.cycles().current().kind() != CycleKind::Interview {
        return Err(refuse("review_interview_required", tr("scope.review_interview_required")));
    }
    if text.len() > MAX_RULES {
        return Err(refuse("rules_too_large", tr("scope.rules_too_large")));
    }
    let bytes = text.into_bytes();
    let engine = tracking.applied().engine();
    let proposed = sp::ApprovedPolicy::compile(engine, &bytes).map_err(|e| refuse("rules_rejected", e.to_string()))?;
    let after = crate::artifact::evaluate_scope(&root, &proposed).map_err(|e| refuse("unobservable", e.to_string()))?;
    if !after.blockers().is_empty() {
        return Err(refuse_with("blocked", tr("scope.blocked"), json!({"blockers": after.blockers().len()})));
    }
    let applied = session.approved_policy().map_err(|e| refuse("unavailable", e.to_string()))?.expect("추적 기록이 있다");
    let now = crate::artifact::evaluate_scope(&root, &applied).map_err(|e| refuse("unobservable", e.to_string()))?;
    let paths = |evaluation: &sp::TreeEvaluation| -> Vec<String> {
        evaluation.tracked().iter().map(|file| file.path.as_str().to_owned()).collect()
    };
    let (tracked_now, tracked_after) = (paths(&now), paths(&after));
    let seen = reading(&disk_now(&root));
    let Some(disk) = disk_value(&seen) else {
        return Err(refuse("policy_file_unreadable", tr("scope.policy_file_unreadable")));
    };
    let gil = gil_of(&session)?;
    let before = read_ledger(&gil)?;
    let mut after_ledger = before.clone().unwrap_or_else(Ledger::new);
    let mut proposal = Proposal {
        revision: after_ledger.next_revision,
        digest: String::new(),
        engine: engine.number(),
        rules_b64: B64.encode(&bytes),
        rules_digest: proposed.identity().rules_digest().hex(),
        chain: standing.chain.to_string(),
        boundary: cycle.to_string(),
        issuer: standing.issuer,
        policy_before: standing.applied.policy.to_string(),
        world_before: confirmed.world.to_string(),
        confirmed_policy: confirmed.policy.to_string(),
        disk,
        newly_tracked: tracked_after.iter().filter(|path| !tracked_now.contains(path)).cloned().collect(),
        newly_excluded: tracked_now.iter().filter(|path| !tracked_after.contains(path)).cloned().collect(),
        tracked_after,
    };
    proposal.digest = digest_of(&proposal);
    after_ledger.next_revision = after_ledger.next_revision.checked_add(1).ok_or_else(|| refuse("unavailable", "판 번호가 소진됐다"))?;
    after_ledger.proposals.push(proposal);
    save_ledger(&session, &gil, before.as_ref(), &after_ledger)?;
    let mut out = status_of(&root, session.project(), Some(&after_ledger))?;
    out["said"] = json!(tr("scope.proposed"));
    out["next"] = json!(tr("scope.next.awaiting_approval"));
    Ok(out)
}

/// 저장된 기록의 조회 — 아무것도 쓰지 않는다. 응답 유실·재연결·재시작 뒤에 추측하거나 승인을 되풀이하는 대신 이것을 읽는다.
pub(crate) fn status(root: &str) -> Result<Value, Refusal> {
    let root = root_of(root)?;
    status_at(&root)
}

fn status_at(root: &Path) -> Result<Value, Refusal> {
    let state = root.join(crate::STATE_PATH);
    if !state.exists() {
        return Err(refuse("not_started", tr("scope.not_started")));
    }
    let session = ProjectSession::open_read_only(rules()?, &state).map_err(|e| refuse("state_unreadable", e.to_string()))?;
    if session.project().tracking().is_none() {
        return Err(refuse("not_tracking_project", tr("scope.not_tracking_project")));
    }
    let found = read_ledger(&root.join(crate::artifact::GIL_DIR))?;
    status_of(root, session.project(), found.as_ref())
}

// ── App 전용: 검토 화면 ────────────────────────────────────────────────────

/// 한 capability 가 묶이는 값 — 쓰임과, 그 쓰임이 가리키는 정확한 대상.
pub(crate) type GrantFor = (Purpose, String);

/// Monitor 가 보여 줄 값과, 지금 줄 수 있는 capability 의 대상들. **조회다** — 아무것도 쓰지 않는다.
pub(crate) fn review(root: &Path) -> Result<(Value, Vec<GrantFor>), Refusal> {
    let mut view = status_at(root)?;
    let mut grants = Vec::new();
    let halted = view["tracking"]["archive_halt"].as_str().map(str::to_owned);
    match &halted {
        // 중단 중에는 해제만 줄 수 있다 — 승인·재시도는 표식이 있는 동안 받지 않는다(T12).
        Some(at) => grants.push((Purpose::Release, at.clone())),
        None => {
            grants.push((Purpose::Halt, "halt".to_string()));
            if view["phase"] == "awaiting_approval"
                && let (Some(revision), Some(digest)) = (view["proposal"]["revision"].as_u64(), view["proposal"]["digest"].as_str())
            {
                grants.push((Purpose::Decide, format!("{revision}:{digest}")));
            }
            if let Some(event) = view["tracking"]["waiting"]["event"].as_str()
                && view["tracking"]["standing"]["state"] == "closed_boundary"
            {
                grants.push((Purpose::Retry, event.to_string()));
            }
        }
    }
    view["cleanups"] = json!(CLEANUPS);
    Ok((view, grants))
}

// ── App 전용: Monitor 응답과 적용 ──────────────────────────────────────────

/// Monitor 버튼의 응답을 기록한다. **capability 확인은 부르는 MCP App 계층이 이미 했다** — 이 함수는 모델·CLI 경로에 노출하지 않는다.
/// 같은 응답의 재전송은 저장된 결과의 조회로 답하고(새 판·사건·시도를 만들지 않는다), 다른 응답은 거절한다.
pub(crate) fn decide(root: &Path, revision: u32, digest: &str, approve: bool) -> Result<Value, Refusal> {
    let decision = if approve { "approve" } else { "request_changes" };
    let mut session = open(root)?;
    let gil = gil_of(&session)?;
    let before = read_ledger(&gil)?.ok_or_else(|| refuse("no_proposal", tr("scope.no_proposal")))?;
    let proposal = before.proposal(digest).cloned().ok_or_else(|| refuse("stale_proposal", tr("scope.stale_proposal")))?;
    if proposal.revision != revision {
        return Err(refuse("stale_proposal", tr("scope.stale_proposal")));
    }
    let existing = before.responses_to(digest).next().cloned();
    if let Some(existing) = &existing {
        if existing.decision != decision {
            return Err(refuse("response_final", tr("scope.response_final")));
        }
        // 같은 응답의 재전송 — 저장된 판·사건을 조회해 이어 간다(§5.3 표). 판·사건·시도를 만들지 않는다.
        let mut out = status_of(root, session.project(), Some(&before))?;
        out["resent"] = json!(true);
        return Ok(out);
    }
    session.project().refuse_while_halted().map_err(|said| refuse("halted", said))?;
    if before.latest().map(|latest| &latest.digest) != Some(&proposal.digest) || !still_current(&proposal, &standing(session.project())?) {
        return Err(refuse("stale_proposal", tr("scope.stale_proposal")));
    }
    let mut after = before.clone();
    after.responses.push(Response { revision, proposal_digest: digest.into(), decision: decision.into(), source: MONITOR.into() });
    save_ledger(&session, &gil, Some(&before), &after)?;
    if !approve {
        return status_of(root, session.project(), Some(&after));
    }
    // 승인 응답은 저장됐다. 그것은 아직 정책 적용이 아니다 — 적용은 아래 저장 1 이 다시 읽혀 확인될 때다.
    let applied = apply(&mut session, root, &gil, &after, &proposal);
    let found = read_ledger(&gil).ok().flatten();
    let mut out = status_of(root, session.project(), found.as_ref().or(Some(&after)))?;
    out["facts"] = json!({"approval_response": "recorded"});
    match applied {
        Ok(facts) => {
            for (key, value) in facts.as_object().into_iter().flatten() {
                out["facts"][key] = value.clone();
            }
        }
        Err(held) => {
            out["facts"]["policy_applied"] = json!(false);
            out["application_held"] = json!({"code": held.code, "said": held.said, "detail": held.detail});
        }
    }
    Ok(out)
}

/// 승인된 제안의 적용 — 잠금 안에서 ①②③ 을 다시 확인한 뒤 저장 1, 파일 동기화, 기준점 보관 한 시도.
fn apply(session: &mut ProjectSession, root: &Path, gil: &Path, found: &Ledger, proposal: &Proposal) -> Result<Value, Refusal> {
    // 이미 이 제안으로 적용된 판이 있으면 조회로 끝난다.
    if let Some((policy, event)) = applied_as(tracking(session.project()), &proposal.digest) {
        return Ok(json!({"policy_applied": true, "policy": policy.to_string(), "event": event.to_string(), "repeated": true}));
    }
    // ① 사전 조건 — 열린 Chain 의 닫힌 경계, 열린 단계 없음, 되돌아온 직후가 아님, 미완 복원 없음, 중단 표식 없음, 종료 경계 아님.
    session.project().refuse_while_halted().map_err(|said| refuse("halted", said))?;
    let standing = standing(session.project())?;
    if !still_current(proposal, &standing) {
        return Err(refuse("stale_proposal", tr("scope.stale_proposal")));
    }
    if session.project().cycles().pending_revisit().is_some() {
        return Err(refuse("revisit_pending", tr("scope.revisit_pending")));
    }
    if restore_mark(gil) != MarkSeen::Absent {
        return Err(refuse("restore_unfinished", tr("scope.restore_unfinished")));
    }
    let (StandingAt::ClosedBoundary { cycle }, Some(confirmed)) = (standing.at, standing.confirmed.clone()) else {
        return Err(refuse("not_at_boundary", tr("scope.not_at_boundary")));
    };
    // ② 계속 추적한 경로의 변경 검사 — **적용 정책**으로 관측하고 확정 정책부터 적용 정책까지의 구간으로 견준다. 제안 정책을 쓰지
    //    않으므로 새 제외 규칙으로 기존 변경을 가릴 수 없다(§5.4).
    match session.world_state().map_err(|e| refuse("unobservable", e.to_string()))? {
        crate::session::WorldState::Clean { .. } => {}
        crate::session::WorldState::Dirty { world, changes } => {
            return Err(refuse_with("unconfirmed_change", tr("scope.unconfirmed_change"),
                json!({"world": world.to_string(), "changes": changes})));
        }
        crate::session::WorldState::Unknown { said, .. } => {
            return Err(refuse_with("unobservable", tr("scope.unobservable"), json!(said)));
        }
    }
    // ③ 제안과 지금의 규칙·실제 추적 목록·정책 파일 — 승인된 바이트를 그대로 다시 해석한다.
    let bytes = B64.decode(&proposal.rules_b64).map_err(|_| refuse("scope_record_unreadable", "원문을 읽지 못했다"))?;
    let engine = sp::EngineVersion::from_number(proposal.engine).ok_or_else(|| refuse("scope_record_unreadable", "모르는 엔진"))?;
    let policy = sp::ApprovedPolicy::compile(engine, &bytes).map_err(|e| refuse("rules_rejected", e.to_string()))?;
    let rules = policy.identity().rules_digest().clone();
    if rules.hex() != proposal.rules_digest {
        return Err(refuse("scope_record_unreadable", "원문과 주소가 다르다"));
    }
    let evaluation = crate::artifact::evaluate_scope(root, &policy).map_err(|e| refuse("unobservable", e.to_string()))?;
    let tracked: Vec<String> = evaluation.tracked().iter().map(|file| file.path.as_str().to_owned()).collect();
    if !evaluation.blockers().is_empty() || tracked != proposal.tracked_after {
        return Err(refuse("scope_changed", tr("scope.scope_changed")));
    }
    let seen = disk_now(root);
    let disk = reading(&seen);
    let base = tracking(session.project()).sync_base().clone();
    let Some(shown) = disk_value(&disk) else {
        // 읽지 못함은 변경의 증거도 동일의 증거도 아니다 — 저장하지 않고 이번 적용만 멈춘다.
        return Err(refuse("policy_file_unreadable", tr("scope.policy_file_unreadable")));
    };
    if shown != proposal.disk {
        return Err(refuse("policy_file_changed", tr("scope.policy_file_changed")));
    }
    let chosen = match sp::base_for_new_policy(&rules, &base, &disk) {
        Ok(sp::NewPolicyBase::Chosen(chosen)) => chosen,
        // 세 내용이 모두 다르다 — 저장 1 을 하지 않는다(2026-11-22 결정). 기존 적용 정책과 디스크 파일은 그대로다.
        Ok(sp::NewPolicyBase::Held(three)) => {
            return Err(refuse_with("held_three_way", tr("scope.held_three_way"),
                json!({"base": three.base.hex(), "approved": three.new_rules.hex(), "disk": three.disk.hex()})));
        }
        // 계약이 새 기준을 정하지 않은 조합 — 고르지 않고 저장하지 않는다. 성공도 영구 금지도 아니다.
        Err(undecided) => {
            return Err(refuse_with("base_undecided", tr("scope.base_undecided"), json!(undecided.to_string())));
        }
    };
    // 정책 원문 객체를 상태보다 먼저 확정한다(§5.1).
    let store = session.object_store().map_err(|e| refuse("unavailable", e.to_string()))?;
    sp::store_policy_source(&store, engine, &rules, &bytes)
        .map_err(|e| refuse_with("policy_source_not_stored", tr("scope.policy_source_not_stored"), json!(e.to_string())))?;
    // 저장 1 — 새 판 + scope_change 사건(기준점 없음) + sync_base 를 한 번의 상태 저장으로.
    let state = tracking(session.project());
    let name = PolicyName::new(state.ledger().next_policy_id()).ok_or_else(|| refuse("unavailable", "정책 이름이 소진됐다"))?;
    let proposal_digest = sp::ProposalDigest::parse(&proposal.digest).ok_or_else(|| refuse("scope_record_unreadable", "제안 digest 의 꼴"))?;
    let record = sp::PolicyRecord::new(name, engine, rules.clone(), proposal_digest);
    let event = TrackedEvent {
        kind: EventKind::ScopeChange,
        chain: standing.chain,
        after_cycle: Some(cycle),
        review_cycle: Some(cycle),
        next_cycle_id: standing.issuer,
        before: Some(standing.applied.policy),
        after: name,
        world_before: Some(confirmed.world),
        archived: None,
        failed: Vec::new(),
    };
    let next = session.project().with_scope_change(record, chosen.clone(), event)
        .map_err(|e| refuse("record_refused", e.to_string()))?;
    let saved = save_step(session, &next, "approval");
    // 저장 결과를 실제 저장소에서 다시 읽어 판정한다 — 저장 답만으로 승인·적용 완료를 말하지 않는다.
    let reread = match session.reload() {
        Ok(reread) => reread,
        Err(e) => {
            return Err(refuse_with("approval_uncertain", tr("scope.approval_uncertain"),
                json!({"save": saved.err().map(|e| e.to_string()), "reread": e.to_string()})));
        }
    };
    let Some((policy_name, event_name)) = applied_as(tracking(&reread), &proposal.digest) else {
        *session.project_mut() = reread;
        return Err(refuse_with("approval_not_saved", tr("scope.approval_not_saved"), json!(saved.err().map(|e| e.to_string()))));
    };
    *session.project_mut() = reread;
    // 원장의 적용 기록은 최선 노력이다 — 적용의 근거는 상태 파일이다.
    let mut logged = found.clone();
    logged.events.push(Event::Applied {
        revision: proposal.revision,
        proposal_digest: proposal.digest.clone(),
        policy: policy_name.to_string(),
        event: event_name.to_string(),
    });
    let _ = save_ledger(session, gil, Some(found), &logged);

    // 시험만 쓰는 자리 — 승인 저장과 기준점 보관 사이의 외부 변경을 흉내 낸다.
    #[cfg(test)]
    if let Some(hook) = AFTER_APPROVAL.with(|slot| slot.borrow_mut().take()) {
        hook();
    }
    // 정책 파일 동기화 — 실패해도 정책은 유효하다. 따로 표시한다.
    let file = sync_policy_file(session, root, &bytes, &rules, &chosen, &disk);
    // 기준점 보관 — 승인 흐름의 한 시도. 실패하면 보관 대기가 남고, 그 뒤의 시도는 명시 재시도뿐이다.
    let baseline = attempt(session, root, gil, event_name);
    Ok(json!({
        "policy_applied": true,
        "policy": policy_name.to_string(),
        "event": event_name.to_string(),
        "policy_file": file,
        "baseline": baseline,
    }))
}

/// 한 번의 상태 저장. 시험은 `SAVE_FAULT` 로 이 자리의 실패(저장하지 않음)와 저장 뒤 오류를 넣는다.
fn save_step(session: &ProjectSession, next: &Project, step: &'static str) -> Result<(), crate::SessionError> {
    #[cfg(test)]
    {
        let fault = SAVE_FAULT.with(|slot| {
            let mut slot = slot.borrow_mut();
            match *slot {
                Some((at, _)) if at == step => slot.take(),
                _ => None,
            }
        });
        if let Some((_, stored)) = fault {
            if stored {
                session.save_project(next)?;
            }
            return Err(crate::SessionError::Observe { said: "시험이 넣은 저장 실패".into() });
        }
    }
    // 통합 시험이 새 프로세스에 넣는 실패 자리 — 복원·되돌아가기의 failpoint 와 같은 규율이며 **release 빌드에는 없다.**
    #[cfg(debug_assertions)]
    if std::env::var("GIL_SCOPE_FAIL").is_ok_and(|at| at == step) {
        return Err(crate::SessionError::Observe { said: format!("failpoint GIL_SCOPE_FAIL={step}") });
    }
    let _ = step;
    session.save_project(next)
}

#[cfg(test)]
thread_local! {
    /// (자리, 저장한 뒤 실패로 보고하는가) — `approval`·`sync_start`·`sync_done`·`baseline`·`failure`·`halt`·`release`.
    pub(crate) static SAVE_FAULT: std::cell::RefCell<Option<(&'static str, bool)>> = const { std::cell::RefCell::new(None) };
    /// 정책 파일 쓰기의 실패 주입 자리 이름(`write_with` 의 단계).
    pub(crate) static WRITE_FAULT: std::cell::RefCell<Option<&'static str>> = const { std::cell::RefCell::new(None) };
    /// 승인 저장(저장 1)이 확인된 직후, 파일 동기화·기준점 보관 전에 한 번 부른다.
    pub(crate) static AFTER_APPROVAL: std::cell::RefCell<Option<Box<dyn FnOnce()>>> = const { std::cell::RefCell::new(None) };
    /// 다시 읽기 한 번을 실패로 만든다 — 결과 불명.
    pub(crate) static REREAD_FAULT: std::cell::RefCell<Option<()>> = const { std::cell::RefCell::new(None) };
}

#[cfg(not(test))]
fn write_policy(root: &Path, bytes: &[u8], expected: Option<&ContentDigest>) -> sp::PolicyWrite {
    sp::write_root_policy_file(root, bytes, expected)
}

#[cfg(test)]
fn write_policy(root: &Path, bytes: &[u8], expected: Option<&ContentDigest>) -> sp::PolicyWrite {
    let at = WRITE_FAULT.with(|slot| *slot.borrow());
    sp::write_with(root, bytes, expected, &mut |step| match at {
        Some(fault) if fault == step => Err(std::io::Error::other("시험이 넣은 실패")),
        _ => Ok(()),
    })
}

/// 정책 파일 동기화(§5.2 의 세 걸음, T13). 승인·적용은 이미 유효하며 이 결과는 따로 선다.
///
/// 디스크가 확인된 기준과 같을 때만 쓴다. 쓰기 전에 기준을 `unconfirmed` 로, 쓰기 성공을 확인한 뒤에 적용 원문으로 저장한다.
/// `unconfirmed` 에서는 자동으로 쓰지 않는다.
fn sync_policy_file(
    session: &mut ProjectSession,
    root: &Path,
    bytes: &[u8],
    rules: &ContentDigest,
    base: &sp::SyncBase,
    disk: &sp::DiskReading,
) -> &'static str {
    let expected = match (disk, base) {
        (sp::DiskReading::Present(found), _) if found == rules => return "already_matches",
        (sp::DiskReading::Unreadable { .. }, _) => return "not_written_unreadable",
        (_, sp::SyncBase::Unconfirmed) => return "not_written_base_unconfirmed",
        (sp::DiskReading::Absent, sp::SyncBase::Absent) => None,
        (sp::DiskReading::Present(found), sp::SyncBase::Present(kept)) if found == kept => Some(found.clone()),
        _ => return "not_written",
    };
    let store = |session: &mut ProjectSession, value: sp::SyncBase, step: &'static str| -> bool {
        let Some(next) = session.project().with_sync_base(value) else { return false };
        match save_step(session, &next, step) {
            Ok(()) => {
                *session.project_mut() = next;
                true
            }
            Err(_) => false,
        }
    };
    // 0. 잠금 안에서 디스크가 확인된 기준과 같음을 다시 본다. 아니면 쓰지 않는다.
    if reading(&disk_now(root)) != *disk {
        return "not_written_disk_changed";
    }
    // 1. 쓰기 시작을 먼저 남긴다. 이 저장이 실패하면 파일을 쓰지 않는다.
    if !store(session, sp::SyncBase::Unconfirmed, "sync_start") {
        return "not_written_start_not_saved";
    }
    // 2. 파일 쓰기 — 임시 파일 → fsync → 교체 직전 재관측 → 교체 → 다시 읽기.
    match write_policy(root, bytes, expected.as_ref()) {
        sp::PolicyWrite::Replaced { reread: sp::DiskReading::Present(found) } if &found == rules => {
            // 3. 쓰기 성공 뒤에 기준을 옮긴다. 실패하면 디스크 = 적용 원문, 기준 unconfirmed — 동기화됨으로 읽힌다.
            match store(session, sp::SyncBase::Present(rules.clone()), "sync_done") {
                true => "written",
                false => "written_base_unconfirmed",
            }
        }
        // 교체 전의 확인된 장애 — 같은 잠금 안에서 다시 읽은 디스크가 이전 기준과 같을 때만 그 기준으로 되돌린다.
        sp::PolicyWrite::NotReplaced { .. } if reading(&disk_now(root)) == *disk => {
            match store(session, base.clone(), "sync_done") {
                true => "update_failed",
                false => "update_failed_base_unconfirmed",
            }
        }
        // 교체 여부를 알 수 없거나 그사이 디스크가 달라졌다 — 되돌리지 않는다. 자동으로 다시 쓰지 않는다.
        _ => "unsettled",
    }
}

// ── 기준점 보관: 실제 저장 경계 ────────────────────────────────────────────

/// `baseline_retry` 의 내부 저장 경계를 실제 `state.yaml` 에 잇는다. 세 문 모두 프로젝트 잠금 안이다.
struct StateAdapter<'s> {
    session: &'s mut ProjectSession,
    gil: PathBuf,
}

impl StateAdapter<'_> {
    /// 디스크의 기록을 새로 읽고(load 의 검사 전체) 같은 상태에서 입력을 뽑는다.
    fn fresh(&self) -> Result<(Project, OwnedRecords), String> {
        #[cfg(test)]
        if REREAD_FAULT.with(|slot| slot.borrow_mut().take()).is_some() {
            return Err("시험이 넣은 다시 읽기 실패".into());
        }
        let project = self.session.reload().map_err(|e| e.to_string())?;
        let records = owned(&project, restore_mark(&self.gil))?;
        Ok((project, records))
    }
}

impl BaselineState for StateAdapter<'_> {
    fn save_baseline(&mut self, commit: &BaselineCommit) -> SaveAnswer {
        let not = |said: String| SaveAnswer::NotStored { said };
        // 쓰는 그 시점에 결속 전체를 다시 확인한다 — 경계·발급기·적용 정책·마지막 확정 세계와 확정 정책·그 근거·시도 수.
        let (now, records) = match self.fresh() {
            Ok(found) => found,
            Err(said) => return not(said),
        };
        if let Err(lost) = commit.binding().holds_in(&records.inputs()) {
            return not(format!("결속이 달라졌다: {lost:?}"));
        }
        // 한 저장 단위: 세계 이름 등록 + baseline 의 경계·발급기·snapshot_ref + archived 시도. 앞선 기록은 그대로다.
        let (next, snapshot) = match now.with_baseline(commit.binding().event(), commit.manifest().clone()) {
            Ok(next) => next,
            Err(e) => return not(e.to_string()),
        };
        if snapshot != commit.snapshot() {
            return not(format!("등록부가 준 이름 {snapshot} 이 이 시도가 정한 {} 과 다르다", commit.snapshot()));
        }
        match save_step(self.session, &next, "baseline") {
            Ok(()) => SaveAnswer::Stored,
            // 오류가 났지만 닿았는지 모른다 — 다시 읽기가 가린다.
            Err(e) => SaveAnswer::Uncertain { said: e.to_string() },
        }
    }

    fn reread(&mut self) -> Result<OwnedRecords, String> {
        self.fresh().map(|(_, records)| records)
    }

    fn record_failure(&mut self, failure: &FailedAttempt) -> FailureRecord {
        let not = |said: String| FailureRecord::NotRecorded { said };
        let (now, records) = match self.fresh() {
            Ok(found) => found,
            Err(said) => return not(said),
        };
        if let Err(lost) = failure.binding().holds_in(&records.inputs()) {
            return not(format!("결속이 달라졌다: {lost:?}"));
        }
        let next = match now.with_failed_attempt(failure.binding().event(), cause_word(failure.cause())) {
            Ok(next) => next,
            Err(e) => return not(e.to_string()),
        };
        match save_step(self.session, &next, "failure") {
            Ok(()) => FailureRecord::Recorded,
            Err(e) => not(e.to_string()),
        }
    }
}

/// 실패 사유의 저장 이름(§5.3) — 미확정 변경, 불안정한 관측, 관측 불가, 저장 실패.
fn cause_word(cause: &FailureCause) -> &'static str {
    match cause {
        FailureCause::UnconfirmedChange { .. } => "external_change",
        FailureCause::UnstableObservation { .. } => "unstable_observation",
        FailureCause::ObservationRefused { .. } | FailureCause::TargetChanged { .. } => "unobservable",
        FailureCause::ObjectStore { .. } | FailureCause::StateNotStored { .. } => "store_failure",
    }
}

/// 기준점 보관의 **한 시도** — 지금 서 있는 경계에서 새 검사와 안정 관측부터 한다. 앞선 시도의 객체나 기록으로 성공을 추정하지 않는다.
fn attempt(session: &mut ProjectSession, root: &Path, gil: &Path, event: EventName) -> Value {
    let records = match owned(session.project(), restore_mark(gil)) {
        Ok(records) => records,
        Err(said) => return json!({"result": "not_started", "why": said}),
    };
    let run = {
        let mut adapter = StateAdapter { session: &mut *session, gil: gil.to_path_buf() };
        run_retry(event, records.inputs(), root, &mut adapter)
    };
    // 메모리의 값을 저장된 기록에 맞춘다. 읽지 못하면 결과를 확정으로 말하지 않는다.
    let refreshed = match session.reload() {
        Ok(project) => {
            *session.project_mut() = project;
            true
        }
        Err(_) => false,
    };
    let mut out = match run {
        Err(why) => json!({"result": "not_started", "why": why.to_string()}),
        Ok(run) => match run.outcome {
            RetryOutcome::Confirmed { commit, delta, .. } => json!({
                "result": "archived",
                "snapshot": commit.snapshot().to_string(),
                "archived_at": run.binding.boundary().to_string(),
                "compared": delta.compared(),
                "newly_tracked": delta.newly_tracked().len(),
            }),
            RetryOutcome::Failed { cause, now } => {
                let (still_waiting, recorded) = match &now {
                    AfterFailure::StillWaiting { record, .. } => (Some(true), matches!(record, FailureRecord::Recorded)),
                    AfterFailure::BindingGone { .. } => (Some(false), false),
                    AfterFailure::NotVerified { .. } => (None, false),
                };
                json!({
                    "result": "failed",
                    "cause": cause_word(&cause),
                    "detail": format!("{cause:?}"),
                    // 「이 시도가 확정하지 못했다」와 「지금 사건이 여전히 대기다」는 다른 사실이다. null 은 확인하지 못함이다.
                    "still_waiting": still_waiting,
                    "failure_recorded": recorded,
                })
            }
            // 저장됐을 수도 있다 — 성공도 확정된 대기도 아니다. 조회로 확인한다.
            RetryOutcome::Unknown { why, .. } => json!({"result": "unknown", "why": format!("{why:?}")}),
        },
    };
    out["record_reread"] = json!(refreshed);
    out
}

/// **명시 재시도** — Monitor 의 요청으로만 부른다. 지금 서 있는 유효한 닫힌 경계에서 한다(과거 경계로 되돌아가지 않는다).
/// 대기가 아니거나, 중단 표식이 있거나, 열린 사이클·종료 경계면 시도를 만들지 않는다.
pub(crate) fn retry(root: &Path, event: &str) -> Result<Value, Refusal> {
    let mut session = open(root)?;
    let gil = gil_of(&session)?;
    session.project().refuse_while_halted().map_err(|said| refuse("halted", said))?;
    let Some(waiting) = session.project().waiting_event() else {
        return Err(refuse("not_waiting", tr("scope.not_waiting")));
    };
    if waiting.to_string() != event {
        return Err(refuse("not_waiting", tr("scope.not_waiting")));
    }
    let standing = standing(session.project())?;
    let StandingAt::ClosedBoundary { cycle } = standing.at else {
        return Err(refuse("not_at_boundary", tr("scope.retry_needs_boundary")));
    };
    if standing.chain_closed || standing.closing_boundary {
        return Err(refuse("chain_boundary", tr("scope.chain_boundary")));
    }
    // 요청을 시도보다 먼저 남긴다. 남기지 못하면 시도하지 않는다.
    let before = read_ledger(&gil)?;
    let mut after = before.clone().unwrap_or_else(Ledger::new);
    after.events.push(Event::RetryRequested { event: event.to_string(), boundary: cycle.to_string(), source: MONITOR.into() });
    save_ledger(&session, &gil, before.as_ref(), &after)?;
    let baseline = attempt(&mut session, root, &gil, waiting);
    let mut out = status_of(root, session.project(), Some(&after))?;
    out["facts"] = json!({"baseline": baseline});
    Ok(out)
}

// ── 즉시 보관 중단(§5.6, O-2) ──────────────────────────────────────────────

/// 중단 표식을 적는다 — Monitor 의 요청. 현재 상태를 그대로 둔다: 열린 단계를 닫지 않고, 실패로 판정하지 않고, 복원·삭제·범위 변경을
/// 하지 않는다. 이미 표식이 있으면 같은 표식을 돌려준다.
pub(crate) fn halt(root: &Path) -> Result<Value, Refusal> {
    let mut session = open(root)?;
    let gil = gil_of(&session)?;
    let (next, at) = session.project().with_halt().map_err(|e| refuse("record_refused", e.to_string()))?;
    if session.project().tracking().and_then(TrackingState::halt).is_none() {
        let saved = save_step(&session, &next, "halt");
        let reread = session.reload().map_err(|e| refuse_with("halt_uncertain", tr("scope.halt_uncertain"), json!(e.to_string())))?;
        let stored = reread.tracking().and_then(TrackingState::halt) == Some(at);
        *session.project_mut() = reread;
        if !stored {
            return Err(refuse_with("halt_not_saved", tr("scope.halt_not_saved"), json!(saved.err().map(|e| e.to_string()))));
        }
        let before = read_ledger(&gil).ok().flatten();
        let mut after = before.clone().unwrap_or_else(Ledger::new);
        after.events.push(Event::HaltRequested { at: at.to_string(), source: MONITOR.into() });
        let _ = save_ledger(&session, &gil, before.as_ref(), &after);
    }
    let found = read_ledger(&gil).ok().flatten();
    let mut out = status_of(root, session.project(), found.as_ref())?;
    out["said"] = json!(crate::tr!("scope.halted", at = at));
    out["next"] = json!(tr("scope.next.halted"));
    Ok(out)
}

/// 중단 표식을 지운다 — Monitor 에서 사용자가 **현재 상태(`at`)와 정리 방법**을 확인했을 때만. 그 밖의 어떤 기록도 바꾸지 않고
/// 아무것도 실행하지 않는다: 보관·재시도·복원·닫기·종료 승인이 아니며, 해제 뒤의 진행은 별도의 명시 행동이다.
pub(crate) fn release(root: &Path, at: &str, cleanup: &str) -> Result<Value, Refusal> {
    if !CLEANUPS.contains(&cleanup) {
        return Err(refuse("cleanup_unknown", tr("scope.cleanup_unknown")));
    }
    let mut session = open(root)?;
    let gil = gil_of(&session)?;
    let halted: Option<HaltAt> = session.project().tracking().and_then(TrackingState::halt);
    let Some(now) = halted else {
        return Err(refuse("not_halted", tr("scope.not_halted")));
    };
    if now.to_string() != at {
        return Err(refuse("halt_place_differs", tr("scope.halt_place_differs")));
    }
    let next = session.project().without_halt().map_err(|e| refuse("record_refused", e.to_string()))?;
    let saved = save_step(&session, &next, "release");
    let reread = session.reload().map_err(|e| refuse_with("release_uncertain", tr("scope.release_uncertain"), json!(e.to_string())))?;
    let cleared = reread.tracking().and_then(TrackingState::halt).is_none();
    *session.project_mut() = reread;
    if !cleared {
        return Err(refuse_with("release_not_saved", tr("scope.release_not_saved"), json!(saved.err().map(|e| e.to_string()))));
    }
    let before = read_ledger(&gil).ok().flatten();
    let mut after = before.clone().unwrap_or_else(Ledger::new);
    after.events.push(Event::HaltReleased { at: at.to_string(), cleanup: cleanup.to_string(), source: MONITOR.into() });
    let _ = save_ledger(&session, &gil, before.as_ref(), &after);
    let mut out = status_of(root, session.project(), Some(&after))?;
    out["said"] = json!(tr("scope.released"));
    out["next"] = json!(tr("scope.next.released"));
    out["cleanup_confirmed"] = json!(cleanup);
    Ok(out)
}

// ── load 의 승인 연결 ──────────────────────────────────────────────────────

/// P2 이후의 모든 판이 범위 변경 원장의 **정확히 하나의 Monitor 승인 응답**에 묶여 있는가(§5.1). P1 은 시작 준비 기록이 본다.
pub(crate) fn verify_approvals(gil: &Path, tracking: &TrackingState) -> Result<(), String> {
    let later = tracking.ledger().policies().iter().skip(1);
    if later.clone().next().is_none() {
        return Ok(());
    }
    let found = ledger::read(gil)?.ok_or("범위 변경 사건이 있는데 그 제안·응답 기록이 없다")?;
    check_ledger(&found)?;
    for record in later {
        let digest = record.proposal().hex();
        let proposal = found.proposal(&digest).ok_or_else(|| format!("{} 의 제안을 범위 변경 기록에서 찾지 못했다", record.name()))?;
        if proposal.rules_digest != record.rules().hex() || proposal.engine != record.engine().number() {
            return Err(format!("{} 의 원문·엔진이 승인된 제안과 다르다", record.name()));
        }
        let approvals: Vec<&Response> = found.responses_to(&digest).collect();
        if !matches!(approvals.as_slice(), [only] if only.decision == "approve" && only.source == MONITOR) {
            return Err(format!("{} 의 제안에 정확히 하나의 Monitor 승인 응답이 없다", record.name()));
        }
    }
    Ok(())
}

// 이 시험은 합성 Interview 를 맨 `approved: yes` 로 걷는다. 승인 계획 프로필(`approved-plan`)의 Core 는 그 승인을 거절하는 것이 계약이라
// 그 프로필에서는 이 모듈이 컴파일되지 않는다 — 그 프로필에서의 0건은 실행 성공이 아니다.
#[cfg(all(test, not(feature = "approved-plan")))]
mod tests;
