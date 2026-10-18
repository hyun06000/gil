//! Windows mirror of `visual_start_tests.rs` (Windows contract §6, W1-*). Runs only on a
//! Windows host; a Mac cross-compile of this file is not evidence.
use super::*;
use std::collections::BTreeMap;
use std::os::windows::fs::MetadataExt;
use std::sync::atomic::{AtomicU64, Ordering};

/// `ERROR_DISK_FULL`
const DISK_FULL: i32 = 112;

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
// Include directories, attributes and link targets, not just state.yaml.
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
                (meta.file_attributes(), bytes),
            );
            if meta.is_dir() && !meta.is_symlink() {
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
    assert!(bytes.starts_with(b"format: 5\n"));
    let read = ProjectSession::open_read_only(rules(), root.join(crate::STATE_PATH)).unwrap();
    assert_eq!(read.project().current_cycle().kind(), CycleKind::Interview);
    let value: serde_norway::Value = serde_norway::from_slice(&bytes).unwrap();
    assert!(read.project().visual_state().is_some());
    assert_eq!(
        read.project().visual_state().unwrap().registry(),
        &crate::visual_report::registry::VisualRegistry::default()
    );
    assert_eq!(read.project().cycles().nodes().len(), 1);
    assert_eq!(read.project().current_cycle().steps().nodes().len(), 0);
    assert!(value["visuals"].is_mapping());
}
fn powershell() -> PathBuf {
    PathBuf::from(std::env::var_os("SystemRoot").unwrap())
        .join("System32/WindowsPowerShell/v1.0/powershell.exe")
}
/// Junction fixture (no privilege required). Paths travel as environment values, never as
/// shell source.
fn junction(link: &Path, target: &Path) {
    let made = std::process::Command::new(powershell())
        .args(["-NoProfile", "-NonInteractive", "-Command",
            "$ErrorActionPreference='Stop'; New-Item -ItemType Junction -Path $env:GIL_TEST_LINK -Target $env:GIL_TEST_TARGET | Out-Null"])
        .env("GIL_TEST_LINK", link)
        .env("GIL_TEST_TARGET", target)
        .output()
        .unwrap();
    assert!(made.status.success(), "junction fixture creation failed");
}
/// Deny directory creation for everyone (including administrators) while held.
struct DenyCreate(PathBuf);
impl DenyCreate {
    fn new(dir: &Path) -> Self {
        let icacls = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/icacls.exe");
        let out = std::process::Command::new(icacls)
            .arg(dir)
            .args(["/deny", "*S-1-1-0:(AD,WD)"])
            .output()
            .unwrap();
        assert!(out.status.success(), "icacls deny failed");
        Self(dir.to_owned())
    }
}
impl Drop for DenyCreate {
    fn drop(&mut self) {
        let icacls = PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32/icacls.exe");
        let _ = std::process::Command::new(icacls)
            .arg(&self.0)
            .args(["/remove:d", "*S-1-1-0"])
            .output();
    }
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
        let session = create(rules(), &root, &mut |stage| {
            if stage == "before_publish" {
                before_publish = true;
                assert!(!root.join(crate::STATE_PATH).exists());
                assert!(
                    fs::read(root.join(".gil/state.yaml.writing"))?.starts_with(b"format: 5\n")
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
                fs::create_dir(root.join("held")).unwrap();
                junction(&root.join(".gil"), &root.join("held"));
            }
            _ => {
                fs::create_dir(root.join(".gil")).unwrap();
                fs::write(root.join(crate::STATE_PATH), b"format: 999\n").unwrap();
            }
        }
        let before = inventory(&root);
        assert!(ProjectSession::start_visual(rules(), &root).is_err(), "{kind}");
        assert_eq!(inventory(&root), before, "{kind}");
        if kind == 5 {
            assert_eq!(fs::read_dir(root.join("held")).unwrap().count(), 0, "junction target received writes");
        }
    }
}

#[test]
fn junction_root_is_refused_without_creating_storage() {
    let base = scratch();
    let target = base.join("real");
    fs::create_dir(&target).unwrap();
    let alias = base.join("alias");
    junction(&alias, &target);
    assert!(ProjectSession::start_visual(rules(), &alias).is_err());
    assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
    assert!(!alias.join(".gil").exists());
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
    let result = {
        let _deny = DenyCreate::new(&root);
        ProjectSession::start_visual(rules(), &root)
    };
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
        let result = create(rules(), &root, &mut |at| {
            if at == stage {
                Err(std::io::Error::from_raw_os_error(DISK_FULL))
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
            assert!(!root.join(crate::STATE_PATH).exists(), "{stage}");
        }
        let before = inventory(&root);
        assert!(ProjectSession::start_visual(rules(), &root).is_err());
        assert_eq!(inventory(&root), before, "retry must preserve {stage}");
    }
}

#[test]
fn concurrent_state_entry_is_not_overwritten_at_publication() {
    let root = scratch();
    let result = create(rules(), &root, &mut |stage| {
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
            .starts_with(b"format: 5\n")
    );
}

#[test]
fn changed_directory_lock_or_temporary_entry_is_preserved_and_refused() {
    for kind in ["directory", "lock", "temp"] {
        let root = scratch();
        let result = create(rules(), &root, &mut |stage| {
            if stage == "encoded" {
                match kind {
                    "directory" => {
                        // The reserved directory is pinned and holds the open lock: no rename.
                        assert!(fs::rename(root.join(".gil"), root.join("held")).is_err());
                        return Err(std::io::Error::other("fixture stops after the refused rename"));
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
        match kind {
            "directory" => assert!(root.join(".gil/project.lock").exists() && !root.join("held").exists()),
            "lock" => assert!(fs::read(root.join(".gil/project.lock")).unwrap().starts_with(b"keep")),
            _ => assert!(fs::read(root.join(".gil/state.yaml.writing")).unwrap().starts_with(b"keep")),
        }
        let before = inventory(&root);
        assert!(ProjectSession::start_visual(rules(), &root).is_err());
        assert_eq!(inventory(&root), before);
    }
}

#[test]
fn capture_refusal_preserves_source_and_does_not_commit_or_hide_the_problem() {
    let root = scratch();
    fs::write(root.join("source.txt"), b"keep original").unwrap();
    fs::create_dir(root.join("images")).unwrap();
    junction(&root.join("linked"), &root.join("images"));
    assert!(ProjectSession::start_visual(rules(), &root).is_err());
    assert_eq!(fs::read(root.join("source.txt")).unwrap(), b"keep original");
    assert!(fs::symlink_metadata(root.join("linked")).unwrap().is_symlink());
    assert!(!root.join(crate::STATE_PATH).exists());
    let before = inventory(&root);
    assert!(ProjectSession::start_visual(rules(), &root).is_err());
    assert_eq!(inventory(&root), before);
}

#[test]
#[ignore = "child entry, explicitly invoked by the abort parent test"]
fn crash_child() {
    let root = PathBuf::from(std::env::var_os("GIL_VISUAL_START_TEST_ROOT").unwrap());
    let stage = std::env::var("GIL_VISUAL_START_TEST_STAGE").unwrap();
    create(rules(), &root, &mut |at| {
        if at == stage {
            // No destructors, no further writes: the OS reclaims handles and the lock.
            std::process::abort();
        }
        Ok(())
    })
    .unwrap();
    panic!("child did not reach checkpoint");
}

#[test]
fn killed_creator_leaves_absent_or_complete_state_and_preserves_evidence() {
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
        assert!(!output.status.success(), "{stage}: {output:?}");
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

// ── Windows contract §6 W6-*: owner-only storage, actual cross-account access, non-local roots ──

fn system32(tool: &str) -> PathBuf {
    PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32").join(tool)
}
/// `icacls` listing of one entry, for assertions about what is *not* repaired.
fn acl_listing(path: &Path) -> String {
    let out = std::process::Command::new(system32("icacls.exe")).arg(path).output().unwrap();
    String::from_utf8_lossy(&out.stdout).to_string()
}
fn open_private_check(path: &Path) -> std::io::Result<()> {
    let dir = crate::windows_fs::open_dir(path, crate::windows_fs::SHARE_ALL, false)?;
    crate::windows_fs::private_dir(&dir)
}

#[test]
fn new_gil_is_owner_only_with_a_protected_inheritable_dacl_and_children_inherit_it() {
    let root = scratch();
    drop(ProjectSession::start_visual(rules(), &root).unwrap());
    let gil = root.join(".gil");
    // Effective security descriptor read through a handle: owner = current user, protected,
    // exactly two inheritable full-access ACEs (user, SYSTEM). This is the structural check;
    // actual cross-account access is a separate test.
    open_private_check(&gil).unwrap();
    let listing = acl_listing(&gil);
    assert!(listing.contains("NT AUTHORITY\\SYSTEM:(OI)(CI)(F)"), "{listing}");
    assert!(!listing.contains("BUILTIN\\Users") && !listing.contains("Authenticated Users") && !listing.contains("Everyone"), "{listing}");
    // Records created inside inherit only from that DACL (no entry grants anyone else).
    for child in [gil.join("state.yaml"), gil.join("project.lock"), gil.join("artifacts")] {
        let listing = acl_listing(&child);
        assert!(listing.contains("(I)"), "{child:?}: {listing}");
        assert!(!listing.contains("BUILTIN\\Users") && !listing.contains("Authenticated Users") && !listing.contains("Everyone"), "{child:?}: {listing}");
    }
    // Reopening passes the same check and changes nothing.
    assert!(ProjectSession::open_read_only(rules(), gil.join("state.yaml")).is_ok());
}

#[test]
fn weakened_gil_dacl_is_refused_on_reopen_and_never_repaired() {
    let root = scratch();
    drop(ProjectSession::start_visual(rules(), &root).unwrap());
    let gil = root.join(".gil");
    let state = gil.join("state.yaml");
    let before = fs::read(&state).unwrap();
    // A later grant to Everyone (as a sync tool or a user might apply) widens the records.
    let out = std::process::Command::new(system32("icacls.exe"))
        .arg(&gil).args(["/grant", "*S-1-1-0:(OI)(CI)R"]).output().unwrap();
    assert!(out.status.success());
    let widened = acl_listing(&gil);
    assert!(widened.contains("Everyone"), "{widened}");
    assert!(open_private_check(&gil).is_err());
    let refused = match ProjectSession::open_read_only(rules(), &state) {
        Ok(_) => panic!("widened .gil was accepted"),
        Err(e) => e.to_string(),
    };
    assert!(refused.contains("소유자 전용"), "{refused}");
    assert!(ProjectSession::open(rules(), &state).is_err());
    // Nothing repaired, nothing rewritten.
    assert_eq!(acl_listing(&gil), widened);
    assert_eq!(fs::read(&state).unwrap(), before);
    // A text-only (format 4) Project is not subject to this check.
    let legacy = scratch();
    ProjectSession::start(rules(), legacy.join(crate::STATE_PATH)).unwrap().commit().unwrap();
    let out = std::process::Command::new(system32("icacls.exe"))
        .arg(legacy.join(".gil")).args(["/grant", "*S-1-1-0:(OI)(CI)R"]).output().unwrap();
    assert!(out.status.success());
    assert!(ProjectSession::open_read_only(rules(), legacy.join(crate::STATE_PATH)).is_ok());
}

/// Deletes the temporary local account even when an assertion fails.
struct TestAccount {
    name: String,
    password: String,
}
impl TestAccount {
    fn create() -> Self {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let name = format!("giltest{}", nanos % 1_000_000_000);
        let password = format!("Gil!{}aZ{}", nanos % 100_000_000, std::process::id());
        let out = std::process::Command::new(system32("net.exe"))
            .args(["user", &name, &password, "/add", "/y", "/expires:never"])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "the other-account access check could not run on this host (net user /add failed): {}",
            String::from_utf8_lossy(&out.stderr)
        );
        Self { name, password }
    }
    /// Run one `cmd /c` line as the other account. Output lands in `out_dir`, which must be
    /// writable by that account. Returns (exit code, stdout, stderr).
    fn run(&self, command: &str, out_dir: &Path) -> (i32, String, String) {
        let stdout = out_dir.join(format!("out-{}.txt", std::process::id()));
        let stderr = out_dir.join(format!("err-{}.txt", std::process::id()));
        let _ = fs::remove_file(&stdout);
        let _ = fs::remove_file(&stderr);
        let script = "$ErrorActionPreference='Stop'; \
            $secure = ConvertTo-SecureString $env:GIL_TEST_PASS -AsPlainText -Force; \
            $credential = New-Object System.Management.Automation.PSCredential($env:GIL_TEST_USER, $secure); \
            $process = Start-Process -FilePath $env:GIL_TEST_CMD -ArgumentList @('/c', $env:GIL_TEST_LINE) -Credential $credential -WorkingDirectory $env:GIL_TEST_CWD -PassThru -Wait -WindowStyle Hidden -RedirectStandardOutput $env:GIL_TEST_OUT -RedirectStandardError $env:GIL_TEST_ERR; \
            exit $process.ExitCode";
        let out = std::process::Command::new(powershell())
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .env("GIL_TEST_USER", &self.name)
            .env("GIL_TEST_PASS", &self.password)
            .env("GIL_TEST_CMD", system32("cmd.exe"))
            .env("GIL_TEST_LINE", command)
            .env("GIL_TEST_CWD", out_dir)
            .env("GIL_TEST_OUT", &stdout)
            .env("GIL_TEST_ERR", &stderr)
            .output()
            .unwrap();
        let launcher_stderr = String::from_utf8_lossy(&out.stderr).to_string();
        let code = out.status.code().unwrap_or(-1);
        let read = |p: &Path| fs::read(p).map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_default();
        let (o, e) = (read(&stdout), read(&stderr));
        assert!(
            !launcher_stderr.contains("cannot be run due to the error"),
            "the other-account logon could not run on this host: {launcher_stderr}"
        );
        (code, o, e)
    }
}
impl Drop for TestAccount {
    fn drop(&mut self) {
        let _ = std::process::Command::new(system32("net.exe"))
            .args(["user", &self.name, "/delete"])
            .output();
    }
}
/// Removes the broad folder on the system drive root even when an assertion fails.
struct BroadFolder(PathBuf);
impl BroadFolder {
    fn create() -> Self {
        let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
        let path = PathBuf::from(format!("{drive}\\gil-acl-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() % 1_000_000));
        fs::create_dir(&path).unwrap();
        // Explicitly broad: every authenticated user may read, create and modify below here.
        let out = std::process::Command::new(system32("icacls.exe"))
            .arg(&path).args(["/grant", "*S-1-5-11:(OI)(CI)M"]).output().unwrap();
        assert!(out.status.success());
        Self(path)
    }
}
impl Drop for BroadFolder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn another_local_account_cannot_read_or_write_gil_records_under_a_broad_folder() {
    let broad = BroadFolder::create();
    fs::write(broad.0.join("control.txt"), b"readable by any authenticated user\r\n").unwrap();
    let root = broad.0.join("project");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("notes.txt"), b"project file\r\n").unwrap();
    drop(ProjectSession::start_visual(rules(), &root).unwrap());
    let gil = root.join(".gil");
    let state = gil.join("state.yaml");
    assert!(acl_listing(&root).contains("Authenticated Users"), "fixture must be broad");
    let account = TestAccount::create();
    let q = |p: &Path| format!("\"{}\"", p.display());
    // Control: the account works and the broad folder really is readable and writable by it.
    let (code, out, err) = account.run(&format!("type {}", q(&broad.0.join("control.txt"))), &broad.0);
    assert_eq!(code, 0, "control read failed: {err}");
    assert!(out.contains("readable by any authenticated user"), "{out}");
    let (code, _, err) = account.run(&format!("echo other>{}", q(&broad.0.join("other-wrote.txt"))), &broad.0);
    assert_eq!(code, 0, "control write failed: {err}");
    assert!(broad.0.join("other-wrote.txt").exists());
    let (code, out, _) = account.run(&format!("type {}", q(&root.join("notes.txt"))), &broad.0);
    assert_eq!(code, 0, "the surrounding project folder is broad by construction: {out}");
    // The records: no read, no listing, no write, no delete for the other account.
    let (code, out, err) = account.run(&format!("type {}", q(&state)), &broad.0);
    assert_ne!(code, 0, "state.yaml readable by another account: {out}");
    assert!(err.contains("Access is denied") || err.contains("denied"), "{err}");
    assert!(!out.contains("format:"), "{out}");
    let (code, out, _) = account.run(&format!("dir {}", q(&gil)), &broad.0);
    assert_ne!(code, 0, ".gil listable by another account: {out}");
    let (code, _, _) = account.run(&format!("echo probe>{}", q(&gil.join("probe.txt"))), &broad.0);
    assert_ne!(code, 0);
    assert!(!gil.join("probe.txt").exists(), "another account wrote into .gil");
    let (code, _, _) = account.run(&format!("del /f /q {}", q(&state)), &broad.0);
    assert!(code != 0 || state.exists(), "another account deleted state.yaml");
    assert!(state.exists());
    // The owner still reads the Project as before.
    assert!(ProjectSession::open_read_only(rules(), &state).is_ok());
    drop(account);
}

/// Deletes the local share and the mapped drive even when an assertion fails.
struct MappedShare {
    share: String,
    letter: Option<String>,
}
impl MappedShare {
    fn create(dir: &Path) -> Self {
        let share = format!("gilacl{}", std::process::id());
        let out = std::process::Command::new(system32("net.exe"))
            .args(["share", &format!("{share}={}", dir.display()), "/grant:everyone,FULL"])
            .output()
            .unwrap();
        assert!(out.status.success(), "the network-root check could not run on this host (net share failed): {}",
            String::from_utf8_lossy(&out.stdout));
        let mut made = Self { share, letter: None };
        for letter in ["Y:", "X:", "W:", "V:", "U:", "T:"] {
            if Path::new(&format!("{letter}\\")).exists() { continue; }
            let out = std::process::Command::new(system32("net.exe"))
                .args(["use", letter, &format!("\\\\localhost\\{}", made.share)])
                .output()
                .unwrap();
            if out.status.success() { made.letter = Some(letter.to_string()); break; }
        }
        assert!(made.letter.is_some(), "the network-root check could not map a drive on this host");
        made
    }
}
impl Drop for MappedShare {
    fn drop(&mut self) {
        if let Some(letter) = &self.letter {
            let _ = std::process::Command::new(system32("net.exe")).args(["use", letter, "/delete", "/y"]).output();
        }
        let _ = std::process::Command::new(system32("net.exe")).args(["share", &self.share, "/delete", "/y"]).output();
    }
}

#[test]
fn unc_and_mapped_network_roots_are_refused_without_creating_storage() {
    let base = scratch();
    let real = base.join("project");
    fs::create_dir(&real).unwrap();
    let mapped = MappedShare::create(&base);
    // UNC form of the same folder: not a drive-letter path at all.
    let unc = PathBuf::from(format!("\\\\localhost\\{}\\project", mapped.share));
    assert!(fs::metadata(&unc).map(|m| m.is_dir()).unwrap_or(false), "share not reachable: {unc:?}");
    assert!(ProjectSession::start_visual(rules(), &unc).is_err());
    assert!(!real.join(".gil").exists(), "UNC root created storage");
    // Mapped network drive: a drive letter whose type is not DRIVE_FIXED.
    let drive = PathBuf::from(format!("{}\\project", mapped.letter.as_ref().unwrap()));
    assert!(fs::metadata(&drive).map(|m| m.is_dir()).unwrap_or(false), "mapped drive not reachable: {drive:?}");
    assert!(ProjectSession::start_visual(rules(), &drive).is_err());
    assert!(!real.join(".gil").exists(), "mapped network root created storage");
    // The same folder through its local fixed path is accepted.
    drop(ProjectSession::start_visual(rules(), &real).unwrap());
    assert_initial(&real);
    drop(mapped);
}
