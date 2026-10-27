//! The built `gil.exe` driven over MCP stdio **as a temporary standard (non-administrator) local
//! user** in that user's own profile folder (no OneDrive, no synchronization agent): format-5
//! creation, reopen through `gil_status`/`gil_context`, the effective `.gil` DACL, and the exact
//! refusal that appears when an external agent re-enables inheritance on `.gil` after creation.
//! Windows contract §6 W11; investigation of the 2026-10-21 PC report.
#![cfg(all(windows, feature = "visual-authoring"))]
use std::fs;
use std::path::{Path, PathBuf};

fn system32(tool: &str) -> PathBuf {
    PathBuf::from(std::env::var_os("SystemRoot").unwrap()).join("System32").join(tool)
}
fn powershell() -> PathBuf {
    system32("WindowsPowerShell/v1.0/powershell.exe")
}
/// Readable and writable by every authenticated user: the standard user needs the executable,
/// the driver script and a place for its output.
struct SharedFolder(PathBuf);
impl SharedFolder {
    fn create() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
        let path = PathBuf::from(format!("{drive}\\gil-ordinary-{}-{}", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
        fs::create_dir(&path).unwrap();
        let out = std::process::Command::new(system32("icacls.exe")).arg(&path).args(["/grant", "*S-1-5-11:(OI)(CI)M"]).output().unwrap();
        assert!(out.status.success());
        Self(path)
    }
}
impl Drop for SharedFolder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
struct Account {
    name: String,
    password: String,
}
impl Account {
    fn create() -> Self {
        // Unique per process *and* per call: the three tests of this binary run on parallel
        // threads, and the Windows clock ticks coarsely enough that two of them read the same
        // nanosecond value (CI run 39921435283: "The account already exists"). Names stay
        // within the 20-character limit: "gilou" + ≤ 5 + 1 + ≤ 6 digits.
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let name = format!("gilou{}{}{}", std::process::id() % 100_000, NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % 10, nanos % 1_000_000);
        let password = format!("Gil!{}bY{}", nanos % 100_000_000, std::process::id());
        let out = std::process::Command::new(system32("net.exe")).args(["user", &name, &password, "/add", "/y", "/expires:never"]).output().unwrap();
        assert!(out.status.success(), "cannot create a standard user here: {}", String::from_utf8_lossy(&out.stderr));
        Self { name, password }
    }
    /// Run a PowerShell script file as this user (script text is loaded from the file and
    /// executed as a script block, so no execution-policy change is needed). A `-Credential`
    /// process gets the other user's environment, not ours, so every value travels as a
    /// positional script argument embedded in the inner command line (ASCII paths only).
    fn run_script(&self, script: &Path, out_dir: &Path, args: &[&str]) -> (i32, String, String) {
        self.run_script_with_profile(script, out_dir, args, true)
    }
    /// `load_profile` decides whether the standard user's registry hive is loaded for the
    /// process (`Start-Process -LoadUserProfile`). Without it the hive may be absent, which is
    /// exactly the "cannot query the sync-root sources" state the Core must refuse.
    fn run_script_with_profile(&self, script: &Path, out_dir: &Path, args: &[&str], load_profile: bool) -> (i32, String, String) {
        let stdout = out_dir.join("ou-stdout.txt");
        let stderr = out_dir.join("ou-stderr.txt");
        let _ = fs::remove_file(&stdout);
        let _ = fs::remove_file(&stderr);
        let launcher = "try { \
            $secure = New-Object System.Security.SecureString; \
            foreach ($ch in $env:GIL_TEST_PASS.ToCharArray()) { $secure.AppendChar($ch) }; \
            $credential = New-Object System.Management.Automation.PSCredential($env:GIL_TEST_USER, $secure); \
            $inner = $env:GIL_TEST_INNER; \
            $extra = @{}; if ($env:GIL_TEST_LOAD_PROFILE -eq '1') { $extra['LoadUserProfile'] = $true }; \
            $process = Start-Process -FilePath $env:GIL_TEST_PS -ArgumentList @('-NoProfile', '-NonInteractive', '-Command', $inner) -Credential $credential -WorkingDirectory $env:GIL_TEST_CWD -PassThru -Wait -RedirectStandardOutput $env:GIL_TEST_OUT -RedirectStandardError $env:GIL_TEST_ERR @extra; \
            exit $process.ExitCode \
            } catch { Write-Error ($_ | Out-String); exit 99 }";
        for text in std::iter::once(script.to_str().unwrap()).chain(args.iter().copied()) {
            assert!(text.is_ascii() && !text.contains('\''), "inner command values must be plain ASCII: {text}");
        }
        let quoted: Vec<String> = args.iter().map(|a| format!("'{a}'")).collect();
        let inner = format!("& ([scriptblock]::Create((Get-Content -Raw -LiteralPath '{}'))) {}", script.display(), quoted.join(" "));
        let mut command = std::process::Command::new(powershell());
        command.args(["-NoProfile", "-NonInteractive", "-Command", launcher])
            .env("GIL_TEST_USER", &self.name)
            .env("GIL_TEST_PASS", &self.password)
            .env("GIL_TEST_PS", powershell())
            .env("GIL_TEST_INNER", inner)
            .env("GIL_TEST_CWD", out_dir)
            .env("GIL_TEST_OUT", &stdout)
            .env("GIL_TEST_ERR", &stderr)
            .env("GIL_TEST_LOAD_PROFILE", if load_profile { "1" } else { "0" });
        let out = command.output().unwrap();
        let code = out.status.code().unwrap_or(-1);
        assert_ne!(code, 99, "the standard-user logon could not run on this host: {}", String::from_utf8_lossy(&out.stderr));
        let read = |p: &Path| fs::read(p).map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_default();
        (code, read(&stdout), read(&stderr))
    }
}
impl Drop for Account {
    fn drop(&mut self) {
        let _ = std::process::Command::new(system32("net.exe")).args(["user", &self.name, "/delete"]).output();
    }
}

/// The driver: speaks MCP over stdio to the executable named by GIL_EXE, in a synthetic folder
/// under the running user's own profile, then prints the tool results and the effective ACLs.
/// MODE=normal: create → status → context. MODE=reinherit: create, then re-enable inheritance on
/// .gil the way an external agent would, then status.
const DRIVER: &str = r#"
param($Exe, $Mode)
$ErrorActionPreference = 'Stop'
$root = Join-Path $env:USERPROFILE ('gil-synthetic-' + $Mode)
New-Item -ItemType Directory -Path $root | Out-Null
$sid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$hiveLoaded = Test-Path -LiteralPath ("Registry::HKEY_USERS\" + $sid)
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = $Exe
$psi.Arguments = 'mcp --serve'
$psi.WorkingDirectory = $root
$psi.UseShellExecute = $false
$psi.RedirectStandardInput = $true
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$psi.EnvironmentVariables['GIL_MONITOR_STATE_DIR'] = (Join-Path $root 'bindings')
$p = [System.Diagnostics.Process]::Start($psi)
function Send($o) { $p.StandardInput.WriteLine(($o | ConvertTo-Json -Compress -Depth 12)); $p.StandardInput.Flush() }
function Recv() { return $p.StandardOutput.ReadLine() }
Send @{jsonrpc='2.0'; id=1; method='initialize'; params=@{protocolVersion='2025-06-18'; capabilities=@{}; clientInfo=@{name='ordinary-user'; version='1'}}}
$null = Recv
Send @{jsonrpc='2.0'; method='notifications/initialized'}
Send @{jsonrpc='2.0'; id=2; method='tools/call'; params=@{name='gil_start'; arguments=@{project_root=$root}}}
$start = Recv
$layout = Join-Path $env:LOCALAPPDATA 'GIL\project-state-v1'
$recordRoot = Get-ChildItem -LiteralPath $layout -Directory | Select-Object -First 1 -ExpandProperty FullName
$gil = Join-Path $recordRoot '.gil'
$migration = ''
$sourceHash = ''
if ($Mode -eq 'migrate') {
  Copy-Item -LiteralPath $gil -Destination (Join-Path $root '.gil') -Recurse
  & icacls (Join-Path $root '.gil') /inheritance:e | Out-Null
  $sourceHash = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $root '.gil\state.yaml')).Hash
  Remove-Item -LiteralPath $recordRoot -Recurse -Force
  Send @{jsonrpc='2.0'; id=22; method='tools/call'; params=@{name='gil_start'; arguments=@{project_root=$root}}}
  $migration = Recv
  $recordRoot = Get-ChildItem -LiteralPath $layout -Directory | Where-Object { $_.Name -notlike '.migrating-*' } | Select-Object -First 1 -ExpandProperty FullName
  if (-not $recordRoot) { throw "migration created no external record; response=$migration" }
  $gil = Join-Path $recordRoot '.gil'
}
if ($Mode -eq 'reinherit') {
  & icacls $gil /inheritance:e | Out-Null
}
Send @{jsonrpc='2.0'; id=3; method='tools/call'; params=@{name='gil_status'; arguments=@{project_root=$root}}}
$status = Recv
Send @{jsonrpc='2.0'; id=4; method='tools/call'; params=@{name='gil_context'; arguments=@{project_root=$root}}}
$context = Recv
$p.StandardInput.Close()
$null = $p.WaitForExit(15000)
"WHOAMI=" + (whoami)
"HIVE_LOADED=" + $hiveLoaded
"LOCAL_APP_DATA=" + $env:LOCALAPPDATA
"ROOT_ENTRIES=" + ((Get-ChildItem -LiteralPath $root -Force | ForEach-Object { $_.Name }) -join ';')
"GROUPS=" + ((whoami /groups /fo csv /nh) -join ';')
"START=" + $start
"MIGRATION=" + $migration
"STATUS=" + $status
"CONTEXT=" + $context
"ACL_ROOT=" + ((icacls $root) -join ' | ')
"RECORD_ROOT=" + $recordRoot
"ACL_GIL=" + ((icacls $gil) -join ' | ')
"ACL_STATE=" + ((icacls (Join-Path $gil 'state.yaml')) -join ' | ')
"STATE_HEAD=" + ((Get-Content -LiteralPath (Join-Path $gil 'state.yaml') -TotalCount 1) -join '')
"SOURCE_HASH=" + $sourceHash
"EXTERNAL_HASH=" + ((Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $gil 'state.yaml')).Hash)
"#;

