//! Migration rehearsal on synthetic, privately owned directories ONLY.
//! These copy helpers are not a production backup/migration API.
#![cfg(unix)]
mod common;
use gil::{CycleKind, NodeKind, ProjectSession};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

fn private_dir(path: &Path) {
    fs::DirBuilder::new().mode(0o700).create(path).unwrap();
}
fn home() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    loop {
        let p = std::env::temp_dir().join(format!(
            "gil-migration-rehearsal-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        match fs::DirBuilder::new().mode(0o700).create(&p) {
            Ok(()) => return p,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => panic!("private fixture directory: {e}"),
        }
    }
}
fn inventory(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, at: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for e in fs::read_dir(at).unwrap() {
            let path = e.unwrap().path();
            let m = fs::symlink_metadata(&path).unwrap();
            assert!(!m.file_type().is_symlink(), "never follow fixture links");
            if m.is_dir() {
                walk(root, &path, out)
            } else {
                assert!(
                    m.is_file() && m.nlink() == 1,
                    "regular independent fixture files only"
                );
                out.insert(
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}
fn copy_fixture(root: &Path, to: &Path) {
    let files = inventory(root);
    private_dir(to);
    for (rel, bytes) in &files {
        let target = to.join(rel);
        let mut parent = to.to_path_buf();
        for component in rel.parent().unwrap().components() {
            parent.push(component);
            if !parent.exists() {
                private_dir(&parent)
            }
        }
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(target)
            .unwrap();
        f.write_all(bytes).unwrap();
        f.sync_all().unwrap();
    }
    fs::File::open(to).unwrap().sync_all().unwrap();
    assert_eq!(inventory(to), files);
    assert_eq!(inventory(root), files);
}
fn fixture(base: &Path, closed: bool) -> PathBuf {
    let root = base.join("source");
    private_dir(&root);
    // A real referenced blob, not just an empty manifest.
    fs::write(root.join("work.txt"), "원래 세계\n").unwrap();
    let mut session = ProjectSession::start(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    let mut p = common::bootstrap_from(session.project().clone());
    common::up_to_verify(&mut p);
    if closed {
        let r = common::full_report(
            p.current_cycle().rules(),
            CycleKind::Experiment,
            NodeKind::Verify,
        )
        .with("execution", "  공백과 한글 🧭\n두 번째 줄\n")
        .with("result", "관측 원문\n");
        let world = p.world_manifest(common::snapshot(1)).unwrap().clone();
        p.close_verify_step(r, world).unwrap();
    }
    *session.project_mut() = p;
    session.commit().unwrap();
    drop(session);
    root
}
fn logical(bytes: &[u8]) -> serde_norway::Value {
    let mut v: serde_norway::Value = serde_norway::from_slice(bytes).unwrap();
    let map = v.as_mapping_mut().unwrap();
    map.remove(serde_norway::Value::from("format"));
    map.remove(serde_norway::Value::from("visuals"));
    v
}
fn upgrade(copy: &Path) {
    let session =
        ProjectSession::open_read_only(common::spec(), copy.join(gil::STATE_PATH)).unwrap();
    session.verify_snapshot_objects().unwrap();
    let candidate = session.project().prepare_visual_upgrade().unwrap();
    gil::save(&candidate, copy.join(gil::STATE_PATH)).unwrap();
    drop(session);
    ProjectSession::open_read_only(common::spec(), copy.join(gil::STATE_PATH)).unwrap();
}

#[test]
fn backup_clone_upgrade_and_separate_restore_preserve_all_original_facts() {
    for closed in [false, true] {
        let base = home();
        let root = fixture(&base, closed);
        let held =
            ProjectSession::open_read_only(common::spec(), root.join(gil::STATE_PATH)).unwrap();
        held.verify_snapshot_objects().unwrap();
        let before = inventory(&root);
        let backup = base.join("backup");
        copy_fixture(&root, &backup);
        let candidate = base.join("candidate");
        copy_fixture(&backup, &candidate);
        upgrade(&candidate);
        let after = inventory(&candidate);
        assert_eq!(
            logical(&before[Path::new(gil::STATE_PATH)]),
            logical(&after[Path::new(gil::STATE_PATH)])
        );
        for (path, bytes) in &before {
            if path != Path::new(gil::STATE_PATH) {
                assert_eq!(&after[path], bytes)
            }
        }
        assert_eq!(after.len(), before.len());
        let restored = base.join("restored");
        copy_fixture(&backup, &restored);
        ProjectSession::open_read_only(common::spec(), restored.join(gil::STATE_PATH))
            .unwrap()
            .verify_snapshot_objects()
            .unwrap();
        assert_eq!(inventory(&restored), before);
        assert_eq!(inventory(&root), before);
        let state = fs::read(candidate.join(gil::STATE_PATH)).unwrap();
        let read = ProjectSession::open_read_only(common::spec(), candidate.join(gil::STATE_PATH))
            .unwrap();
        assert!(read.project().prepare_visual_upgrade().is_err());
        assert_eq!(fs::read(candidate.join(gil::STATE_PATH)).unwrap(), state);
        if !closed {
            let mut p = read.project().clone();
            let r = common::full_report(
                p.current_cycle().rules(),
                CycleKind::Experiment,
                NodeKind::Verify,
            );
            let world = p.world_manifest(common::snapshot(1)).unwrap().clone();
            assert!(
                p.close_verify_step(r, world).is_err(),
                "open Verify must now declare visuals"
            );
        }
        drop(held);
    }
}

#[test]
fn failed_candidate_publication_keeps_original_backup_and_candidate_state() {
    let base = home();
    let root = fixture(&base, true);
    let before = inventory(&root);
    let backup = base.join("backup");
    copy_fixture(&root, &backup);
    let candidate = base.join("candidate");
    copy_fixture(&backup, &candidate);
    fs::create_dir(candidate.join(".gil/state.yaml.writing")).unwrap();
    let read =
        ProjectSession::open_read_only(common::spec(), candidate.join(gil::STATE_PATH)).unwrap();
    assert!(
        gil::save(
            &read.project().prepare_visual_upgrade().unwrap(),
            candidate.join(gil::STATE_PATH)
        )
        .is_err()
    );
    assert_eq!(inventory(&root), before);
    assert_eq!(inventory(&backup), before);
    assert_eq!(
        fs::read(candidate.join(gil::STATE_PATH)).unwrap(),
        before[Path::new(gil::STATE_PATH)]
    );
}

#[test]
fn busy_or_recovery_state_blocks_preflight_without_cleanup() {
    let base = home();
    let root = fixture(&base, true);
    let held = ProjectSession::open_read_only(common::spec(), root.join(gil::STATE_PATH)).unwrap();
    let before = inventory(&root);
    assert!(ProjectSession::open_read_only(common::spec(), root.join(gil::STATE_PATH)).is_err());
    assert_eq!(inventory(&root), before);
    drop(held);
    fs::create_dir_all(root.join(".gil/restore/active")).unwrap();
    fs::write(
        root.join(".gil/restore/active/PLAN"),
        b"preserve recovery evidence",
    )
    .unwrap();
    let before = inventory(&root);
    assert!(ProjectSession::open_read_only(common::spec(), root.join(gil::STATE_PATH)).is_err());
    assert_eq!(inventory(&root), before);
}

#[test]
fn missing_backup_object_is_not_a_verified_backup() {
    let base = home();
    let root = fixture(&base, true);
    let before = inventory(&root);
    let backup = base.join("backup");
    copy_fixture(&root, &backup);
    let blob = before
        .keys()
        .find(|p| p.to_string_lossy().contains("/blobs/"))
        .unwrap();
    fs::remove_file(backup.join(blob)).unwrap(); // Deliberate corruption of our own copy only.
    assert_ne!(inventory(&backup), before);
    // State loading alone succeeds: it is not a full backup audit.
    let readable =
        ProjectSession::open_read_only(common::spec(), backup.join(gil::STATE_PATH)).unwrap();
    assert!(readable.verify_snapshot_objects().is_err());
    assert_eq!(inventory(&root), before);
}

#[test]
fn corrupt_referenced_blob_is_detected_without_repair() {
    let base = home();
    let root = fixture(&base, true);
    let backup = base.join("backup");
    copy_fixture(&root, &backup);
    let blob = inventory(&backup)
        .keys()
        .find(|p| p.to_string_lossy().contains("/blobs/"))
        .unwrap()
        .clone();
    fs::write(backup.join(blob), b"corrupt bytes").unwrap();
    let before = inventory(&backup);
    let read =
        ProjectSession::open_read_only(common::spec(), backup.join(gil::STATE_PATH)).unwrap();
    assert!(read.verify_snapshot_objects().is_err());
    assert_eq!(inventory(&backup), before);
}

#[test]
fn closed_cycle_report_and_journey_survive_without_retroactive_visual_declarations() {
    let base = home();
    let root = base.join("source");
    private_dir(&root);
    common::sow_world(&root);
    gil::save(
        &common::graph_with_one_closed_cycle("success"),
        root.join(gil::STATE_PATH),
    )
    .unwrap();
    let before = inventory(&root);
    let candidate = base.join("candidate");
    copy_fixture(&root, &candidate);
    upgrade(&candidate);
    assert_eq!(
        logical(&before[Path::new(gil::STATE_PATH)]),
        logical(&fs::read(candidate.join(gil::STATE_PATH)).unwrap())
    );
    let read =
        ProjectSession::open_read_only(common::spec(), candidate.join(gil::STATE_PATH)).unwrap();
    let owner = gil::visual_report::registry::ReportOwner::Step(
        gil::StepRef::new(gil::CycleRef::new(2).unwrap(), 3).unwrap(),
    );
    assert!(
        read.project()
            .visual_state()
            .unwrap()
            .registry()
            .report(owner)
            .is_none()
    );
    assert_eq!(inventory(&root), before);
}

#[test]
fn linked_blob_is_refused_without_reading_or_changing_external_target() {
    let base = home();
    let root = fixture(&base, true);
    let backup = base.join("backup");
    copy_fixture(&root, &backup);
    let blob = inventory(&backup)
        .keys()
        .find(|p| p.to_string_lossy().contains("/blobs/"))
        .unwrap()
        .clone();
    let external = base.join("external");
    fs::write(&external, b"private sentinel").unwrap();
    fs::remove_file(backup.join(&blob)).unwrap();
    std::os::unix::fs::symlink(&external, backup.join(&blob)).unwrap();
    let state = fs::read(backup.join(gil::STATE_PATH)).unwrap();
    let read =
        ProjectSession::open_read_only(common::spec(), backup.join(gil::STATE_PATH)).unwrap();
    assert!(read.verify_snapshot_objects().is_err());
    assert_eq!(fs::read(&external).unwrap(), b"private sentinel");
    assert_eq!(fs::read(backup.join(gil::STATE_PATH)).unwrap(), state);
    assert!(
        fs::symlink_metadata(backup.join(blob))
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[test]
#[ignore = "explicit old release binary required; never inferred from PATH"]
fn actual_old_writer_refuses_format_five_without_mutating_it() {
    let binary =
        std::env::var_os("GIL_MIGRATION_OLD_BINARY").expect("explicit release executable required");
    let base = home();
    let root = fixture(&base, false);
    let candidate = base.join("candidate");
    copy_fixture(&root, &candidate);
    upgrade(&candidate);
    let before = inventory(&candidate);
    let output = std::process::Command::new(binary)
        .arg("close")
        .current_dir(&candidate)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        text.contains('5') && (text.contains("format") || text.contains("형식")),
        "must refuse format, not fail for some other reason: {text}"
    );
    assert_eq!(inventory(&candidate), before);
}
