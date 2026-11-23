//! 루트 `.gilignore` **파일 쓰기**(공유 계약 §5.2 쓰기 2단계) — 시작 확정이 부르는 유일한 정책 파일 쓰기.
//!
//! ```text
//! 프로젝트 루트와 그 `.gil/start/` 를 연결 파일을 따라가지 않고 연다
//!   → `.gil/start/` 안에 고유한 임시 이름을 배타 생성(0644) → 원문 바이트를 그대로 쓰고 fsync
//!   → **교체 직전** 루트 `.gilignore` 를 ①–⑥ 관측으로 다시 읽어 부르는 쪽이 기대한 상태(없음 또는 그 지문)인지 확인
//!     — 다르면 교체하지 않는다(그사이 사용자가 고친 파일을 덮어쓰지 않는다)
//!   → renameat(임시 → 루트 `.gilignore`) → 루트 fsync → 다시 읽어 결과를 그대로 돌려준다
//! ```
//!
//! 임시 파일은 GIL 내부 기록 자리에 두므로 중단돼 남아도 추적 세계에 끼지 않는다. 이 함수는 **쓰기만** 한다: 쓰기 전 판정과
//! `sync_base` 저장은 부르는 쪽이 같은 프로젝트 잠금 안에서 한다. 원문을 정규화하지 않는다. 마지막 재관측과 rename 사이의
//! 아주 짧은 틈에 생긴 외부 쓰기까지 막는다고 주장하지 않는다(GIL 은 다른 프로세스의 쓰기를 잠그지 않는다).
//! 교체가 일어났는지 알 수 없으면 `Unknown` 이며 되돌리지 않는다.
#![cfg(all(feature = "tracking-policy", unix))]

use std::ffi::CString;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::path::Path;

use super::sync::DiskReading;
use crate::artifact::ContentDigest;

/// 쓰기 한 번의 결과 — [`super::sync_order::ReplaceResult`] 로 옮길 수 있는 세 갈래.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PolicyWrite {
    /// 교체했고 다시 읽은 디스크가 이것이다(같기를 기대하지만 견주는 것은 부르는 쪽이다).
    Replaced { reread: DiskReading },
    /// 교체 전에 장애로 멈췄다(만들기·쓰기·rename 실패) — 기존 `.gilignore` 는 건드리지 않았다.
    NotReplaced { reason: String },
    /// 교체 직전 재관측이 부르는 쪽이 기대한 디스크와 다른 상태를 발견해 교체하지 않았다 — 장애가 아니라 **조건의 변경**이다.
    ConditionChanged { reread: DiskReading },
    /// 교체 단계에서 실패해 교체 여부를 모른다.
    Unknown { reason: String },
}

/// 원문 `bytes` 로 루트 `.gilignore` 를 교체한다. `expected` 는 교체 직전에 있어야 할 디스크(`None` = 없음).
pub(crate) fn write_root_policy_file(project_root: &Path, bytes: &[u8], expected: Option<&ContentDigest>) -> PolicyWrite {
    write_with(project_root, bytes, expected, &mut |_| Ok(()))
}

/// 시험의 끼어들기 자리: 단계 이름을 받아 오류를 돌려주면 그 자리에서 실패한 것으로 본다.
pub(crate) fn write_with(
    project_root: &Path,
    bytes: &[u8],
    expected: Option<&ContentDigest>,
    fault: &mut dyn FnMut(&'static str) -> std::io::Result<()>,
) -> PolicyWrite {
    let not = |reason: String| PolicyWrite::NotReplaced { reason };
    if !project_root.is_absolute() {
        return not("프로젝트 루트가 절대 경로가 아니다".into());
    }
    let root = match crate::internal_fs::dir_nofollow(project_root) {
        Ok(root) => root,
        Err(error) => return not(format!("프로젝트 루트를 열지 못했다: {error}")),
    };
    let start = match crate::internal_fs::child_dir(&root, c".gil").and_then(|gil| crate::internal_fs::child_dir(&gil, c"start")) {
        Ok(start) => start,
        Err(error) => return not(format!("시작 준비 폴더를 열지 못했다: {error}")),
    };
    let mut nonce = [0u8; 8];
    if getrandom::fill(&mut nonce).is_err() {
        return not("임시 이름을 만들지 못했다".into());
    }
    let temp = CString::new(format!("gilignore.{}.writing", nonce.iter().map(|b| format!("{b:02x}")).collect::<String>()))
        .expect("생성 이름");
    let name = CString::new(crate::artifact::gilignore::GILIGNORE).expect("고정 이름");
    let mut file = match fault("create").and_then(|()| crate::internal_fs::create_file_at(&start, &temp, 0o644)) {
        Ok(file) => file,
        Err(error) => return not(format!("임시 파일을 만들지 못했다: {error}")),
    };
    let written = fault("write")
        .and_then(|()| file.write_all(bytes))
        .and_then(|()| file.flush())
        .and_then(|()| crate::internal_fs::fsync(&file));
    drop(file);
    let discard = || { let _ = crate::internal_fs::unlink_at(&start, &temp); };
    if let Err(error) = written {
        discard();
        return not(format!("임시 파일에 쓰지 못했다: {error}"));
    }
    // 교체 직전의 재관측 — 판정 뒤에 사용자가 고친 파일을 덮어쓰지 않는다.
    let _ = fault("before_recheck");
    let now = super::policy_file_read::read_root_policy_file(project_root);
    let as_expected = match (&now, expected) {
        (DiskReading::Absent, None) => true,
        (DiskReading::Present(found), Some(want)) => found == want,
        _ => false,
    };
    if !as_expected {
        discard();
        return PolicyWrite::ConditionChanged { reread: now };
    }
    // 같은 파일 시스템 안의 두 디렉터리 핸들 사이 rename. 연결 파일을 따라가지 않는다.
    let renamed = fault("rename").and_then(|()| {
        if unsafe { libc::renameat(start.as_raw_fd(), temp.as_ptr(), root.as_raw_fd(), name.as_ptr()) } < 0 {
            Err(std::io::Error::last_os_error())
        } else { Ok(()) }
    });
    if let Err(error) = renamed {
        discard();
        return not(format!("교체하지 못했다: {error}"));
    }
    if let Err(error) = fault("sync").and_then(|()| crate::internal_fs::fsync(&root)) {
        return PolicyWrite::Unknown { reason: format!("교체 뒤 루트를 디스크에 밀어 넣지 못했다: {error}") };
    }
    PolicyWrite::Replaced { reread: super::policy_file_read::read_root_policy_file(project_root) }
}
