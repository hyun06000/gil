//! Windows mirror of `visual_start_tests.rs` (Windows contract §6, W1-*). Runs only on a
//! Windows host; a Mac cross-compile of this file is not evidence.
const VISUAL_HEAD: &[u8] = if cfg!(feature = "chain-core") { b"format: 7\n" } else { b"format: 5\n" };

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
        let result = create(rules(), &root, &root, true, &mut |at| {
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
    create(rules(), &root, &root, true, &mut |at| {
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
    /// Run one command line as the other account through a batch file in `out_dir` (which that
    /// account can read and write). Returns (exit code, stdout, stderr); the launcher's own
    /// output is folded into the panic message when the logon itself cannot run here.
    fn run(&self, command: &str, out_dir: &Path) -> (i32, String, String) {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let n = SERIAL.fetch_add(1, Ordering::Relaxed);
        let batch = out_dir.join(format!("job-{}-{n}.cmd", std::process::id()));
        let stdout = out_dir.join(format!("out-{}-{n}.txt", std::process::id()));
        let stderr = out_dir.join(format!("err-{}-{n}.txt", std::process::id()));
        fs::write(&batch, format!("@echo off\r\nwhoami\r\n{command}\r\nexit /b %ERRORLEVEL%\r\n")).unwrap();
        // SecureString built without the PowerShell Security module, which Windows PowerShell
        // could not load on the runner (CI run 39129041207).
        let script = "try { \
            $secure = New-Object System.Security.SecureString; foreach ($ch in $env:GIL_TEST_PASS.ToCharArray()) { $secure.AppendChar($ch) }; \
            $credential = New-Object System.Management.Automation.PSCredential($env:GIL_TEST_USER, $secure); \
            $process = Start-Process -FilePath $env:GIL_TEST_CMD -ArgumentList @('/c', $env:GIL_TEST_BAT) -Credential $credential -WorkingDirectory $env:GIL_TEST_CWD -PassThru -Wait -RedirectStandardOutput $env:GIL_TEST_OUT -RedirectStandardError $env:GIL_TEST_ERR; \
            exit $process.ExitCode \
            } catch { Write-Error ($_ | Out-String); exit 99 }";
        let out = std::process::Command::new(powershell())
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .env("GIL_TEST_USER", &self.name)
            .env("GIL_TEST_PASS", &self.password)
            .env("GIL_TEST_CMD", system32("cmd.exe"))
            .env("GIL_TEST_BAT", &batch)
            .env("GIL_TEST_CWD", out_dir)
            .env("GIL_TEST_OUT", &stdout)
            .env("GIL_TEST_ERR", &stderr)
            .output()
            .unwrap();
        let launcher = format!("launcher stdout: {} / stderr: {}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        let code = out.status.code().unwrap_or(-1);
        assert_ne!(code, 99, "the other-account logon could not run on this host: {launcher}");
        let read = |p: &Path| fs::read(p).map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_default();
        let (o, e) = (read(&stdout), read(&stderr));
        assert!(
            o.to_lowercase().contains(&self.name.to_lowercase()),
            "the command did not run as {}: code {code}, stdout {o:?}, stderr {e:?}, {launcher}", self.name
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
    // cmd keeps ERRORLEVEL 0 when a redirection target cannot be opened (CI run 39129490528):
    // the evidence is the absent file and the denial on stderr, not the exit code.
    let (_, _, err) = account.run(&format!("echo probe>{}", q(&gil.join("probe.txt"))), &broad.0);
    assert!(!gil.join("probe.txt").exists(), "another account wrote into .gil");
    assert!(err.contains("denied"), "write into .gil was not an access denial: {err}");
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

// ── Windows contract §6 W8-*: handle-first reservation (design A) ─────────────────────────────

use super::reserve::hooks;

fn ace_summary(file: &fs::File) -> String {
    // Principals and flags as icacls would list them are asserted elsewhere; here the product
    // checks are reused on the handle so the assertion is the effective descriptor.
    match crate::windows_fs::private_dir(file) {
        Ok(()) => "owner-only".into(),
        Err(e) => e.to_string(),
    }
}

#[test]
fn reservation_is_owner_only_before_normalization_and_nothing_else_is_written() {
    thread_local! { static SEEN: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) }; }
    fn observe(file: &fs::File) {
        SEEN.with(|s| s.borrow_mut().push(ace_summary(file)));
    }
    hooks::BEFORE_NORMALIZE.with(|c| c.set(Some(observe)));
    let root = scratch();
    let result = ProjectSession::start_visual(rules(), &root);
    hooks::BEFORE_NORMALIZE.with(|c| c.set(None));
    result.unwrap();
    let seen = SEEN.with(|s| s.borrow().clone());
    assert_eq!(seen.len(), 1, "the reservation hook must run exactly once");
    // Straight out of NtCreateFile, before SetSecurityInfo: owner, protected bit and the two
    // ACEs must already be in place (so there is no moment with an inherited, wider DACL).
    assert_eq!(seen[0], "owner-only", "pre-normalization descriptor: {}", seen[0]);
    assert_initial(&root);
}

#[test]
fn existing_entry_collision_leaves_that_entry_and_its_acl_untouched() {
    for kind in ["file", "directory", "junction"] {
        let root = scratch();
        let gil = root.join(".gil");
        let target = root.join("elsewhere");
        fs::create_dir(&target).unwrap();
        match kind {
            "file" => fs::write(&gil, b"not ours").unwrap(),
            "directory" => {
                fs::create_dir(&gil).unwrap();
                fs::write(gil.join("keep.txt"), b"keep").unwrap();
            }
            _ => junction(&gil, &target),
        }
        // Give the colliding entry a distinctive DACL so any change would show.
        let out = std::process::Command::new(system32("icacls.exe"))
            .arg(&gil).args(["/grant", "*S-1-1-0:(R)"]).output().unwrap();
        assert!(out.status.success(), "{kind}");
        let before_acl = acl_listing(&gil);
        let before = inventory(&root);
        let err = ProjectSession::start_visual(rules(), &root).unwrap_err().to_string();
        assert!(!err.contains("제거하고 거절"), "{kind}: collision must fail before any cleanup: {err}");
        assert_eq!(inventory(&root), before, "{kind}");
        assert_eq!(acl_listing(&gil), before_acl, "{kind}: colliding entry's ACL changed");
        if kind == "junction" {
            assert_eq!(fs::read_dir(&target).unwrap().count(), 0);
        }
    }
}

#[test]
fn verification_failure_removes_only_the_fresh_reservation() {
    let root = scratch();
    let sibling = root.join("sibling");
    fs::create_dir(&sibling).unwrap();
    fs::write(sibling.join("keep.txt"), b"keep").unwrap();
    fs::write(root.join("work.txt"), b"work").unwrap();
    let before = inventory(&root);
    hooks::FAIL_VERIFY.with(|c| c.set(true));
    let result = ProjectSession::start_visual(rules(), &root);
    hooks::FAIL_VERIFY.with(|c| c.set(false));
    let err = result.unwrap_err().to_string();
    assert!(err.contains("제거하고 거절"), "{err}");
    assert!(!root.join(".gil").exists(), "the fresh reservation must be gone");
    assert_eq!(inventory(&root), before, "only the reservation may disappear");
    // The folder is usable again afterwards.
    drop(ProjectSession::start_visual(rules(), &root).unwrap());
    assert_initial(&root);
}

#[test]
fn handle_transition_swap_is_detected_and_refused_without_touching_either_object() {
    fn swap(gil: &Path) {
        // No delete-denying handle is held at this moment: a same-user writer can move the
        // reservation away and plant another directory under the name.
        let held = gil.with_file_name("held-reservation");
        fs::rename(gil, &held).unwrap();
        fs::create_dir(gil).unwrap();
        fs::write(gil.join("planted.txt"), b"planted").unwrap();
    }
    let root = scratch();
    hooks::DURING_TRANSITION.with(|c| c.set(Some(swap)));
    let result = ProjectSession::start_visual(rules(), &root);
    hooks::DURING_TRANSITION.with(|c| c.set(None));
    let err = result.unwrap_err().to_string();
    assert!(err.contains("핸들 전환"), "{err}");
    // The planted directory is neither deleted nor changed: inherited ACL, contents intact.
    let planted = root.join(".gil");
    assert_eq!(fs::read(planted.join("planted.txt")).unwrap(), b"planted");
    assert!(open_private_check(&planted).is_err(), "planted directory must keep its inherited ACL");
    assert!(!planted.join("state.yaml").exists() && !planted.join("project.lock").exists(), "nothing written");
    // Our reservation survives under the other name, owner-only and empty: not cleaned by name.
    let held = root.join("held-reservation");
    assert!(open_private_check(&held).is_ok(), "our reservation keeps its descriptor");
    assert_eq!(fs::read_dir(&held).unwrap().count(), 0);
}

#[test]
fn widened_child_entries_are_refused_on_reopen_or_on_read_but_never_repaired() {
    use crate::artifact::ObjectStore;
    let root = scratch();
    drop(ProjectSession::start_visual(rules(), &root).unwrap());
    let gil = root.join(".gil");
    let state = gil.join("state.yaml");
    // Two blobs through the real store under the explicit owner-only contract.
    let store = ObjectStore::owner_only(&gil);
    let used = store.put_blob(&b"blob to widen"[..]).unwrap();
    let other = store.put_blob(&b"blob left alone"[..]).unwrap();
    let hex = |bytes: &[u8]| -> String {
        use sha2::{Digest, Sha256};
        Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
    };
    let blob_path = |h: &str| gil.join("artifacts/blobs/sha256").join(&h[..2]).join(&h[2..]);
    let used_path = blob_path(&hex(b"blob to widen"));
    assert!(used_path.exists(), "published blob path");
    let _ = &other;
    assert!(ProjectSession::open_read_only(rules(), &state).is_ok());
    // 1. A widened blob is not noticed by reopening (documented limit) but refused when read.
    let out = std::process::Command::new(system32("icacls.exe")).arg(&used_path).args(["/grant", "*S-1-1-0:(R)"]).output().unwrap();
    assert!(out.status.success());
    let widened_blob_acl = acl_listing(&used_path);
    assert!(ProjectSession::open_read_only(rules(), &state).is_ok(), "an unused widened blob is not detected at reopen (contract §8 item 3)");
    let mut sink = Vec::new();
    assert!(store.read_blob(&used, &mut sink).is_err(), "widened blob must be refused on read");
    assert!(sink.is_empty());
    let mut sink = Vec::new();
    store.read_blob(&other, &mut sink).unwrap();
    assert_eq!(sink, b"blob left alone");
    assert_eq!(acl_listing(&used_path), widened_blob_acl, "no repair");
    // 2. A widened control file is refused at reopen; nothing is repaired or rewritten.
    let before = fs::read(&state).unwrap();
    let out = std::process::Command::new(system32("icacls.exe")).arg(&state).args(["/grant", "*S-1-1-0:(R)"]).output().unwrap();
    assert!(out.status.success());
    let widened_state_acl = acl_listing(&state);
    let refused = match ProjectSession::open_read_only(rules(), &state) {
        Ok(_) => panic!("widened state.yaml was accepted"),
        Err(e) => e.to_string(),
    };
    assert!(refused.contains("소유자 전용") && refused.contains("state.yaml"), "{refused}");
    assert_eq!(acl_listing(&state), widened_state_acl);
    assert_eq!(fs::read(&state).unwrap(), before);
    // 3. A structure directory that stops inheriting is refused too (`artifacts/manifests` always
    //    exists after creation; `artifacts/blobs` does not for an empty folder, CI run 39162190894).
    let root2 = scratch();
    drop(ProjectSession::start_visual(rules(), &root2).unwrap());
    let manifests = root2.join(".gil/artifacts/manifests");
    assert!(manifests.is_dir());
    let out = std::process::Command::new(system32("icacls.exe")).arg(&manifests).args(["/inheritance:d"]).output().unwrap();
    assert!(out.status.success(), "icacls: {} {}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let refused = match ProjectSession::open_read_only(rules(), root2.join(crate::STATE_PATH)) {
        Ok(_) => panic!("protected artifacts/manifests was accepted"),
        Err(e) => e.to_string(),
    };
    assert!(refused.contains("artifacts/manifests"), "{refused}");
}

// ── Windows contract §6 W9-*: explicit storage contract and read-only verification order ──────

fn grant_everyone_read(path: &Path) {
    let out = std::process::Command::new(system32("icacls.exe")).arg(path).args(["/grant", "*S-1-1-0:(R)"]).output().unwrap();
    assert!(out.status.success(), "icacls grant: {}", String::from_utf8_lossy(&out.stdout));
}
fn revoke_everyone(path: &Path) {
    let out = std::process::Command::new(system32("icacls.exe")).arg(path).args(["/remove:g", "*S-1-1-0"]).output().unwrap();
    assert!(out.status.success(), "icacls remove: {}", String::from_utf8_lossy(&out.stdout));
}

#[test]
fn widened_gil_never_skips_object_checks_under_the_owner_only_contract() {
    use crate::artifact::ObjectStore;
    let root = scratch();
    drop(ProjectSession::start_visual(rules(), &root).unwrap());
    let gil = root.join(".gil");
    let protected = ObjectStore::owner_only(&gil);
    let digest = protected.put_blob(&b"stored before widening"[..]).unwrap();
    grant_everyone_read(&gil);
    // The contract is explicit: a widened .gil is a refusal for reads and for writes.
    let mut sink = Vec::new();
    assert!(protected.read_blob(&digest, &mut sink).is_err(), "read must not skip the check");
    assert!(sink.is_empty());
    let tmp_before = fs::read_dir(gil.join("artifacts/tmp")).map(|d| d.count()).unwrap_or(0);
    assert!(protected.put_blob(&b"new bytes after widening"[..]).is_err(), "write must not skip the check");
    assert_eq!(fs::read_dir(gil.join("artifacts/tmp")).map(|d| d.count()).unwrap_or(0), tmp_before, "no temporary created");
    // The same bytes through the inherited (format-4) contract are not subject to the check.
    let mut sink = Vec::new();
    ObjectStore::at(&gil).read_blob(&digest, &mut sink).unwrap();
    assert_eq!(sink, b"stored before widening");
    // Reopening the image Project is refused as well; nothing is repaired.
    let listing = acl_listing(&gil);
    assert!(ProjectSession::open_read_only(rules(), gil.join("state.yaml")).is_err());
    assert_eq!(acl_listing(&gil), listing);
}

/// Restores ownership and removes the deny ACE even when an assertion fails.
struct UnreadableSecurity(PathBuf, String);
impl UnreadableSecurity {
    fn new(dir: &Path) -> Self {
        let who = std::process::Command::new(system32("whoami.exe")).output().unwrap();
        let account = String::from_utf8(who.stdout).unwrap().trim().to_string();
        // Owner → SYSTEM (owners are implicitly granted READ_CONTROL), then deny READ_CONTROL.
        for args in [vec!["/setowner", "*S-1-5-18"], vec!["/deny", &format!("{account}:(RC)")]] {
            let out = std::process::Command::new(system32("icacls.exe")).arg(dir).args(&args).output().unwrap();
            assert!(out.status.success(), "icacls {args:?}: {}", String::from_utf8_lossy(&out.stdout));
        }
        Self(dir.to_owned(), account)
    }
}
impl Drop for UnreadableSecurity {
    fn drop(&mut self) {
        let _ = std::process::Command::new(system32("icacls.exe")).arg(&self.0).args(["/remove:d", &self.1]).output();
        let _ = std::process::Command::new(system32("icacls.exe")).arg(&self.0).args(["/setowner", &self.1]).output();
    }
}

#[test]
fn acl_query_error_is_a_refusal_not_a_downgrade_to_the_inherited_contract() {
    use crate::artifact::ObjectStore;
    let root = scratch();
    drop(ProjectSession::start_visual(rules(), &root).unwrap());
    let gil = root.join(".gil");
    let protected = ObjectStore::owner_only(&gil);
    let digest = protected.put_blob(&b"readable while verifiable"[..]).unwrap();
    let before = inventory(&root);
    {
        let _unreadable = UnreadableSecurity::new(&gil);
        let mut sink = Vec::new();
        assert!(protected.read_blob(&digest, &mut sink).is_err(), "a security query error must refuse");
        assert!(sink.is_empty());
        assert!(protected.put_blob(&b"must not be published"[..]).is_err());
        assert!(crate::windows_fs::verify_storage_tree(&gil).is_err());
        assert!(ProjectSession::open_read_only(rules(), gil.join("state.yaml")).is_err());
        assert!(ProjectSession::open(rules(), gil.join("state.yaml")).is_err());
    }
    assert_eq!(inventory(&root), before, "nothing published or cleaned while unverifiable");
}

#[test]
fn acl_refusal_happens_before_recovery_and_reaping_and_leaves_them_untouched() {
    let root = scratch();
    drop(ProjectSession::start_visual(rules(), &root).unwrap());
    let gil = root.join(".gil");
    let state = gil.join("state.yaml");
    // Pending material of every kind: an unfinished temporary object, a half-written state,
    // and a recovery record that `open` would otherwise act on first.
    fs::create_dir_all(gil.join("artifacts/tmp")).unwrap();
    fs::write(gil.join("artifacts/tmp").join(format!("{}-7", std::process::id())), b"unfinished object").unwrap();
    fs::write(gil.join("state.yaml.writing"), b"format: 5\nhalf").unwrap();
    fs::create_dir_all(gil.join("restore/active")).unwrap();
    fs::write(gil.join("restore/active/PLAN"), b"bogus plan").unwrap();
    grant_everyone_read(&state);
    let before = inventory(&root);
    let refused = match ProjectSession::open(rules(), &state) {
        Ok(_) => panic!("widened state.yaml was accepted"),
        Err(e) => e.to_string(),
    };
    assert!(refused.contains("소유자 전용"), "refusal must be the ACL gate, not recovery: {refused}");
    assert_eq!(inventory(&root), before, "recovery records, temporaries and the lock file must be untouched");
    // Once the widening is removed the same open reaches the recovery step (whatever it says).
    revoke_everyone(&state);
    let after_gate = match ProjectSession::open(rules(), &state) {
        Ok(_) => "opened".to_string(),
        Err(e) => e.to_string(),
    };
    assert!(!after_gate.contains("소유자 전용"), "{after_gate}");
    // A second Project with only a temporary: refused untouched while widened, reaped once not.
    let root2 = scratch();
    drop(ProjectSession::start_visual(rules(), &root2).unwrap());
    let gil2 = root2.join(".gil");
    let temp = gil2.join("artifacts/tmp").join(format!("{}-9", std::process::id()));
    fs::create_dir_all(temp.parent().unwrap()).unwrap();
    fs::write(&temp, b"unfinished").unwrap();
    grant_everyone_read(&gil2.join("state.yaml"));
    assert!(ProjectSession::open(rules(), gil2.join("state.yaml")).is_err());
    assert_eq!(fs::read(&temp).unwrap(), b"unfinished", "temporary must survive the refusal");
    revoke_everyone(&gil2.join("state.yaml"));
    drop(ProjectSession::open(rules(), gil2.join("state.yaml")).unwrap());
    assert!(!temp.exists(), "after the gate passes, reaping proceeds as before");
}

#[test]
fn format_four_projects_keep_the_inherited_contract_and_their_flow() {
    use crate::artifact::ObjectStore;
    let root = scratch();
    fs::write(root.join("note.txt"), b"text project").unwrap();
    ProjectSession::start(rules(), root.join(crate::STATE_PATH)).unwrap().commit().unwrap();
    let gil = root.join(".gil");
    grant_everyone_read(&gil);
    // No ACL gate for format 4: open, reap, read and write all work as before.
    let temp = gil.join("artifacts/tmp").join(format!("{}-11", std::process::id()));
    fs::create_dir_all(temp.parent().unwrap()).unwrap();
    fs::write(&temp, b"leftover").unwrap();
    let session = ProjectSession::open(rules(), root.join(crate::STATE_PATH)).unwrap();
    assert!(!temp.exists(), "format-4 reaping unchanged");
    assert!(session.project().visual_state().is_none());
    drop(session);
    let store = ObjectStore::at(&gil);
    let digest = store.put_blob(&b"format-4 bytes"[..]).unwrap();
    let mut sink = Vec::new();
    store.read_blob(&digest, &mut sink).unwrap();
    assert_eq!(sink, b"format-4 bytes");
    assert!(ProjectSession::open_read_only(rules(), root.join(crate::STATE_PATH)).is_ok());
}

// ── Windows contract §6 W10-*: declared-format classification before recovery ────────────────

use crate::session::open_hooks;

/// Pending material of every kind that `open` would otherwise act on.
fn plant_pending(gil: &Path) {
    fs::create_dir_all(gil.join("artifacts/tmp")).unwrap();
    fs::write(gil.join("artifacts/tmp").join(format!("{}-21", std::process::id())), b"unfinished object").unwrap();
    fs::write(gil.join("state.yaml.writing"), b"format: 5\nhalf").unwrap();
    fs::create_dir_all(gil.join("restore/active")).unwrap();
    fs::write(gil.join("restore/active/PLAN"), b"bogus plan").unwrap();
}

#[test]
fn format_five_state_with_leading_comment_and_blank_lines_opens_like_the_loader() {
    let root = scratch();
    drop(ProjectSession::start_visual(rules(), &root).unwrap());
    let state = root.join(crate::STATE_PATH);
    let original = fs::read(&state).unwrap();
    let mut decorated = b"# written by a tool\n\n# format is not on the first line\n\n".to_vec();
    decorated.extend_from_slice(&original);
    fs::write(&state, &decorated).unwrap();
    let read = ProjectSession::open_read_only(rules(), &state).unwrap();
    assert!(read.project().visual_state().is_some());
    drop(read);
    assert_eq!(fs::read(&state).unwrap(), decorated, "a read-only open rewrites nothing");
    drop(ProjectSession::open(rules(), &state).unwrap());
}

#[test]
fn unclear_format_is_refused_before_recovery_and_reaping_with_everything_preserved() {
    for case in ["absent", "undeclared", "unknown", "unreadable"] {
        let root = scratch();
        drop(ProjectSession::start_visual(rules(), &root).unwrap());
        let gil = root.join(".gil");
        let state = gil.join("state.yaml");
        match case {
            "absent" => fs::remove_file(&state).unwrap(),
            "undeclared" => fs::write(&state, b"# nothing declared\nvisuals: {}\n").unwrap(),
            "unknown" => fs::write(&state, b"# comment\nformat: 99\n").unwrap(),
            _ => fs::write(&state, [0xff, 0xfe, b'f', b'o', b'r', b'm', b'a', b't', b':', b' ', b'5', b'\n']).unwrap(),
        }
        plant_pending(&gil);
        let before = inventory(&root);
        assert!(ProjectSession::open(rules(), &state).is_err(), "{case}");
        assert!(ProjectSession::open_read_only(rules(), &state).is_err(), "{case}");
        assert_eq!(inventory(&root), before, "{case}: recovery record, temporaries, .writing and lock must be untouched");
    }
}

#[test]
fn format_change_between_classification_and_lock_is_refused_untouched() {
    // 5 → 4 (textual): the declared format differs under the lock.
    fn downgrade(state: &Path) {
        let text = fs::read_to_string(state).unwrap().replacen(if cfg!(feature = "chain-core") { "format: 7" } else { "format: 5" }, if cfg!(feature = "chain-core") { "format: 6" } else { "format: 4" }, 1);
        fs::write(state, text).unwrap();
    }
    let root = scratch();
    drop(ProjectSession::start_visual(rules(), &root).unwrap());
    let gil = root.join(".gil");
    plant_pending(&gil);
    open_hooks::BETWEEN_CLASSIFY_AND_LOCK.with(|c| c.set(Some(downgrade)));
    let result = ProjectSession::open(rules(), gil.join("state.yaml"));
    open_hooks::BETWEEN_CLASSIFY_AND_LOCK.with(|c| c.set(None));
    let before = inventory(&root); // captured after the hook rewrote state.yaml
    let err = result.err().map(|e| e.to_string()).expect("changed format must refuse");
    assert!(err.contains("잠금 전"), "{err}");
    assert_eq!(inventory(&root), before);
    // 4 → 5: a text Project rewritten as format 5 between classification and lock must get
    // the ACL verification under the lock (its .gil inherits, so it is refused) and no reaping.
    fn upgrade(state: &Path) {
        let text = fs::read_to_string(state).unwrap().replacen(if cfg!(feature = "chain-core") { "format: 6" } else { "format: 4" }, if cfg!(feature = "chain-core") { "format: 7" } else { "format: 5" }, 1);
        fs::write(state, text).unwrap();
    }
    let root4 = scratch();
    ProjectSession::start(rules(), root4.join(crate::STATE_PATH)).unwrap().commit().unwrap();
    let gil4 = root4.join(".gil");
    let temp = gil4.join("artifacts/tmp").join(format!("{}-23", std::process::id()));
    fs::create_dir_all(temp.parent().unwrap()).unwrap();
    fs::write(&temp, b"unfinished").unwrap();
    open_hooks::BETWEEN_CLASSIFY_AND_LOCK.with(|c| c.set(Some(upgrade)));
    let result = ProjectSession::open(rules(), gil4.join("state.yaml"));
    open_hooks::BETWEEN_CLASSIFY_AND_LOCK.with(|c| c.set(None));
    let err = result.err().map(|e| e.to_string()).expect("format change must refuse");
    assert!(err.contains("잠금 전"), "{err}");
    assert_eq!(fs::read(&temp).unwrap(), b"unfinished", "no reaping before the gate");
}

#[test]
fn format_four_recovery_and_reaping_still_run_after_classification() {
    let root = scratch();
    fs::write(root.join("note.txt"), b"text project").unwrap();
    ProjectSession::start(rules(), root.join(crate::STATE_PATH)).unwrap().commit().unwrap();
    let gil = root.join(".gil");
    let temp = gil.join("artifacts/tmp").join(format!("{}-25", std::process::id()));
    fs::create_dir_all(temp.parent().unwrap()).unwrap();
    fs::write(&temp, b"leftover").unwrap();
    // A leading comment on a format-4 state is classified like the loader does: still format 4.
    let state = gil.join("state.yaml");
    let mut text = b"# tool comment\n\n".to_vec();
    text.extend_from_slice(&fs::read(&state).unwrap());
    fs::write(&state, &text).unwrap();
    let session = ProjectSession::open(rules(), &state).unwrap();
    assert!(session.project().visual_state().is_none());
    assert!(!temp.exists(), "format-4 reaping unchanged");
    drop(session);
    // No pending recovery record: the read-only path reports none.
    assert!(ProjectSession::open_read_only(rules(), &state).is_ok());
}
