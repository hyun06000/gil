//! N1/N2 development Core entry. No CLI/MCP routing or installed defaults change here.
use super::*;

impl ProjectSession {
    /// Create and durably publish a new image-report Project in an explicit absolute folder.
    /// Unlike `start`, success is already committed. Any existing `.gil` entry is refused,
    /// including an empty directory or remnants of an interrupted attempt. Errors may leave
    /// partial storage or a fully published state: preserve it and inspect read-only; do not retry.
    /// macOS Apple Silicon and Windows local fixed NTFS only until native publication is
    /// verified on other platforms (Windows contract v0.1 §4 W1).
    pub fn start_visual(rules: RuleSet, root: impl AsRef<Path>) -> Result<Self, SessionError> {
        #[cfg(any(all(target_os = "macos", target_arch = "aarch64"), windows))]
        {
            create(rules, root.as_ref(), &mut |_| Ok(()))
        }
        #[cfg(not(any(all(target_os = "macos", target_arch = "aarch64"), windows)))]
        {
            let _ = (rules, root);
            Err(SessionError::Action(ActionError::Visual(
                "이미지 프로젝트 생성은 이 플랫폼에서 아직 검수되지 않았다".into(),
            )))
        }
    }
}

#[cfg(any(all(target_os = "macos", target_arch = "aarch64"), windows))]
fn create(
    rules: RuleSet,
    root: &Path,
    checkpoint: &mut impl FnMut(&str) -> std::io::Result<()>,
) -> Result<ProjectSession, SessionError> {
    let error = |source| {
        SessionError::Store(StoreError::Write {
            path: "새 이미지 프로젝트 (실패 시 남은 기록을 보존하고 읽기로 확인한다)".into(),
            source,
        })
    };
    if !root.is_absolute() {
        return Err(error(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "선택한 프로젝트 폴더의 절대 경로가 필요하다",
        )));
    }
    // Exclusive reservation, not exists()+create_dir_all(): only one creator may win.
    // In particular, never acquire a lock in, recover, or reap an already existing directory.
    let reserved = reserve::Reserved::take(root).map_err(error)?;
    let root = reserved.root.clone();
    let gil = root.join(".gil");
    let identity = || reserved.identity();
    reserved.sync_parent().map_err(error)?;
    checkpoint("reserved").map_err(error)?;
    identity().map_err(error)?;
    let lock = ProjectLock::acquire(&gil).map_err(SessionError::from)?;
    let state_path = gil.join("state.yaml");
    identity().map_err(error)?;
    initializable(&gil, &state_path)?;
    let objects = ObjectStore::at(&gil);
    let world = capture(&root, &objects)?;
    checkpoint("captured").map_err(error)?;
    // This is an in-memory initial boundary, not migration of a persisted format-4 state.
    let project = Project::start(rules, world)
        .prepare_visual_upgrade()
        .map_err(SessionError::Action)?;
    let bytes = store::encode(&project).map_err(SessionError::Store)?;
    let mut guarded = |stage: &str| {
        checkpoint(stage)?;
        identity()?;
        lock.validate(&gil.join(LOCK_FILE))
    };
    guarded("encoded").map_err(error)?;
    crate::visual_report::state_file::write_new(&state_path, &bytes, &mut guarded)
        .map_err(error)?;
    reserved.sync_parent().map_err(error)?;
    guarded("completed").map_err(error)?;
    Ok(ProjectSession {
        project,
        state_path,
        _lock: lock,
    })
}

