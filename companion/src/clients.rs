//! Explicit host registration. This never runs during setup or update.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize)]
pub struct CodexCapability {
    supported: bool,
    runtime_installed: bool,
    cli_found: bool,
    runtime_version: Option<String>,
}

#[derive(Serialize)]
pub struct CodexConnection {
    state: &'static str,
}

#[cfg(windows)]
#[derive(Deserialize)]
struct RuntimeReceipt {
    schema: u32,
    version: String,
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

#[derive(Deserialize)]
struct PluginList {
    installed: Vec<ListedPlugin>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListedPlugin {
    plugin_id: String,
    name: String,
    installed: bool,
    enabled: bool,
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
    let runtime_version = runtime_version();
    CodexCapability {
        supported: cfg!(windows),
        runtime_installed: launcher.is_ok_and(|one| one.is_file()),
        cli_found: cfg!(windows) && command().is_ok(),
        runtime_version,
    }
}

fn runtime_version() -> Option<String> {
    #[cfg(windows)]
    {
        let local = std::env::var_os("LOCALAPPDATA")?;
        let bytes = std::fs::read(PathBuf::from(local).join("GIL/runtime/current.json")).ok()?;
        let receipt: RuntimeReceipt = serde_json::from_slice(&bytes).ok()?;
        (receipt.schema == 1 && !receipt.version.is_empty()).then_some(receipt.version)
    }
    #[cfg(not(windows))]
    None
}

fn codex_candidates_from(local: Option<&Path>, configured: Option<PathBuf>) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = configured {
        candidates.push(path);
    }
    if let Some(local) = local {
        candidates.push(
            local
                .join("Programs")
                .join("OpenAI")
                .join("Codex")
                .join("bin")
                .join("codex.exe"),
        );

        let desktop_bin = local.join("OpenAI").join("Codex").join("bin");
        candidates.push(desktop_bin.join("codex.exe"));
        let mut relocated = std::fs::read_dir(&desktop_bin)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter(|entry| !entry.file_name().to_string_lossy().starts_with(".staging-"))
            .map(|entry| entry.path().join("codex.exe"))
            .filter(|path| path.is_file())
            .collect::<Vec<_>>();
        relocated.sort_by_key(|path| {
            std::fs::metadata(path)
                .and_then(|metadata| metadata.modified())
                .ok()
        });
        relocated.reverse();
        candidates.extend(relocated);
    }
    // A separately installed CLI may be available even when Codex Desktop is not.
    candidates.push(PathBuf::from("codex.exe"));
    candidates
}

fn new_command(executable: &Path) -> std::process::Command {
    let command = std::process::Command::new(executable);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut command = command;
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        command
    }
    #[cfg(not(windows))]
    command
}

fn command() -> Result<std::process::Command, String> {
    #[cfg(windows)]
    let candidates = codex_candidates_from(
        std::env::var_os("LOCALAPPDATA").as_deref().map(Path::new),
        std::env::var_os("CODEX_CLI_PATH").map(PathBuf::from),
    );
    #[cfg(not(windows))]
    let candidates = codex_candidates_from(None, None);

    for executable in candidates {
        let Ok(output) = new_command(&executable).arg("--version").output() else {
            continue;
        };
        let version = if output.stdout.is_empty() {
            &output.stderr
        } else {
            &output.stdout
        };
        if output.status.success()
            && String::from_utf8_lossy(version)
                .trim()
                .starts_with("codex-cli ")
        {
            return Ok(new_command(&executable));
        }
    }
    Err("Codex 앱의 연결 도구를 찾지 못했다. Codex를 완전히 종료했다 다시 연 뒤 재시도한다.".into())
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
    let output = command()?
        .args(["mcp", "list", "--json"])
        .output()
        .map_err(|err| format!("Codex 연결 도구를 실행하지 못했다: {err}"))?;
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

fn legacy_plugins() -> Result<Vec<String>, String> {
    let output = command()?
        .args(["plugin", "list", "--json"])
        .output()
        .map_err(|err| format!("Codex 플러그인 목록을 읽지 못했다: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "Codex 플러그인 목록을 읽지 못했다: {}",
            output_detail(&output)
        ));
    }
    let listed: PluginList = serde_json::from_slice(&output.stdout)
        .map_err(|err| format!("Codex 플러그인 목록 응답을 읽지 못했다: {err}"))?;
    Ok(listed
        .installed
        .into_iter()
        .filter(|plugin| {
            plugin.installed && plugin.enabled && plugin.name == "gil-companion-prototype"
        })
        .map(|plugin| plugin.plugin_id)
        .collect())
}

