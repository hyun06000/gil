//! 정책 파일 동기화의 **기준**과 판정표 — 계약 §5.2 「정책 파일 동기화의 기준」.
//!
//! 여기 있는 것은 자료형과 순수 함수뿐이다. 파일을 읽거나 쓰지 않고, 잠그지 않고, 경로를 검사하지
//! 않고, 상태를 저장하지 않는다. [`PolicyFileReading`] 은 **파일을 쓸 권한도 승인 영수증도 아니다.**
//! 쓰기 직전 재확인(잠금 안의 0단계)·경로 안전 검사·`sync_base` 저장 순서(1–3단계)는 후속 계층이 진다.

use std::io;

use crate::artifact::ContentDigest;

/// Core 가 덮어써도 된다고 **확인한** 디스크 상태 — 세 값뿐이다.
///
/// `Present` 만 지문을 지닌다. 다른 두 값에 지문이 붙은 꼴은 이 타입으로 만들 수 없고, 저장 글자에서도
/// 거절된다([`super::StoredSyncBase`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SyncBase {
    /// 디스크의 `.gilignore` 가 이 내용임을 확인했고, 그 뒤 Core 가 쓰기를 시작한 적이 없다.
    Present(ContentDigest),
    /// 파일이 없음을 확인했고, 그 뒤 Core 가 쓰기를 시작한 적이 없다.
    Absent,
    /// Core 가 쓰기를 시작했으나 끝을 확인하지 못했다. 덮어써도 되는 상태를 모른다.
    Unconfirmed,
}

/// 호출자가 관측한 디스크의 루트 `.gilignore`. **읽지 못함은 없음이 아니다.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DiskReading {
    /// 파일이 있고 그 바이트의 지문이 이것이다.
    Present(ContentDigest),
    /// 파일이 없다.
    Absent,
    /// 읽지 못했다 — 권한, 디렉터리, 입출력 오류 등.
    Unreadable { reason: String },
}

impl DiskReading {
    /// 호출자가 읽은 결과를 관측으로 옮긴다. `NotFound` 만 없음이고 그 밖의 오류는 모두 읽지 못함이다.
    pub(crate) fn from_read(read: io::Result<Vec<u8>>) -> DiskReading {
        match read {
            Ok(bytes) => DiskReading::Present(
                crate::artifact::digest_of(bytes.as_slice()).expect("메모리의 바이트는 언제나 읽힌다"),
            ),
            Err(error) if error.kind() == io::ErrorKind::NotFound => DiskReading::Absent,
            Err(error) => DiskReading::Unreadable { reason: error.to_string() },
        }
    }
}

/// §5.2 판정표의 결과 — 무엇이 보이는가. **어느 것도 쓰기 허가가 아니다.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PolicyFileReading {
    /// 디스크가 적용 원문과 같다(기준은 무엇이든). 표시: 동기화됨. Core 는 쓰지 않는다.
    MatchesApplied,
    /// 적용 원문과 다르고 확인된 기준(`present`·`absent`)과 같다. 표시: 갱신 필요(구판 잔류).
    /// 후속 계층이 §5.2 의 세 걸음(잠금 안 재확인 → `unconfirmed` 저장 → 쓰기 → 새 기준 저장)을 따를
    /// **수 있는 후보**일 뿐이다.
    StaleMatchesBase,
    /// 적용 원문과도 확인된 기준과도 다르다. 표시: 사용자 편집·삭제 — 차이 표시. Core 는 쓰지 않는다.
    UserChanged { deleted: bool },
    /// 적용 원문과 다르고 기준이 `unconfirmed` 다. 표시: 갱신 필요 — 원인 미확인. **자동으로 쓰지 않는다.**
    BaseUnconfirmed,
    /// 디스크를 읽지 못해 판정할 수 없다. 동기화됨으로 보고하지 않고 쓰지 않는다.
    Unreadable { reason: String },
}

/// §5.2 판정표. 입력은 모두 호출자가 건넨다: 적용 원문의 주소(어느 판이 적용 정책인지는 이 함수가
/// 정하지 않는다), 저장된 기준, 디스크 관측.
pub(crate) fn classify_policy_file(
    applied_rules: &ContentDigest,
    base: &SyncBase,
    disk: &DiskReading,
) -> PolicyFileReading {
    let on_disk = match disk {
        DiskReading::Unreadable { reason } => return PolicyFileReading::Unreadable { reason: reason.clone() },
        DiskReading::Present(found) if found == applied_rules => return PolicyFileReading::MatchesApplied,
        DiskReading::Present(found) => Some(found),
        DiskReading::Absent => None,
    };
    match (base, on_disk) {
        (SyncBase::Unconfirmed, _) => PolicyFileReading::BaseUnconfirmed,
        (SyncBase::Present(base), Some(found)) if base == found => PolicyFileReading::StaleMatchesBase,
        (SyncBase::Absent, None) => PolicyFileReading::StaleMatchesBase,
        (SyncBase::Present(_) | SyncBase::Absent, found) => {
            PolicyFileReading::UserChanged { deleted: found.is_none() }
        }
    }
}
