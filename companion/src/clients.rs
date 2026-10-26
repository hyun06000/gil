//! Explicit host registration. This never runs during setup or update.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize)]
pub struct CodexCapability {
    supported: bool,
    runtime_installed: bool,
}

#[derive(Serialize)]
pub struct CodexConnection {
    state: &'static str,
}

#[derive(Deserialize)]
struct ListedServer {
    name: String,
    transport: ListedTransport,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum ListedTransport {
    Stdio {
        command: String,
        args: Vec<String>,
    },
    #[serde(other)]
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Registration {
    Missing,
    Ours,
    Conflict,
}

fn launcher_path() -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        let local = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA가 없다")?;
        Ok(PathBuf::from(local)
            .join("GIL")
            .join("runtime")
            .join("gil-launcher.exe"))
    }
    #[cfg(not(windows))]
    Err("Codex 자동 연결은 Windows 제품 설치본에서만 제공한다".into())
}

fn registration_args(launcher: &Path) -> Vec<std::ffi::OsString> {
    ["mcp", "add", "gil", "--"]
        .into_iter()
        .map(Into::into)
        .chain([
            launcher.as_os_str().to_owned(),
            "mcp".into(),
            "--serve".into(),
        ])
        .collect()
}

#[tauri::command]
pub fn codex_capability() -> CodexCapability {
    let launcher = launcher_path();
    CodexCapability {
        supported: cfg!(windows),
        runtime_installed: launcher.is_ok_and(|one| one.is_file()),
    }
}

fn command() -> std::process::Command {
    #[cfg(windows)]
    let mut command = std::process::Command::new("codex.exe");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    #[cfg(not(windows))]
    let command = std::process::Command::new("codex.exe");
    command
}

fn classify_registration(launcher: &Path, servers: Vec<ListedServer>) -> Registration {
    let Some(server) = servers.into_iter().find(|one| one.name == "gil") else {
        return Registration::Missing;
    };
    let ListedTransport::Stdio { command, args } = server.transport else {
        return Registration::Conflict;
    };
    let expected = launcher.to_string_lossy();
    if command.eq_ignore_ascii_case(&expected) && args == ["mcp", "--serve"] {
        Registration::Ours
    } else {
        Registration::Conflict
    }
}

fn registration(launcher: &Path) -> Result<Registration, String> {
    let output = command()
        .args(["mcp", "list", "--json"])
        .output()
        .map_err(|err| {
            format!(
                "Codex CLI(codex.exe)를 실행하지 못했다: {err}. Codex를 설치하거나 PATH를 확인한다."
            )
        })?;
    if !output.status.success() {
        return Err(format!(
            "Codex MCP 목록을 읽지 못했다: {}",
            output_detail(&output)
        ));
    }
    let servers: Vec<ListedServer> = serde_json::from_slice(&output.stdout)
        .map_err(|err| format!("Codex MCP 목록 응답을 읽지 못했다: {err}"))?;
    Ok(classify_registration(launcher, servers))
}

#[tauri::command]
pub async fn codex_connection() -> Result<CodexConnection, String> {
    let launcher = launcher_path()?;
    tauri::async_runtime::spawn_blocking(move || {
        let state = match registration(&launcher)? {
            Registration::Missing => "not_connected",
            Registration::Ours => "connected",
            Registration::Conflict => "conflict",
        };
        Ok(CodexConnection { state })
    })
    .await
    .map_err(|err| format!("Codex 연결 상태를 확인하지 못했다: {err}"))?
}

#[tauri::command]
pub async fn connect_codex() -> Result<String, String> {
    let launcher = launcher_path()?;
    if !launcher.is_file() {
        return Err("GIL 런타임이 설치되지 않았다. Companion 설치를 다시 실행한다.".into());
    }
    tauri::async_runtime::spawn_blocking(move || register(&launcher))
        .await
        .map_err(|err| format!("Codex 연결 작업을 마치지 못했다: {err}"))?
}

