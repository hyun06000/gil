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
    /// Monitor 에서 받은 즉시 보관 중단 요청이 표식으로 적용됐다(또는 이미 있는 표식의 기록을 다시 남겼다). `id` 는 **이 중단 건**의
    /// 이름이다 — 같은 자리에서 다시 중단해도 다른 값이다. R1 이 쓴 기록에는 `id` 가 없고(빈 값) 그런 건은 확인 근거가 되지 못한다.
    HaltRequested { at: String, source: String, #[serde(default)] id: String },
    /// Monitor 에서 사용자가 그 중단 건(`id`)의 현재 상태와 정리 방법을 확인하고 해제를 요청했다. 표식을 지우는 저장보다 **먼저** 남긴다.
    /// 정리 방법은 기록일 뿐 실행이 아니다.
    HaltReleased { at: String, cleanup: String, source: String, #[serde(default)] id: String },
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

    /// 아직 해제 기록이 없는 **지금의 중단 건** — 마지막 중단·해제 사건이 `id` 있는 `HaltRequested` 일 때의 (id, at).
    /// 기록이 없거나 마지막이 해제면 `None` 이다. 기록의 부재는 확인 근거가 아니다.
    pub(crate) fn open_halt(&self) -> Option<(&str, &str)> {
        self.events.iter().rev().find_map(|event| match event {
            Event::HaltRequested { id, at, .. } => Some((!id.is_empty()).then_some((id.as_str(), at.as_str()))),
            Event::HaltReleased { .. } => Some(None),
            _ => None,
        })?
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

// ── 중단 요청의 수신 표식 ──────────────────────────────────────────────────
//
// 즉시 보관 중단 표식(`tracking.archive_halt`)은 `state.yaml` 에 있고 프로젝트 잠금 안에서만 적힌다. 다른 명령이 그 잠금을 쥐고 승인·
// 보관·재시도를 진행 중이면 요청은 곧바로 적용될 수 없다. 그래서 요청의 **수신**을 잠금 없이 이 작은 파일로 먼저 남긴다. 진행 중인
// 명령은 다음 쓰기 경계에서 이 파일을 보고 멈추며, 표식의 **적용**은 잠금을 쥔 쪽이 한다. `state.yaml` 의 꼴은 바뀌지 않는다.

const HALT_REQUEST: &str = "halt-request";

fn scope_dir(gil: &Path, create: bool) -> std::io::Result<File> {
    let dir = fsx::directory(gil, false)?;
    let name = cstr(SCOPE_DIR);
    match fsx::child_dir(&dir, &name) {
        Ok(scope) => Ok(scope),
        Err(e) if create && e.kind() == std::io::ErrorKind::NotFound => match fsx::create_dir_at(&dir, &name, 0o700) {
            Ok(made) => {
                fsx::fsync(&dir)?;
                Ok(made)
            }
            // 다른 쪽이 방금 만들었다.
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => fsx::child_dir(&dir, &name),
            Err(e) => Err(e),
        },
        Err(e) => Err(e),
    }
}

/// 받은 중단 요청이 아직 적용을 기다리는가 — 그 요청의 `id`. 없으면 `Ok(None)`. 읽지 못하면 `Err` 이며 부르는 쪽은 그것을
/// 「요청 없음」으로 읽지 않는다.
pub(crate) fn halt_request(gil: &Path) -> Result<Option<String>, String> {
    let scope = match scope_dir(gil, false) {
        Ok(scope) => scope,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("범위 변경 기록 폴더를 열지 못했다: {e}")),
    };
    let file = match fsx::regular_at(&scope, &cstr(HALT_REQUEST)) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("중단 요청 표식을 열지 못했다: {e}")),
    };
    let mut id = String::new();
    file.take(256).read_to_string(&mut id).map_err(|e| format!("중단 요청 표식을 읽지 못했다: {e}"))?;
    Ok(Some(id.trim().to_string()))
}

/// 중단 요청의 수신을 남긴다 — **잠금 없이.** 이미 받은 요청이 있으면 그 `id` 를 돌려주고 새로 쓰지 않는다.
pub(crate) fn receive_halt_request(gil: &Path, id: &str) -> Result<String, String> {
    let scope = scope_dir(gil, true).map_err(|e| format!("범위 변경 기록 폴더를 열지 못했다: {e}"))?;
    match fsx::create_file_at(&scope, &cstr(HALT_REQUEST), 0o600) {
        Ok(mut file) => {
            file.write_all(id.as_bytes()).and_then(|()| file.flush()).and_then(|()| fsx::fsync(&file))
                .and_then(|()| fsx::fsync(&scope))
                .map_err(|e| format!("중단 요청 표식을 쓰지 못했다: {e}"))?;
            Ok(id.to_string())
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            halt_request(gil)?.ok_or_else(|| "중단 요청 표식이 읽는 사이에 사라졌다".to_string())
        }
        Err(e) => Err(format!("중단 요청 표식을 만들지 못했다: {e}")),
    }
}

/// 적용이 확인된 요청의 수신 표식을 치운다. 잠금을 쥔 쪽만 부른다.
pub(crate) fn clear_halt_request(gil: &Path) -> Result<(), String> {
    let scope = match scope_dir(gil, false) {
        Ok(scope) => scope,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };
    match fsx::unlink_at(&scope, &cstr(HALT_REQUEST)) {
        Ok(()) => fsx::fsync(&scope).map_err(|e| e.to_string()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
