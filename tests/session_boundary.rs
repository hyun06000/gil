//! Synthetic Unix control-file boundary tests. Never use user projects.
#![cfg(unix)]
mod common;
use gil::ProjectSession;
use std::os::unix::fs::symlink;
use std::{fs, path::PathBuf};

fn setup(label: &str) -> PathBuf {
    let root = common::scratch(label);
    gil::save(&common::bootstrap(), root.join(gil::STATE_PATH)).unwrap();
    root
}
fn pending(root: &std::path::Path) -> PathBuf {
    let path = root.join(".gil/artifacts/tmp/999-999");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"leave alone").unwrap();
    path
}

#[test]
fn linked_lock_never_grants_authority_or_runs_cleanup() {
    for hard in [false, true] {
        let root = setup(if hard {
            "boundary-lock-hard"
        } else {
            "boundary-lock-symbolic"
        });
        let lock = root.join(".gil/project.lock");
        fs::remove_file(&lock).unwrap();
        let victim = root.join("external");
        fs::write(&victim, b"keep").unwrap();
        if hard {
            fs::hard_link(&victim, &lock).unwrap();
        } else {
            symlink(&victim, &lock).unwrap();
        }
        let temp = pending(&root);
        assert!(ProjectSession::open(common::spec(), root.join(gil::STATE_PATH)).is_err());
        assert!(
            ProjectSession::open_read_only(common::spec(), root.join(gil::STATE_PATH)).is_err()
        );
        assert_eq!(fs::read(victim).unwrap(), b"keep");
        assert_eq!(fs::read(temp).unwrap(), b"leave alone");
    }
}

#[test]
fn unsafe_state_refuses_before_cleanup_or_parsing() {
    for kind in 0..4 {
        let root = setup(&format!("boundary-state-{kind}"));
        let state = root.join(gil::STATE_PATH);
        let victim = root.join("external");
        fs::rename(&state, &victim).unwrap();
        let before = fs::read(&victim).unwrap();
        match kind {
            0 => symlink(&victim, &state).unwrap(),
            1 => fs::hard_link(&victim, &state).unwrap(),
            2 => fs::create_dir(&state).unwrap(),
            _ => {
                use std::os::unix::ffi::OsStrExt;
                let name = std::ffi::CString::new(state.as_os_str().as_bytes()).unwrap();
                assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
            }
        }
        let temp = pending(&root);
        assert!(ProjectSession::open(common::spec(), &state).is_err());
        assert!(ProjectSession::open_read_only(common::spec(), &state).is_err());
        assert!(gil::load(common::spec(), &state).is_err());
        assert_eq!(fs::read(victim).unwrap(), before);
        assert_eq!(fs::read(temp).unwrap(), b"leave alone");
    }
}

#[test]
fn linked_gil_directory_is_not_bootstrapped_or_opened() {
    let root = setup("boundary-gil-link");
    let moved = root.join("held");
    fs::rename(root.join(".gil"), &moved).unwrap();
    symlink(&moved, root.join(".gil")).unwrap();
    let before = fs::read(moved.join("state.yaml")).unwrap();
    assert!(ProjectSession::start(common::spec(), root.join(gil::STATE_PATH)).is_err());
    assert!(ProjectSession::open(common::spec(), root.join(gil::STATE_PATH)).is_err());
    assert_eq!(fs::read(moved.join("state.yaml")).unwrap(), before);
}

#[test]
fn replaced_lock_cannot_authorize_commit() {
    let root = setup("boundary-lock-replaced");
    let state = root.join(gil::STATE_PATH);
    let session = ProjectSession::open(common::spec(), &state).unwrap();
    let before = fs::read(&state).unwrap();
    fs::rename(root.join(".gil/project.lock"), root.join("held-lock")).unwrap();
    fs::write(root.join(".gil/project.lock"), b"").unwrap();
    assert!(session.commit().is_err());
    assert_eq!(fs::read(state).unwrap(), before);
    assert!(!root.join(".gil/state.yaml.writing").exists());
}

#[test]
fn linked_temp_directory_never_deletes_external_files() {
    for component in ["artifacts", "artifacts/tmp"] {
        let root = setup(&format!("boundary-reap-{}", component.replace('/', "-")));
        let temp = pending(&root);
        let moved = root.join("held");
        let path = root.join(".gil").join(component);
        fs::rename(&path, &moved).unwrap();
        symlink(&moved, &path).unwrap();
        assert!(ProjectSession::open(common::spec(), root.join(gil::STATE_PATH)).is_err());
        assert_eq!(fs::read(temp).unwrap(), b"leave alone");
    }
}

#[test]
fn unknown_or_linked_temp_entry_preserves_all_entries() {
    for link in [false, true] {
        let root = setup(&format!("boundary-reap-entry-{link}"));
        let temp = pending(&root);
        let invalid =
            root.join(".gil/artifacts/tmp")
                .join(if link { "999-998" } else { "unknown" });
        let victim = root.join("victim");
        fs::write(&victim, b"keep").unwrap();
        if link {
            symlink(&victim, &invalid).unwrap();
        } else {
            fs::write(&invalid, b"unknown").unwrap();
        }
        assert!(ProjectSession::open(common::spec(), root.join(gil::STATE_PATH)).is_err());
        assert_eq!(fs::read(temp).unwrap(), b"leave alone");
        assert_eq!(fs::read(victim).unwrap(), b"keep");
        assert!(fs::symlink_metadata(invalid).is_ok());
    }
}

#[test]
fn only_recognized_regular_temp_objects_are_reaped() {
    let root = setup("boundary-reap-valid");
    let temp = pending(&root);
    let before = fs::read(root.join(gil::STATE_PATH)).unwrap();
    let session = ProjectSession::open(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    assert!(!temp.exists());
    assert_eq!(fs::read(root.join(gil::STATE_PATH)).unwrap(), before);
    drop(session);
}

#[test]
fn linked_restore_area_or_transaction_is_never_cleaned_up() {
    for transaction in [false, true] {
        let root = setup(&format!("boundary-restore-link-{transaction}"));
        let external = root.join("external");
        fs::create_dir(&external).unwrap();
        fs::write(external.join("keep"), b"keep").unwrap();
        let restore = root.join(".gil/restore");
        if transaction {
            fs::create_dir(&restore).unwrap();
            symlink(&external, restore.join("active")).unwrap();
        } else {
            symlink(&external, &restore).unwrap();
        }
        assert!(ProjectSession::open(common::spec(), root.join(gil::STATE_PATH)).is_err());
        assert!(
            ProjectSession::open_read_only(common::spec(), root.join(gil::STATE_PATH)).is_err()
        );
        assert_eq!(fs::read(external.join("keep")).unwrap(), b"keep");
    }
}
