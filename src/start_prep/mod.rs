//! **정식 시작 전 준비 → 정확한 제안 → Monitor 승인 → 적용 직전 재확인 → 최초 보관 → 정식 시작의 한 번 저장**
//! (구현 순서 4번, Artifact Model §3.6 「시작 대화의 범위 승인과 최초 보관」, 공유 계약 §5.1–§5.3).
//!
//! ```text
//! begin     .gil/ 잠금 → 준비 기록(없으면 만든다: 루트 .gilignore 의 최초 관측 = 시작 전 동기화 기준)
//!           연속성 안내 준비(기존 시작과 같다) → 추천·기본 원문·기존 파일 안내. Chain·Cycle·Step·Snapshot 없음
//! prepare   원문 바이트 → 엔진 해석 → 트리 평가(파일 내용 읽지 않음) → 제안 판 n + digest 를 준비 기록에 추가
//! decide    Monitor app-only 버튼(capability 는 MCP 계층 메모리에만) → 응답 기록 → approve 면 apply
//! apply     잠금 안에서: 정식 시작 여부 → 취소·대체 여부 → 승인 응답 → 원문 재해석 → 정책 파일 판정(결합 예외 포함)
//!           → 추적 목록 재확인 → 원문 객체 보관 → 같은 정책으로 최초 capture → 정책 파일 쓰기(§5.2 세 걸음,
//!           시작 전 기준은 준비 기록) → 정책 판 P1·사건 M1·기준점·첫 Chain·첫 Interview Entry 를 한 번에 저장
//! cancel    준비·승인 기록을 보존하고 취소를 남긴다. 자동 재시작 없음
//! ```
//!
//! 이 모듈은 제품 기본 경로가 아니다 — `tracking-policy` 개발 구성의 `gil_start` 만 여기로 온다.

mod ledger;
mod view;

use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::artifact::start_policy as sp;
use crate::artifact::ContentDigest;
use ledger::{Combination, Disk, Event, Excluded, Identity, Ledger, Proposal, Rec, Response};

pub(crate) use ledger::PREP_DIR;

const B64: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;
/// 원문 상한 — 정책 파일은 사람이 읽는 짧은 글이다.
const MAX_RULES: usize = 256 * 1024;

/// 준비 계층의 거절 — 무엇이 막혔는지 코드와 사람의 말.
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

/// 이 판을 다시 적용하면 안 되는 이유 — **저장된 기록만**으로 판정한다(`Ledger::not_reusable`). 프로세스 메모리는 쓰지 않는다:
/// 새 프로세스가 판정할 수 없는 사실로 막지도 풀지도 않는다.
pub(super) fn reuse_blocked(ledger: &Ledger, p: &Proposal) -> Option<&'static str> {
    ledger.not_reusable(p.revision)
}

// ── 입구 ───────────────────────────────────────────────────────────────────

/// 루트를 확인한다 — 절대 경로, 실제 폴더, 연결 파일이 아님. 상위 폴더나 현재 작업 폴더로 추측하지 않는다.
fn root_of(root: &str) -> Result<PathBuf, Refusal> {
    let path = Path::new(root);
    if !path.is_absolute() {
        return Err(refuse("invalid_root", tr("start_prep.invalid_root")));
    }
    let real = path.canonicalize().map_err(|_| refuse("invalid_root", tr("start_prep.invalid_root")))?;
    if !real.is_dir() {
        return Err(refuse("invalid_root", tr("start_prep.invalid_root")));
    }
    Ok(real)
}

fn identity(root: &Path) -> Result<Identity, Refusal> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::symlink_metadata(root).map_err(|_| refuse("invalid_root", tr("start_prep.invalid_root")))?;
    Ok(Identity { root: root.display().to_string(), device: meta.dev(), inode: meta.ino() })
}

fn session_refusal(error: crate::SessionError) -> Refusal {
    match error {
        crate::SessionError::AlreadyStarted { .. } => refuse("already_started", tr("start_prep.already_started")),
        other => refuse("unavailable", other.to_string()),
    }
}

fn gate(root: &Path, create: bool) -> Result<crate::session::StartGate, Refusal> {
    crate::ProjectSession::start_gate(root, create).map_err(session_refusal)
}

/// 준비 기록을 읽고 **모든 칸을 다시 잰다** — 원문 바이트의 주소, 저장된 제안 digest 의 재계산, 응답이 가리키는 판,
/// 사건의 판 번호, 지문의 꼴. 하나라도 어긋나면 그 기록을 쓰지 않는다(손상·바꿔 넣은 원문을 승인된 것으로 읽지 않는다).
///
/// 이 검사는 무결성 서명이 아니다. `.gil` 에 쓸 수 있는 쪽이 제안과 응답을 함께 일관되게 지어내는 것까지 막지는
/// 못한다 — 그 경계는 기록을 정식 시작으로 바꾸는 도구가 App 전용 capability 를 요구한다는 데 있다(`src/mcp/start.rs`).
fn read_checked(gil: &Path) -> Result<Option<Ledger>, String> {
    let Some(ledger) = ledger::read(gil)? else { return Ok(None) };
    check_ledger(&ledger)?;
    Ok(Some(ledger))
}

fn check_ledger(l: &Ledger) -> Result<(), String> {
    let bad = |why: &str| Err(format!("시작 준비 기록의 꼴이 맞지 않는다: {why}"));
    let hex = |d: &str| crate::artifact::sha256_from_hex(d).is_some();
    if let Disk::Present { digest } = &l.base && !hex(digest) { return bad("기준 지문"); }
    if l.next_revision as usize != l.proposals.len() + 1 { return bad("제안 번호"); }
    let restarts = l.epoch();
    for (at, p) in l.proposals.iter().enumerate() {
        if p.revision as usize != at + 1 || p.epoch > restarts { return bad("제안 판·시기"); }
        if sp::EngineVersion::from_number(p.engine).is_none() { return bad("엔진"); }
        let Ok(bytes) = B64.decode(&p.rules) else { return bad("원문") };
        let Ok(address) = crate::artifact::digest_of(&bytes[..]) else { return bad("원문") };
        if address.hex() != p.rules_digest { return bad("원문과 주소가 다르다"); }
        match &p.disk { Disk::Present { digest } if !hex(digest) => return bad("디스크 지문"), Disk::Unconfirmed => return bad("제안의 디스크"), _ => {} }
        if let Disk::Present { digest } = &p.base && !hex(digest) { return bad("제안 때의 기준"); }
        let shown = match (&p.disk, &p.disk_text) {
            (Disk::Absent, None) => true,
            (Disk::Present { digest }, Some(text)) => B64.decode(text).ok()
                .and_then(|b| crate::artifact::digest_of(&b[..]).ok()).is_some_and(|d| d.hex() == *digest),
            _ => false,
        };
        if !shown { return bad("제안 때의 디스크 원문"); }
        if planned_effect(&p.base, &p.disk, &address, p.combination.is_some()) != p.policy_file_effect { return bad("정책 파일 효과"); }
        if let Some(c) = &p.combination
            && (!hex(&c.disk_digest) || !hex(&c.approved_rules_digest) || B64.decode(&c.disk_rules).is_err()) { return bad("결합 출처"); }
        if sp::ProposalDigest::parse(&p.digest).is_none() || proposal_digest(&l.project, p) != p.digest {
            return bad("제안 digest 가 저장된 내용과 다르다");
        }
    }
    let mut answered = std::collections::BTreeSet::new();
    for r in &l.responses {
        let Some(p) = l.proposal(&r.proposal_digest) else { return bad("응답이 없는 제안을 가리킨다") };
        if p.revision != r.revision || !answered.insert(&r.proposal_digest) || r.source != "monitor_button"
            || !matches!(r.decision.as_str(), "approve" | "request_changes") { return bad("응답"); }
    }
    let mut attempts = 0u32;
    let mut open = std::collections::BTreeSet::new();
    for e in &l.events {
        let at = match e { Event::Cancel { after_revision } | Event::Restart { after_revision } => *after_revision,
            Event::ConditionChanged { revision, .. } => *revision,
            Event::CombineAnnounced { approved_revision, .. } => *approved_revision,
            Event::SyncStarted { revision, .. } | Event::SyncSettled { revision, .. } => *revision,
            Event::AttemptStarted { revision, attempt, proposal_digest } => {
                attempts += 1;
                // 시도는 하나씩 늘고, 그 판의 정확한 제안을 가리킨다.
                if *attempt != attempts || l.proposals.get((*revision as usize).wrapping_sub(1)).is_none_or(|p| p.digest != *proposal_digest) {
                    return bad("적용 시도");
                }
                open.insert((*revision, *attempt));
                *revision
            }
            Event::AttemptSettled { revision, attempt, outcome, .. } => {
                // 결과는 시작한 시도에 한 번만 붙는다.
                if !open.remove(&(*revision, *attempt)) || !ledger::ATTEMPT_OUTCOMES.contains(&outcome.as_str()) { return bad("적용 시도의 결과"); }
                *revision
            }
        };
        if at >= l.next_revision { return bad("사건의 판"); }
    }
    Ok(())
}

fn load_ledger(gate: &crate::session::StartGate, root: &Path) -> Result<Option<Ledger>, Refusal> {
    let ledger = read_checked(&gate.gil).map_err(|e| refuse("preparation_unreadable", e))?;
    if let Some(ledger) = &ledger
        && ledger.project != identity(root)? {
        return Err(refuse("preparation_other_folder", tr("start_prep.other_folder")));
    }
    Ok(ledger)
}

