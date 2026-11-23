//! 시작 준비 기록 `.gil/start/preparation.json` — 제안·Monitor 응답·취소·재시작·정책 파일 쓰기의 원장.
//!
//! 정식 정책 원장(`state.yaml` 의 `tracking.policies`)과 **다른 원장**이다. 여기 승인 응답이 있다는 사실은 정책 판·
//! start 사건·Snapshot 의 확정이 아니다 — 정식 시작은 `state.yaml` 이 생긴 순간에만 확정된다. 이 원장은 추가만 한다:
//! 저장할 때마다 앞선 제안·응답·사건이 같은 자리에 그대로인지 확인한다. 바뀌는 칸은 시작 전 정책 파일 동기화
//! 기준(`base`)과 다음 제안 번호뿐이다.
//!
//! 쓰기: 프로젝트 잠금 안에서만. `.gil/start/`(0700) 아래 고유한 임시 이름을 배타 생성(0600) → 쓰기 → fsync →
//! 같은 디렉터리 핸들 안 rename → 디렉터리 fsync. 연결 파일을 따라가지 않는다. 기존 항목을 지우지 않는다.
//! Monitor 승인 capability 는 이 파일에 들어가지 않는다.

use std::ffi::CString;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::internal_fs as fsx;

pub(crate) const PREP_DIR: &str = "start";
const LEDGER: &str = "preparation.json";
const SCHEMA: u32 = 1;
/// 원장 하나의 크기 상한 — 추적 목록이 아주 큰 폴더에서도 읽기가 끝없이 커지지 않게 한다.
pub(crate) const MAX_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Ledger {
    pub(crate) schema: u32,
    pub(crate) project: Identity,
    /// 정식 시작 전의 정책 파일 동기화 기준 — Core 가 확인한 루트 `.gilignore` 상태(§5.2 의 `sync_base` 와 같은
    /// 세 값). 정식 시작의 한 번 저장이 이 값을(쓰기 결과까지 반영해) `tracking.policy_file.sync_base` 로 옮긴다.
    pub(crate) base: Disk,
    pub(crate) next_revision: u32,
    pub(crate) proposals: Vec<Proposal>,
    pub(crate) responses: Vec<Response>,
    pub(crate) events: Vec<Event>,
}

