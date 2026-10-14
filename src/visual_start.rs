//! N1/N2 development Core entry. No CLI/MCP routing or installed defaults change here.
use super::*;

impl ProjectSession {
    /// Create and durably publish a new image-report Project in an explicit absolute folder.
    /// Unlike `start`, success is already committed. Any existing `.gil` entry is refused,
    /// including an empty directory or remnants of an interrupted attempt. Errors may leave
    /// partial storage or a fully published state: preserve it and inspect read-only; do not retry.
    /// macOS Apple Silicon only until native publication is verified on other platforms.
    pub fn start_visual(rules: RuleSet, root: impl AsRef<Path>) -> Result<Self, SessionError> {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            create(rules, root.as_ref(), &mut |_| Ok(()))
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            let _ = (rules, root);
            Err(SessionError::Action(ActionError::Visual(
                "이미지 프로젝트 생성은 이 플랫폼에서 아직 검수되지 않았다".into(),
            )))
        }
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn create(
    rules: RuleSet,
    root: &Path,
    checkpoint: &mut impl FnMut(&str) -> std::io::Result<()>,
) -> Result<ProjectSession, SessionError> {
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::fs::MetadataExt;
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
    let root = root.canonicalize().map_err(error)?;
    let parent = fs::File::open(&root).map_err(error)?;
    if !parent.metadata().map_err(error)?.is_dir() {
        return Err(error(std::io::Error::other("프로젝트 폴더가 아니다")));
    }
    let mut filesystem = std::mem::MaybeUninit::<libc::statfs>::uninit();
    if unsafe { libc::fstatfs(parent.as_raw_fd(), filesystem.as_mut_ptr()) } < 0 {
        return Err(error(std::io::Error::last_os_error()));
    }
    if unsafe { filesystem.assume_init() }.f_flags & (libc::MNT_LOCAL as u32) == 0 {
        return Err(error(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "이미지 프로젝트 생성은 로컬 파일 시스템에서만 검수됐다",
        )));
    }
    let gil = root.join(".gil");
    // Exclusive reservation, not exists()+create_dir_all(): only one creator may win.
    // In particular, never acquire a lock in, recover, or reap an already existing directory.
    if unsafe { libc::mkdirat(parent.as_raw_fd(), c".gil".as_ptr(), 0o700) } < 0 {
        return Err(error(std::io::Error::last_os_error()));
    }
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            c".gil".as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(error(std::io::Error::last_os_error()));
    }
    let reserved_dir = unsafe { fs::File::from_raw_fd(fd) };
    let reserved = reserved_dir.metadata().map_err(error)?;
    let identity = || -> std::io::Result<()> {
        let named_root = fs::metadata(&root)?;
        let held_root = parent.metadata()?;
        let named = fs::symlink_metadata(&gil)?;
        if !named.is_dir()
            || named.dev() != reserved.dev()
            || named.ino() != reserved.ino()
            || named_root.dev() != held_root.dev()
            || named_root.ino() != held_root.ino()
        {
            return Err(std::io::Error::other("reserved Project identity changed"));
        }
        Ok(())
    };
    parent.sync_all().map_err(error)?;
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
    parent.sync_all().map_err(error)?;
    guarded("completed").map_err(error)?;
    Ok(ProjectSession {
        project,
        state_path,
        _lock: lock,
    })
}

#[cfg(all(test, target_os = "macos", target_arch = "aarch64"))]
#[path = "visual_start_tests.rs"]
mod tests;
