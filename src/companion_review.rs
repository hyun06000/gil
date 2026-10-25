//! Manual review driver only; compiled into the existing feature-gated testbed, not GIL.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, BufRead, Write},
    path::Path,
    process::{Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

fn check_bundle(root: &Path) -> Result<(), String> {
    let manifest: Value = serde_json::from_slice(
        &fs::read(root.join("review-bundle.json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if manifest["schema"] != "gil-companion-review/1" || manifest["arch"] != std::env::consts::ARCH
    {
        return Err("wrong bundle schema or native CPU".into());
    }
    for name in [
        "gil.exe",
        "gil-companion.exe",
        "gil-runtime-testbed.exe",
        "companion-install.json",
    ] {
        let bytes = fs::read(root.join(name)).map_err(|e| e.to_string())?;
        if manifest["files"][name] != format!("{:x}", Sha256::digest(bytes)) {
            return Err(format!("digest mismatch: {name}"));
        }
    }
    Ok(())
}
fn answer(prompt: &str) -> Result<bool, String> {
    println!("{prompt} [y/N]");
    io::stdout().flush().map_err(|e| e.to_string())?;
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    Ok(line.trim().eq_ignore_ascii_case("y"))
}
fn rpc(
    input: &mut impl Write,
    rx: &mpsc::Receiver<String>,
    id: u64,
    method: &str,
    params: Value,
    events: &mut Vec<Value>,
) -> Result<Value, String> {
    writeln!(
        input,
        "{}",
        json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
    )
    .map_err(|e| e.to_string())?;
    input.flush().map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let line = rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|e| e.to_string())?;
        let value: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        events.push(value.clone());
        if value["id"] == id {
            if value.get("error").is_some() {
                return Err(value.to_string());
            }
            return Ok(value["result"].clone());
        }
    }
}

#[cfg(windows)]
struct CompanionHandle(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl CompanionHandle {
    fn open(pid: u32, expected: &Path) -> Result<Self, String> {
        use windows_sys::Win32::System::Threading::*;
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | 0x00100000, 0, pid) };
        if handle.is_null() {
            return Err(io::Error::last_os_error().to_string());
        }
        let owned = Self(handle);
        let mut text = vec![0u16; 32768];
        let mut n = text.len() as u32;
        if unsafe { QueryFullProcessImageNameW(handle, 0, text.as_mut_ptr(), &mut n) } == 0 {
            return Err(io::Error::last_os_error().to_string());
        }
        let actual = std::path::PathBuf::from(
            String::from_utf16(&text[..n as usize]).map_err(|e| e.to_string())?,
        );
        if actual.canonicalize().map_err(|e| e.to_string())?
            != expected.canonicalize().map_err(|e| e.to_string())?
        {
            return Err("Companion image path mismatch".into());
        }
        Ok(owned)
    }
    fn exited(&self) -> bool {
        unsafe { windows_sys::Win32::System::Threading::WaitForSingleObject(self.0, 10000) == 0 }
    }
}
#[cfg(windows)]
impl Drop for CompanionHandle {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

pub fn run() -> i32 {
    match review() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("BLOCKED: {error}");
            2
        }
    }
}
fn review() -> Result<i32, String> {
    if !cfg!(windows) {
        return Err("this manual bundle is Windows-only".into());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let root = exe.parent().ok_or("bundle directory missing")?;
    check_bundle(root)?;
    if !answer(
        "새 시험 폴더에만 기록합니다. 기존 GIL/Codex 등록은 바꾸지 않습니다. 검수를 시작할까요?",
    )? {
        return Ok(2);
    }
    let nonce = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos()
    );
    let work = root.join(format!("review-{nonce}"));
    fs::create_dir(&work).map_err(|e| e.to_string())?;
    let project = work.join("project");
    fs::create_dir(&project).map_err(|e| e.to_string())?;
    let stderr = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(work.join("core-stderr.log"))
        .map_err(|e| e.to_string())?;
    fs::write(
        work.join("report.json"),
        b"{\"outcome\":\"INCOMPLETE\",\"reason\":\"driver did not finish; not PASS\"}",
    )
    .map_err(|e| e.to_string())?;
    let spawned = Command::new(root.join("gil.exe"))
        .args(["mcp", "--serve"])
        .current_dir(&work)
        .env_remove("GIL_COMPANION_SESSION_SOCKET")
        .env(
            "GIL_COMPANION_REVIEW_NAMESPACE",
            format!(r"\\.\pipe\gil-companion-v1-review-{nonce}"),
        )
        .env("GIL_COMPANION_REVIEW_DIR", &work)
        .env("GIL_MONITOR_STATE_DIR", work.join("bindings"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(stderr)
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(e) => {
            let report = json!({"outcome":"BLOCKED","stage":"spawn Core","error":e.to_string()});
            println!("REPORT-JSON-BEGIN\n{report}\nREPORT-JSON-END");
            fs::write(work.join("report.json"), report.to_string()).map_err(|e| e.to_string())?;
            return Ok(2);
        }
    };
    let mut input = child.stdin.take().ok_or("stdin missing")?;
    let output = child.stdout.take().ok_or("stdout missing")?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in io::BufReader::new(output).lines() {
            match line {
                Ok(line) if line.len() <= 1024 * 1024 => {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
                _ => break,
            }
        }
    });
    let mut events = Vec::new();
    #[cfg(windows)]
    let mut companion = None;
    let result = (|| -> Result<(), String> {
        rpc(
            &mut input,
            &rx,
            1,
            "initialize",
            json!({"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"gil-companion-review","version":"1"}}),
            &mut events,
        )?;
        writeln!(
            input,
            "{}",
            json!({"jsonrpc":"2.0","method":"notifications/initialized"})
        )
        .map_err(|e| e.to_string())?;
        let started = rpc(
            &mut input,
            &rx,
            2,
            "tools/call",
            json!({"name":"gil_start","arguments":{"project_root":project}}),
            &mut events,
        )?;
        if started["structuredContent"]["ok"] != true {
            return Err(format!("Project start refused: {started}"));
        }
        let deadline = Instant::now() + Duration::from_secs(12);
        while !work.join("companion-ready.done").exists() {
            if Instant::now() >= deadline {
                return Err("Companion readiness not observed; see core-stderr.log".into());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let ready: Value = serde_json::from_slice(
            &fs::read(work.join("companion-ready.json")).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        #[cfg(windows)]
        {
            companion = Some(CompanionHandle::open(
                ready["pid"]
                    .as_u64()
                    .and_then(|p| u32::try_from(p).ok())
                    .ok_or("invalid ready pid")?,
                &root.join("gil-companion.exe"),
            )?);
        }
        events.push(json!({"companion_ready":ready}));
        if !answer(
            "트레이(숨겨진 아이콘 포함)에 'GIL Companion 검수'가 보이나요? 아이콘 메뉴를 확인한 뒤 y를 입력하면 MCP 연결을 닫습니다.",
        )? {
            return Err("tray not confirmed by user".into());
        }
        Ok(())
    })();
    // Always close only our Core stdin. Never kill or retry, including on a failed request.
    drop(input);
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut core_exit = None;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                core_exit = Some(status);
                break;
            }
            Err(_) => break,
            Ok(None) => {
                if Instant::now() >= deadline {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    }
    #[cfg(windows)]
    let companion_exit = companion.as_ref().map(CompanionHandle::exited);
    #[cfg(not(windows))]
    let companion_exit: Option<bool> = None;
    let visible_gone = if result.is_ok() {
        answer("MCP 연결을 닫았습니다. 검수 트레이 아이콘이 사라졌나요?").unwrap_or(false)
    } else {
        false
    };
    let pass = result.is_ok()
        && core_exit.as_ref().is_some_and(|s| s.success())
        && companion_exit == Some(true)
        && visible_gone;
    let report = json!({"schema":"gil-companion-review-report/1","outcome":if pass{"PASS"}else{"FAIL"},"error":result.err(),"core_pid":child.id(),"core_exit":core_exit.map(|s|s.to_string()),"companion_exit_confirmed":companion_exit,"tray_disappeared_user_confirmed":visible_gone,"events":events,"limits":["direct MCP client, not Codex Desktop","unsigned review build, not a production installer","no update worker tested"]});
    println!("REPORT-JSON-BEGIN\n{report}\nREPORT-JSON-END");
    fs::write(
        work.join("report.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    println!("Evidence: {}", work.display());
    Ok(if pass { 0 } else { 1 })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rpc_errors_are_not_success() {
        let (tx, rx) = mpsc::channel();
        tx.send(json!({"id":1,"error":{"message":"no"}}).to_string())
            .unwrap();
        assert!(
            rpc(
                &mut Vec::new(),
                &rx,
                1,
                "initialize",
                json!({}),
                &mut Vec::new()
            )
            .is_err()
        );
    }
    #[test]
    fn rpc_skips_notifications_and_preserves_evidence() {
        let (tx, rx) = mpsc::channel();
        tx.send(json!({"method":"notice"}).to_string()).unwrap();
        tx.send(json!({"id":2,"result":{"ok":true}}).to_string())
            .unwrap();
        let mut events = Vec::new();
        assert_eq!(
            rpc(
                &mut Vec::new(),
                &rx,
                2,
                "tools/call",
                json!({}),
                &mut events
            )
            .unwrap()["ok"],
            true
        );
        assert_eq!(events.len(), 2);
    }
}
