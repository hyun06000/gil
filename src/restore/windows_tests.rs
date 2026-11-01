use super::*;
fn scratch(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "gil-recovery-{label}-{}-{}",
        std::process::id(),
        NEXT_TICKET.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    root
}
fn junction(path: &Path, target: &Path) {
    let shell = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
        .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let out = std::process::Command::new(shell)
        .args(["-NoProfile", "-NonInteractive", "-Command", "$ErrorActionPreference='Stop'; New-Item -ItemType Junction -Path $env:GIL_TEST_LINK -Target $env:GIL_TEST_TARGET | Out-Null"])
        .env("GIL_TEST_LINK", path).env("GIL_TEST_TARGET", target).output().unwrap();
    assert!(
        out.status.success(),
        "junction fixture: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
#[test]
fn native_transaction_replaces_creates_deletes_and_cleans_only_its_journal() {
    let root = scratch("transaction");
    let gil = root.join(".gil");
    fs::create_dir(&gil).unwrap();
    fs::create_dir(root.join("nested")).unwrap();
    fs::write(root.join("nested/report"), "baseline").unwrap();
    fs::write(root.join("target-only"), "restored").unwrap();
    let target = observe(&root).unwrap();
    let store = ObjectStore::at(&gil);
    for e in target.entries() {
        store
            .put_blob(File::open(root.join(e.path.as_str())).unwrap())
            .unwrap();
    }
    fs::write(root.join("nested/report"), "partial").unwrap();
    fs::remove_file(root.join("target-only")).unwrap();
    fs::write(root.join("evidence"), "failed proof").unwrap();
    let before = observe(&root).unwrap();
    let done = run(
        &root,
        &gil,
        &store,
        SnapshotRef::new(1).unwrap(),
        &target,
        Some(&before),
    )
    .unwrap();
    assert_eq!((done.replaced, done.created, done.deleted), (1, 1, 1));
    assert_eq!(observe(&root).unwrap(), target);
    assert!(left_behind(&gil).unwrap().is_none());
    assert!(
        run(
            &root,
            &gil,
            &store,
            SnapshotRef::new(1).unwrap(),
            &target,
            Some(&target)
        )
        .unwrap()
        .no_op
    );
}
#[test]
fn junctions_hardlinks_and_drive_syntax_cannot_redirect_destination_operations() {
    let root = scratch("boundary");
    let outside = scratch("outside");
    fs::write(outside.join("sentinel"), "outside").unwrap();
    junction(&root.join("linked"), &outside);
    let held = Dir::open(&root).unwrap();
    assert!(digest(&held, &EntryPath::parse("linked/sentinel").unwrap()).is_err());
    fs::hard_link(outside.join("sentinel"), root.join("hardlink")).unwrap();
    assert!(digest(&held, &EntryPath::parse("hardlink").unwrap()).is_err());
    for name in [
        "C:/sentinel",
        "a:stream",
        "CON",
        "a/../sentinel",
        ".git/HEAD",
        ".gil/state.yaml",
        "a./sentinel",
    ] {
        if let Some(path) = EntryPath::parse(name) {
            assert!(valid(&path).is_err(), "{name}");
        }
    }
    assert_eq!(
        fs::read_to_string(outside.join("sentinel")).unwrap(),
        "outside"
    );
}
#[test]
fn parent_is_pinned_during_copy_and_unpublished_bytes_stay_in_the_journal() {
    let root = scratch("pin-copy");
    fs::create_dir(root.join("nested")).unwrap();
    let journal = scratch("copy-work");
    fs::write(root.join("nested/report"), "before").unwrap();
    let before = digest_of(&b"before"[..]).unwrap();
    let after = digest_of(&b"after"[..]).unwrap();
    let held = Dir::open(&root).unwrap();
    let work = Dir::open(&journal).unwrap();
    write_world(
        &held,
        &work,
        &EntryPath::parse("nested/report").unwrap(),
        Some(&before),
        &after,
        |file| {
            assert!(fs::rename(root.join("nested"), root.join("moved")).is_err());
            assert_eq!(fs::read_dir(root.join("nested")).unwrap().count(), 1);
            file.write_all(b"after")
        },
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(root.join("nested/report")).unwrap(),
        "after"
    );
    assert_eq!(work.names().unwrap().len(), 0);
}
#[test]
fn cleanup_validates_every_entry_before_deleting_even_one_known_file() {
    let root = scratch("cleanup");
    let area = Dir::open(&root).unwrap();
    let child = area.child("cleanup-1-1", true).unwrap();
    child.create(PLAN).unwrap().write_all(b"known").unwrap();
    child
        .create("unexpected")
        .unwrap()
        .write_all(b"keep")
        .unwrap();
    drop(child);
    assert!(cleanup(&area, "cleanup-1-1").is_err());
    assert_eq!(fs::read(root.join("cleanup-1-1/PLAN")).unwrap(), b"known");
    assert_eq!(
        fs::read(root.join("cleanup-1-1/unexpected")).unwrap(),
        b"keep"
    );
}

#[test]
fn private_preservation_paths_longer_than_max_path_can_publish_and_reopen() {
    let root = scratch("long-private-archive");
    let root = Dir::open(&root).unwrap();
    let external = root
        .child(&"x".repeat(100), true)
        .unwrap()
        .child(".gil", true)
        .unwrap();
    let base = external.private_child("revisit-preserved").unwrap();
    let archive = base.private_child(&"a".repeat(64)).unwrap();
    let from = format!("part-{}", "b".repeat(64));
    assert!(archive.path().join(&from).as_os_str().len() > 260);
    let mut f = archive.create(&from).unwrap();
    f.write_all(b"preserved evidence").unwrap();
    f.sync_all().unwrap();
    archive.rename_file(&from, "file-0").unwrap();
    drop(f);
    let reopened = base.private_child(&"a".repeat(64)).unwrap();
    let mut bytes = String::new();
    reopened
        .read("file-0")
        .unwrap()
        .read_to_string(&mut bytes)
        .unwrap();
    assert_eq!(bytes, "preserved evidence");
}