/// The reserved `.gil` entry plus the identity checks that guard the rest of creation.
/// macOS: local mount, `mkdirat` reservation, no-follow descriptor, dev/ino identity.
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod reserve {
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::fs::MetadataExt;
    use std::path::{Path, PathBuf};

    pub(super) struct Reserved {
        pub(super) root: PathBuf,
        parent: std::fs::File,
        _dir: std::fs::File,
        dev: u64,
        ino: u64,
    }

    impl Reserved {
        pub(super) fn take(root: &Path) -> std::io::Result<Self> {
            let root = root.canonicalize()?;
            let parent = std::fs::File::open(&root)?;
            if !parent.metadata()?.is_dir() {
                return Err(std::io::Error::other("프로젝트 폴더가 아니다"));
            }
            let mut filesystem = std::mem::MaybeUninit::<libc::statfs>::uninit();
            if unsafe { libc::fstatfs(parent.as_raw_fd(), filesystem.as_mut_ptr()) } < 0 {
                return Err(std::io::Error::last_os_error());
            }
            if unsafe { filesystem.assume_init() }.f_flags & (libc::MNT_LOCAL as u32) == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "이미지 프로젝트 생성은 로컬 파일 시스템에서만 검수됐다",
                ));
            }
            if unsafe { libc::mkdirat(parent.as_raw_fd(), c".gil".as_ptr(), 0o700) } < 0 {
                return Err(std::io::Error::last_os_error());
            }
            let fd = unsafe {
                libc::openat(
                    parent.as_raw_fd(),
                    c".gil".as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                return Err(std::io::Error::last_os_error());
            }
            let dir = unsafe { std::fs::File::from_raw_fd(fd) };
            let reserved = dir.metadata()?;
            Ok(Self { root, parent, _dir: dir, dev: reserved.dev(), ino: reserved.ino() })
        }
        pub(super) fn identity(&self) -> std::io::Result<()> {
            let named_root = std::fs::metadata(&self.root)?;
            let held_root = self.parent.metadata()?;
            let named = std::fs::symlink_metadata(self.root.join(".gil"))?;
            if !named.is_dir()
                || named.dev() != self.dev
                || named.ino() != self.ino
                || named_root.dev() != held_root.dev()
                || named_root.ino() != held_root.ino()
            {
                return Err(std::io::Error::other("reserved Project identity changed"));
            }
            Ok(())
        }
        pub(super) fn sync_parent(&self) -> std::io::Result<()> {
            self.parent.sync_all()
        }
    }
}

/// Windows (design A): the reservation is created and first held by one `NtCreateFile` call
/// relative to the pinned Project root, so there is no name resolution between creating `.gil`
/// and holding it. Normalization, verification and failure cleanup use that same handle. The
/// handle transition that follows (DELETE access → read-only pinned handle) is detection, not
/// prevention: nothing is written during it, and a mismatch refuses without touching either object.
#[cfg(windows)]
mod reserve {
    use crate::windows_fs as win;
    use std::ffi::OsStr;
    use std::path::{Path, PathBuf};

    /// Test-only fault and observation points. Not compiled into product builds.
    #[cfg(test)]
    pub(crate) mod hooks {
        use std::cell::Cell;
        use std::fs::File;
        use std::path::Path;
        thread_local! {
            /// Observe the freshly created reservation before normalization.
            pub static BEFORE_NORMALIZE: Cell<Option<fn(&File)>> = const { Cell::new(None) };
            /// Force the post-normalization verification to fail.
            pub static FAIL_VERIFY: Cell<bool> = const { Cell::new(false) };
            /// Act on the name while no delete-denying handle is held (handle transition).
            pub static DURING_TRANSITION: Cell<Option<fn(&Path)>> = const { Cell::new(None) };
        }
    }

    pub(super) struct Reserved {
        pub(super) root: PathBuf,
        parent: win::Chain,
        _dir: std::fs::File,
        reserved: win::Id,
        parent_id: win::Id,
    }