fn save(gate: &crate::session::StartGate, before: Option<&Ledger>, after: &Ledger) -> Result<(), Refusal> {
    ledger::write(gate.lock(), &gate.gil, before, after).map_err(|e| refuse("preparation_not_saved", e))
}

/// 루트 `.gilignore` 의 지금 관측(바이트 포함). 읽지 못하면 그 까닭.
fn disk_now(root: &Path) -> Result<Option<(ContentDigest, Vec<u8>)>, String> {
    sp::read_root_policy_source(root)
}

fn disk_value(seen: &Option<(ContentDigest, Vec<u8>)>) -> Disk {
    match seen {
        Some((digest, _)) => Disk::Present { digest: digest.hex() },
        None => Disk::Absent,
    }
}

fn sync_base(disk: &Disk) -> sp::SyncBase {
    match disk {
        Disk::Absent => sp::SyncBase::Absent,
        Disk::Present { digest } => sp::SyncBase::Present(crate::artifact::sha256_from_hex(digest).expect("저장한 지문")),
        Disk::Unconfirmed => sp::SyncBase::Unconfirmed,
    }
}

fn reading(seen: &Result<Option<(ContentDigest, Vec<u8>)>, String>) -> sp::DiskReading {
    match seen {
        Ok(Some((digest, _))) => sp::DiskReading::Present(digest.clone()),
        Ok(None) => sp::DiskReading::Absent,
        Err(reason) => sp::DiskReading::Unreadable { reason: reason.clone() },
    }
}

/// 루트 `.gitignore` — 추천 참고 자료일 뿐 적용 정책 입력이 아니다. 연결 파일을 따라가지 않고 읽는다.
fn gitignore_text(root: &Path) -> Option<String> {
    use std::io::Read;
    let dir = crate::internal_fs::dir_nofollow(root).ok()?;
    let file = crate::internal_fs::regular_at(&dir, c".gitignore").ok()?;
    let mut bytes = Vec::new();
    file.take(MAX_RULES as u64).read_to_end(&mut bytes).ok()?;
    String::from_utf8(bytes).ok()
}

// ── begin ──────────────────────────────────────────────────────────────────

/// This build also holds the image-report creation: a `.gil` without a state file and without a preparation
/// may be the other kind's interrupted creation.
const OTHER_START: bool = cfg!(feature = "visual-authoring");

/// Asked under the lock, before recovery or reaping: in a build with two kinds of new project, a new
/// preparation begins only where nothing was recorded yet — an empty `.gil` or one holding the lock file
/// alone. A started project and an existing preparation are answered by `begin` itself. Everything else
/// (objects, a restore area, an unfinished state file) is left exactly as it is: the call that routed here
/// saw an empty folder, and something appeared since. A build with this start alone keeps its bootstrap rule.
fn no_other_start(gil: &Path) -> Result<(), crate::SessionError> {
    if !OTHER_START { return Ok(()); }
    let seen = |name: &str| std::fs::symlink_metadata(gil.join(name)).is_ok();
    if seen("state.yaml") || seen(PREP_DIR) { return Ok(()); }
    let unreadable = |source: std::io::Error| crate::SessionError::Unreadable { path: gil.display().to_string(), source: source.to_string() };
    for entry in std::fs::read_dir(gil).map_err(unreadable)? {
        let name = entry.map_err(unreadable)?.file_name();
        if name != crate::lock::LOCK_FILE {
            return Err(crate::SessionError::NotEmpty { path: gil.display().to_string(), found: name.to_string_lossy().into_owned() });
        }
    }
    Ok(())
}

/// `gil_start`(tracking-policy 구성) — 준비 기록을 만들거나 이어 간다. 정식 Chain·Cycle·Step·Snapshot 을 만들지 않는다.
pub(crate) fn begin(root: &str) -> Result<Value, Refusal> {
    let root = root_of(root)?;
    let gate = crate::ProjectSession::start_gate_with(&root, true, no_other_start).map_err(|error| match error {
        crate::SessionError::NotEmpty { .. } if OTHER_START => refuse("start_record_unsettled", tr("start_kind.unsettled")),
        other => session_refusal(other),
    })?;
    if gate.started() {
        return Err(refuse("already_started", tr("start_prep.already_started")));
    }
    let before = load_ledger(&gate, &root)?;
    let ledger = match &before {
        Some(ledger) => ledger.clone(),
        None => {
            // 기존 시작과 같은 상태 검사 — 모르는 `.gil` 항목·옛 형식은 건드리지 않고 거절한다.
            gate.initializable().map_err(session_refusal)?;
            let base = match disk_now(&root) {
                Ok(seen) => disk_value(&seen),
                Err(reason) => return Err(refuse("policy_file_unreadable", format!("{}: {reason}", tr("start_prep.policy_file_unreadable")))),
            };
            let ledger = Ledger::new(identity(&root)?, base);
            save(&gate, None, &ledger)?;
            ledger
        }
    };
    // 연속성 안내 블록은 기존 시작과 같이 최초 보관 **전**에 준비한다. 제안 전에 해 두어 추적 목록이 흔들리지 않게 한다.
    let resume = if before.is_none() { Some(crate::project_resume::ensure(&root)) } else { None };
    let mut out = view::status(&root, &ledger, None);
    out["recommendations"] = view::recommendations(&recommendations(&root, &[]), "");
    out["default_rules"] = json!(default_rules(&root));
    out["system_protections"] = view::system_protections();
    if let Some(resume) = resume { out["continuity"] = json!(resume); }
    Ok(out)
}

fn recommendations(root: &Path, agent: &[AgentRec]) -> Vec<sp::Recommendation> {
    let mut list = match gitignore_text(root) {
        Some(text) => sp::review_gitignore(&text).recommendations,
        None => sp::catalog(),
    };
    for rec in agent {
        if let Ok(made) = sp::project_specific(&rec.key, rec.rules.clone(), rec.why.clone()) {
            list.push(made);
        }
    }
    list
}

/// 처음 보여 줄 원문 — 기존 `.gilignore` 가 해석되면 그 원문, 아니면 미리 선택된 일반·비밀값 추천만.
fn default_rules(root: &Path) -> String {
    if let Ok(Some((_, bytes))) = disk_now(root)
        && sp::ApprovedPolicy::compile(sp::EngineVersion::V1, &bytes).is_ok()
        && let Ok(text) = String::from_utf8(bytes) {
        return text;
    }
    let mut text = String::from("# GIL tracking exclusions. Only the exact text approved in the GIL Monitor is applied.\n");
    for rec in sp::catalog().into_iter().filter(|r| r.preselected) {
        text.push_str(&format!("# {}\n", rec.key));
        for rule in rec.rules { text.push_str(&rule); text.push('\n'); }
    }
    text
}

// ── prepare ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(crate) struct AgentRec {
    pub(crate) key: String,
    pub(crate) rules: Vec<String>,
    pub(crate) why: String,
}

#[derive(Debug, Clone)]
pub(crate) struct PrepareInput {
    pub(crate) rules: String,
    pub(crate) project_recommendations: Vec<AgentRec>,
    /// 좁은 결합 예외의 결합안이면 그 앞 승인안의 판.
    pub(crate) combine_with_revision: Option<u32>,
    /// 취소한 준비를 명시적으로 다시 시작한다.
    pub(crate) restart_canceled: bool,
}

/// 원문 하나를 제안 판으로 준비한다. 승인·적용·파일 쓰기를 하지 않는다.
pub(crate) fn prepare(root: &str, input: PrepareInput) -> Result<Value, Refusal> {
    let root = root_of(root)?;
    let gate = gate(&root, false)?;
    if gate.started() {
        return Err(refuse("already_started", tr("start_prep.already_started")));
    }
    let before = load_ledger(&gate, &root)?.ok_or_else(|| refuse("not_preparing", tr("start_prep.not_preparing")))?;
    let mut ledger = before.clone();
    if ledger.canceled() {
        if !input.restart_canceled {
            return Err(refuse("canceled", tr("start_prep.canceled")));
        }
        ledger.events.push(Event::Restart { after_revision: ledger.next_revision.saturating_sub(1) });
    } else if input.restart_canceled {
        return Err(refuse("not_canceled", tr("start_prep.not_canceled")));
    }
    if input.rules.len() > MAX_RULES {
        return Err(refuse("rules_too_large", tr("start_prep.rules_too_large")));
    }
    let bytes = input.rules.as_bytes().to_vec();
    let policy = sp::ApprovedPolicy::compile(sp::EngineVersion::V1, &bytes).map_err(|rejected| refuse_with(
        "rules_rejected", tr("start_prep.rules_rejected"),
        json!(rejected.problems.iter().map(|p| json!({"line": p.line, "text": p.text, "reason": p.reason.to_string()})).collect::<Vec<_>>()),
    ))?;
    let evaluation = crate::artifact::evaluate_scope(&root, &policy)
        .map_err(|e| refuse("scope_unobservable", e.to_string()))?;
    let seen = disk_now(&root).map_err(|reason| refuse("policy_file_unreadable", format!("{}: {reason}", tr("start_prep.policy_file_unreadable"))))?;
    let disk = disk_value(&seen);

    let combination = match input.combine_with_revision {
        None => None,
        Some(revision) => Some(combination(&ledger, &root, revision, &seen, &policy)?),
    };
    let recs = recommendations(&root, &input.project_recommendations);
    let disk_text = seen.as_ref().map(|(_, b)| B64.encode(b));
    let proposal = build_proposal(&ledger, &bytes, &policy, &evaluation, disk, disk_text, &recs, &input.rules, combination)?;
    ledger.next_revision += 1;
    ledger.proposals.push(proposal.clone());
    // 적용의 작은 사건 저장이 상한에 걸리지 않도록 여유를 남긴다.
    if serde_json::to_vec_pretty(&ledger).map_or(true, |b| b.len() as u64 > ledger::MAX_BYTES - 1024 * 1024) {
        return Err(refuse("preparation_too_large", tr("start_prep.preparation_too_large")));
    }
    save(&gate, Some(&before), &ledger)?;
    Ok(view::proposal(&root, &ledger, &proposal))
}

