//! Product-facing Project record placement.
//!
//! Existing inline Projects remain readable. New Windows visual-authoring Projects keep their
//! private records below LOCALAPPDATA so OneDrive and other working-folder providers cannot
//! rewrite the `.gil` DACL. The working tree itself never moves.

use std::path::{Path, PathBuf};

#[cfg(windows)]
use sha2::{Digest, Sha256};

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) const EXTERNAL_LAYOUT: &str = "project-state-v1";

/// Existing external records win over legacy inline records. If neither exists, the inline path
/// is returned on non-Windows platforms and the future external path on Windows.
pub(crate) fn state_for_root(root: &Path) -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        let inline = root.join(crate::STATE_PATH);
        let external = match external_state(root) {
            Ok(path) => path,
            Err(_) if inline.is_file() => return Ok(inline),
            Err(error) => return Err(error),
        };
        if external.is_file() || external.parent().is_some_and(Path::exists) {
            return Ok(external);
        }
        if inline.is_file() {
            return Ok(inline);
        }
        Ok(external)
    }
    #[cfg(not(windows))]
    {
        Ok(root.join(crate::STATE_PATH))
    }
}

#[cfg(windows)]
pub(crate) fn external_storage_root(root: &Path) -> Result<PathBuf, String> {
    let local = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| "Windows 개인 앱 데이터 위치를 찾지 못했다".to_string())?;
    external_storage_root_in(root, &local)
}

#[cfg(windows)]
fn external_state(root: &Path) -> Result<PathBuf, String> {
    Ok(external_storage_root(root)?.join(crate::STATE_PATH))
}

