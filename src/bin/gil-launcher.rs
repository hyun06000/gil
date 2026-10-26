//! Stable Windows MCP entry point. Installers replace versioned Core files, never this running exe.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
#[cfg(windows)]
use std::{
    io,
    process::{Command, Stdio},
};

const RECEIPT: &str = "current.json";
const PREVIOUS_RECEIPT: &str = "current.previous";
const MAX_RECEIPT_BYTES: u64 = 4096;
const ARCHITECTURE: &str = "x86_64";
const COHORT: &str = "gil-mcp-v1";

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Current {
    schema: u8,
    version: String,
    sha256: String,
    architecture: String,
    cohort: String,
}

fn main() -> ExitCode {
    match real_main() {
        Ok(code) => ExitCode::from(code),
        Err(message) => {
            eprintln!("GIL launcher: {message}");
            ExitCode::FAILURE
        }
    }
}

fn real_main() -> Result<u8, String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args == ["--version"] {
        println!("gil-launcher {}", env!("CARGO_PKG_VERSION"));
        return Ok(0);
    }
    if args != ["mcp", "--serve"] {
        return Err("지원하는 실행은 `gil-launcher mcp --serve` 하나뿐이다".into());
    }
    #[cfg(not(windows))]
    return Err("제품 런처는 Windows에서만 실행한다".into());
    #[cfg(windows)]
    {
        let local = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA가 없다")?;
        launch(&PathBuf::from(local).join("GIL").join("runtime"))
    }
}

fn read_current(root: &Path, name: &str) -> Result<Current, String> {
    let path = root.join(name);
    let metadata = fs::symlink_metadata(&path)
        .map_err(|err| format!("현재 버전 기록을 읽지 못했다: {err}"))?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > MAX_RECEIPT_BYTES
    {
        return Err("현재 버전 기록이 안전한 작은 일반 파일이 아니다".into());
    }
    let bytes = fs::read(&path).map_err(|err| format!("현재 버전 기록을 읽지 못했다: {err}"))?;
    let current: Current = serde_json::from_slice(&bytes)
        .map_err(|err| format!("현재 버전 기록이 손상됐다: {err}"))?;
    if current.schema != 1 || !safe_version(&current.version) {
        return Err("현재 버전 기록의 판 또는 버전이 잘못됐다".into());
    }
    if current.architecture != ARCHITECTURE || current.cohort != COHORT {
        return Err("현재 버전 기록의 CPU 또는 계약 cohort가 이 런처와 다르다".into());
    }
    if current.sha256.len() != 64
        || !current
            .sha256
            .bytes()
            .all(|one| one.is_ascii_hexdigit() && !one.is_ascii_uppercase())
    {
        return Err("현재 버전 기록의 digest가 잘못됐다".into());
    }
    Ok(current)
}

fn safe_version(version: &str) -> bool {
    !version.is_empty()
        && version.len() <= 96
        && version != "."
        && version != ".."
        && version
            .bytes()
            .all(|one| one.is_ascii_alphanumeric() || matches!(one, b'.' | b'-' | b'+'))
}

fn core_path(root: &Path, current: &Current) -> PathBuf {
    root.join("versions").join(&current.version).join("gil.exe")
}

fn verify_named_core(root: &Path, receipt: &str) -> Result<(Current, File, PathBuf), String> {
    let current = read_current(root, receipt)?;
    let path = core_path(root, &current);
    let metadata = fs::symlink_metadata(&path)
        .map_err(|err| format!("Core {}를 찾지 못했다: {err}", current.version))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err("선택된 Core가 일반 파일이 아니다".into());
    }

    #[cfg(windows)]
    let mut file = {
        use std::os::windows::fs::OpenOptionsExt;
        std::fs::OpenOptions::new()
            .read(true)
            .share_mode(windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ)
            .open(&path)
            .map_err(|err| format!("Core를 고정하지 못했다: {err}"))?
    };
    #[cfg(not(windows))]
    let mut file = File::open(&path).map_err(|err| format!("Core를 열지 못했다: {err}"))?;

    let mut hash = Sha256::new();
    let mut block = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut block)
            .map_err(|err| format!("Core를 검증하지 못했다: {err}"))?;
        if read == 0 {
            break;
        }
        hash.update(&block[..read]);
    }
    let got = format!("{:x}", hash.finalize());
    if got != current.sha256 {
        return Err(format!(
            "Core {}의 digest가 설치 기록과 다르다",
            current.version
        ));
    }
    Ok((current, file, path))
}

fn verify_core(root: &Path) -> Result<(Current, File, PathBuf), String> {
    match verify_named_core(root, RECEIPT) {
        Ok(core) => Ok(core),
        Err(current) => match verify_named_core(root, PREVIOUS_RECEIPT) {
            Ok(core) => {
                eprintln!(
                    "GIL launcher: 현재 Core를 사용할 수 없어 검증된 직전 Core를 사용한다: {current}"
                );
                Ok(core)
            }
            Err(previous) => Err(format!(
                "현재 Core를 사용할 수 없다: {current}; 직전 Core도 사용할 수 없다: {previous}"
            )),
        },
    }
}