fn setup() -> (SharedFolder, PathBuf, PathBuf, Account) {
    let shared = SharedFolder::create();
    let exe = shared.0.join("gil.exe");
    fs::copy(env!("CARGO_BIN_EXE_gil"), &exe).unwrap();
    let script = shared.0.join("driver.ps1");
    fs::write(&script, DRIVER).unwrap();
    let account = Account::create();
    (shared, exe, script, account)
}
fn field<'a>(out: &'a str, key: &str) -> &'a str {
    out.lines().find_map(|l| l.strip_prefix(&format!("{key}="))).unwrap_or_else(|| panic!("{key} missing in {out}"))
}

#[test]
fn standard_user_creates_reopens_and_keeps_an_owner_only_gil_in_a_normal_folder() {
    let (shared, exe, script, account) = setup();
    let (code, out, err) = account.run_script(&script, &shared.0, &[exe.to_str().unwrap(), "normal"]);
    assert_eq!(code, 0, "driver failed: {err}\n{out}");
    assert!(!field(&out, "GROUPS").to_lowercase().contains("s-1-5-32-544") || field(&out, "GROUPS").contains("Deny"), "the account must not be an effective administrator: {}", field(&out, "GROUPS"));
    let start = field(&out, "START");
    assert_eq!(field(&out, "HIVE_LOADED"), "True", "fixture: the profile hive must be loaded in this mode");
    eprintln!("OBSERVED mode=normal HIVE_LOADED={} BRANCH=started", field(&out, "HIVE_LOADED"));
    assert!(start.contains("\"ok\":true") && start.contains("\"storage_format\":5"), "{start}");
    assert!(start.contains("\"record_storage\":\"private_app_data\""), "{start}");
    assert!(!field(&out, "ROOT_ENTRIES").contains(".gil"), "working tree must not contain private records: {}", field(&out, "ROOT_ENTRIES"));
    assert!(field(&out, "RECORD_ROOT").starts_with(field(&out, "LOCAL_APP_DATA")), "records must be in local app data: {}", field(&out, "RECORD_ROOT"));
    assert!(field(&out, "STATUS").contains("\"ok\":true"), "{}", field(&out, "STATUS"));
    assert!(field(&out, "CONTEXT").contains("\"ok\":true"), "{}", field(&out, "CONTEXT"));
    assert!(field(&out, "STATE_HEAD").starts_with("format: 5"), "{}", field(&out, "STATE_HEAD"));
    let gil = field(&out, "ACL_GIL");
    assert!(!gil.contains("(I)"), ".gil must not inherit: {gil}");
    assert!(gil.contains("NT AUTHORITY\\SYSTEM:(OI)(CI)(F)"), "{gil}");
    assert!(!gil.contains("Authenticated Users") && !gil.contains("BUILTIN\\Users") && !gil.contains("Administrators"), "{gil}");
    let state = field(&out, "ACL_STATE");
    // The icacls line starts with the path (which contains `C:\Users\...`); judge the ACEs only.
    assert!(state.contains(":(I)(F)") && !state.contains("BUILTIN\\Users") && !state.contains("Authenticated Users") && !state.contains("Everyone"), "{state}");
    // ACL_ROOT is recorded for the report; whether a fresh profile subfolder shows inherited
    // entries depends on how Windows created that profile and is not part of the contract.
    let _ = field(&out, "ACL_ROOT");
}