/// Copy an existing inline record into the Windows private app-data store without changing or
/// deleting the source. Publication is one directory rename after a byte-for-byte comparison;
/// an interrupted copy is therefore never selected as the Project's record.
#[cfg(windows)]
#[cfg_attr(not(feature = "visual-authoring"), allow(dead_code))]
pub(crate) fn externalize_inline_if_needed(root: &Path) -> Result<bool, String> {
    use std::collections::BTreeMap;
    use std::ffi::OsStr;
    use std::fs::{File, OpenOptions};
    use std::io::{Read, Write};
    use std::os::windows::fs::MetadataExt;
    use std::time::{SystemTime, UNIX_EPOCH};
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

    const MAX_ENTRIES: usize = 100_000;
    const MAX_BYTES: u64 = 512 * 1024 * 1024;

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Entry {
        Directory,
        File { bytes: u64, sha256: [u8; 32] },
    }

    fn checked_metadata(path: &Path) -> Result<std::fs::Metadata, String> {
        let metadata =
            std::fs::symlink_metadata(path).map_err(|e| format!("기존 기록을 읽지 못했다: {e}"))?;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err("기존 기록 안에 링크 또는 재분석 지점이 있어 자동 이관하지 않았다".into());
        }
        Ok(metadata)
    }

    fn walk(
        base: &Path,
        here: &Path,
        copy_to: Option<&Path>,
        entries: &mut BTreeMap<PathBuf, Entry>,
        total: &mut u64,
    ) -> Result<(), String> {
        let mut children = std::fs::read_dir(here)
            .map_err(|e| format!("기존 기록을 열거하지 못했다: {e}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("기존 기록을 열거하지 못했다: {e}"))?;
        children.sort_by_key(|entry| entry.file_name());
        for child in children {
            let source = child.path();
            let relative = source
                .strip_prefix(base)
                .map_err(|_| "기존 기록 경로가 저장소 밖으로 벗어났다".to_string())?
                .to_path_buf();
            if entries.len() >= MAX_ENTRIES {
                return Err("기존 기록 항목이 너무 많아 자동 이관하지 않았다".into());
            }
            let metadata = checked_metadata(&source)?;
            if metadata.is_dir() {
                if let Some(destination) = copy_to {
                    std::fs::create_dir(destination.join(&relative))
                        .map_err(|e| format!("개인 저장소 폴더를 만들지 못했다: {e}"))?;
                }
                entries.insert(relative.clone(), Entry::Directory);
                walk(base, &source, copy_to, entries, total)?;
            } else if metadata.is_file() {
                *total = total
                    .checked_add(metadata.len())
                    .ok_or_else(|| "기존 기록 크기를 계산하지 못했다".to_string())?;
                if *total > MAX_BYTES {
                    return Err("기존 기록이 512 MiB를 넘어 자동 이관하지 않았다".into());
                }
                let mut input = File::open(&source)
                    .map_err(|e| format!("기존 기록 파일을 열지 못했다: {e}"))?;
                let mut output = if let Some(destination) = copy_to {
                    Some(
                        OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(destination.join(&relative))
                            .map_err(|e| format!("개인 저장소 파일을 만들지 못했다: {e}"))?,
                    )
                } else {
                    None
                };
                let mut digest = Sha256::new();
                let mut copied = 0_u64;
                let mut buffer = [0_u8; 64 * 1024];
                loop {
                    let n = input
                        .read(&mut buffer)
                        .map_err(|e| format!("기존 기록 파일을 읽지 못했다: {e}"))?;
                    if n == 0 {
                        break;
                    }
                    digest.update(&buffer[..n]);
                    copied += n as u64;
                    if let Some(file) = output.as_mut() {
                        file.write_all(&buffer[..n])
                            .map_err(|e| format!("개인 저장소 파일을 쓰지 못했다: {e}"))?;
                    }
                }
                if copied != metadata.len() {
                    return Err("이관 중 기존 기록 파일의 크기가 바뀌었다".into());
                }
                if let Some(file) = output.as_mut() {
                    file.sync_all()
                        .map_err(|e| format!("개인 저장소 파일을 확정하지 못했다: {e}"))?;
                }
                entries.insert(
                    relative,
                    Entry::File {
                        bytes: copied,
                        sha256: digest.finalize().into(),
                    },
                );
            } else {
                return Err(
                    "기존 기록 안에 일반 파일이나 폴더가 아닌 항목이 있어 자동 이관하지 않았다"
                        .into(),
                );
            }
        }
        Ok(())
    }

    let inline = root.join(crate::STATE_PATH);
    if !inline.is_file() {
        return Ok(false);
    }
    let declared = crate::windows_fs::read_state(&inline)
        .map_err(|e| format!("기존 기록 형식을 확인하지 못해 자동 이관하지 않았다: {e}"))?;
    let format = crate::store::declared_format(&declared, &inline)
        .map_err(|e| format!("기존 기록 형식을 확인하지 못해 자동 이관하지 않았다: {e}"))?;
    if format != crate::VISUAL_FORMAT {
        return Ok(false);
    }
    let source = inline.parent().expect("inline state has a parent");
    if !source.join(crate::lock::LOCK_FILE).is_file() {
        return Err("기존 기록의 잠금 파일이 없어 자동 이관하지 않았다".into());
    }
    // The old ACL may be broader than GIL accepts, but the ordinary GIL advisory lock is still
    // safe to take through the no-follow control-file path. Holding it closes the final race
    // between the source re-scan and publishing the verified copy.
    let _source_lock = crate::lock::ProjectLock::acquire(source)
        .map_err(|e| format!("기존 기록이 사용 중이라 자동 이관하지 않았다: {e}"))?;
    let final_root = external_storage_root(root)?;
    if final_root.exists() {
        return Ok(false);
    }
    let parent = final_root
        .parent()
        .ok_or_else(|| "Windows 개인 저장소 상위 폴더가 없다".to_string())?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("Windows 개인 저장소 상위 폴더를 만들지 못했다: {e}"))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "시스템 시간을 읽지 못했다".to_string())?
        .as_nanos();
    let staging = parent.join(format!(".migrating-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&staging)
        .map_err(|e| format!("Windows 개인 저장소 준비 폴더를 만들지 못했다: {e}"))?;
    let held_parent = File::open(&staging)
        .map_err(|e| format!("Windows 개인 저장소 준비 폴더를 열지 못했다: {e}"))?;
    let held_gil = crate::windows_fs::create_private_dir_at(&held_parent, OsStr::new(".gil"))
        .map_err(|e| format!("Windows 개인 기록 폴더를 만들지 못했다: {e}"))?;
    crate::windows_fs::set_private_dacl_on(&held_gil)
        .map_err(|e| format!("Windows 개인 기록 폴더를 보호하지 못했다: {e}"))?;
    crate::windows_fs::private_dir(&held_gil)
        .map_err(|e| format!("Windows 개인 기록 폴더 보호를 확인하지 못했다: {e}"))?;
    drop(held_gil);
    drop(held_parent);

    let destination = staging.join(".gil");
    let mut copied = BTreeMap::new();
    let mut copied_bytes = 0;
    walk(
        source,
        source,
        Some(&destination),
        &mut copied,
        &mut copied_bytes,
    )?;
    let mut source_after = BTreeMap::new();
    let mut source_bytes = 0;
    walk(source, source, None, &mut source_after, &mut source_bytes)?;
    let mut destination_after = BTreeMap::new();
    let mut destination_bytes = 0;
    walk(
        &destination,
        &destination,
        None,
        &mut destination_after,
        &mut destination_bytes,
    )?;
    if copied != source_after
        || copied != destination_after
        || copied_bytes != source_bytes
        || copied_bytes != destination_bytes
    {
        return Err(
            "이관 중 기존 기록이 바뀌었거나 복사본이 일치하지 않아 원본을 그대로 보존하고 중단했다"
                .into(),
        );
    }
    crate::windows_fs::verify_storage_tree(&destination)
        .map_err(|e| format!("복사한 개인 기록의 보호를 확인하지 못했다: {e}"))?;
    std::fs::rename(&staging, &final_root).map_err(|e| {
        format!("검증한 개인 기록을 게시하지 못했다(기존 기록은 그대로 보존했다): {e}")
    })?;
    Ok(true)
}

#[cfg(windows)]
fn external_storage_root_in(root: &Path, local: &Path) -> Result<PathBuf, String> {
    let canonical = root
        .canonicalize()
        .map_err(|e| format!("프로젝트 폴더의 실제 위치를 확인하지 못했다: {e}"))?;
    if !canonical.is_dir() {
        return Err("프로젝트 자리가 폴더가 아니다".into());
    }
    // `canonicalize` supplies the filesystem's canonical spelling. Hash its UTF-16 code units
    // rather than a lossy UTF-8 rendering so every valid Windows path has a stable identity.
    use std::os::windows::ffi::OsStrExt;
    let mut identity = Vec::new();
    for unit in canonical.as_os_str().encode_wide() {
        identity.extend_from_slice(&unit.to_le_bytes());
    }
    let key = format!("{:x}", Sha256::digest(&identity));
    Ok(local.join("GIL").join(EXTERNAL_LAYOUT).join(key))
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn the_same_project_path_has_one_external_storage_root() {
        let scratch =
            std::env::temp_dir().join(format!("gil-external-state-{}", std::process::id()));
        let root = scratch.join("Project");
        std::fs::create_dir_all(&root).unwrap();
        let base = scratch.join("LocalAppData");
        let first = external_storage_root_in(&root, &base).unwrap();
        let second = external_storage_root_in(&root, &base).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.parent().unwrap(),
            base.join("GIL").join(EXTERNAL_LAYOUT)
        );
        assert_eq!(first.file_name().unwrap().to_string_lossy().len(), 64);
        std::fs::remove_dir_all(scratch).ok();
    }
}