#[cfg(windows)]
fn launch(root: &Path) -> Result<u8, String> {
    use std::os::windows::io::AsRawHandle;
    let (_current, _pinned, path) = verify_core(root)?;
    let mut child = Command::new(path)
        .args(["mcp", "--serve"])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|err| format!("Core를 시작하지 못했다: {err}"))?;
    let job = Job::new().and_then(|job| job.assign(child.as_raw_handle()).map(|()| job));
    let job = match job {
        Ok(job) => job,
        Err(err) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("Core 수명 경계를 만들지 못했다: {err}"));
        }
    };
    let status = child
        .wait()
        .map_err(|err| format!("Core 종료를 확인하지 못했다: {err}"))?;
    drop(job);
    Ok(status
        .code()
        .and_then(|one| u8::try_from(one).ok())
        .unwrap_or(1))
}

#[cfg(windows)]
struct Job(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Job {
    fn new() -> io::Result<Self> {
        use windows_sys::Win32::System::JobObjects::*;
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let job = Self(handle);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let ok = unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&limits) as u32,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(job)
    }

    fn assign(&self, process: std::os::windows::io::RawHandle) -> io::Result<()> {
        use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
        let ok = unsafe { AssignProcessToJobObject(self.0, process.cast()) };
        if ok == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}

#[cfg(windows)]
impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU64, Ordering};
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "gil-launcher-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn fixture(version: &str, bytes: &[u8]) -> (TempDir, Current) {
        let dir = TempDir::new();
        let current = Current {
            schema: 1,
            version: version.into(),
            sha256: format!("{:x}", Sha256::digest(bytes)),
            architecture: ARCHITECTURE.into(),
            cohort: COHORT.into(),
        };
        let core = core_path(dir.path(), &current);
        fs::create_dir_all(core.parent().unwrap()).unwrap();
        File::create(core).unwrap().write_all(bytes).unwrap();
        fs::write(
            dir.path().join(RECEIPT),
            serde_json::to_vec(&current).unwrap(),
        )
        .unwrap();
        (dir, current)
    }

    #[test]
    fn a_receipt_selects_only_its_version_directory_and_digest() {
        let (dir, expected) = fixture("1.2.3-preview.4+windows.x64", b"core");
        let (current, _file, path) = verify_core(dir.path()).unwrap();
        assert_eq!(current, expected);
        assert_eq!(
            path,
            dir.path()
                .join("versions/1.2.3-preview.4+windows.x64/gil.exe")
        );
    }

    #[test]
    fn traversal_unknown_fields_bad_digests_and_changed_core_are_refused() {
        for version in ["", ".", "..", "../bad", "bad\\name", "bad/name", "한글"] {
            assert!(!safe_version(version), "{version}");
        }
        let (dir, _) = fixture("1.2.3", b"core");
        fs::write(
            core_path(dir.path(), &read_current(dir.path(), RECEIPT).unwrap()),
            b"changed",
        )
        .unwrap();
        assert!(verify_core(dir.path()).unwrap_err().contains("digest"));
        fs::write(dir.path().join(RECEIPT), r#"{"schema":1,"version":"1.2.3","sha256":"00","architecture":"x86_64","cohort":"gil-mcp-v1","extra":true}"#).unwrap();
        assert!(read_current(dir.path(), RECEIPT).is_err());
    }

    #[test]
    fn a_broken_new_core_falls_back_once_to_the_previous_complete_core() {
        let (dir, old) = fixture("1.2.3", b"old");
        fs::rename(dir.path().join(RECEIPT), dir.path().join(PREVIOUS_RECEIPT)).unwrap();
        let new = Current {
            schema: 1,
            version: "1.2.4".into(),
            sha256: "0".repeat(64),
            architecture: ARCHITECTURE.into(),
            cohort: COHORT.into(),
        };
        let new_path = core_path(dir.path(), &new);
        fs::create_dir_all(new_path.parent().unwrap()).unwrap();
        fs::write(new_path, b"broken").unwrap();
        fs::write(dir.path().join(RECEIPT), serde_json::to_vec(&new).unwrap()).unwrap();
        let (selected, _, _) = verify_core(dir.path()).unwrap();
        assert_eq!(selected, old);
    }

    #[test]
    fn a_receipt_for_another_architecture_or_contract_is_refused() {
        let (dir, mut current) = fixture("1.2.3", b"core");
        current.architecture = "aarch64".into();
        fs::write(
            dir.path().join(RECEIPT),
            serde_json::to_vec(&current).unwrap(),
        )
        .unwrap();
        assert!(verify_core(dir.path()).unwrap_err().contains("CPU"));

        current.architecture = ARCHITECTURE.into();
        current.cohort = "gil-mcp-v2".into();
        fs::write(
            dir.path().join(RECEIPT),
            serde_json::to_vec(&current).unwrap(),
        )
        .unwrap();
        assert!(verify_core(dir.path()).unwrap_err().contains("cohort"));
    }
}
