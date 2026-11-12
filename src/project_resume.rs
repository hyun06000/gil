//! Project-local prompts, never Host hooks. Called only before a new initial capture or
//! after a Verify has been durably opened, under the Project lock. Readers never install.
use std::{fs::{self, File}, io::{self, Read, Seek, SeekFrom, Write}, path::Path};
const BLOCK: &str = include_str!("project_resume.md");
const MARKER: &str = "GIL PROJECT RESUME";
const LIMIT: u64 = 24 * 1024; // leave room under the default Host instruction budget

fn codex_file(root: &Path) -> &'static str {
    if fs::symlink_metadata(root.join("AGENTS.override.md")).is_ok() { "AGENTS.override.md" }
    else { "AGENTS.md" }
}
fn names(root: &Path) -> [&'static str; 2] { [codex_file(root), "CLAUDE.md"] }
fn inspect(file: &mut File, name: &str) -> io::Result<bool> {
    let m = file.metadata()?;
    if !m.is_file() || m.len() > LIMIT { return Err(io::Error::other("not a small regular instruction file")); }
    #[cfg(unix)] {
        use std::os::unix::fs::MetadataExt;
        if m.nlink() != 1 { return Err(io::Error::other("linked instruction file")); }
    }
    #[cfg(windows)] if crate::windows_fs::info(file)?.nNumberOfLinks != 1 {
        return Err(io::Error::other("linked instruction file"));
    }
    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::new(); file.take(LIMIT + 1).read_to_end(&mut bytes)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| io::Error::other("instruction encoding requires review"))?;
    if name == "AGENTS.override.md" && text.trim().is_empty() {
        return Err(io::Error::other("empty override must not hide existing AGENTS.md"));
    }
    if text.contains(MARKER) {
        if text.matches(MARKER).count() == 2 && text.contains(BLOCK) { return Ok(true); }
        return Err(io::Error::other("existing or interrupted GIL block requires review; preserved"));
    }
    if bytes.len() + BLOCK.len() + 2 > LIMIT as usize { return Err(io::Error::other("instruction budget requires review")); }
    Ok(false)
}

#[cfg(unix)]
fn access(root: &Path, name: &str, write: bool) -> io::Result<(File, File)> {
    use std::os::fd::{AsRawFd, FromRawFd};
    if !root.is_absolute() { return Err(io::Error::other("absolute project root required")); }
    // Match Core's selected-root identity (e.g. macOS /var -> /private/var),
    // then pin each canonical component; instruction entries themselves never follow links.
    let canonical = root.canonicalize()?;
    let mut dir = File::open("/")?;
    for part in canonical.components() {
        match part {
            std::path::Component::RootDir => {},
            std::path::Component::Normal(part) => {
                use std::os::unix::ffi::OsStrExt;
                let name = std::ffi::CString::new(part.as_bytes()).map_err(|_| io::Error::other("invalid project component"))?;
                dir = crate::internal_fs::child_dir(&dir, &name)?;
            },
            _ => return Err(io::Error::other("normalized project root required")),
        }
    }
    let name = std::ffi::CString::new(name).unwrap();
    let flags = if write {libc::O_RDWR | libc::O_APPEND} else {libc::O_RDONLY};
    let open = |flags| {
        let fd = unsafe {libc::openat(dir.as_raw_fd(), name.as_ptr(), flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK, 0o644)};
        if fd < 0 {Err(io::Error::last_os_error())} else {Ok(unsafe {File::from_raw_fd(fd)})}
    };
    let f = open(flags).or_else(|e| {
        if write && e.kind() == io::ErrorKind::NotFound { open(flags | libc::O_CREAT | libc::O_EXCL) } else { Err(e) }
    })?;
    Ok((dir, f))
}
#[cfg(windows)]
fn access(root: &Path, name: &str, write: bool) -> io::Result<(crate::windows_fs::Chain, File)> {
    use crate::windows_fs as w;
    let chain = w::Chain::open(root)?;
    let access = if write {w::READ_WRITE} else {w::READ};
    let share = if write {windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ} else {w::SHARE_PINNED};
    let f = w::open_entry_at(chain.dir(), name.as_ref(), access, share, Some(false)).or_else(|e| {
        if write && e.kind() == io::ErrorKind::NotFound {w::create_file_at(chain.dir(), name.as_ref(), access, share)} else {Err(e)}
    })?;
    Ok((chain, f))
}
#[cfg(not(any(unix, windows)))]
fn access(_: &Path, _: &str, _: bool) -> io::Result<(File, File)> {Err(io::Error::other("unsupported instruction filesystem"))}

