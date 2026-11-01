// New visual creation is format 5 normally and format 7 with real Chain containment.
const VISUAL_HEAD: &[u8] = if cfg!(feature = "chain-core") { b"format: 7\n" } else { b"format: 5\n" };

use super::*;
use std::collections::BTreeMap;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::sync::atomic::{AtomicU64, Ordering};

fn rules() -> RuleSet {
    RuleSet::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/spec/gil-spec.yaml")).unwrap()
}
fn scratch() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "gil-visual-start-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    root
}
// Include directories, file permissions and symlink targets, not just state.yaml.
fn inventory(root: &Path) -> BTreeMap<PathBuf, (u32, Vec<u8>)> {
    fn walk(root: &Path, path: &Path, out: &mut BTreeMap<PathBuf, (u32, Vec<u8>)>) {
        for entry in fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            let meta = fs::symlink_metadata(&path).unwrap();
            let bytes = if meta.is_symlink() {
                fs::read_link(&path)
                    .unwrap()
                    .as_os_str()
                    .as_encoded_bytes()
                    .to_vec()
            } else if meta.is_file() {
                fs::read(&path).unwrap()
            } else {
                Vec::new()
            };
            out.insert(
                path.strip_prefix(root).unwrap().to_owned(),
                (meta.mode(), bytes),
            );
            if meta.is_dir() {
                walk(root, &path, out);
            }
        }
    }
    let mut result = BTreeMap::new();
    walk(root, root, &mut result);
    result
}
fn assert_initial(root: &Path) {
    let bytes = fs::read(root.join(crate::STATE_PATH)).unwrap();
    assert!(bytes.starts_with(VISUAL_HEAD));
    let read = ProjectSession::open_read_only(rules(), root.join(crate::STATE_PATH)).unwrap();
    assert_eq!(read.project().current_cycle().kind(), CycleKind::Interview);
    let value: serde_norway::Value = serde_norway::from_slice(&bytes).unwrap();
    assert!(read.project().visual_state().is_some());
    assert_eq!(read.project().current_chain().is_some(), cfg!(feature = "chain-core"));
    assert_eq!(
        read.project().visual_state().unwrap().registry(),
        &crate::visual_report::registry::VisualRegistry::default()
    );
    assert_eq!(read.project().cycles().nodes().len(), 1);
    assert_eq!(read.project().current_cycle().steps().nodes().len(), 0);
    assert!(value["visuals"].is_mapping());
}

#[test]
fn first_publication_is_visual_and_existing_git_files_are_unchanged() {
    for git in [false, true] {
        let root = scratch();
        if git {
            fs::create_dir(root.join(".git")).unwrap();
            fs::write(root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
            fs::write(
                root.join(".git/config"),
                "[core]\n\trepositoryformatversion = 0\n",
            )
            .unwrap();
            fs::write(root.join(".gitignore"), "build/\n").unwrap();
            fs::write(root.join("한글 작업.txt"), "기존 내용\n").unwrap();
        }
        let before = inventory(&root);
        let mut before_publish = false;
        let session = create(rules(), &root, &root, true, &mut |stage| {
            if stage == "before_publish" {
                before_publish = true;
                assert!(!root.join(crate::STATE_PATH).exists());
                assert!(
                    fs::read(root.join(".gil/state.yaml.writing"))?.starts_with(VISUAL_HEAD)
                );
            }
            Ok(())
        })
        .unwrap();
        assert!(before_publish);
        assert!(session.project().visual_state().is_some());
        drop(session);
        assert_initial(&root);
        let after = inventory(&root);
        for (path, bytes) in before {
            assert_eq!(after.get(&path), Some(&bytes));
        }
        assert!(!root.join(".gil/state.yaml.writing").exists());
    }
}

#[test]
fn existing_records_and_even_empty_or_partial_storage_are_not_reused() {
    for kind in 0..7 {
        let root = scratch();
        match kind {
            0 => {
                let s = ProjectSession::start(rules(), root.join(crate::STATE_PATH)).unwrap();
                s.commit().unwrap();
            }
            1 => {
                drop(ProjectSession::start_visual(rules(), &root).unwrap());
            }
            2 => {
                fs::create_dir(root.join(".gil")).unwrap();
            }
            3 => {
                fs::create_dir_all(root.join(".gil/artifacts/tmp")).unwrap();
                fs::write(
                    root.join(".gil/artifacts/tmp/unfinished"),
                    b"preserve partial object",
                )
                .unwrap();
                fs::write(root.join(".gil/state.yaml.writing"), b"format: 5\npartial").unwrap();
            }
            4 => {
                fs::write(root.join(".gil"), b"not a directory").unwrap();
            }
            5 => {
                symlink(root.join("absent"), root.join(".gil")).unwrap();
            }
            _ => {
                fs::create_dir(root.join(".gil")).unwrap();
                fs::write(root.join(crate::STATE_PATH), b"format: 999\n").unwrap();
            }
        }
        let before = inventory(&root);
        assert!(ProjectSession::start_visual(rules(), &root).is_err());
        assert_eq!(inventory(&root), before);
    }
}

#[test]
fn exclusive_reservation_allows_only_one_concurrent_creator() {
    let root = scratch();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let workers: Vec<_> = (0..2)
        .map(|_| {
            let root = root.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                ProjectSession::start_visual(rules(), root).is_ok()
            })
        })
        .collect();
    let successes = workers
        .into_iter()
        .filter_map(|t| t.join().unwrap().then_some(()))
        .count();
    assert_eq!(successes, 1);
    assert_initial(&root);
}