#[tauri::command]
pub async fn codex_connection() -> Result<CodexConnection, String> {
    let launcher = launcher_path()?;
    tauri::async_runtime::spawn_blocking(move || {
        let state = if !legacy_plugins()?.is_empty() {
            "needs_repair"
        } else {
            match registration(&launcher)? {
                Registration::Missing => "not_connected",
                Registration::Ours => "connected",
                Registration::Conflict => "needs_repair",
            }
        };
        Ok(CodexConnection { state })
    })
    .await
    .map_err(|err| format!("Codex 연결 상태를 확인하지 못했다: {err}"))?
}

#[tauri::command]
pub async fn repair_codex_connection() -> Result<String, String> {
    let launcher = launcher_path()?;
    if !launcher.is_file() {
        return Err("GIL 런타임이 설치되지 않았다. Companion 설치를 다시 실행한다.".into());
    }
    tauri::async_runtime::spawn_blocking(move || repair_registration(&launcher))
        .await
        .map_err(|err| format!("Codex 연결 교체 작업을 마치지 못했다: {err}"))?
}

fn repair_registration(launcher: &Path) -> Result<String, String> {
    for plugin in legacy_plugins()? {
        let output = command()?
            .args(["plugin", "remove", &plugin, "--json"])
            .output()
            .map_err(|err| format!("이전 GIL 플러그인을 제거하지 못했다: {err}"))?;
        if !output.status.success() {
            return Err(format!(
                "이전 GIL 플러그인을 제거하지 못했다: {}",
                output_detail(&output)
            ));
        }
    }

    if registration(launcher)? == Registration::Conflict {
        let output = command()?
            .args(["mcp", "remove", "gil"])
            .output()
            .map_err(|err| format!("이전 gil 연결을 제거하지 못했다: {err}"))?;
        if !output.status.success() {
            return Err(format!(
                "이전 gil 연결을 제거하지 못했다: {}",
                output_detail(&output)
            ));
        }
    }
    register(launcher)?;
    if !legacy_plugins()?.is_empty() || registration(launcher)? != Registration::Ours {
        return Err("Codex 연결 교체가 끝까지 확인되지 않았다".into());
    }
    Ok("이전 GIL 플러그인과 연결을 정리하고 새 GIL Core에 연결했다. Codex를 완전히 종료한 뒤 다시 연다.".into())
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
    let output = command()?
        .args(registration_args(launcher))
        .output()
        .map_err(|err| format!("Codex 연결 도구를 실행하지 못했다: {err}"))?;
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
    let output = command()?
        .args(["mcp", "remove", "gil"])
        .output()
        .map_err(|err| format!("Codex 연결 도구를 실행하지 못했다: {err}"))?;
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

    #[test]
    fn only_enabled_legacy_gil_plugins_need_repair() {
        let plugins: PluginList = serde_json::from_str(
            r#"{"installed":[{"pluginId":"gil-companion-prototype@old","name":"gil-companion-prototype","installed":true,"enabled":true},{"pluginId":"gil-companion-prototype@off","name":"gil-companion-prototype","installed":true,"enabled":false},{"pluginId":"other@old","name":"other","installed":true,"enabled":true}]}"#,
        )
        .unwrap();
        let found = plugins
            .installed
            .into_iter()
            .filter(|plugin| {
                plugin.installed && plugin.enabled && plugin.name == "gil-companion-prototype"
            })
            .map(|plugin| plugin.plugin_id)
            .collect::<Vec<_>>();
        assert_eq!(found, ["gil-companion-prototype@old"]);
    }

    #[test]
    fn desktop_cli_candidates_ignore_incomplete_staging_directories() {
        let root = std::env::temp_dir().join(format!(
            "gil-codex-candidates-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let current = root.join("OpenAI/Codex/bin/0123456789abcdef");
        let staging = root.join("OpenAI/Codex/bin/.staging-fedcba9876543210-test");
        std::fs::create_dir_all(&current).unwrap();
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(current.join("codex.exe"), b"current").unwrap();
        std::fs::write(staging.join("codex.exe"), b"incomplete").unwrap();

        let configured = PathBuf::from(r"C:\configured\codex.exe");
        let candidates = codex_candidates_from(Some(&root), Some(configured.clone()));
        assert_eq!(candidates.first(), Some(&configured));
        assert!(candidates.contains(&current.join("codex.exe")));
        assert!(!candidates.contains(&staging.join("codex.exe")));
        assert_eq!(candidates.last(), Some(&PathBuf::from("codex.exe")));

        std::fs::remove_dir_all(root).unwrap();
    }
}