/// Existing bytes are never rewritten/truncated. Interrupted appends are preserved and
/// surfaced for review, not automatically repaired. No backup or registry outside the project.
pub(crate) fn ensure(root: &Path) -> String {
    let mut results = Vec::new();
    for name in names(root) {
        let outcome = (|| -> io::Result<&str> {
            // Inspect without creating first; malformed/link/oversize files remain untouched.
            match access(root, name, false) {
                Ok((_dir, mut f)) => { if inspect(&mut f, name)? { return Ok("ready"); } },
                Err(e) if e.kind() == io::ErrorKind::NotFound => {},
                Err(e) => return Err(e),
            }
            let (_dir, mut f) = access(root, name, true)?;
            f.try_lock().map_err(|_| io::Error::other("instruction file is busy"))?;
            if inspect(&mut f, name)? { return Ok("ready"); }
            // Recheck the named entry and selected directory after taking the file lock.
            #[cfg(unix)] {
                let (now_dir, now_file) = access(root, name, false)?;
                if !crate::internal_fs::same(&_dir, &now_dir)? || !crate::internal_fs::same(&f, &now_file)? {
                    return Err(io::Error::other("instruction path changed concurrently"));
                }
            }
            f.seek(SeekFrom::End(0))?;
            f.write_all(format!("\n\n{BLOCK}").as_bytes())?;
            f.sync_all()?;
            Ok("added; original bytes preserved")
        })();
        results.push(match outcome {Ok(s) => format!("{name}: {s}"), Err(e) => format!("{name}: review_required ({e}); preserve remaining files")});
    }
    crate::tr!("resume.ensured", results = results.join("; "))
}

