//! 범위 변경의 **제안·Monitor 응답 원장** — `.gil/scope/changes.json`.
//!
//! 공유 계약 §5.1 은 「제안·모니터 응답의 원장은 3·4번 작업이 설계하되, `proposal_digest` 로 정확히 하나의 모니터 승인 응답을 찾을 수
//! 있어야 한다」고 적는다. 최초 시작의 준비 기록(`.gil/start/preparation.json`)과 같은 규율을 쓴다: 추가만 하고(저장마다 앞선 기록의
//! 보존을 확인), 프로젝트 잠금 안에서만 쓰고, 0700/0600, 배타 임시 파일 → fsync → rename → 디렉터리 fsync.
//!
//! 이 원장은 **정책 원장이 아니다.** 승인 응답의 기록은 정책 판·사건의 확정이 아니며, 확정은 `state.yaml` 의 한 번 저장(저장 1)뿐이다.
//! 서명된 기록도 아니다 — `.gil` 에 쓸 수 있는 쪽이 제안과 응답을 함께 지어내는 것까지 막지 못한다(준비 기록과 같은 한계). 응답의 출처
//! 값 `monitor_button` 은 채널의 이름이며 개인 신원의 인증이 아니다.
use std::ffi::CString;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::internal_fs as fsx;

pub(crate) const SCOPE_DIR: &str = "scope";
const LEDGER: &str = "changes.json";
const SCHEMA: u32 = 1;
const MAX_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Ledger {
    pub(crate) schema: u32,
    pub(crate) next_revision: u32,
    pub(crate) proposals: Vec<Proposal>,
    pub(crate) responses: Vec<Response>,
    pub(crate) events: Vec<Event>,
}

/// 디스크의 루트 `.gilignore` 를 제안할 때 본 값.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Disk {
    Absent,
    Present { digest: String },
}

/// Monitor 가 보여 주는 **정확한 제안 판** — 원문·판·효과·검토 Interview·승인 경계를 한 digest 로 묶는다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Proposal {
    pub(crate) revision: u32,
    /// 아래 모든 칸(이 칸 제외)의 sha256.
    pub(crate) digest: String,
    pub(crate) engine: u32,
    /// 승인할 `.gilignore` 원문의 정확한 바이트(base64). 정규화하지 않는다.
    pub(crate) rules_b64: String,
    /// 그 원문의 주소.
    pub(crate) rules_digest: String,
    pub(crate) chain: String,
    /// 제안할 때 서 있던 닫힌 Interview — 변경안을 검토한 인터뷰이자 승인을 저장할 경계.
    pub(crate) boundary: String,
    /// 제안할 때의 Cycle 발급기 값.
    pub(crate) issuer: u32,
    /// 제안할 때의 적용 정책 판.
    pub(crate) policy_before: String,
    /// 제안할 때의 마지막 확정 세계와 그 확정 정책.
    pub(crate) world_before: String,
    pub(crate) confirmed_policy: String,
    /// 제안할 때의 디스크 정책 파일.
    pub(crate) disk: Disk,
    /// 제안 원문으로 본 실제 추적 목록(파일 내용 없음).
    pub(crate) tracked_after: Vec<String>,
    /// 효과 — 적용 정책은 추적하지 않는데 제안이 추적하는 경로 / 적용 정책은 추적하는데 제안이 제외하는 경로.
    pub(crate) newly_tracked: Vec<String>,
    pub(crate) newly_excluded: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Response {
    pub(crate) revision: u32,
    pub(crate) proposal_digest: String,
    /// `approve` | `request_changes`
    pub(crate) decision: String,
    /// 채널 값은 `monitor_button` 하나다.
    pub(crate) source: String,
}

/// 원장에 남기는 사건. 관리 사건(`tracking.events`)이 아니다 — 요청·결과의 이력이다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Event {
    /// 저장 1 을 다시 읽어 확인했다.
    Applied { revision: u32, proposal_digest: String, policy: String, event: String },
    /// Monitor 에서 받은 명시 재시도 요청 — 시도를 시작하기 전에 남긴다.
    RetryRequested { event: String, boundary: String, source: String },
    /// Monitor 에서 받은 즉시 보관 중단 요청.
    HaltRequested { at: String, source: String },
    /// Monitor 에서 사용자가 현재 상태와 정리 방법을 확인하고 해제했다. 정리 방법은 기록일 뿐 실행이 아니다.
    HaltReleased { at: String, cleanup: String, source: String },
}

impl Ledger {
    pub(crate) fn new() -> Ledger {
        Ledger { schema: SCHEMA, next_revision: 1, proposals: Vec::new(), responses: Vec::new(), events: Vec::new() }
    }

    pub(crate) fn latest(&self) -> Option<&Proposal> {
        self.proposals.last()
    }