/// 준비한 폴더의 정체성. 폴더를 복사·이동하면 다른 준비로 보고 거절한다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Identity {
    pub(crate) root: String,
    pub(crate) device: u64,
    pub(crate) inode: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Disk {
    Absent,
    Present { digest: String },
    Unconfirmed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Proposal {
    pub(crate) revision: u32,
    /// 아래 값 전체(원문 바이트의 주소·추적 목록·추천과 선택·디스크·결합 출처·프로젝트)를 묶은 sha256.
    pub(crate) digest: String,
    pub(crate) epoch: u32,
    pub(crate) engine: u32,
    /// 정확한 원문 바이트(base64). 정규화하지 않는다.
    pub(crate) rules: String,
    pub(crate) rules_digest: String,
    /// 제안할 때 Core 가 본 루트 `.gilignore` (`absent` 또는 `present`).
    pub(crate) disk: Disk,
    /// 제안할 때의 시작 전 동기화 기준(준비 기록의 `base`). 정책 파일 효과는 이 값·`disk`·원문만으로 정한다.
    pub(crate) base: Disk,
    /// 제안할 때 본 디스크 원문(base64, 없으면 `None`) — 화면의 차이는 이 저장값에서만 계산한다.
    pub(crate) disk_text: Option<String>,
    /// 이 판이 승인되면 정책 파일에 일어날 일: `create`·`replace`·`unchanged`·`replace_shown_disk`(결합안)·
    /// `not_written_unconfirmed`·`held_base_undecided`·`held_three_way`. 제안 때 정하고 저장한다.
    pub(crate) policy_file_effect: String,
    pub(crate) tracked: Vec<String>,
    pub(crate) excluded: Vec<Excluded>,
    pub(crate) blockers: Vec<String>,
    pub(crate) notices: Vec<String>,
    pub(crate) recommendations: Vec<Rec>,
    pub(crate) combination: Option<Combination>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Excluded {
    pub(crate) path: String,
    pub(crate) kind: String,
    pub(crate) reason: String,
    /// `not_explored`(완전히 제외 — 안을 보지 않았다), `explored_for_reinclusion`, 파일이면 빈 글자.
    pub(crate) interior: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Rec {
    pub(crate) key: String,
    pub(crate) category: String,
    pub(crate) sources: Vec<String>,
    pub(crate) rules: Vec<String>,
    /// 처음 보여 줄 때의 선택(일반·비밀값 후보는 선택, 프로젝트별·빌드 출력·Git 출처는 미선택).
    pub(crate) default_selected: bool,
    /// 이 제안의 원문에 그 줄들이 모두 들어 있는가 — 사용자가 확인한 최종 선택.
    pub(crate) in_rules: bool,
    pub(crate) reason: String,
    pub(crate) impacts: Vec<String>,
}

/// 좁은 결합 예외(최초 기준 absent, 승인 B 뒤 생긴 디스크 C)에서 만든 결합안 D 의 출처.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Combination {
    pub(crate) approved_revision: u32,
    pub(crate) approved_digest: String,
    pub(crate) approved_rules_digest: String,
    /// D 승인 화면이 보여 준 디스크 C 의 지문과 원문(base64). 적용 직전 디스크가 정확히 이것이어야 D 로 교체한다.
    pub(crate) disk_digest: String,
    pub(crate) disk_rules: String,
    /// D 를 준비할 때 C 를 같은 엔진으로 평가한 추적 목록. C 를 해석하지 못하면 `None` 과 `disk_problems`.
    pub(crate) disk_tracked: Option<Vec<String>>,
    pub(crate) disk_problems: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Response {
    pub(crate) revision: u32,
    pub(crate) proposal_digest: String,
    /// `approve` | `request_changes`
    pub(crate) decision: String,
    /// 언제나 `monitor_button`. 개인 신원 인증이 아니다.
    pub(crate) source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Event {
    /// 사용자의 명시 취소. 기록은 지우지 않는다.
    Cancel { after_revision: u32 },
    /// 취소 뒤의 명시 재시작. 그 앞의 제안·승인은 다시 쓰지 않는다.
    Restart { after_revision: u32 },
    /// 승인 B 를 적용하려는데 처음 확인한 기준이 absent 이고 디스크에 다른 C 가 생겼다 — 합칠 예정임을 한 번 안내했다.
    CombineAnnounced { approved_revision: u32, disk_digest: String },
    /// 정식 시작 전 Core 가 정책 파일 쓰기를 시작했다(§5.2 1단계 — 기준 unconfirmed).
    SyncStarted { revision: u32, target: String },
    /// 그 쓰기의 결과와 남긴 기준.
    SyncSettled { revision: u32, result: String, base: Disk },
    /// 적용 직전 재확인이 승인 조건(규칙·디스크 정책 파일·추적 목록·경로)의 변경을 발견했다. 이 판은 다시 적용하지 않는다 —
    /// 새 제안과 그 판의 Monitor 승인이 필요하다. 앞선 사실(제안·응답·객체·동기화)은 지우지 않는다.
    ConditionChanged { revision: u32, what: String },
    /// 승인된 판의 적용 시도 하나를 **조건을 관측하기 전에** 남긴다 — 정확한 승인 대상(판·digest)에 묶인다. 이 기록의 저장을
    /// 확인하지 못하면 관측·적용으로 나아가지 않는다. 결과(`AttemptSettled`)가 없는 시도는 새 프로세스에서도 미해결로 보류한다.
    AttemptStarted { revision: u32, attempt: u32, proposal_digest: String },
    /// 그 시도의 확정된 결과: `condition_changed`(확인된 승인 조건 변경 — 그 판을 다시 적용하지 않는다) 또는
    /// `stopped`(읽기 불가·관측 불안정·확인된 저장 장애·기존 보류로 안전하게 멈췄다 — 승인은 그대로, 다시 시도할 수 있다).
    /// `code` 는 멈춘 까닭의 보고 코드다. 정식 시작의 확정은 이 원장이 아니라 상태 파일이 말한다.
    AttemptSettled { revision: u32, attempt: u32, outcome: String, code: String },
}

/// 적용 시도 결과의 두 값.
pub(crate) const ATTEMPT_OUTCOMES: [&str; 2] = ["condition_changed", "stopped"];

impl Ledger {
    pub(crate) fn new(project: Identity, base: Disk) -> Ledger {
        Ledger { schema: SCHEMA, project, base, next_revision: 1, proposals: Vec::new(), responses: Vec::new(), events: Vec::new() }
    }

    /// 지금 준비의 시기 — 재시작할 때마다 하나씩 는다. 그 앞 시기의 제안·승인은 쓰지 않는다.
    pub(crate) fn epoch(&self) -> u32 {
        self.events.iter().filter(|e| matches!(e, Event::Restart { .. })).count() as u32
    }

    /// 명시 취소가 마지막 시기의 마지막 취소·재시작 사건인가.
    pub(crate) fn canceled(&self) -> bool {
        self.events.iter().rev().find(|e| matches!(e, Event::Cancel { .. } | Event::Restart { .. }))
            .is_some_and(|e| matches!(e, Event::Cancel { .. }))
    }

    pub(crate) fn latest(&self) -> Option<&Proposal> {
        self.proposals.last().filter(|p| p.epoch == self.epoch())
    }

    pub(crate) fn proposal(&self, digest: &str) -> Option<&Proposal> {
        self.proposals.iter().find(|p| p.digest == digest)
    }

    /// 그 판을 **다시 적용하면 안 되는 이유**가 기록에 있는가 — 저장된 사실만으로 판정한다.
    ///
    /// - `condition_changed`: 재확인이 승인 조건의 변경을 발견해 기록했다(`ConditionChanged`, 또는 파일 쓰기 결과로 남긴
    ///   `SyncSettled(condition_changed)` — 둘 중 하나만 남은 상태도 같다).
    ///   `AttemptSettled(condition_changed)` 도 같다.
    /// - `sync_outcome_unrecorded`: 그 판의 정책 파일 쓰기를 시작했다(`SyncStarted`)는 기록 뒤에 결과(`SyncSettled`)가 없다 —
    ///   중단이나 저장 실패로 쓰기 결과와 그때의 재확인 결과를 모른다. 모르는 상태를 같은 승인의 재사용 허가로 읽지 않는다.
    /// - `attempt_outcome_unrecorded`: 그 판의 적용 시도를 시작했다(`AttemptStarted`)는 기록 뒤에 결과(`AttemptSettled`)가
    ///   없다 — 중단이나 결과 저장 실패로 그 시도가 무엇을 관측했는지 모른다. 변경이라고도, 문제없었다고도 보지 않는다.
    ///   지금 디스크를 다시 읽는 것으로 이 상태를 지우지 않는다. 승인 기록은 지우지 않으며 승인을 취소하지도 않는다.
    pub(crate) fn not_reusable(&self, revision: u32) -> Option<&'static str> {
        let mut open_write = false;
        let mut open_attempts = std::collections::BTreeSet::new();
        for e in &self.events {
            match e {
                Event::ConditionChanged { revision: r, .. } if *r == revision => return Some("condition_changed"),
                Event::SyncSettled { revision: r, result, .. } if *r == revision => {
                    if result == "condition_changed" { return Some("condition_changed"); }
                    open_write = false;
                }
                Event::SyncStarted { revision: r, .. } if *r == revision => open_write = true,
                Event::AttemptStarted { revision: r, attempt, .. } if *r == revision => { open_attempts.insert(*attempt); }
                Event::AttemptSettled { revision: r, attempt, outcome, .. } if *r == revision => {
                    if outcome == "condition_changed" { return Some("condition_changed"); }
                    open_attempts.remove(attempt);
                }
                _ => {}
            }
        }
        if open_write {
            Some("sync_outcome_unrecorded")
        } else if !open_attempts.is_empty() {
            Some("attempt_outcome_unrecorded")
        } else {
            None
        }
    }

    /// 다음 적용 시도 번호 — 원장 전체에서 하나씩 는다.
    pub(crate) fn next_attempt(&self) -> u32 {
        self.events.iter().filter(|e| matches!(e, Event::AttemptStarted { .. })).count() as u32 + 1
    }

    pub(crate) fn response(&self, digest: &str) -> Option<&Response> {
        self.responses.iter().find(|r| r.proposal_digest == digest)
    }

    /// 앞선 기록이 그대로 남고 끝에만 붙었는가 — 저장 전 확인.
    fn extends(&self, before: &Ledger) -> bool {
        self.schema == before.schema
            && self.project == before.project
            && self.next_revision >= before.next_revision
            && self.proposals.starts_with(&before.proposals)
            && self.responses.starts_with(&before.responses)
            && self.events.starts_with(&before.events)
    }
}

/// 준비 기록 읽기. 기록이 없으면 `Ok(None)`. 잠금 없이 불러도 아무것도 쓰지 않는다.
pub(crate) fn read(gil: &Path) -> Result<Option<Ledger>, String> {
    let dir = match fsx::directory(gil, false) {
        Ok(dir) => dir,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!(".gil 을 열지 못했다: {e}")),
    };
    let start = match fsx::child_dir(&dir, &cstr(PREP_DIR)) {
        Ok(start) => start,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("시작 준비 폴더를 열지 못했다: {e}")),
    };
    let file = match fsx::regular_at(&start, &cstr(LEDGER)) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("시작 준비 기록을 열지 못했다: {e}")),
    };
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|e| format!("시작 준비 기록을 읽지 못했다: {e}"))?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("시작 준비 기록이 너무 크다".into());
    }
    let ledger: Ledger = serde_json::from_slice(&bytes).map_err(|e| format!("시작 준비 기록의 꼴이 맞지 않는다: {e}"))?;
    if ledger.schema != SCHEMA {
        return Err(format!("이 빌드가 모르는 시작 준비 기록 판 {}", ledger.schema));
    }
    Ok(Some(ledger))
}