#[test]
fn inheritance_re_enabled_after_creation_reproduces_the_reopen_refusal() {
    let (shared, exe, script, account) = setup();
    let (code, out, err) = account.run_script(&script, &shared.0, &[exe.to_str().unwrap(), "reinherit"]);
    assert_eq!(code, 0, "driver failed: {err}\n{out}");
    assert_eq!(field(&out, "HIVE_LOADED"), "True");
    eprintln!("OBSERVED mode=reinherit HIVE_LOADED={} BRANCH=started-then-protection-lost", field(&out, "HIVE_LOADED"));
    let start = field(&out, "START");
    assert!(start.contains("\"ok\":true") && start.contains("\"storage_format\":5"), "{start}");
    let status = field(&out, "STATUS");
    assert!(status.contains("\"ok\":false"), "{status}");
    assert!(status.contains("DACL still inherits from the parent folder"), "exact PC symptom expected: {status}");
    assert!(status.contains("\"code\":\"storage_protection_lost\"") && status.contains("\"records_preserved\":true"), "stable code as a standard user: {status}");
    assert!(status.contains("변경을 요청하지 않는다"), "{status}");
    let context = field(&out, "CONTEXT");
    assert!(context.contains("\"ok\":false") && context.contains("\"code\":\"storage_protection_lost\""), "{context}");
    let gil = field(&out, "ACL_GIL");
    assert!(gil.contains("(I)"), "the external re-inheritance is visible on .gil: {gil}");
    // The records were preserved, not repaired or deleted.
    assert!(field(&out, "STATE_HEAD").starts_with("format: 5"), "{}", field(&out, "STATE_HEAD"));
}