    pub(crate) fn proposal(&self, digest: &str) -> Option<&Proposal> {
        self.proposals.iter().find(|proposal| proposal.digest == digest)
    }

    /// 그 제안 digest 의 응답들. 정상 기록에는 많아야 하나다.
    pub(crate) fn responses_to<'a>(&'a self, digest: &'a str) -> impl Iterator<Item = &'a Response> {
        self.responses.iter().filter(move |response| response.proposal_digest == digest)
    }

    fn extends(&self, before: &Ledger) -> bool {
        self.schema == before.schema
            && self.next_revision >= before.next_revision
            && self.proposals.starts_with(&before.proposals)
            && self.responses.starts_with(&before.responses)
            && self.events.starts_with(&before.events)
    }
}

fn cstr(name: &str) -> CString {
    CString::new(name).expect("고정·생성 이름에는 NUL 이 없다")
}

/// 원장 읽기. 기록이 없으면 `Ok(None)`. 아무것도 쓰지 않는다.
pub(crate) fn read(gil: &Path) -> Result<Option<Ledger>, String> {
    let dir = match fsx::directory(gil, false) {
        Ok(dir) => dir,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!(".gil 을 열지 못했다: {e}")),
    };
    let scope = match fsx::child_dir(&dir, &cstr(SCOPE_DIR)) {
        Ok(scope) => scope,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("범위 변경 기록 폴더를 열지 못했다: {e}")),
    };
    let file = match fsx::regular_at(&scope, &cstr(LEDGER)) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("범위 변경 기록을 열지 못했다: {e}")),
    };
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|e| format!("범위 변경 기록을 읽지 못했다: {e}"))?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("범위 변경 기록이 너무 크다".into());
    }
    let ledger: Ledger = serde_json::from_slice(&bytes).map_err(|e| format!("범위 변경 기록의 꼴이 맞지 않는다: {e}"))?;
    if ledger.schema != SCHEMA {
        return Err(format!("이 빌드가 모르는 범위 변경 기록 판 {}", ledger.schema));
    }
    Ok(Some(ledger))
}

/// 잠금을 쥔 채로 원장을 저장한다. `before` 는 같은 잠금 안에서 방금 읽은 값이다(없으면 처음 만든다). 추가만 한다.
pub(crate) fn write(lock: &crate::lock::ProjectLock, gil: &Path, before: Option<&Ledger>, after: &Ledger) -> Result<(), String> {
    if let Some(before) = before
        && !after.extends(before)
    {
        return Err("범위 변경 기록은 추가만 한다 — 앞선 제안·응답·사건을 바꾸는 저장을 거절했다".into());
    }
    lock.validate(&gil.join(crate::lock::LOCK_FILE)).map_err(|e| format!("프로젝트 잠금을 확인하지 못했다: {e}"))?;
    let bytes = serde_json::to_vec_pretty(after).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("범위 변경 기록이 상한을 넘어 저장하지 않았다".into());
    }
    let dir = fsx::directory(gil, false).map_err(|e| format!(".gil 을 열지 못했다: {e}"))?;
    let name = cstr(SCOPE_DIR);
    let scope = match fsx::child_dir(&dir, &name) {
        Ok(scope) => scope,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let made = fsx::create_dir_at(&dir, &name, 0o700).map_err(|e| format!("범위 변경 기록 폴더를 만들지 못했다: {e}"))?;
            fsx::fsync(&dir).map_err(|e| e.to_string())?;
            made
        }
        Err(e) => return Err(format!("범위 변경 기록 폴더를 열지 못했다: {e}")),
    };
    let mut nonce = [0u8; 8];
    getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;
    let temp = cstr(&format!("{LEDGER}.{}.writing", nonce.iter().map(|b| format!("{b:02x}")).collect::<String>()));
    let mut file: File = fsx::create_file_at(&scope, &temp, 0o600).map_err(|e| format!("임시 기록을 만들지 못했다: {e}"))?;
    let written = file.write_all(&bytes).and_then(|()| file.flush()).and_then(|()| fsx::fsync(&file));
    drop(file);
    if let Err(e) = written {
        let _ = fsx::unlink_at(&scope, &temp);
        return Err(format!("범위 변경 기록을 쓰지 못했다: {e}"));
    }
    if let Err(e) = fsx::rename_at(&scope, &temp, &cstr(LEDGER)) {
        let _ = fsx::unlink_at(&scope, &temp);
        return Err(format!("범위 변경 기록을 교체하지 못했다: {e}"));
    }
    fsx::fsync(&scope).map_err(|e| format!("범위 변경 기록의 교체를 디스크에 밀어 넣지 못했다(결과 불확실): {e}"))?;
    lock.validate(&gil.join(crate::lock::LOCK_FILE)).map_err(|e| format!("프로젝트 잠금을 확인하지 못했다: {e}"))
}