#[test]
fn denied_or_invalid_folder_does_not_create_storage() {
    let root = scratch();
    assert!(ProjectSession::start_visual(rules(), Path::new("relative")).is_err());
    assert!(ProjectSession::start_visual(rules(), root.join("missing")).is_err());
    fs::write(root.join("file"), b"keep").unwrap();
    assert!(ProjectSession::start_visual(rules(), root.join("file")).is_err());
    assert!(!root.join(".gil").exists());
    // Running as root would bypass permissions and would not constitute this test.
    assert_ne!(unsafe { libc::geteuid() }, 0);
    fs::set_permissions(&root, fs::Permissions::from_mode(0o500)).unwrap();
    let result = ProjectSession::start_visual(rules(), &root);
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.is_err());
    assert!(!root.join(".gil").exists());
    assert_eq!(fs::read(root.join("file")).unwrap(), b"keep");
}

#[test]
fn failures_before_and_after_publication_preserve_work_and_remaining_storage() {
    for stage in [
        "reserved",
        "captured",
        "encoded",
        "partial_write",
        "file_synced",
        "before_publish",
        "after_publish",
        "directory_synced",
        "completed",
    ] {
        let root = scratch();
        fs::write(root.join("report.txt"), "変化なし\n보고서\n").unwrap();
        let result = create(rules(), &root, &root, true, &mut |at| {
            if at == stage {
                Err(std::io::Error::from_raw_os_error(libc::ENOSPC))
            } else {
                Ok(())
            }
        });
        assert!(result.is_err(), "{stage}");
        assert_eq!(
            fs::read_to_string(root.join("report.txt")).unwrap(),
            "変化なし\n보고서\n"
        );
        if ["after_publish", "directory_synced", "completed"].contains(&stage) {
            assert_initial(&root); // success response lost, complete format 5 remains readable
        } else {
            assert!(!root.join(crate::STATE_PATH).exists());
        }
        let before = inventory(&root);
        assert!(ProjectSession::start_visual(rules(), &root).is_err());
        assert_eq!(inventory(&root), before, "retry must preserve {stage}");
    }
}