fn combination(ledger: &Ledger, root: &Path, revision: u32, seen: &Option<(ContentDigest, Vec<u8>)>, d: &sp::ApprovedPolicy) -> Result<Combination, Refusal> {
    let not = |said: String| refuse("combination_not_allowed", said);
    let approved = ledger.proposals.iter().find(|p| p.revision == revision && p.epoch == ledger.epoch())
        .ok_or_else(|| not(tr("start_prep.combination_no_approved")))?;
    if ledger.response(&approved.digest).is_none_or(|r| r.decision != "approve")
        || ledger.latest().map(|p| &p.digest) != Some(&approved.digest) {
        return Err(not(tr("start_prep.combination_no_approved")));
    }
    if ledger.base != Disk::Absent || approved.disk != Disk::Absent || approved.combination.is_some() {
        return Err(not(tr("start_prep.combination_not_absent")));
    }
    let Some((c_digest, c_bytes)) = seen else { return Err(not(tr("start_prep.combination_no_disk"))) };
    if c_digest.hex() == approved.rules_digest {
        return Err(not(tr("start_prep.combination_disk_is_approved")));
    }
    // 결합안은 적용이 그 C 를 보고 안내(combine_required)를 남긴 뒤에만 받는다.
    if !ledger.events.iter().any(|e| matches!(e, Event::CombineAnnounced { approved_revision, disk_digest }
        if *approved_revision == approved.revision && *disk_digest == c_digest.hex())) {
        return Err(not(tr("start_prep.combination_not_announced")));
    }
    // D 가 우연히 B 나 C 와 바이트가 같아도 거절하지 않는다 — 결합안은 여전히 그 자체의 판이며 별도 Monitor 승인을 받는다.
    // 바이트가 같다는 이유로 B 의 승인을 다시 쓰거나 자동 승인하지 않는다.
    let _ = d;
    // C 의 실제 효과도 D 를 준비하는 지금 한 번 평가해 저장한다 — 화면은 이 저장값만 보여 준다.
    let (disk_tracked, disk_problems) = match sp::ApprovedPolicy::compile(sp::EngineVersion::V1, c_bytes) {
        Ok(c_policy) => match crate::artifact::evaluate_scope(root, &c_policy) {
            Ok(evaluation) => (Some(evaluation.tracked().iter().map(|f| f.path.as_str().to_owned()).collect()), Vec::new()),
            Err(e) => (None, vec![e.to_string()]),
        },
        Err(rejected) => (None, rejected.problems.iter().map(|p| format!("line {}: {}", p.line, p.reason)).collect()),
    };
    Ok(Combination {
        approved_revision: approved.revision,
        approved_digest: approved.digest.clone(),
        approved_rules_digest: approved.rules_digest.clone(),
        disk_digest: c_digest.hex(),
        disk_rules: B64.encode(c_bytes),
        disk_tracked,
        disk_problems,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_proposal(
    ledger: &Ledger, bytes: &[u8], policy: &sp::ApprovedPolicy, evaluation: &sp::TreeEvaluation,
    disk: Disk, disk_text: Option<String>, recs: &[sp::Recommendation], text: &str, combination: Option<Combination>,
) -> Result<Proposal, Refusal> {
    let tracked: Vec<String> = evaluation.tracked().iter().map(|f| f.path.as_str().to_owned()).collect();
    let excluded: Vec<Excluded> = evaluation.excluded().iter().map(|e| Excluded {
        path: e.path.as_str().to_owned(),
        kind: e.kind.code().to_owned(),
        reason: match &e.reason {
            sp::ExclusionReason::Rule(rule) => format!("line {}: {}", rule.line, rule.pattern),
            sp::ExclusionReason::System(protection) => format!("system: {}", protection.code()),
        },
        interior: match &e.interior {
            Some(sp::Interior::Skip) => "not_explored".into(),
            Some(sp::Interior::Explore { .. }) => "explored_for_reinclusion".into(),
            None => String::new(),
        },
    }).collect();
    let lines: std::collections::BTreeSet<&str> = text.lines().collect();
    let recommendations: Vec<Rec> = recs.iter().map(|r| Rec {
        key: r.key.clone(),
        category: r.category.code().to_owned(),
        sources: r.sources.iter().map(|s| match s {
            sp::RecommendationSource::GilCatalog => "gil_catalog".to_owned(),
            sp::RecommendationSource::Gitignore { line } => format!("gitignore:{line}"),
            sp::RecommendationSource::Agent => "agent".to_owned(),
        }).collect(),
        rules: r.rules.clone(),
        default_selected: r.preselected,
        in_rules: !r.rules.is_empty() && r.rules.iter().all(|rule| lines.contains(rule.as_str())),
        reason: r.reason.to_string(),
        impacts: r.impacts.iter().map(ToString::to_string).collect(),
    }).collect();
    let mut proposal = Proposal {
        revision: ledger.next_revision,
        digest: String::new(),
        epoch: ledger.epoch(),
        engine: sp::EngineVersion::V1.number(),
        rules: B64.encode(bytes),
        rules_digest: policy.identity().rules_digest().hex(),
        policy_file_effect: planned_effect(&ledger.base, &disk, policy.identity().rules_digest(), combination.is_some()).into(),
        base: ledger.base.clone(),
        disk,
        disk_text,
        tracked,
        excluded,
        blockers: evaluation.blockers().iter().map(ToString::to_string).collect(),
        notices: evaluation.notices().iter().map(ToString::to_string).collect(),
        recommendations,
        combination,
    };
    proposal.digest = proposal_digest(&ledger.project, &proposal);
    Ok(proposal)
}

/// 이 판을 승인할 수 있는가 — 차단 사유(하위 `.gilignore`)가 없고, 정책 파일 효과가 보류가 아닐 때만. 보류로 정해진 판을
/// 승인받아 두었다가 나중에 쓰기 권한으로 바꾸는 길을 만들지 않는다.
pub(super) fn approvable(p: &Proposal) -> bool {
    p.blockers.is_empty() && !p.policy_file_effect.starts_with("held_")
}

/// 이 판이 승인되면 정책 파일에 일어날 일 — 제안 때의 기준·디스크·원문만으로 정한다(적용 때의 판정 [`policy_file_plan`] 과 같은
/// 규칙이며, 적용은 그때의 실제 디스크로 다시 판정한다). 표시를 위해 쓰기 권한을 넓히지 않는다.
fn planned_effect(base: &Disk, disk: &Disk, rules: &ContentDigest, combination: bool) -> &'static str {
    let reading = match disk {
        Disk::Absent => sp::DiskReading::Absent,
        Disk::Present { digest } => sp::DiskReading::Present(crate::artifact::sha256_from_hex(digest).expect("잰 지문")),
        Disk::Unconfirmed => return "held_base_undecided",
    };
    if matches!(&reading, sp::DiskReading::Present(found) if found == rules) {
        return "unchanged";
    }
    if combination {
        return "replace_shown_disk";
    }
    match sp::base_for_new_policy(rules, &sync_base(base), &reading) {
        Ok(sp::NewPolicyBase::Chosen(sp::SyncBase::Unconfirmed)) => "not_written_unconfirmed",
        Ok(sp::NewPolicyBase::Chosen(_)) if matches!(reading, sp::DiskReading::Absent) => "create",
        Ok(sp::NewPolicyBase::Chosen(_)) => "replace",
        Ok(sp::NewPolicyBase::Held(_)) => "held_three_way",
        Err(_) => "held_base_undecided",
    }
}

/// 제안 digest — Monitor 가 보여 준 판의 의미 있는 내용 전체(프로젝트·판·원문 주소·엔진·추적/제외 효과·차단·
/// 추천과 선택·디스크·결합 출처)를 묶는다. 같은 원문이라도 다른 폴더·판·목록이면 다른 digest 다.
fn proposal_digest(project: &Identity, p: &Proposal) -> String {
    let canonical = json!({
        "project": project, "revision": p.revision, "epoch": p.epoch, "engine": p.engine, "rules_digest": p.rules_digest,
        "disk": p.disk, "base": p.base, "disk_text": p.disk_text, "policy_file_effect": p.policy_file_effect, "tracked": p.tracked, "excluded": p.excluded, "blockers": p.blockers, "notices": p.notices,
        "recommendations": p.recommendations, "combination": p.combination,
    });
    let mut hash = Sha256::new();
    hash.update(b"gil-start-proposal-v1\0");
    hash.update(serde_json::to_vec(&canonical).expect("JSON 값"));
    format!("{:x}", hash.finalize())
}

fn rules_bytes(p: &Proposal) -> Vec<u8> {
    B64.decode(&p.rules).expect("저장한 원문")
}

// ── 조회 ───────────────────────────────────────────────────────────────────

/// 저장 상태를 조회한다 — 잠그지 않고, 만들지 않고, 쓰지 않는다. 재연결·응답 유실 뒤의 같은 결과 확인에 쓴다.
pub(crate) fn status(root: &str) -> Result<Value, Refusal> {
    let root = root_of(root)?;
    let gil = root.join(crate::artifact::GIL_DIR);
    let started = started_record(&root)?;
    let ledger = read_checked(&gil).map_err(|e| refuse("preparation_unreadable", e))?;
    match ledger {
        None if started.is_none() => Ok(json!({"phase": "not_preparing", "said": tr("start_prep.phase.not_preparing")})),
        None => Err(refuse("preparation_missing", tr("start_prep.preparation_missing"))),
        Some(ledger) => {
            if ledger.project != identity(&root)? {
                return Err(refuse("preparation_other_folder", tr("start_prep.other_folder")));
            }
            Ok(view::status(&root, &ledger, started.as_ref()))
        }
    }
}

/// Monitor 의 준비 화면이 받을 수 있는 capability 의 쓰임 — 버튼 하나에 하나.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Purpose {
    /// 승인 대기 중인 최신 제안의 승인/수정 요청.
    Decide,
    /// Monitor 로 이미 승인된 최신 제안의 적용 재시도(보류·응답 유실 뒤). 새 승인이 아니다.
    Apply,
}

/// Monitor 의 준비 화면이 읽는 값 — 최신 제안 하나와, 버튼을 누를 수 있는 판이면 그 쓰임.
pub(crate) fn review(root: &Path) -> Result<(Value, Option<(u32, String, Purpose)>), Refusal> {
    let gil = root.join(crate::artifact::GIL_DIR);
    let started = started_record(root)?;
    let ledger = read_checked(&gil).map_err(|e| refuse("preparation_unreadable", e))?
        .ok_or_else(|| refuse("not_preparing", tr("start_prep.not_preparing")))?;
    if ledger.project != identity(root)? {
        return Err(refuse("preparation_other_folder", tr("start_prep.other_folder")));
    }
    let mut value = view::status(root, &ledger, started.as_ref());
    let mut grant = None;
    if let Some(p) = ledger.latest() {
        let shown = view::proposal(root, &ledger, p);
        value["proposal"] = shown["proposal"].clone();
        value["current"] = shown["current"].clone();
        let open = started.is_none() && !ledger.canceled();
        let blocked = reuse_blocked(&ledger, p).is_some();
        match ledger.response(&p.digest).map(|r| r.decision.as_str()) {
            None if open && approvable(p) => grant = Some((p.revision, p.digest.clone(), Purpose::Decide)),
            Some("approve") if open && !blocked => grant = Some((p.revision, p.digest.clone(), Purpose::Apply)),
            _ => {}
        }
    }
    Ok((value, grant))
}

/// Monitor 가 줄여 보인 목록의 전체를 **같은 제안 판**(판 번호 + digest)에서 쪽 단위로 읽는다. 읽기만 한다 — 승인·적용·
/// capability 를 만들지 않는다. 그 판이 이제 최신이 아니면 그 사실도 함께 알린다.
pub(crate) fn list(root: &Path, revision: u32, digest: &str, which: &str, offset: usize) -> Result<Value, Refusal> {
    let ledger = read_checked(&root.join(crate::artifact::GIL_DIR)).map_err(|e| refuse("preparation_unreadable", e))?
        .ok_or_else(|| refuse("not_preparing", tr("start_prep.not_preparing")))?;
    if ledger.project != identity(root)? {
        return Err(refuse("preparation_other_folder", tr("start_prep.other_folder")));
    }
    let p = ledger.proposal(digest).filter(|p| p.revision == revision)
        .ok_or_else(|| refuse("unknown_proposal", tr("start_prep.unknown_proposal")))?;
    let mut page = view::list_page(p, which, offset).ok_or_else(|| refuse("unknown_list", tr("start_tool.bad_input")))?;
    page["latest"] = json!(ledger.latest().is_some_and(|l| l.digest == p.digest));
    Ok(page)
}

/// 정식 시작이 확정됐다면 그 기록(P1 의 proposal_digest·M1·A1·C1). 상태 파일이 없으면 `None`.
pub(crate) struct StartedRecord {
    pub(crate) proposal_digest: String,
    pub(crate) snapshot: String,
    pub(crate) chain: String,
    pub(crate) sync: String,
}

/// The lock holder saw a state file and it is gone at the unlocked read that follows. Answered as a held
/// request, never as a panic: nothing was written, and the caller reads the stored state again.
fn state_gone() -> Refusal {
    refuse("state_unreadable", tr("start_tool.uncertain"))
}

fn started_record(root: &Path) -> Result<Option<StartedRecord>, Refusal> {
    let state = root.join(crate::STATE_PATH);
    if !state.exists() {
        return Ok(None);
    }
    let session = crate::ProjectSession::open_read_only(crate::RuleSet::builtin().map_err(|e| refuse("unavailable", e.to_string()))?, &state)
        .map_err(|e| refuse("state_unreadable", e.to_string()))?;
    let Some(tracking) = session.project().tracking() else {
        return Err(refuse("not_tracking_project", tr("start_prep.not_tracking_project")));
    };
    Ok(Some(StartedRecord {
        proposal_digest: tracking.applied().proposal().hex(),
        snapshot: tracking.start().snapshot.to_string(),
        chain: tracking.start().chain.to_string(),
        sync: view::sync_word(tracking.sync_base()).into(),
    }))
}

// ── decide / apply / cancel ────────────────────────────────────────────────

/// Monitor 버튼의 응답을 기록한다. **capability 확인은 부르는 MCP App 계층이 이미 했다** — 이 함수는 모델·CLI
/// 경로에 노출하지 않는다. 같은 응답의 재전송은 같은 결과, 다른 응답은 거절. approve 면 곧바로 적용을 시도한다.
pub(crate) fn decide(root: &Path, revision: u32, digest: &str, approve: bool) -> Result<Value, Refusal> {
    let decision = if approve { "approve" } else { "request_changes" };
    {
        let gate = gate(root, false)?;
        let before = load_ledger(&gate, root)?.ok_or_else(|| refuse("not_preparing", tr("start_prep.not_preparing")))?;
        if let Some(existing) = before.response(digest) {
            if existing.revision != revision {
                return Err(refuse("stale_proposal", tr("start_prep.stale_proposal")));
            }
            if existing.decision != decision {
                return Err(refuse("response_final", tr("start_prep.response_final")));
            }
        } else {
            if gate.started() {
                return Err(refuse("already_started", tr("start_prep.already_started")));
            }
            if before.canceled() {
                return Err(refuse("canceled", tr("start_prep.canceled")));
            }
            let latest = before.latest().ok_or_else(|| refuse("stale_proposal", tr("start_prep.stale_proposal")))?;
            if latest.revision != revision || latest.digest != digest {
                return Err(refuse("stale_proposal", tr("start_prep.stale_proposal")));
            }
            if approve && !approvable(latest) {
                return Err(refuse("blocked", tr(if latest.blockers.is_empty() { "start_prep.policy_file_held" } else { "start_prep.blocked" })));
            }
            let mut after = before.clone();
            after.responses.push(Response { revision, proposal_digest: digest.into(), decision: decision.into(), source: "monitor_button".into() });
            save(&gate, Some(&before), &after)?;
        }
    }
    if !approve {
        return status(&root.display().to_string());
    }
    // 승인 응답은 이미 저장됐다. 적용이 보류돼도 그 사실과 보류 이유를 함께 돌려준다 — 승인 기록 저장 ≠ 정식 시작.
    match apply_path(root, digest) {
        Ok(started) => Ok(started),
        Err(held) => {
            let mut out = status(&root.display().to_string())?;
            out["application_held"] = json!({"code": held.code, "said": held.said, "detail": held.detail});
            Ok(out)
        }
    }
}

/// 기록된 Monitor 승인의 적용을 다시 시도한다 — **App 전용 `gil_start_apply`** 가 그 판의 capability 를 확인한 뒤에만
/// 부른다. 승인을 만들지 않으며, 판·digest 가 최신 승인 제안과 다르면 거절한다. 모델 도구에는 이 문이 없다.
pub(crate) fn apply(root: &Path, revision: u32, digest: &str) -> Result<Value, Refusal> {
    let ledger = read_checked(&root.join(crate::artifact::GIL_DIR)).map_err(|e| refuse("preparation_unreadable", e))?
        .ok_or_else(|| refuse("not_preparing", tr("start_prep.not_preparing")))?;
    if ledger.proposal(digest).is_some_and(|p| p.revision != revision) {
        return Err(refuse("stale_proposal", tr("start_prep.stale_proposal")));
    }
    apply_path(root, digest)
}

fn apply_path(root: &Path, digest: &str) -> Result<Value, Refusal> {
    let gate = gate(root, false)?;
    // ① 이미 정식 시작이 확정됐는가 — 같은 승인이면 같은 결과를 조회해 돌려준다. 새 판·사건·Chain 을 만들지 않는다.
    if gate.started() {
        drop(gate);
        let started = started_record(root)?.ok_or_else(state_gone)?;
        if started.proposal_digest == digest {
            return Ok(view::started(root, &started, true));
        }
        return Err(refuse("started_with_other", tr("start_prep.started_with_other")));
    }
    let before = load_ledger(&gate, root)?.ok_or_else(|| refuse("not_preparing", tr("start_prep.not_preparing")))?;
    // ② 취소·대체·승인 여부. 앞선 판의 승인은 새 판에 쓰지 않는다.
    if before.canceled() {
        return Err(refuse("canceled", tr("start_prep.canceled")));
    }
    let proposal = before.proposal(digest).cloned().ok_or_else(|| refuse("unknown_proposal", tr("start_prep.unknown_proposal")))?;
    if before.latest().map(|p| &p.digest) != Some(&proposal.digest) {
        return Err(refuse("superseded", tr("start_prep.superseded")));
    }
    if before.response(digest).is_none_or(|r| r.decision != "approve" || r.source != "monitor_button") {
        return Err(refuse("not_approved", tr("start_prep.not_approved")));
    }
    gate.initializable().map_err(session_refusal)?;
    // ③ 승인된 바이트를 그대로 다시 해석한다 — 표시용·저장용 정책을 따로 만들지 않는다.
    let bytes = rules_bytes(&proposal);
    let policy = sp::ApprovedPolicy::compile(sp::EngineVersion::from_number(proposal.engine).expect("저장한 엔진"), &bytes)
        .map_err(|e| refuse("rules_rejected", e.to_string()))?;
    let rules = policy.identity().rules_digest().clone();
    if rules.hex() != proposal.rules_digest {
        return Err(refuse("preparation_unreadable", "원문과 주소가 다르다"));
    }
    // 저장된 사실만으로 이 판의 재사용을 판정한다: 확인된 조건 변경, 결과가 기록되지 않은 쓰기, 결과가 기록되지 않은 적용 시도.
    //    프로세스 메모리는 이 판정에 쓰지 않는다 — 모르는 상태를 재사용 허가로 읽지 않는다.
    if let Some(code) = reuse_blocked(&before, &proposal) {
        return Err(refuse(code, tr(&format!("start_prep.{code}"))));
    }
    // 적용 시도를 **조건을 관측하기 전에** 원장에 남긴다. 저장을 확인하지 못하면 관측·적용으로 나아가지 않는다. 이 뒤의 어떤
    //    중단·결과 저장 실패도, 새 프로세스가 이 기록을 보고 결과가 없는 시도로 보류한다.
    let attempt = before.next_attempt();
    let mut opened = before.clone();
    opened.events.push(Event::AttemptStarted { revision: proposal.revision, attempt, proposal_digest: proposal.digest.clone() });
    if let Err(e) = attempt_start_save(&gate, &before, &opened) {
        return Err(refuse_with("attempt_not_recorded", tr("start_prep.attempt_not_recorded"), json!(e.detail)));
    }
    let project = match observe_and_sync(&gate, root, &opened, &proposal, &policy, &bytes, &rules, attempt) {
        Ok(ready) => ready,
        Err(End::Final(held)) => return Err(held),
        Err(End::Stop(held)) => return Err(settle(&gate, root, &proposal, attempt, held)),
    };
    let (project, file) = project;
    // ⑨ 정책 판 P1·사건 M1·기준점·첫 Chain·첫 Interview Entry — 한 번의 상태 저장.
    let session = gate.into_session(project);
    let committed = commit(&session);
    drop(session);
    // ⑩ 저장 결과를 다시 읽어(같은 load 검사 전체) 말한다. 저장이 오류를 돌려줬어도 결과가 불확실할 수 있으므로 조회로 가린다:
    //    확정돼 있으면 시작이고(응답만 불확실했다), 상태 파일이 없으면 시작하지 않았고(그 시도는 멈춤으로 확정한다), 읽을 수
    //    없으면 불확실로 보고하고 시도를 미해결로 둔다 — 다음 조회가 상태 파일로 가린다.
    let started = match (started_record(root), committed) {
        (Ok(Some(started)), _) => started,
        (Ok(None), Err(e)) => {
            let held = refuse_with("start_not_saved", tr("start_prep.start_not_saved"), json!(e));
            return Err(match crate::ProjectSession::start_gate(root, false) {
                Ok(again) if !again.started() => settle(&again, root, &proposal, attempt, held),
                _ => unresolved(held),
            });
        }
        (Ok(None), Ok(())) | (Err(_), _) => return Err(refuse("start_uncertain", tr("start_prep.start_uncertain"))),
    };
    let mut out = view::started(root, &started, false);
    out["policy_file"] = json!(file);
    out["facts"]["policy_file_write"] = json!(file);
    Ok(out)
}

/// 적용 시도가 정식 시작 전에 멈춘 까닭.
enum End {
    /// 안전하게 멈췄다(읽기 불가·관측 불안정·확인된 저장 장애·기존 보류) — 그 결과를 `AttemptSettled(stopped)` 로 확정해야 한다.
    Stop(Refusal),
    /// 결과 기록까지 이미 끝났거나(확인된 조건 변경), 결과를 기록하지 못해 시도를 미해결로 남긴다 — 더 기록하지 않는다.
    Final(Refusal),
}

impl From<Refusal> for End {
    fn from(refusal: Refusal) -> End {
        End::Stop(refusal)
    }
}

/// 적용 시도의 관측과 정책 파일 동기화 — 같은 잠금 안이다. 정식 시작에 쓸 Project 와 파일 쓰기 결과를 돌려준다.
#[allow(clippy::too_many_arguments)]
fn observe_and_sync(
    gate: &crate::session::StartGate, root: &Path, opened: &Ledger, proposal: &Proposal, policy: &sp::ApprovedPolicy,
    bytes: &[u8], rules: &ContentDigest, attempt: u32,
) -> Result<(crate::Project, &'static str), End> {
    let condition = |what: &str, held: Refusal| End::Final(condition(gate, root, proposal, attempt, what, held));
    let mut ledger = opened.clone();
    // ④ 정책 파일 — 잠금 안에서 다시 관측한다. 이 관측은 쓰기 허가가 아니다: 아래 판정이 허가를 정한다.
    let seen = disk_now(root);
    let plan = policy_file_plan(&mut ledger, proposal, rules, &seen);
    if ledger != *opened {
        save(gate, Some(opened), &ledger)?; // 결합 안내 기록
        return Err(End::Stop(plan.expect_err("안내를 기록한 판정은 보류다")));
    }
    let plan = match plan {
        Ok(plan) => plan,
        Err(held) if held.code == "policy_file_changed" => return Err(condition("policy_file", held)),
        Err(held) => return Err(End::Stop(held)),
    };
    // ⑤ 추적 목록·경로 재확인 — 규칙·목록·경로 조건이 바뀌면 이 판으로 시작하지 않는다(새 제안·재승인). 관측이 흔들렸을 뿐이면
    //    (동시 변경·읽기 실패) 조건 변경이 아니므로 승인은 그대로이고 다시 시도할 수 있다. 내용만 바뀐 것은 아래 안정 관측이 본다.
    let evaluation = match crate::artifact::evaluate_scope(root, policy) {
        Ok(evaluation) => evaluation,
        Err(e) if is_condition(&e) => return Err(condition("path",
            refuse_with("condition_changed", tr("start_prep.condition_changed"), json!({"what": "path", "said": e.to_string()})))),
        Err(e) => return Err(End::Stop(refuse_with("observation_unstable", tr("start_prep.observation_unstable"), json!(e.to_string())))),
    };
    if !evaluation.blockers().is_empty() {
        let held = refuse_with("blocked", tr("start_prep.blocked"), json!(evaluation.blockers().iter().map(ToString::to_string).collect::<Vec<_>>()));
        return Err(condition("nested_policy_file", held));
    }
    let now: Vec<String> = evaluation.tracked().iter().map(|f| f.path.as_str().to_owned()).collect();
    if now != proposal.tracked {
        return Err(condition("tracked_list", scope_changed(&proposal.tracked, &now)));
    }
    // ⑥ 원문 객체를 상태보다 먼저 확정한다. 실패는 저장 장애이며 승인은 그대로다.
    sp::store_policy_source(&gate.store, policy.identity().engine(), rules, bytes)
        .map_err(|e| refuse("policy_source_not_stored", format!("{e:?}")))?;
    // ⑦ 같은 정책으로 최초 세계를 두 번 관측·보관한다. 실패하면 정식 시작을 만들지 않고 정책 파일도 건드리지 않는다.
    //    루트 `.gilignore` 는 시스템 보호라 보관 세계에 들어가지 않으므로, 파일 쓰기를 뒤에 두어도 보관 내용은 같다.
    let world = match capture(root, policy, &gate.store) {
        Ok(world) => world,
        Err(e) if is_condition(&e) => return Err(condition("path",
            refuse_with("condition_changed", tr("start_prep.condition_changed"), json!({"what": "path", "said": e.to_string()})))),
        Err(e) => return Err(End::Stop(refuse_with("capture_failed", tr("start_prep.capture_failed"), json!(e.to_string())))),
    };
    if world.tracked_paths() != proposal.tracked {
        return Err(condition("tracked_list", scope_changed(&proposal.tracked, &world.tracked_paths())));
    }
    let address = gate.store.put_manifest(&world.manifest).map_err(|e| refuse("capture_failed", e.to_string()))?;
    // ⑧ 정책 파일 쓰기(§5.2) — 시작 전 기준은 준비 기록에 둔다. 판정(④) 뒤 같은 잠금 안이다. 확인된 쓰기 장애·결과 불확실은
    //    승인을 취소하지 않고 기준으로 따로 남긴다. 재확인이 디스크의 변경을 발견하면 쓰지 않고 시작도 하지 않는다.
    let (final_base, file) = match sync_policy_file(gate, &mut ledger, opened, proposal, bytes, rules, plan, root, attempt)? {
        Synced::Done { base, result } => (base, result),
        Synced::Final(held) => return Err(End::Final(held)),
        Synced::Stopped(held) => return Err(End::Stop(held)),
    };
    let record = sp::PolicyRecord::new(
        sp::PolicyName::new(1).expect("P1"), policy.identity().engine(), rules.clone(),
        sp::ProposalDigest::parse(&proposal.digest).expect("저장한 digest"),
    );
    let project = crate::Project::start_tracked(crate::RuleSet::builtin().map_err(|e| refuse("unavailable", e.to_string()))?, address, record, final_base)
        .map_err(|e| refuse("start_not_valid", e.to_string()))?;
    Ok((project, file))
}

/// 안전하게 멈춘 시도의 결과를 확정한다(`AttemptSettled(stopped)`). 저장했으면 원래 보류 그대로 — 승인은 그대로이고 다시 시도할
/// 수 있다. 저장하지 못했으면 그 시도는 원장에서 미해결로 남으므로 「멈췄으나 결과를 기록하지 못함」으로 보고한다.
fn settle(gate: &crate::session::StartGate, root: &Path, proposal: &Proposal, attempt: u32, held: Refusal) -> Refusal {
    let recorded = match load_ledger(gate, root) {
        Ok(Some(saved)) => {
            let mut after = saved.clone();
            after.events.push(Event::AttemptSettled { revision: proposal.revision, attempt, outcome: "stopped".into(), code: held.code.into() });
            attempt_settle_save(gate, &saved, &after)
        }
        _ => false,
    };
    if recorded { held } else { unresolved(held) }
}

/// 결과를 기록하지 못한 시도의 보고 — 멈춘 까닭은 싣되, 승인을 유지한 재시도 가능으로 말하지 않는다.
fn unresolved(held: Refusal) -> Refusal {
    refuse_with("attempt_outcome_unrecorded", tr("start_prep.attempt_outcome_unrecorded"),
        json!({"recorded": false, "stopped": held.code, "said": held.said, "detail": held.detail}))
}

/// 관측 오류 가운데 **승인 조건의 변경**(경로 종류·연결 파일·특수 항목·중첩 경계·하위 정책 파일·적을 수 없는 이름)과, 관측이
/// 흔들렸을 뿐인 경우(두 관측 사이의 변경·읽기 실패·객체 창고 오류)를 가른다. 뒤쪽은 승인을 그대로 두고 다시 시도할 수 있다.
fn is_condition(error: &crate::artifact::ObserveError) -> bool {
    use crate::artifact::ObserveError as E;
    !matches!(error, E::ChangedDuringObservation | E::ReadFailed { .. } | E::ObjectStore(_))
}

/// 승인 조건의 변경을 준비 기록에 남기고(이 판은 다시 적용하지 않는다) 그 보류를 돌려준다. 앞선 사실은 지우지 않는다.
///
/// 조건 변경 기록과 그 시도의 결과(`AttemptSettled(condition_changed)`)를 **한 번의 저장**으로 남긴다. 기록을 읽거나 저장하지
/// 못하면 차단이 확정됐다고 말하지 않는다: 변경을 발견했으나 그 사실이 저장되지 않았다(`condition_change_unrecorded`)고
/// 알린다. 그래도 관측 전에 남긴 시도 기록(`AttemptStarted`)에 결과가 없으므로, 이 프로세스든 새 프로세스든 그 판을
/// 미해결 시도로 보류한다 — 이 막음은 프로세스 메모리가 아니라 저장된 기록에서 온다.
fn condition(gate: &crate::session::StartGate, root: &Path, proposal: &Proposal, attempt: u32, what: &str, held: Refusal) -> Refusal {
    let recorded = match load_ledger(gate, root) {
        Ok(Some(saved)) => {
            let mut after = saved.clone();
            after.events.push(Event::ConditionChanged { revision: proposal.revision, what: what.into() });
            after.events.push(Event::AttemptSettled { revision: proposal.revision, attempt, outcome: "condition_changed".into(), code: held.code.into() });
            condition_save(gate, &saved, &after)
        }
        _ => false,
    };
    record_outcome(held, recorded)
}

/// 조건 변경 보류의 보고 — 기록됐으면 원래 보류 그대로(기록됨 표시), 아니면 「발견했으나 기록하지 못함」.
fn record_outcome(held: Refusal, recorded: bool) -> Refusal {
    if recorded {
        let mut held = held;
        held.detail = json!({"recorded": true, "detail": held.detail});
        return held;
    }
    refuse_with("condition_change_unrecorded", tr("start_prep.condition_change_unrecorded"),
        json!({"recorded": false, "detected": held.code, "said": held.said, "detail": held.detail}))
}

#[cfg(not(test))]
fn attempt_start_save(gate: &crate::session::StartGate, before: &Ledger, after: &Ledger) -> Result<(), Refusal> {
    save(gate, Some(before), after)
}

/// 시험: `Some(false)` = 저장하지 않고 실패를 보고, `Some(true)` = 저장한 뒤 실패를 보고(결과 불명).
#[cfg(test)]
fn attempt_start_save(gate: &crate::session::StartGate, before: &Ledger, after: &Ledger) -> Result<(), Refusal> {
    match ATTEMPT_START_FAULT.with(|f| f.borrow_mut().take()) {
        Some(false) => Err(refuse("preparation_not_saved", "시험이 넣은 저장 실패")),
        Some(true) => save(gate, Some(before), after).and(Err(refuse("preparation_not_saved", "시험이 넣은 결과 불명"))),
        None => save(gate, Some(before), after),
    }
}

#[cfg(not(test))]
fn attempt_settle_save(gate: &crate::session::StartGate, before: &Ledger, after: &Ledger) -> bool {
    save(gate, Some(before), after).is_ok()
}

#[cfg(test)]
fn attempt_settle_save(gate: &crate::session::StartGate, before: &Ledger, after: &Ledger) -> bool {
    if ATTEMPT_SETTLE_FAULT.with(|f| f.borrow_mut().take()).is_some() {
        return false;
    }
    save(gate, Some(before), after).is_ok()
}

#[cfg(not(test))]
fn condition_save(gate: &crate::session::StartGate, before: &Ledger, after: &Ledger) -> bool {
    save(gate, Some(before), after).is_ok()
}

#[cfg(test)]
fn condition_save(gate: &crate::session::StartGate, before: &Ledger, after: &Ledger) -> bool {
    if CONDITION_SAVE_FAULT.with(|f| f.borrow_mut().take()).is_some() {
        return false;
    }
    save(gate, Some(before), after).is_ok()
}

#[cfg(not(test))]
fn capture(root: &Path, policy: &sp::ApprovedPolicy, store: &crate::artifact::ObjectStore) -> Result<crate::artifact::ScopedWorld, crate::artifact::ObserveError> {
    crate::artifact::capture_scoped(root, policy, store)
}

#[cfg(not(test))]
fn commit(session: &crate::ProjectSession) -> Result<(), String> {
    session.commit().map_err(|e| e.to_string())
}

#[cfg(test)]
fn capture(root: &Path, policy: &sp::ApprovedPolicy, store: &crate::artifact::ObjectStore) -> Result<crate::artifact::ScopedWorld, crate::artifact::ObserveError> {
    if CAPTURE_FAULT.with(|f| f.borrow_mut().take()).is_some() {
        return Err(crate::artifact::ObserveError::ChangedDuringObservation);
    }
    crate::artifact::capture_scoped(root, policy, store)
}

#[cfg(test)]
fn commit(session: &crate::ProjectSession) -> Result<(), String> {
    if COMMIT_FAULT.with(|f| f.borrow_mut().take()).is_some() {
        return Err("시험이 넣은 저장 실패".into());
    }
    session.commit().map_err(|e| e.to_string())
}

fn scope_changed(approved: &[String], now: &[String]) -> Refusal {
    let a: std::collections::BTreeSet<&String> = approved.iter().collect();
    let b: std::collections::BTreeSet<&String> = now.iter().collect();
    let added: Vec<&&String> = b.difference(&a).take(200).collect();
    let removed: Vec<&&String> = a.difference(&b).take(200).collect();
    refuse_with("scope_changed", tr("start_prep.scope_changed"), json!({"added": added, "removed": removed}))
}

/// 정책 파일을 쓸 것인가 — 공유 계약 §5.2 의 승인 저장 판정과 좁은 결합 예외.
#[derive(Debug, Clone, PartialEq)]
enum FilePlan {
    /// 디스크가 이미 승인 원문이다.
    Matches,
    /// 쓸 수 있다 — `previous` 는 준비 기록의 확인된 기준, `expect` 는 판정 때 본 디스크(`None` = 없음).
    /// 교체 직전에 디스크가 `expect` 와 다르면 쓰지 않는다.
    Write { previous: Disk, expect: Option<String> },
    /// 쓰지 않는다(기준 unconfirmed 이거나 디스크를 읽지 못함). 남길 기준.
    Keep { base: sp::SyncBase },
}

fn policy_file_plan(
    ledger: &mut Ledger,
    proposal: &Proposal,
    rules: &ContentDigest,
    seen: &Result<Option<(ContentDigest, Vec<u8>)>, String>,
) -> Result<FilePlan, Refusal> {
    let disk = reading(seen);
    // 디스크가 이미 승인 원문이면(사용자가 맞췄거나, 앞선 시도의 쓰기가 닿은 뒤 중단됐다) 쓰지 않고 기준만 그 원문이다.
    if matches!(&disk, sp::DiskReading::Present(found) if found == rules) {
        return Ok(FilePlan::Matches);
    }
    // 승인 조건: 승인된 판이 보여 준 디스크(`proposal.disk`)와 지금 디스크가 다르면 그것은 규칙 입력의 변경이다. 예외는
    // 좁은 결합 예외의 입구(기준·B 의 디스크 모두 없음 → 그 뒤 처음 생긴 C)와 결합안 자체(아래)뿐이다.
    let shown = match &proposal.disk {
        Disk::Absent => sp::DiskReading::Absent,
        Disk::Present { digest } => sp::DiskReading::Present(crate::artifact::sha256_from_hex(digest).expect("잰 지문")),
        Disk::Unconfirmed => sp::DiskReading::Unreadable { reason: String::new() },
    };
    if let sp::DiskReading::Unreadable { reason } = &disk {
        // 읽지 못함은 변경의 증거도 동일의 증거도 아니다 — 쓰지 않고 이번 적용을 멈춘다(승인은 그대로, 다시 시도할 수 있다).
        return Err(refuse_with("policy_file_unreadable", tr("start_prep.policy_file_unreadable"), json!(reason)));
    }
    let changed = || Err(refuse("policy_file_changed", tr("start_prep.policy_file_changed")));
    if let Some(comb) = &proposal.combination {
        // 결합안 D: 승인 화면이 보여 준 C 가 아직 그대로일 때만 C → D 교체를 허락한다. 승인 뒤 달라진 디스크는 덮어쓰지 않는다.
        return match (&ledger.base, &disk) {
            (Disk::Absent, sp::DiskReading::Present(found)) if found.hex() == comb.disk_digest =>
                Ok(FilePlan::Write { previous: Disk::Absent, expect: Some(found.hex()) }),
            // 앞선 시도가 쓰기를 시작했으나 끝을 확인하지 못했다 — 디스크가 아직 보여 준 C 여도 자동으로 쓰지 않는다(§5.2).
            (Disk::Unconfirmed, sp::DiskReading::Present(found)) if found.hex() == comb.disk_digest =>
                Ok(FilePlan::Keep { base: sp::SyncBase::Unconfirmed }),
            _ => Err(refuse("policy_file_changed", tr("start_prep.changed_after_approval"))),
        };
    }
    let base = sync_base(&ledger.base);
    match sp::base_for_new_policy(rules, &base, &disk) {
        // 쓰기·유지로 정해지는 조합이라도, 승인된 판이 보여 준 디스크와 지금 디스크가 다르면 규칙 입력이 바뀐 것이다.
        Ok(sp::NewPolicyBase::Chosen(_)) if disk != shown => changed(),
        Ok(sp::NewPolicyBase::Chosen(chosen)) => Ok(match (&disk, &chosen) {
            (sp::DiskReading::Present(found), _) if found == rules => FilePlan::Matches,
            (sp::DiskReading::Absent, sp::SyncBase::Absent) => FilePlan::Write { previous: Disk::Absent, expect: None },
            (sp::DiskReading::Present(found), sp::SyncBase::Present(kept)) if found == kept =>
                FilePlan::Write { previous: Disk::Present { digest: kept.hex() }, expect: Some(found.hex()) },
            _ => FilePlan::Keep { base: chosen },
        }),
        Ok(sp::NewPolicyBase::Held(three)) => Err(refuse_with("held_three_way", tr("start_prep.held_three_way"), json!({
            "base": three.base.hex(), "approved": three.new_rules.hex(), "disk": three.disk.hex(),
            "disk_text": seen.as_ref().ok().and_then(|s| s.as_ref()).map(|(_, b)| String::from_utf8_lossy(b).into_owned()),
        }))),
        // 좁은 결합 예외의 입구는 **승인된 B 의 제안 당시 디스크도 없음**이었을 때뿐이다(B 승인 뒤 처음 생긴 C). B 의 제안이
        // 이미 어떤 파일을 보여 줬다면 그것은 이 예외가 아니다 — 표시만으로 쓰기 권한을 만들지 않고 아래 보류(BaseUndecided)로 둔다.
        Err(sp::BaseUndecided { base: sp::SyncBase::Absent, disk: sp::DiskReading::Present(found) })
            if proposal.disk == Disk::Absent => {
            // 좁은 결합 예외의 입구 — 한 번 안내하고 결합안 D 의 준비를 기다린다. 허락을 따로 묻지 않는다.
            let announced = ledger.events.iter().any(|e| matches!(e,
                Event::CombineAnnounced { approved_revision, disk_digest } if *approved_revision == proposal.revision && *disk_digest == found.hex()));
            if !announced {
                ledger.events.push(Event::CombineAnnounced { approved_revision: proposal.revision, disk_digest: found.hex() });
            }
            let c_text = seen.as_ref().ok().and_then(|s| s.as_ref()).map(|(_, b)| String::from_utf8_lossy(b).into_owned()).unwrap_or_default();
            let b_text = String::from_utf8_lossy(&rules_bytes(proposal)).into_owned();
            Err(refuse_with("combine_required", tr("start_prep.combine_required"), json!({
                "approved_revision": proposal.revision, "approved_rules": b_text, "disk_rules": c_text,
                "disk_digest": found.hex(), "diff_disk_vs_approved": view::line_diff(&b_text, &c_text),
                "announce_once": !announced,
            })))
        }
        Err(_) if disk != shown => changed(),
        Err(undecided) => Err(refuse_with("base_undecided", tr("start_prep.base_undecided"), json!(format!("{undecided:?}")))),
    }
}

/// 정책 파일 쓰기의 결과 — 시작을 이어 가도 되는 결과(쓰기 성공·확인된 쓰기 장애·결과 불확실은 모두 기준으로 따로 남는다)와,
/// 재확인이 승인 조건(디스크)의 변경을 발견해 시작을 멈춰야 하는 결과.
enum Synced {
    Done { base: sp::SyncBase, result: &'static str },
    /// 더 기록하지 않는 보류 — 조건 변경을 그 시도의 결과와 함께 기록했거나(또는 기록하지 못했거나), 쓰기 결과를 기록하지 못해
    /// 그 시도를 미해결로 남긴다.
    Final(Refusal),
    /// 읽기 불가·관측 불안정 — 변경도 동일함도 증거가 없다. 이번 적용만 멈추고 승인을 취소하지 않는다(시도는 `stopped` 로 확정).
    Stopped(Refusal),
}

#[allow(clippy::too_many_arguments)]
fn sync_policy_file(
    gate: &crate::session::StartGate, ledger: &mut Ledger, before: &Ledger, proposal: &Proposal, bytes: &[u8],
    rules: &ContentDigest, plan: FilePlan, root: &Path, attempt: u32,
) -> Result<Synced, Refusal> {
    let present = sp::SyncBase::Present(rules.clone());
    let changed = || refuse("policy_file_changed", tr("start_prep.policy_file_changed"));
    match plan {
        FilePlan::Matches => Ok(Synced::Done { base: present, result: "already_matches" }),
        FilePlan::Keep { base } => Ok(Synced::Done { base, result: "not_written" }),
        FilePlan::Write { previous, expect } => {
            // 0단계 재확인: 판정(④) 뒤 capture 까지 시간이 흘렀다. 디스크가 판정 때와 다르면 쓰지 않고 시작도 하지 않는다 —
            // 사용자가 그사이 정책 파일을 고쳤다면 그것은 규칙 변경이며 새 제안·승인의 일이다.
            let want = match &expect { Some(d) => Disk::Present { digest: d.clone() }, None => Disk::Absent };
            #[cfg(test)]
            if UNREADABLE_BEFORE_SYNC.with(|e| e.borrow_mut().take()).is_some() {
                let _ = std::fs::remove_file(root.join(".gilignore"));
                std::fs::create_dir(root.join(".gilignore")).expect("시험 자리");
            }
            match disk_now(root) {
                Ok(seen) if disk_value(&seen) == want => {}
                Ok(_) => return Ok(Synced::Final(condition(gate, root, proposal, attempt, "policy_file", changed()))),
                // 읽지 못함은 없음이 아니다 — 쓰지 않고 이번 적용만 멈춘다.
                Err(reason) => return Ok(Synced::Stopped(refuse_with("policy_file_unreadable", tr("start_prep.policy_file_unreadable"), json!(reason)))),
            }
            // 1단계: 쓰기 시작을 먼저 남긴다(기준 unconfirmed). 이 저장이 실패하면 쓰지 않는다.
            let mut marked = ledger.clone();
            marked.base = Disk::Unconfirmed;
            marked.events.push(Event::SyncStarted { revision: proposal.revision, target: rules.hex() });
            save(gate, Some(before), &marked)?;
            *ledger = marked.clone();
            // 2단계: 파일 쓰기.
            let expected = expect.as_deref().map(|d| crate::artifact::sha256_from_hex(d).expect("판정이 잰 지문"));
            let outcome = write_policy(root, bytes, expected.as_ref());
            // 판정 뒤 디스크가 판정 때와 다르면(교체 직전 재관측, 또는 교체 전 장애 뒤의 재관측) 조건 변경이다: 쓰지 않았고
            // 기준은 unconfirmed 로 남으며(쓰기를 시작했으므로 되돌리지 않는다) 시작하지 않는다.
            let (base, result) = match outcome {
                sp::PolicyWrite::Replaced { reread: sp::DiskReading::Present(found) } if &found == rules => (present, "written"),
                // 교체 직전 재관측이 기대와 **다른 상태를 확인**했다 — 조건 변경.
                sp::PolicyWrite::ConditionChanged { .. } => (sp::SyncBase::Unconfirmed, "condition_changed"),
                // 교체 직전 재관측을 하지 못했다 — 쓰지 않았지만 변경의 증거도 아니다. 이번 적용만 멈춘다.
                sp::PolicyWrite::Unobservable { .. } => (sp::SyncBase::Unconfirmed, "unreadable_before_replace"),
                sp::PolicyWrite::NotReplaced { .. } => match disk_now(root) {
                    // 교체 전의 확인된 장애 — 같은 잠금 안에서 다시 읽은 디스크가 이전 기준과 같을 때만 그 기준으로 되돌린다.
                    Ok(seen) if disk_value(&seen) == previous => (sync_base(&previous), "update_failed"),
                    Ok(seen) if disk_value(&seen) == want => (sp::SyncBase::Unconfirmed, "unsettled"),
                    Ok(_) => (sp::SyncBase::Unconfirmed, "condition_changed"),
                    Err(_) => (sp::SyncBase::Unconfirmed, "unreadable_after_failure"),
                },
                // 교체 뒤 재확인 불일치·읽기 실패·교체 여부 불명 — 쓰기 결과 불명. 승인을 취소하지 않고 기준을 unconfirmed 로 남긴다.
                _ => (sp::SyncBase::Unconfirmed, "unsettled"),
            };
            // 3단계: 결과의 기준을 준비 기록에도 남긴다. 실패해도 정식 시작의 저장이 같은 값을 지닌다.
            let mut settled = marked.clone();
            settled.base = match &base {
                sp::SyncBase::Absent => Disk::Absent,
                sp::SyncBase::Present(d) => Disk::Present { digest: d.hex() },
                sp::SyncBase::Unconfirmed => Disk::Unconfirmed,
            };
            settled.events.push(Event::SyncSettled { revision: proposal.revision, result: result.into(), base: settled.base.clone() });
            if result == "condition_changed" {
                // 결과와 조건 변경을 **한 번의 저장**으로 남긴다 — 둘 중 하나만 남는 상태를 만들지 않는다.
                settled.events.push(Event::ConditionChanged { revision: proposal.revision, what: "policy_file".into() });
                settled.events.push(Event::AttemptSettled { revision: proposal.revision, attempt, outcome: "condition_changed".into(), code: "policy_file_changed".into() });
                let recorded = condition_save(gate, &marked, &settled);
                if recorded { *ledger = settled; }
                return Ok(Synced::Final(record_outcome(changed(), recorded)));
            }
            let settled_saved = settle_save(gate, &marked, &settled);
            if settled_saved { *ledger = settled; }
            if matches!(result, "unreadable_before_replace" | "unreadable_after_failure") {
                let stopped = refuse_with("policy_file_unreadable", tr("start_prep.policy_file_unreadable"), json!({"result": result, "recorded": settled_saved}));
                if settled_saved {
                    return Ok(Synced::Stopped(stopped));
                }
                // 결과를 저장하지 못했으면 기록에는 쓰기 시작만 남는다 — 그 판은 다시 적용하지 않는다(`sync_outcome_unrecorded`).
                // 승인을 유지한 재시도로 말하지 않으며, 시도도 미해결로 남긴다.
                return Ok(Synced::Final(refuse_with("sync_outcome_unrecorded", tr("start_prep.sync_outcome_unrecorded"),
                    json!({"recorded": false, "stopped": stopped.code, "detail": stopped.detail}))));
            }
            Ok(Synced::Done { base, result })
        }
    }
}

#[cfg(not(test))]
fn settle_save(gate: &crate::session::StartGate, before: &Ledger, after: &Ledger) -> bool {
    save(gate, Some(before), after).is_ok()
}

#[cfg(test)]
fn settle_save(gate: &crate::session::StartGate, before: &Ledger, after: &Ledger) -> bool {
    if SETTLE_SAVE_FAULT.with(|f| f.borrow_mut().take()).is_some() {
        return false;
    }
    save(gate, Some(before), after).is_ok()
}

#[cfg(not(test))]
fn write_policy(root: &Path, bytes: &[u8], expected: Option<&ContentDigest>) -> sp::PolicyWrite {
    sp::write_root_policy_file(root, bytes, expected)
}

#[cfg(test)]
thread_local! {
    pub(crate) static WRITE_FAULT: std::cell::RefCell<Option<&'static str>> = const { std::cell::RefCell::new(None) };
    /// 교체 직전 재관측 바로 앞에서 디스크에 쓸 바이트 — 판정 뒤의 사용자 편집을 흉내 낸다.
    pub(crate) static EDIT_BEFORE_REPLACE: std::cell::RefCell<Option<Vec<u8>>> = const { std::cell::RefCell::new(None) };
    /// 한 번의 최초 capture 를 관측 불안정(두 관측 사이 변경)으로 실패시킨다 — 다시 시도할 수 있는 장애.
    pub(crate) static CAPTURE_FAULT: std::cell::RefCell<Option<()>> = const { std::cell::RefCell::new(None) };
    /// 조건 변경 기록의 저장 한 번을 실패로 만든다.
    pub(crate) static CONDITION_SAVE_FAULT: std::cell::RefCell<Option<()>> = const { std::cell::RefCell::new(None) };
    /// 정책 파일 쓰기 결과(`SyncSettled`) 저장 한 번을 실패로 만든다.
    pub(crate) static SETTLE_SAVE_FAULT: std::cell::RefCell<Option<()>> = const { std::cell::RefCell::new(None) };
    /// 교체 직전 재관측 바로 앞에서 루트 `.gilignore` 자리를 디렉터리로 바꾼다 — 읽기 불가를 흉내 낸다.
    pub(crate) static UNREADABLE_BEFORE_REPLACE: std::cell::RefCell<Option<()>> = const { std::cell::RefCell::new(None) };
    /// 쓰기 직전의 0단계 재확인 바로 앞에서 같은 자리를 디렉터리로 바꾼다 — 쓰기 시작 전의 읽기 불가.
    pub(crate) static UNREADABLE_BEFORE_SYNC: std::cell::RefCell<Option<()>> = const { std::cell::RefCell::new(None) };
    /// 적용 시도 시작 기록의 저장 한 번 — `false` 는 저장하지 않고 실패 보고, `true` 는 저장한 뒤 실패 보고.
    pub(crate) static ATTEMPT_START_FAULT: std::cell::RefCell<Option<bool>> = const { std::cell::RefCell::new(None) };
    /// 안전하게 멈춘 시도의 결과(`AttemptSettled(stopped)`) 저장 한 번을 실패로 만든다.
    pub(crate) static ATTEMPT_SETTLE_FAULT: std::cell::RefCell<Option<()>> = const { std::cell::RefCell::new(None) };
    /// 한 번의 최종 상태 저장을 실패로 보고하게 한다(저장은 하지 않는다).
    pub(crate) static COMMIT_FAULT: std::cell::RefCell<Option<()>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn write_policy(root: &Path, bytes: &[u8], expected: Option<&ContentDigest>) -> sp::PolicyWrite {
    let at = WRITE_FAULT.with(|f| *f.borrow());
    let edit = EDIT_BEFORE_REPLACE.with(|e| e.borrow_mut().take());
    let unreadable = UNREADABLE_BEFORE_REPLACE.with(|e| e.borrow_mut().take()).is_some();
    sp::write_with(root, bytes, expected, &mut |step| {
        if step == "before_recheck" && let Some(edit) = &edit {
            std::fs::write(root.join(".gilignore"), edit).expect("시험 편집");
        }
        if step == "before_recheck" && unreadable {
            let _ = std::fs::remove_file(root.join(".gilignore"));
            std::fs::create_dir(root.join(".gilignore")).expect("시험 자리");
        }
        match at {
            Some(fault) if fault == step => Err(std::io::Error::other("시험이 넣은 실패")),
            _ => Ok(()),
        }
    })
}

/// 사용자의 명시 취소(모델 도구 `gil_start_cancel` 로 전달). 기록을 지우지 않는다.
pub(crate) fn cancel(root: &str) -> Result<Value, Refusal> {
    let root = root_of(root)?;
    let gate = gate(&root, false)?;
    if gate.started() {
        drop(gate);
        let started = started_record(&root)?.ok_or_else(state_gone)?;
        let mut out = view::started(&root, &started, true);
        out["said"] = json!(tr("start_prep.cancel_after_start"));
        return Ok(out);
    }
    let before = load_ledger(&gate, &root)?.ok_or_else(|| refuse("not_preparing", tr("start_prep.not_preparing")))?;
    if before.canceled() {
        return Ok(view::status(&root, &before, None));
    }
    let mut after = before.clone();
    after.events.push(Event::Cancel { after_revision: after.next_revision.saturating_sub(1) });
    save(&gate, Some(&before), &after)?;
    Ok(view::status(&root, &after, None))
}

/// load 검사: 정식 정책 P1 의 `proposal_digest` 로 준비 기록의 **정확히 하나의 Monitor 승인 응답**을 찾고, 그 제안의
/// 원문 주소·엔진이 P1 과 같은가. capability 는 어디에도 없다 — 응답의 출처 표식만 본다.
pub(crate) fn verify_started_approval(gil: &Path, record: &sp::PolicyRecord) -> Result<(), String> {
    let ledger = read_checked(gil)?.ok_or_else(|| tr("start_prep.load_no_preparation"))?;
    let digest = record.proposal().hex();
    let proposals: Vec<&Proposal> = ledger.proposals.iter().filter(|p| p.digest == digest).collect();
    let responses: Vec<&Response> = ledger.responses.iter().filter(|r| r.proposal_digest == digest).collect();
    match (proposals.as_slice(), responses.as_slice()) {
        ([p], [r]) if r.decision == "approve" && r.source == "monitor_button" && r.revision == p.revision
            && p.rules_digest == record.rules().hex() && p.engine == record.engine().number()
            && p.blockers.is_empty() => Ok(()),
        _ => Err(tr("start_prep.load_approval_mismatch")),
    }
}

#[cfg(test)]
mod tests;
