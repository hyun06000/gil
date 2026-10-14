//! Validate the complete known transaction shape before deleting any entry.
//! Operations stay relative to held directories; no recursive pathname deletion.
use super::*;
use crate::internal_fs as guarded;
use std::ffi::CString;
use std::os::fd::AsRawFd;
use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};

fn refused() -> io::Error {
    io::Error::other("unknown or replaced cleanup entry")
}
struct Entry {
    name: CString,
    device: u64,
    inode: u64,
}
fn inspect(dir: &File, name: CString) -> io::Result<Entry> {
    let file = guarded::regular_at(dir, &name)?;
    let meta = file.metadata()?;
    Ok(Entry {
        name,
        device: meta.dev(),
        inode: meta.ino(),
    })
}
fn unlink(dir: &File, entry: &Entry) -> io::Result<()> {
    let file = guarded::regular_at(dir, &entry.name)?;
    let meta = file.metadata()?;
    if meta.dev() != entry.device || meta.ino() != entry.inode {
        return Err(refused());
    }
    guarded::unlink_at(dir, &entry.name)
}
fn remove_dir(parent: &File, name: &CString, held: &File) -> io::Result<()> {
    if !guarded::same(held, &guarded::child_dir(parent, name)?)? {
        return Err(refused());
    }
    if unsafe { libc::unlinkat(parent.as_raw_fd(), name.as_ptr(), libc::AT_REMOVEDIR) } < 0 {
        return Err(io::Error::last_os_error());
    }
    parent.sync_all()
}
pub(super) fn remove(at: &Path) -> io::Result<()> {
    let area = at.parent().ok_or_else(refused)?;
    if area.file_name() != Some(std::ffi::OsStr::new(RESTORE)) {
        return Err(refused());
    }
    let gil = guarded::directory(area.parent().ok_or_else(refused)?, false)?;
    let parent = guarded::child_dir(&gil, c"restore")?;
    let name = CString::new(at.file_name().ok_or_else(refused)?.as_bytes())?;
    let dir = match guarded::child_dir(&parent, &name) {
        Ok(dir) => dir,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    let mut controls = Vec::new();
    let mut backups = None;
    for child in guarded::names(&dir)? {
        match child.to_str().map_err(|_| refused())? {
            PLAN | COMMITTED => controls.push(inspect(&dir, child)?),
            BACKUP => {
                let held = guarded::child_dir(&dir, &child)?;
                let mut files = Vec::new();
                for file in guarded::names(&held)? {
                    if file.to_bytes().is_empty() || !file.to_bytes().iter().all(u8::is_ascii_digit)
                    {
                        return Err(refused());
                    }
                    files.push(inspect(&held, file)?);
                }
                backups = Some((held, files));
            }
            _ => return Err(refused()),
        }
    }
    // No deletes occurred during the complete shape validation above.
    if !guarded::same(&parent, &guarded::child_dir(&gil, c"restore")?)?
        || !guarded::same(&dir, &guarded::child_dir(&parent, &name)?)?
    {
        return Err(refused());
    }
    if let Some((held, files)) = backups {
        if !guarded::same(&held, &guarded::child_dir(&dir, c"backup")?)? {
            return Err(refused());
        }
        for file in &files {
            unlink(&held, file)?;
        }
        held.sync_all()?;
        remove_dir(&dir, &CString::new(BACKUP)?, &held)?;
    }
    for file in &controls {
        unlink(&dir, file)?;
    }
    dir.sync_all()?;
    remove_dir(&parent, &name, &dir)
}