fn register(launcher: &Path) -> Result<String, String> {
    match registration(launcher)? {
        Registration::Ours => return Ok("Codex에 GIL이 이미 연결되어 있다.".into()),
        Registration::Conflict => {
            return Err(
                "Codex에 이름이 같은 다른 gil 등록이 있다. 자동으로 덮어쓰지 않는다.".into(),
            );
        }
        Registration::Missing => {}
    }
    let output = command()
        .args(registration_args(launcher))
        .output()
        .map_err(|err| {
            format!(
                "Codex CLI(codex.exe)를 실행하지 못했다: {err}. Codex를 설치하거나 PATH를 확인한다."
            )
        })?;
    if !output.status.success() {
        return Err(format!(
            "Codex가 GIL 등록을 거절했다: {}. 기존 gil 등록은 자동으로 덮어쓰지 않는다.",
            output_detail(&output)
        ));
    }
    Ok("Codex에 GIL을 연결했다. 새 대화부터 적용된다.".into())
}

#[tauri::command]
pub async fn disconnect_codex() -> Result<String, String> {
    let launcher = launcher_path()?;
    tauri::async_runtime::spawn_blocking(move || disconnect(&launcher))
        .await
        .map_err(|err| format!("Codex 연결 해제 작업을 마치지 못했다: {err}"))?
}

fn disconnect(launcher: &Path) -> Result<String, String> {
    match registration(launcher)? {
        Registration::Missing => return Ok("Codex에서 GIL 연결이 이미 해제되어 있다.".into()),
        Registration::Conflict => {
            return Err(
                "이름이 같은 gil 등록이 GIL 고정 런처를 가리키지 않아 삭제하지 않는다.".into(),
            );
        }
        Registration::Ours => {}
    }
    let output = command()
        .args(["mcp", "remove", "gil"])
        .output()
        .map_err(|err| format!("Codex CLI(codex.exe)를 실행하지 못했다: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "Codex가 GIL 연결 해제를 거절했다: {}",
            output_detail(&output)
        ));
    }
    if registration(launcher)? != Registration::Missing {
        return Err("Codex가 성공을 보고했지만 gil 등록이 남아 있다".into());
    }
    Ok("Codex에서 GIL 연결을 해제했다. 이미 열린 대화는 닫은 뒤 제거를 계속한다.".into())
}

fn output_detail(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if stderr.is_empty() {
        format!("종료 코드 {:?}", output.status.code())
    } else {
        stderr
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registration_uses_the_official_stdio_cli_shape_without_a_shell() {
        let path = Path::new(r"C:\Users\A Person\AppData\Local\GIL\runtime\gil-launcher.exe");
        assert_eq!(
            registration_args(path),
            [
                "mcp",
                "add",
                "gil",
                "--",
                r"C:\Users\A Person\AppData\Local\GIL\runtime\gil-launcher.exe",
                "mcp",
                "--serve",
            ]
            .map(std::ffi::OsString::from)
        );
    }

    #[test]
    fn only_the_exact_installed_launcher_registration_is_owned() {
        let launcher = Path::new(r"C:\Users\Me\AppData\Local\GIL\runtime\gil-launcher.exe");
        let ours = r#"[{"name":"gil","transport":{"type":"stdio","command":"C:\\Users\\Me\\AppData\\Local\\GIL\\runtime\\gil-launcher.exe","args":["mcp","--serve"]}}]"#;
        let servers: Vec<ListedServer> = serde_json::from_str(ours).unwrap();
        assert_eq!(classify_registration(launcher, servers), Registration::Ours);

        let foreign =
            r#"[{"name":"gil","transport":{"type":"stdio","command":"other.exe","args":[]}}]"#;
        let servers: Vec<ListedServer> = serde_json::from_str(foreign).unwrap();
        assert_eq!(
            classify_registration(launcher, servers),
            Registration::Conflict
        );
        assert_eq!(
            classify_registration(launcher, Vec::new()),
            Registration::Missing
        );
    }
}