/// 잠금을 쥔 채로 원장을 저장한다. `before` 는 같은 잠금 안에서 방금 읽은 값이다(없으면 처음 만든다).
pub(crate) fn write(lock: &crate::lock::ProjectLock, gil: &Path, before: Option<&Ledger>, after: &Ledger) -> Result<(), String> {
    if let Some(before) = before
        && !after.extends(before) {
        return Err("시작 준비 기록은 추가만 한다 — 앞선 제안·응답·사건을 바꾸는 저장을 거절했다".into());
    }
    lock.validate(&gil.join(crate::lock::LOCK_FILE)).map_err(|e| format!("프로젝트 잠금을 확인하지 못했다: {e}"))?;
    let bytes = serde_json::to_vec_pretty(after).map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("시작 준비 기록이 상한을 넘어 저장하지 않았다".into());
    }
    let dir = fsx::directory(gil, false).map_err(|e| format!(".gil 을 열지 못했다: {e}"))?;
    let name = cstr(PREP_DIR);
    let start = match fsx::child_dir(&dir, &name) {
        Ok(start) => start,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let made = fsx::create_dir_at(&dir, &name, 0o700).map_err(|e| format!("시작 준비 폴더를 만들지 못했다: {e}"))?;
            fsx::fsync(&dir).map_err(|e| e.to_string())?;
            made
        }
        Err(e) => return Err(format!("시작 준비 폴더를 열지 못했다: {e}")),
    };
    let mut nonce = [0u8; 8];
    getrandom::fill(&mut nonce).map_err(|e| e.to_string())?;
    let temp = cstr(&format!("{LEDGER}.{}.writing", nonce.iter().map(|b| format!("{b:02x}")).collect::<String>()));
    let mut file: File = fsx::create_file_at(&start, &temp, 0o600).map_err(|e| format!("임시 기록을 만들지 못했다: {e}"))?;
    let written = file.write_all(&bytes).and_then(|()| file.flush()).and_then(|()| fsx::fsync(&file));
    drop(file);
    if let Err(e) = written {
        let _ = fsx::unlink_at(&start, &temp);
        return Err(format!("시작 준비 기록을 쓰지 못했다: {e}"));
    }
    if let Err(e) = fsx::rename_at(&start, &temp, &cstr(LEDGER)) {
        let _ = fsx::unlink_at(&start, &temp);
        return Err(format!("시작 준비 기록을 교체하지 못했다: {e}"));
    }
    fsx::fsync(&start).map_err(|e| format!("시작 준비 기록의 교체를 디스크에 밀어 넣지 못했다(결과 불확실): {e}"))?;
    lock.validate(&gil.join(crate::lock::LOCK_FILE)).map_err(|e| format!("프로젝트 잠금을 확인하지 못했다: {e}"))
}

fn cstr(name: &str) -> CString {
    CString::new(name).expect("고정·생성 이름에는 NUL 이 없다")
}
