//! Explicit relative attachment paths. Unix resolution is descriptor-relative and no-follow;
//! Windows opens every component as the entry itself under pinned ancestors (Windows contract §4).
//! Other platforms fail closed until an equivalent handle-based implementation is tested.
use super::image::VerifiedImage;
use crate::artifact::EntryPath;
use std::fs::File;
use std::io::{Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// What must stay equal between the retained handle and its current name.
#[cfg(unix)]
type Stamp = std::fs::Metadata;
#[cfg(windows)]
type Stamp = crate::windows_fs::Stamp;
#[cfg(not(any(unix, windows)))]
type Stamp = ();

fn stamp(file: &File) -> Result<Stamp, String> {
    #[cfg(unix)]
    {
        file.metadata().map_err(|_| refused())
    }
    #[cfg(windows)]
    {
        crate::windows_fs::Stamp::of(file).map_err(|_| refused())
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = file;
        Err(refused())
    }
}

pub(crate) struct AttachmentFile {
    root: PathBuf,
    relative: String,
    file: File,
    before: Stamp,
    pub(crate) image: VerifiedImage,
}

fn refused() -> String {
    "첨부 파일 경계 또는 안정성 검사를 통과하지 못했다".into()
}

pub(crate) fn relative(text: &str) -> Result<EntryPath, String> {
    let parsed = EntryPath::parse(text).ok_or_else(refused)?;
    for part in text.split('/') {
        let upper = part.to_ascii_uppercase();
        let stem = upper.split('.').next().unwrap_or("");
        if part.chars().any(|c| c.is_control() || c == ':')
            || part.ends_with(['.', ' '])
            || part.eq_ignore_ascii_case(".gil")
            || part.eq_ignore_ascii_case(".git")
            || matches!(stem, "CON" | "PRN" | "AUX" | "NUL")
            || (stem.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.as_bytes()[3].is_ascii_digit())
        {
            return Err(refused());
        }
    }
    Ok(parsed)
}

impl AttachmentFile {
    pub(crate) fn read(root: &Path, text: &str) -> Result<Self, String> {
        relative(text)?;
        Self::read_at(root, text)
    }

    fn read_at(root: &Path, text: &str) -> Result<Self, String> {
        let root = root.canonicalize().map_err(|_| refused())?;
        let mut file = open(&root, text)?;
        let before = stamp(&file)?;
        let image = VerifiedImage::read(&mut file).map_err(|e| e.to_string())?;
        let held = Self {
            root,
            relative: text.into(),
            file,
            before,
            image,
        };
        held.check_metadata()?;
        Ok(held)
    }
    fn check_metadata(&self) -> Result<(), String> {
        let now = stamp(&self.file)?;
        let named = stamp(&open(&self.root, &self.relative)?)?;
        if same(&self.before, &now) && same(&self.before, &named) {
            Ok(())
        } else {
            Err(refused())
        }
    }
    /// Re-read the retained handle and check the current path still names that same file.
    pub(crate) fn revalidate(&mut self) -> Result<(), String> {
        self.check_metadata()?;
        self.file.seek(SeekFrom::Start(0)).map_err(|_| refused())?;
        let again = VerifiedImage::read(&mut self.file).map_err(|e| e.to_string())?;
        if again.bytes() != self.image.bytes() {
            return Err(refused());
        }
        self.check_metadata()
    }
}

/// Internal names are derived only from validated registry metadata, never caller paths.
/// This guards the read itself; it does not secure Session opening or storage writes.
pub(crate) fn read_stored(
    root: &Path,
    metadata: &super::registry::NewVisualAsset,
) -> Result<VerifiedImage, String> {
    use super::image::MAX_ENCODED_BYTES;
    use super::registry::ImageKind;
    if metadata.image_kind != ImageKind::Png || metadata.byte_len.get() > MAX_ENCODED_BYTES as u64 {
        return Err(refused());
    }
    let hex: String = metadata.digest.iter().map(|b| format!("{b:02x}")).collect();
    let name = format!(".gil/artifacts/blobs/sha256/{}/{}", &hex[..2], &hex[2..]);
    let mut held = AttachmentFile::read_at(root, &name)?;
    if held
        .image
        .metadata(metadata.source_kind, metadata.source_description.clone())
        .map_err(|e| e.to_string())?
        != *metadata
    {
        return Err("저장된 이미지 내용이 Report의 기록과 일치하지 않는다".into());
    }
    held.revalidate()?;
    Ok(held.image)
}

#[cfg(unix)]
fn same(a: &Stamp, b: &Stamp) -> bool {
    use std::os::unix::fs::MetadataExt;
    a.is_file()
        && b.is_file()
        && a.nlink() == 1
        && b.nlink() == 1
        && a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}
#[cfg(windows)]
fn same(a: &Stamp, b: &Stamp) -> bool {
    a == b && a.is_single_regular()
}
#[cfg(not(any(unix, windows)))]
fn same(_: &Stamp, _: &Stamp) -> bool {
    false
}

#[cfg(unix)]
fn open(root: &Path, text: &str) -> Result<File, String> {
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::MetadataExt;
    use std::path::Component;
    if !root.is_absolute() {
        return Err(refused());
    }
    let mut dir = File::open("/").map_err(|_| refused())?;
    let mut parts: Vec<_> = root
        .components()
        .filter_map(|c| match c {
            Component::Normal(p) => Some(p),
            _ => None,
        })
        .collect();
    parts.extend(Path::new(text).components().filter_map(|c| match c {
        Component::Normal(p) => Some(p),
        _ => None,
    }));
    let end = parts.len().checked_sub(1).ok_or_else(refused)?;
    for (index, part) in parts.iter().enumerate() {
        let name = std::ffi::CString::new(part.as_bytes()).map_err(|_| refused())?;
        let flags = libc::O_RDONLY
            | libc::O_CLOEXEC
            | libc::O_NOFOLLOW
            | libc::O_NONBLOCK
            | if index != end { libc::O_DIRECTORY } else { 0 };
        // Each component is opened relative to a retained directory descriptor; no symlink traversal.
        let fd = unsafe { libc::openat(dir.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err(refused());
        }
        // Ownership of a successful openat descriptor transfers exactly once to File.
        dir = unsafe { File::from_raw_fd(fd) };
    }
    let meta = dir.metadata().map_err(|_| refused())?;
    if !meta.is_file() || meta.nlink() != 1 {
        return Err(refused());
    }
    Ok(dir)
}
/// Windows: pin each directory of the canonical root and of the relative path without following
/// reparse points, then open the final entry itself. Sharing is not restricted (like Unix);
/// changes are detected through `Stamp` and the byte re-read instead.
#[cfg(windows)]
fn open(root: &Path, text: &str) -> Result<File, String> {
    use crate::windows_fs as win;
    use std::path::Component;
    let mut chain = win::Chain::open(root).map_err(|_| refused())?;
    let parts: Vec<_> = Path::new(text)
        .components()
        .filter_map(|c| match c {
            Component::Normal(p) => Some(p),
            _ => None,
        })
        .collect();
    let (last, dirs) = parts.split_last().ok_or_else(refused)?;
    for part in dirs {
        chain.push(part, false).map_err(|_| refused())?;
    }
    let at = chain.child(last).map_err(|_| refused())?;
    let file = win::open_file(&at, win::READ, win::SHARE_ALL, windows_sys::Win32::Storage::FileSystem::OPEN_EXISTING)
        .map_err(|_| refused())?;
    if !win::Stamp::of(&file).map_err(|_| refused())?.is_single_regular() {
        return Err(refused());
    }
    // Stored objects under .gil must still carry only the inherited owner-only ACEs.
    if parts.first().is_some_and(|p| p.eq_ignore_ascii_case(".gil")) {
        win::inherited_only(&file, false).map_err(|_| refused())?;
    }
    Ok(file)
}
#[cfg(not(any(unix, windows)))]
fn open(_: &Path, _: &str) -> Result<File, String> {
    Err("이 플랫폼의 안전한 이미지 첨부 경로는 아직 검수되지 않았다".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn path_contract_is_shared_across_platforms() {
        for bad in [
            "",
            ".",
            "..",
            "../a.png",
            "/a.png",
            "a//b",
            "./a",
            "a/../b",
            "a\\b",
            "a:b",
            ".gil/a",
            "a/.GIT/config",
            "NUL.png",
            "COM1.png",
            "a. ",
            "a\n.png",
            "a\0.png",
        ] {
            assert!(relative(bad).is_err(), "{bad:?}");
        }
        assert!(relative("화면 자료/결과.png").is_ok());
    }
    #[cfg(not(any(unix, windows)))]
    #[test]
    fn unsupported_platform_never_opens_a_source_file() {
        assert!(open(Path::new("C:/project"), "screen.png").is_err());
    }
}
