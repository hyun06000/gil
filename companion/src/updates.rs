//! User-initiated, signed whole-app updates. Nothing in setup calls these commands.

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

const PUBLIC_KEY: Option<&str> = option_env!("GIL_UPDATER_PUBLIC_KEY");
const ENDPOINT: Option<&str> = option_env!("GIL_UPDATER_ENDPOINT");

#[derive(Serialize)]
pub struct UpdateCapability {
    configured: bool,
    current_version: String,
}

#[derive(Serialize)]
pub struct AvailableUpdate {
    current_version: String,
    version: String,
    body: Option<String>,
    date: Option<String>,
}

#[derive(Clone, Serialize)]
struct UpdateProgress {
    downloaded: u64,
    total: Option<u64>,
}

fn settings() -> Result<(&'static str, tauri::Url), String> {
    let key = PUBLIC_KEY.filter(|one| !one.trim().is_empty())
        .ok_or("이 빌드에는 업데이트 공개키가 없다")?;
    let endpoint = ENDPOINT.filter(|one| !one.trim().is_empty())
        .ok_or("이 빌드에는 업데이트 주소가 없다")?;
    let endpoint = endpoint.parse().map_err(|err| format!("업데이트 주소가 잘못됐다: {err}"))?;
    Ok((key, endpoint))
}

fn updater(app: &AppHandle) -> Result<tauri_plugin_updater::Updater, String> {
    let (key, endpoint) = settings()?;
    app.updater_builder()
        .pubkey(key)
        .endpoints(vec![endpoint]).map_err(|err| err.to_string())?
        .build().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn update_capability(app: AppHandle) -> UpdateCapability {
    UpdateCapability {
        configured: settings().is_ok(),
        current_version: app.package_info().version.to_string(),
    }
}

#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Option<AvailableUpdate>, String> {
    let updater = updater(&app)
        .map_err(|err| format!("업데이트 설정을 열지 못했다: {err}"))?;
    let found = updater.check().await
        .map_err(|err| format!("업데이트 서버 응답을 처리하지 못했다: {err}"))?;
    Ok(found.map(|one| AvailableUpdate {
        current_version: one.current_version,
        version: one.version,
        body: one.body,
        date: one.date.map(|date| date.to_string()),
    }))
}

#[tauri::command]
pub async fn install_update(app: AppHandle, expected_version: String) -> Result<(), String> {
    let updater = updater(&app)
        .map_err(|err| format!("업데이트 설정을 열지 못했다: {err}"))?;
    let Some(update) = updater.check().await
        .map_err(|err| format!("업데이트 서버 응답을 처리하지 못했다: {err}"))? else {
        return Err("설치할 업데이트가 없다".into());
    };
    if update.version != expected_version {
        return Err(format!(
            "확인한 버전은 {expected_version}인데 지금 배포된 버전은 {}다. 다시 확인한다.",
            update.version
        ));
    }

    let progress_app = app.clone();
    let mut downloaded = 0u64;
    update.download_and_install(
        move |chunk, total| {
            downloaded = downloaded.saturating_add(chunk as u64);
            let _ = progress_app.emit("gil://update-progress", UpdateProgress { downloaded, total });
        },
        {
            let app = app.clone();
            move || {
                let _ = app.emit("gil://update-downloaded", ());
            }
        },
    ).await.map_err(|err| format!("업데이트 파일을 설치하지 못했다: {err}"))
}

#[cfg(test)]
mod tests {
    use tauri_plugin_updater::RemoteRelease;

    #[test]
    fn development_builds_do_not_pretend_updates_are_configured() {
        if option_env!("GIL_UPDATER_PUBLIC_KEY").is_none()
            || option_env!("GIL_UPDATER_ENDPOINT").is_none()
        {
            assert!(super::settings().is_err());
        }
    }

    #[test]
    fn published_owner_manifest_is_valid_for_the_pinned_updater() {
        let release: RemoteRelease = serde_json::from_str(include_str!(
            "../../distribution/windows/owner-update.json"
        )).expect("Tauri updater manifest");
        assert_eq!(release.version.to_string(), "0.1.1-owner.5");
        assert!(release
            .download_url("windows-x86_64")
            .expect("Windows x64 URL")
            .as_str()
            .starts_with("https://github.com/hyun06000/gil/releases/download/"));
        assert!(!release
            .signature("windows-x86_64")
            .expect("Windows x64 signature")
            .is_empty());
    }
}