    impl Reserved {
        pub(super) fn take(root: &Path) -> std::io::Result<Self> {
            // The user-selected path itself must not pass through any reparse point
            // (Distribution §6.2b); only then is the canonical form used for the chain.
            let selected = win::Chain::open(root)?;
            let root = root.canonicalize()?;
            if !std::fs::metadata(&root)?.is_dir() {
                return Err(std::io::Error::other("프로젝트 폴더가 아니다"));
            }
            let parent = win::Chain::open(&root)?;
            if win::id(parent.dir())? != win::id(selected.dir())? {
                return Err(win::refused());
            }
            drop(selected);
            win::local_fixed_ntfs(&root, parent.dir()).map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "이미지 프로젝트 생성은 로컬 고정 NTFS 드라이브에서만 검수됐다",
                )
            })?;
            let name = OsStr::new(".gil");
            // 1. Exclusive creation with the owner-only descriptor; the handle is to the object
            //    this call created. Any existing entry fails here untouched.
            let stage = |at: &'static str| move |e: std::io::Error| std::io::Error::new(e.kind(), format!("{at}: {e}"));
            let fresh = win::create_private_dir_at(parent.dir(), name).map_err(stage("reserve/create"))?;
            #[cfg(test)]
            if let Some(observe) = hooks::BEFORE_NORMALIZE.with(|c| c.get()) {
                observe(&fresh);
            }
            // 2. Normalize and verify through the same handle.
            let verified = win::set_private_dacl_on(&fresh).map_err(stage("reserve/normalize")).and_then(|()| win::private_dir(&fresh).map_err(stage("reserve/verify")));
            #[cfg(test)]
            let verified = if hooks::FAIL_VERIFY.with(|c| c.get()) {
                Err(std::io::Error::other("injected verification failure"))
            } else {
                verified
            };
            if let Err(problem) = verified {
                // 3. Only the object behind `fresh` (ours, exclusive, empty) is removed; never a name.
                let _ = win::delete(&fresh);
                return Err(std::io::Error::other(format!(
                    "생성한 .gil 의 접근 권한이 소유자 전용으로 확인되지 않아 제거하고 거절했다: {problem}"
                )));
            }
            let born = win::id(&fresh).map_err(stage("reserve/identity"))?;
            // 4. Handle transition (detection, not prevention). `fresh` holds DELETE access, which
            //    would collide with the delete-unshared pins used by the lock and state writes.
            let keep = win::reopen(&fresh, win::READ, win::SHARE_ALL).map_err(stage("reserve/reopen"))?;
            drop(fresh);
            #[cfg(test)]
            if let Some(act) = hooks::DURING_TRANSITION.with(|c| c.get()) {
                act(&root.join(".gil"));
            }
            let pinned = win::open_dir_at(parent.dir(), name, win::SHARE_PINNED).map_err(stage("reserve/pin"))?;
            if win::id(&pinned)? != born || win::id(&keep)? != born {
                // Replaced while no delete-denying handle was held: refuse. Nothing was written,
                // nothing is cleaned by name, and the object now under the name is not modified.
                return Err(std::io::Error::other(
                    "예약한 .gil 이 핸들 전환 중 다른 항목으로 바뀌었다. 아무것도 쓰거나 지우지 않고 거절한다",
                ));
            }
            drop(keep);
            let parent_id = win::id(parent.dir())?;
            Ok(Self { root, parent, _dir: pinned, reserved: born, parent_id })
        }
        pub(super) fn identity(&self) -> std::io::Result<()> {
            let named = win::open_dir(&self.root.join(".gil"), win::SHARE_ALL, false)?;
            let named_root = win::open_dir(&self.root, win::SHARE_ALL, false)?;
            if win::id(&named)? != self.reserved || win::id(&named_root)? != self.parent_id {
                return Err(std::io::Error::other("reserved Project identity changed"));
            }
            Ok(())
        }
        pub(super) fn sync_parent(&self) -> std::io::Result<()> {
            win::flush_dir(self.parent.path(), self.parent.dir())
        }
    }
}

#[cfg(all(test, target_os = "macos", target_arch = "aarch64"))]
#[path = "visual_start_tests.rs"]
mod tests;
#[cfg(all(test, windows))]
#[path = "visual_start_tests_windows.rs"]
mod tests;
