//! No-follow reads of recovery control files and backups. Destination writes are separate.
use super::*;
use crate::internal_fs as guarded;
use std::ffi::CString;
use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};

fn transaction(active: &Path) -> io::Result<File> {
    let restore = active
        .parent()
        .ok_or_else(|| io::Error::other("missing restore parent"))?;
    let gil = restore
        .parent()
        .ok_or_else(|| io::Error::other("missing .gil parent"))?;
    if restore.file_name() != Some(std::ffi::OsStr::new(RESTORE)) {
        return Err(io::Error::other("invalid restore area"));
    }
    let dir = guarded::directory(gil, false)?;
    let dir = guarded::child_dir(&dir, c"restore")?;
    let name = CString::new(
        active
            .file_name()
            .ok_or_else(|| io::Error::other("missing transaction"))?
            .as_bytes(),
    )?;
    guarded::child_dir(&dir, &name)
}
fn single(file: File) -> io::Result<File> {
    if file.metadata()?.nlink() != 1 {
        return Err(io::Error::other("linked recovery file"));
    }
    Ok(file)
}
pub(super) fn control(active: &Path, name: &str) -> io::Result<File> {
    if !matches!(name, PLAN | COMMITTED) {
        return Err(io::Error::other("unknown control file"));
    }
    single(guarded::regular_at(
        &transaction(active)?,
        &CString::new(name)?,
    )?)
}
pub(super) fn backup(active: &Path, id: u64) -> io::Result<File> {
    let dir = guarded::child_dir(&transaction(active)?, c"backup")?;
    single(guarded::regular_at(&dir, &CString::new(id.to_string())?)?)
}
