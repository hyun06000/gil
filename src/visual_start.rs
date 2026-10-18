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

/// Windows: local fixed NTFS only; `CreateDirectoryW` is the exclusive reservation and the
/// reserved entry is opened as itself (never a reparse point). The root's ancestors stay pinned
/// for the creation, and identity is the volume serial plus file id of both entries.
#[cfg(windows)]
mod reserve {
    use crate::windows_fs as win;
    use std::ffi::OsStr;
    use std::path::{Path, PathBuf};

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
            // (Distribution §6.2b; CI run 39125709523 showed canonicalize() resolving a junction
            // root into its target). Only then is the canonical form used for the chain.
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
            let gil = parent.child(OsStr::new(".gil"))?;
            // Any existing entry (file, directory, link, remnant) fails here; nothing is reused.
            // The reservation is born owner-only (current user + SYSTEM, protected DACL).
            win::create_dir_exclusive_private(&gil)?;
            // Normalize through SetSecurityInfo so the descriptor carries the auto-inherit
            // control and children are flagged inherited (CI run 39129041207), then verify the
            // effective descriptor through a handle.
            let deletable = win::open_dir_deletable(&gil)?;
            if let Err(problem) = win::set_private_dacl(&gil).and_then(|()| win::private_dir(&deletable)) {
                // Ours, exclusive and still empty: remove it through the handle (never a name
                // that could have been replaced) and refuse. Nothing else was written.
                let _ = win::delete(&deletable);
                return Err(std::io::Error::other(format!(
                    "생성한 .gil 의 접근 권한이 소유자 전용으로 확인되지 않아 제거하고 거절했다: {problem}"
                )));
            }
            // Keep only a read handle: a retained DELETE access would collide with the pinned
            // (delete-unshared) opens of the lock and state writes (CI run 39127711288).
            let dir = win::open_dir(&gil, win::SHARE_ALL, false)?;
            if win::id(&dir)? != win::id(&deletable)? {
                return Err(win::refused());
            }
            drop(deletable);
            let reserved = win::id(&dir)?;
            let parent_id = win::id(parent.dir())?;
            Ok(Self { root, parent, _dir: dir, reserved, parent_id })
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
