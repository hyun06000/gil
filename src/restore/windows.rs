//! Product recovery on Windows. All writable paths are beneath pinned, no-reparse ancestors.
//! PLAN/backups remain compatible; `work` holds unpublished files so a crash cannot strand
//! temporary bytes in the user's working tree. Never enters the legacy pathname writer.
use super::*;
use crate::{artifact::EntryPath, recovery_fs_windows::Dir, windows_fs as win};
use std::io::{Read, Write};
#[cfg(test)]
#[path = "windows_tests.rs"]
mod tests;

fn err(e: impl std::fmt::Display) -> RestoreFailure {
    RestoreFailure::Io {
        stage: Stage::Recovery,
        doing: "Windows 승인 복원을 안전하게 끝내지",
        path: "approved recovery".into(),
        said: e.to_string(),
    }
}
fn valid(path: &EntryPath) -> io::Result<()> {
    let text = path.as_str();
    if text
        .split('/')
        .any(|s| s.eq_ignore_ascii_case(".gil") || s.eq_ignore_ascii_case(".git"))
        || preview::inspect_text(text).iter().any(|r| {
            matches!(
                r.severity(preview::TargetOs::Windows),
                preview::Severity::Blocks | preview::Severity::Unverified
            )
        })
    {
        return Err(win::refused());
    }
    Ok(())
}
fn parent(root: &Dir, path: &EntryPath, create: bool) -> io::Result<(Dir, String)> {
    valid(path)?;
    let mut parts: Vec<_> = path.as_str().split('/').collect();
    let leaf = parts.pop().ok_or_else(win::refused)?.to_owned();
    let mut dir = Dir::open(root.path())?;
    for part in parts {
        dir = dir.child(part, create)?;
    }
    Ok((dir, leaf))
}
fn digest(root: &Dir, path: &EntryPath) -> io::Result<Option<ContentDigest>> {
    let result = parent(root, path, false)
        .and_then(|(dir, leaf)| dir.read(&leaf))
        .and_then(digest_of);
    match result {
        Ok(v) => Ok(Some(v)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
fn write_world(
    root: &Dir,
    work: &Dir,
    path: &EntryPath,
    expected: Option<&ContentDigest>,
    after: &ContentDigest,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    let (dir, name) = parent(root, path, true)?;
    let ticket = NEXT_TICKET.fetch_add(1, Ordering::Relaxed);
    let tmp = format!("{}{}", std::process::id(), ticket);
    let mut f = work.create(&tmp)?;
    write(&mut f)?;
    f.sync_all()?;
    use io::{Seek, SeekFrom};
    f.seek(SeekFrom::Start(0))?;
    if digest_of(f.try_clone()?)? != *after {
        return Err(win::refused());
    }
    if digest(root, path)?.as_ref() != expected {
        return Err(win::refused());
    }
    dir.rename(&f, &name, expected.is_some())?;
    work.sync()
}
fn rollback(root: &Dir, active: &Dir, plan: &Plan) -> Result<(), RestoreFailure> {
    failpoint::fail_at("in-rollback", Stage::Rollback)?;
    let backup = active.child(BACKUP, false).map_err(err)?;
    // Validate every path and backup before changing a working file.
    for step in plan.steps() {
        valid(&step.path).map_err(err)?;
        if let Some(n) = step.operation.backup() {
            let mut f = backup.read(&n.to_string()).map_err(err)?;
            verify_backup(&mut f, step.operation.before().unwrap()).map_err(err)?;
        }
        let now = digest(root, &step.path).map_err(err)?;
        let after = match &step.operation {
            Operation::Create { target } | Operation::Replace { target, .. } => Some(target),
            _ => None,
        };
        if now.as_ref() != step.operation.before() && now.as_ref() != after {
            return Err(err(
                "복원 중 별도 편집이 생겼다. 원본 증거와 현재 파일을 보존한다",
            ));
        }
    }
    let work = active.child("work", true).map_err(err)?;
    for step in plan.steps() {
        let now = digest(root, &step.path).map_err(err)?;
        if now.as_ref() == step.operation.before() {
            continue;
        }
        match step.operation.backup() {
            Some(n) => {
                let mut f = backup.read(&n.to_string()).map_err(err)?;
                write_world(
                    root,
                    &work,
                    &step.path,
                    now.as_ref(),
                    step.operation.before().unwrap(),
                    |out| io::copy(&mut f, out).map(|_| ()),
                )
                .map_err(err)?;
            }
            None => {
                let (dir, name) = parent(root, &step.path, false).map_err(err)?;
                dir.delete_file(&name, now.as_ref().unwrap()).map_err(err)?;
            }
        }
    }
    if observe(root.path()).map_err(err)? != plan.before_world() {
        return Err(RestoreFailure::RollbackDidNotLand);
    }
    Ok(())
}
fn read_plan(active: &Dir) -> Result<Plan, RestoreFailure> {
    let mut bytes = Vec::new();
    active
        .read(PLAN)
        .map_err(err)?
        .take(64 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .map_err(err)?;
    let plan = Plan::decode(&bytes).map_err(RestoreFailure::Plan)?;
    for step in plan.steps() {
        valid(&step.path).map_err(err)?;
    }
    Ok(plan)
}
fn cleanup(area: &Dir, name: &str) -> Result<(), RestoreFailure> {
    let dir = area.child(name, false).map_err(err)?;
    let mut files = Vec::new();
    let mut children = Vec::new();
    // Validate the entire shape before deleting anything; no recursive pathname removal.
    for entry in dir.names().map_err(err)? {
        match entry.as_str() {
            PLAN | COMMITTED => files.push((
                entry.clone(),
                digest_of(dir.read(&entry).map_err(err)?).map_err(err)?,
            )),
            BACKUP | "work" => {
                let child = dir.child(&entry, false).map_err(err)?;
                let mut entries = Vec::new();
                for file in child.names().map_err(err)? {
                    if file.is_empty() || !file.bytes().all(|b| b.is_ascii_digit()) {
                        return Err(err("unknown recovery file"));
                    }
                    entries.push((
                        file.clone(),
                        digest_of(child.read(&file).map_err(err)?).map_err(err)?,
                    ));
                }
                children.push((entry, child, entries));
            }
            _ => return Err(err("unknown recovery entry")),
        }
    }
    for (name, child, entries) in children {
        for (file, digest) in entries {
            child.delete_file(&file, &digest).map_err(err)?;
        }
        drop(child);
        dir.remove_dir(&name).map_err(err)?;
    }
    for (file, digest) in files {
        dir.delete_file(&file, &digest).map_err(err)?;
    }
    drop(dir);
    area.remove_dir(name).map_err(err)
}
fn finish(area: &Dir) -> Result<(), RestoreFailure> {
    let name = format!(
        "{CLEANUP}{}-{}",
        std::process::id(),
        NEXT_TICKET.fetch_add(1, Ordering::Relaxed)
    );
    area.rename_dir(ACTIVE, &name).map_err(err)?;
    failpoint::crash_at("before-cleanup");
    cleanup(area, &name)
}
fn area(gil: &Path, create: bool) -> io::Result<Dir> {
    Dir::open(gil)?.child(RESTORE, create)
}
fn entries(dir: &Dir) -> Result<Vec<(String, Kind)>, RestoreFailure> {
    let mut entries = Vec::new();
    for name in dir.names().map_err(err)? {
        let kind = Area::classify(&name).ok_or_else(|| err("unknown restore area"))?;
        dir.child(&name, false).map_err(err)?;
        entries.push((name, kind));
    }
    entries.sort_by_key(|(_, kind)| *kind != Kind::Active);
    Ok(entries)
}
pub(super) fn left_behind(gil: &Path) -> Result<Option<String>, RestoreFailure> {
    let dir = match area(gil, false) {
        Ok(d) => d,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(err(e)),
    };
    let names: Vec<_> = entries(&dir)?.into_iter().map(|(n, _)| n).collect();
    Ok((!names.is_empty()).then(|| names.join(", ")))
}
pub(super) fn recover(root: &Path, gil: &Path) -> Result<(), RestoreFailure> {
    let dir = match area(gil, false) {
        Ok(d) => d,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(err(e)),
    };
    let list = entries(&dir)?;
    if list.is_empty() {
        return Ok(());
    }
    let root = Dir::open(root).map_err(err)?;
    for (name, kind) in list {
        if kind != Kind::Active {
            cleanup(&dir, &name)?;
            continue;
        }
        let active = dir.child(&name, false).map_err(err)?;
        match active.read(COMMITTED) {
            Ok(mut f) => {
                let mut b = String::new();
                f.read_to_string(&mut b).map_err(err)?;
                if b != "committed\n" {
                    return Err(err("unknown commit marker"));
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let plan = read_plan(&active)?;
                rollback(&root, &active, &plan)?;
            }
            Err(e) => return Err(err(e)),
        }
        drop(active);
        finish(&dir)?;
    }
    Ok(())
}
pub(super) fn run(
    root: &Path,
    gil: &Path,
    store: &ObjectStore,
    world: SnapshotRef,
    target: &Manifest,
    expected: Option<&Manifest>,
) -> Result<Restored, RestoreFailure> {
    let held = Dir::open(root).map_err(err)?;
    verify_blobs(store, target, world)?;
    let before = observe(root).map_err(err)?;
    if expected.is_some_and(|e| e != &before) {
        return Err(RestoreFailure::Observe {
            stage: Stage::Preflight,
            said: "승인한 파일 상태가 바뀌었다".into(),
        });
    }
    let steps = plan::steps(root, &before, target);
    for step in &steps {
        valid(&step.path).map_err(err)?;
        digest(&held, &step.path).map_err(err)?;
    }
    let counts = tally(&steps);
    let plan = Plan::new(world, steps);
    let result = Restored {
        world,
        no_op: plan.steps().is_empty(),
        replaced: counts.0,
        created: counts.1,
        deleted: counts.2,
    };
    if result.no_op {
        return Ok(result);
    }
    let area = area(gil, true).map_err(err)?;
    if !entries(&area)?.is_empty() {
        return Err(err("unfinished recovery must be resumed first"));
    }
    // Publishing must be atomic: the retained control area and project must share a volume.
    if held.volume().map_err(err)? != area.volume().map_err(err)? {
        return Err(err(
            "기록 저장소와 작업 폴더의 볼륨이 달라 원자적 복원을 거절했다",
        ));
    }
    let stage_name = format!(
        "{PREPARING}{}-{}",
        std::process::id(),
        NEXT_TICKET.fetch_add(1, Ordering::Relaxed)
    );
    let stage = area.child(&stage_name, true).map_err(err)?;
    let backup = stage.child(BACKUP, true).map_err(err)?;
    for step in plan.steps() {
        if let Some(n) = step.operation.backup() {
            let (dir, name) = parent(&held, &step.path, false).map_err(err)?;
            let mut from = dir.read(&name).map_err(err)?;
            let mut to = backup.create(&n.to_string()).map_err(err)?;
            io::copy(&mut from, &mut to).map_err(err)?;
            to.sync_all().map_err(err)?;
            verify_backup(&mut to, step.operation.before().unwrap()).map_err(err)?;
        }
    }
    let mut control = stage.create(PLAN).map_err(err)?;
    control.write_all(&plan.encode()).map_err(err)?;
    control.sync_all().map_err(err)?;
    drop(control);
    backup.sync().map_err(err)?;
    stage.sync().map_err(err)?;
    drop(backup);
    drop(stage);
    failpoint::crash_at("prepared-not-armed");
    area.rename_dir(&stage_name, ACTIVE).map_err(err)?;
    failpoint::crash_at("after-prepare");
    failpoint::meddle("after-prepare", root);
    let active = area.child(ACTIVE, false).map_err(err)?;
    let work = active.child("work", true).map_err(err)?;
    let forward = (|| -> Result<(), RestoreFailure> {
        for (index, step) in plan.steps().iter().enumerate() {
            if digest(&held, &step.path).map_err(err)?.as_ref() != step.operation.before() {
                return Err(err("승인 후 파일이 바뀌어 현재 파일을 보존한다"));
            }
            match &step.operation {
                Operation::Delete { before, .. } => {
                    let (dir, name) = parent(&held, &step.path, false).map_err(err)?;
                    dir.delete_file(&name, before).map_err(err)?;
                }
                Operation::Replace { target, .. } | Operation::Create { target } => {
                    write_world(
                        &held,
                        &work,
                        &step.path,
                        step.operation.before(),
                        target,
                        |f| {
                            store
                                .read_blob(target, f)
                                .map(|_| ())
                                .map_err(io::Error::other)
                        },
                    )
                    .map_err(err)?;
                }
            }
            if index == 0 {
                failpoint::crash_at("after-first-apply");
                failpoint::fail_at("after-first-apply", Stage::Apply)?;
            }
        }
        failpoint::crash_at("after-apply");
        failpoint::fail_at("after-apply", Stage::Apply)?;
        failpoint::meddle("after-apply", root);
        if observe(root).map_err(err)? != *target {
            return Err(RestoreFailure::NotTheTargetWorld);
        }
        Ok(())
    })();
    drop(work);
    if let Err(cause) = forward {
        rollback(&held, &active, &plan).map_err(|then| RestoreFailure::RecoveryRequired {
            world,
            cause: Box::new(err(&cause)),
            then: Box::new(then),
        })?;
        drop(active);
        finish(&area)?;
        return Err(RestoreFailure::RolledBack {
            world,
            cause: Box::new(cause),
        });
    }
    failpoint::crash_at("before-commit");
    let mut marker = active.create(COMMITTED).map_err(err)?;
    marker.write_all(b"committed\n").map_err(err)?;
    marker.sync_all().map_err(err)?;
    drop(marker);
    active.sync().map_err(err)?;
    drop(active);
    failpoint::crash_at("after-commit");
    finish(&area)?;
    Ok(result)
}