#[test]
fn an_existing_inline_visual_record_is_copied_verified_and_left_in_place() {
    let (shared, exe, script, account) = setup();
    let (code, out, err) = account.run_script(&script, &shared.0, &[exe.to_str().unwrap(), "migrate"]);
    assert_eq!(code, 0, "driver failed: {err}\n{out}");
    let migrated = field(&out, "MIGRATION");
    assert!(migrated.contains("\"ok\":true"), "{migrated}");
    assert!(migrated.contains("Windows 개인 앱 데이터"), "{migrated}");
    assert!(field(&out, "ROOT_ENTRIES").contains(".gil"), "the preserved inline source is missing: {}", field(&out, "ROOT_ENTRIES"));
    assert_eq!(field(&out, "SOURCE_HASH"), field(&out, "EXTERNAL_HASH"));
    assert!(field(&out, "STATUS").contains("\"ok\":true"), "{}", field(&out, "STATUS"));
}

/// Sync-root registry discovery is no longer a prerequisite: records live in private local app
/// data, so a missing user hive does not force the working folder to move.
#[test]
fn without_a_loaded_profile_the_start_matches_the_observed_hive_state() {
    let (shared, exe, script, account) = setup();
    let (code, out, err) = account.run_script_with_profile(&script, &shared.0, &[exe.to_str().unwrap(), "noprofile"], false);
    assert_eq!(code, 0, "driver failed: {err}\n{out}");
    let start = field(&out, "START");
    let entries = field(&out, "ROOT_ENTRIES");
    let branch = if field(&out, "LOCAL_APP_DATA").is_empty() { "no-local-app-data" } else { "started" };
    eprintln!("OBSERVED mode=noprofile HIVE_LOADED={} BRANCH={branch}", field(&out, "HIVE_LOADED"));
    if field(&out, "LOCAL_APP_DATA").is_empty() {
        assert!(start.contains("Windows 개인 앱 데이터 위치를 찾지 못했다"), "{start}");
    } else {
        assert!(start.contains("\"ok\":true") && start.contains("\"record_storage\":\"private_app_data\""), "{start}");
        assert!(!entries.contains(".gil"), "working tree received internal records: {entries}");
        assert!(field(&out, "STATUS").contains("\"ok\":true"), "{}", field(&out, "STATUS"));
    }
}