#[test]
fn concurrent_state_entry_is_not_overwritten_at_publication() {
    let root = scratch();
    let result = create(rules(), &root, &root, true, &mut |stage| {
        if stage == "before_publish" {
            fs::write(root.join(crate::STATE_PATH), b"other writer")?;
        }
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(
        fs::read(root.join(crate::STATE_PATH)).unwrap(),
        b"other writer"
    );
    assert!(
        fs::read(root.join(".gil/state.yaml.writing"))
            .unwrap()
            .starts_with(VISUAL_HEAD)
    );
}

#[test]
fn changed_directory_lock_or_temporary_entry_is_preserved_and_refused() {
    for kind in ["directory", "lock", "temp"] {
        let root = scratch();
        let result = create(rules(), &root, &root, true, &mut |stage| {
            if stage == "encoded" {
                match kind {
                    "directory" => {
                        fs::rename(root.join(".gil"), root.join("held"))?;
                        fs::create_dir(root.join(".gil"))?;
                        fs::write(root.join(".gil/marker"), b"keep replaced directory")?;
                    }
                    "lock" => {
                        fs::rename(root.join(".gil/project.lock"), root.join(".gil/held.lock"))?;
                        fs::write(root.join(".gil/project.lock"), b"keep replaced lock")?;
                    }
                    _ => {
                        fs::write(root.join(".gil/state.yaml.writing"), b"keep temp")?;
                    }
                }
            }
            Ok(())
        });
        assert!(result.is_err(), "{kind}");
        assert!(!root.join(crate::STATE_PATH).exists());
        let marker = match kind {
            "directory" => ".gil/marker",
            "lock" => ".gil/project.lock",
            _ => ".gil/state.yaml.writing",
        };
        assert!(fs::read(root.join(marker)).unwrap().starts_with(b"keep"));
        let before = inventory(&root);
        assert!(ProjectSession::start_visual(rules(), &root).is_err());
        assert_eq!(inventory(&root), before);
    }
}

#[test]
fn capture_refusal_preserves_source_and_does_not_commit_or_hide_the_problem() {
    let root = scratch();
    fs::write(root.join("source.txt"), b"keep original").unwrap();
    symlink(root.join("source.txt"), root.join("link.txt")).unwrap();
    assert!(ProjectSession::start_visual(rules(), &root).is_err());
    assert_eq!(fs::read(root.join("source.txt")).unwrap(), b"keep original");
    assert!(
        fs::symlink_metadata(root.join("link.txt"))
            .unwrap()
            .is_symlink()
    );
    assert!(!root.join(crate::STATE_PATH).exists());
    let before = inventory(&root);
    assert!(ProjectSession::start_visual(rules(), &root).is_err());
    assert_eq!(inventory(&root), before);
}

#[test]
#[ignore = "child entry, explicitly invoked by the SIGKILL parent test"]
fn crash_child() {
    let root = PathBuf::from(std::env::var_os("GIL_VISUAL_START_TEST_ROOT").unwrap());
    let stage = std::env::var("GIL_VISUAL_START_TEST_STAGE").unwrap();
    create(rules(), &root, &root, true, &mut |at| {
        if at == stage {
            unsafe {
                libc::kill(libc::getpid(), libc::SIGKILL);
            }
            // Signal delivery may lag the syscall return. Never execute another write
            // after requesting death, or this would not test the selected boundary.
            loop {
                std::thread::park();
            }
        }
        Ok(())
    })
    .unwrap();
    panic!("child did not reach checkpoint");
}

#[test]
fn killed_creator_leaves_absent_or_complete_state_and_preserves_evidence() {
    use std::os::unix::process::ExitStatusExt;
    for stage in [
        "reserved",
        "partial_write",
        "before_publish",
        "after_publish",
        "directory_synced",
    ] {
        let root = scratch();
        fs::write(root.join("work.txt"), b"keep original work").unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "session::visual_start::tests::crash_child",
            ])
            .env("GIL_VISUAL_START_TEST_ROOT", &root)
            .env("GIL_VISUAL_START_TEST_STAGE", stage)
            .output()
            .unwrap();
        assert_eq!(
            output.status.signal(),
            Some(libc::SIGKILL),
            "{stage}: {output:?}"
        );
        assert_eq!(
            fs::read(root.join("work.txt")).unwrap(),
            b"keep original work"
        );
        if ["after_publish", "directory_synced"].contains(&stage) {
            assert_initial(&root);
        } else {
            assert!(!root.join(crate::STATE_PATH).exists(), "{stage}");
        }
        let before = inventory(&root);
        assert!(ProjectSession::start_visual(rules(), &root).is_err());
        assert_eq!(inventory(&root), before);
    }
}