pub(crate) fn status(root: &Path) -> String {
    let mut pending = Vec::new();
    for name in names(root) {
        let ready = access(root, name, false).and_then(|(_d, mut f)| inspect(&mut f, name));
        match ready {
            Ok(true) => {},
            Ok(false) => pending.push(format!("{name}: missing block")),
            Err(e) => pending.push(format!("{name}: {e}")),
        }
    }
    if pending.is_empty() { crate::locale::text("resume.ready").into() }
    else { crate::tr!("resume.pending", pending = pending.join(", ")) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Temp(std::path::PathBuf);
    impl Temp {fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("gil-resume-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&p).unwrap(); Self(p.canonicalize().unwrap())
    }}
    impl Drop for Temp {fn drop(&mut self) {let _ = fs::remove_dir_all(&self.0);}}
    #[test]
    fn preserves_original_bytes_uses_existing_override_and_never_touches_siblings() {
        let t = Temp::new(); let root = t.0.join("project"); fs::create_dir(&root).unwrap();
        fs::write(t.0.join("AGENTS.md"), b"parent instructions").unwrap();
        fs::write(root.join("AGENTS.md"), b"fallback preserved").unwrap();
        for name in ["AGENTS.override.md", "CLAUDE.md"] {fs::write(root.join(name), b"custom rules\r\nkeep these bytes").unwrap();}
        assert!(ensure(&root).contains("added"));
        for name in names(&root) {
            let bytes = fs::read(root.join(name)).unwrap();
            assert!(bytes.starts_with(b"custom rules\r\nkeep these bytes"));
            assert!(std::str::from_utf8(&bytes).unwrap().contains(BLOCK));
            assert!(ensure(&root).contains("ready"));
            assert_eq!(bytes, fs::read(root.join(name)).unwrap());
        }
        assert_eq!(fs::read(root.join("AGENTS.md")).unwrap(), b"fallback preserved");
        assert_eq!(fs::read(t.0.join("AGENTS.md")).unwrap(), b"parent instructions");
        assert!(!root.join(".claude").exists()); assert!(!root.join(".codex").exists());
    }
    #[test]
    fn readers_do_not_create_guides_and_malformed_or_large_files_are_preserved() {
        let t = Temp::new(); assert!(status(&t.0).contains("pending"));
        assert_eq!(fs::read_dir(&t.0).unwrap().count(),0);
        for bad in [b"custom\n<!-- GIL PROJECT RESUME v1 -->partial".to_vec(), vec![b'x'; LIMIT as usize], vec![255]] {
            fs::write(t.0.join("CLAUDE.md"), &bad).unwrap();
            assert!(ensure(&t.0).contains("review_required"));
            assert_eq!(fs::read(t.0.join("CLAUDE.md")).unwrap(), bad);
        }
    }
    #[test]
    fn empty_override_does_not_hide_existing_guidance() {
        let t = Temp::new();fs::write(t.0.join("AGENTS.override.md"), "").unwrap();
        fs::write(t.0.join("AGENTS.md"), "keep").unwrap();
        assert!(ensure(&t.0).contains("empty override"));
        assert_eq!(fs::read(t.0.join("AGENTS.override.md")).unwrap(),b"");
        assert_eq!(fs::read(t.0.join("AGENTS.md")).unwrap(),b"keep");
    }
    #[test]
    fn contended_instruction_file_is_never_appended() {
        let t = Temp::new();fs::write(t.0.join("CLAUDE.md"), "keep").unwrap();
        let f = File::options().read(true).write(true).open(t.0.join("CLAUDE.md")).unwrap();f.lock().unwrap();
        assert!(ensure(&t.0).contains("review_required"));
        drop(f);
        assert_eq!(fs::read(t.0.join("CLAUDE.md")).unwrap(),b"keep");
    }
    #[cfg(unix)]
    #[test]
    fn symlinks_and_hardlinks_cannot_modify_another_project() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let t = Temp::new();let root=t.0.join("project");fs::create_dir(&root).unwrap();
        let outside=t.0.join("other.md");fs::write(&outside,"other project").unwrap();
        symlink(&outside,root.join("CLAUDE.md")).unwrap();
        fs::hard_link(&outside,root.join("AGENTS.md")).unwrap();
        assert!(ensure(&root).contains("review_required"));
        assert_eq!(fs::read(&outside).unwrap(),b"other project");
        fs::remove_file(root.join("CLAUDE.md")).unwrap();
        fs::create_dir(root.join("CLAUDE.md")).unwrap();
        assert!(ensure(&root).contains("review_required"));
        let alias=t.0.join("alias");symlink(&root,&alias).unwrap();
        assert!(ensure(&alias).contains("review_required"));
        let safe=t.0.join("safe");fs::create_dir(&safe).unwrap();
        fs::write(safe.join("CLAUDE.md"),"private rules").unwrap();
        fs::set_permissions(safe.join("CLAUDE.md"),fs::Permissions::from_mode(0o600)).unwrap();
        assert!(ensure(&safe).contains("added"));
        assert_eq!(fs::metadata(safe.join("CLAUDE.md")).unwrap().permissions().mode() & 0o777,0o600);
        let safe_alias=t.0.join("safe-alias");symlink(&safe,&safe_alias).unwrap();
        let before=fs::read(safe.join("CLAUDE.md")).unwrap();
        assert!(ensure(&safe_alias).contains("ready"));
        assert_eq!(fs::read(safe.join("CLAUDE.md")).unwrap(),before);
    }
}
