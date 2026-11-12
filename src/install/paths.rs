//! Where things live on this PC. Every rule takes the environment as a value so the tests can
//! lay out a fake user profile in a temporary folder.

use std::path::{Path, PathBuf};

/// The parts of the process environment the installer reads. Nothing else is consulted.
#[derive(Debug, Clone, Default)]
pub struct Env {
    /// `%LOCALAPPDATA%` (Windows).
    pub local_app_data: Option<PathBuf>,
    /// `%APPDATA%` (Windows).
    pub app_data: Option<PathBuf>,
    /// `%USERPROFILE%` on Windows, `$HOME` elsewhere.
    pub home: Option<PathBuf>,
    /// `CODEX_HOME`, when set.
    pub codex_home: Option<PathBuf>,
    /// Running inside an MSIX package (Microsoft Store channel). The executable then lives in
    /// the read-only package folder and Hosts register the App Execution Alias instead.
    pub packaged: bool,
}

impl Env {
    pub fn from_process() -> Env {
        let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
        Env {
            local_app_data: var("LOCALAPPDATA"),
            app_data: var("APPDATA"),
            home: if cfg!(windows) { var("USERPROFILE") } else { var("HOME") },
            codex_home: var("CODEX_HOME"),
            packaged: packaged_now(),
        }
    }
}

/// Whether this process carries MSIX package identity (installed from the Microsoft Store or
/// sideloaded as a signed package). Asked of the OS, never inferred from the path.
pub fn packaged_now() -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::APPMODEL_ERROR_NO_PACKAGE;
        use windows_sys::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName;
        let mut length: u32 = 0;
        // With a zero-length buffer the call only tells us whether there is a package at all.
        let code = unsafe { GetCurrentPackageFullName(&mut length, std::ptr::null_mut()) };
        code != APPMODEL_ERROR_NO_PACKAGE
    }
    #[cfg(not(windows))]
    false
}

/// `%LOCALAPPDATA%\Microsoft\WindowsApps\gil.exe` — the App Execution Alias Windows creates for
/// the packaged app. It is a stable path on every PC, so Hosts register it directly.
pub fn alias_exe(env: &Env) -> Result<PathBuf, String> {
    let local = env.local_app_data.as_ref().ok_or("LOCALAPPDATA is not set")?;
    Ok(local.join("Microsoft").join("WindowsApps").join("gil.exe"))
}

/// `%LOCALAPPDATA%\Programs\gil\gil.exe` — the one path Hosts register. macOS gets the
/// equivalent under Application Support so the same code can be exercised there; the macOS
/// user channel itself is still the Plugin/archive described in the install guide.
pub fn fixed_exe(env: &Env) -> Result<PathBuf, String> {
    if env.packaged {
        alias_exe(env)
    } else if cfg!(windows) {
        let local = env.local_app_data.as_ref().ok_or("LOCALAPPDATA is not set")?;
        Ok(local.join("Programs").join("gil").join("gil.exe"))
    } else if cfg!(target_os = "macos") {
        let home = env.home.as_ref().ok_or("HOME is not set")?;
        Ok(home.join("Library").join("Application Support").join("GIL").join("bin").join("gil"))
    } else {
        Err("gil install supports Windows (and macOS for development) only".into())
    }
}

/// Every `claude_desktop_config.json` the Claude Desktop on this PC may read, as existing
/// folders. On Windows the MSIX build reads its package cache
/// (`%LOCALAPPDATA%\Packages\Claude_*\LocalCache\Roaming\Claude`) while its *Edit Config*
/// button opens `%APPDATA%\Claude`; both are returned so the entry lands wherever the app looks.
/// The package id is discovered, never hard-coded. Empty means Claude Desktop is not installed.
pub fn claude_configs(env: &Env) -> Vec<PathBuf> {
    claude_config_dirs(env).into_iter().map(|dir| dir.join("claude_desktop_config.json")).collect()
}

fn claude_config_dirs(env: &Env) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if cfg!(windows) || cfg!(test) {
        if let Some(local) = &env.local_app_data {
            let packages = local.join("Packages");
            let mut found: Vec<PathBuf> = std::fs::read_dir(&packages).into_iter().flatten().filter_map(Result::ok)
                .filter(|entry| entry.file_name().to_string_lossy().starts_with("Claude_"))
                .map(|entry| entry.path().join("LocalCache").join("Roaming").join("Claude"))
                .filter(|dir| dir.is_dir())
                .collect();
            found.sort();
            dirs.extend(found);
        }
        if let Some(appdata) = &env.app_data {
            let dir = appdata.join("Claude");
            if dir.is_dir() { dirs.push(dir); }
        }
    }
    if cfg!(target_os = "macos") && !cfg!(test) {
        if let Some(home) = &env.home {
            let dir = home.join("Library").join("Application Support").join("Claude");
            if dir.is_dir() { dirs.push(dir); }
        }
    }
    dirs
}

/// `CODEX_HOME\config.toml`, else `~/.codex/config.toml`. The folder is the sign that Codex is
/// installed (Codex does not create the file itself). `None` means Codex is not installed.
pub fn codex_config(env: &Env) -> Option<PathBuf> {
    let dir = match &env.codex_home {
        Some(configured) => configured.clone(),
        None => env.home.as_ref()?.join(".codex"),
    };
    dir.is_dir().then(|| dir.join("config.toml"))
}

#[allow(dead_code)]
pub(crate) fn display(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_configs_discovers_package_cache_and_roaming_folders_without_a_hard_coded_id() {
        let root = super::super::tests::scratch("claude-paths");
        let local = root.join("Local");
        let roaming = root.join("Roaming");
        let env = Env { local_app_data: Some(local.clone()), app_data: Some(roaming.clone()), home: None, codex_home: None, packaged: false };
        assert!(claude_configs(&env).is_empty(), "nothing installed yet");

        let cache = local.join("Packages").join("Claude_abc123def").join("LocalCache").join("Roaming").join("Claude");
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::create_dir_all(local.join("Packages").join("Other_x").join("LocalCache").join("Roaming").join("Claude")).unwrap();
        std::fs::create_dir_all(roaming.join("Claude")).unwrap();
        let found = claude_configs(&env);
        assert_eq!(found, vec![cache.join("claude_desktop_config.json"), roaming.join("Claude").join("claude_desktop_config.json")]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn codex_config_prefers_codex_home_and_needs_the_folder() {
        let root = super::super::tests::scratch("codex-paths");
        let home = root.join("home");
        std::fs::create_dir_all(&home).unwrap();
        let env = Env { local_app_data: None, app_data: None, home: Some(home.clone()), codex_home: None, packaged: false };
        assert_eq!(codex_config(&env), None, "no .codex folder → not installed");
        std::fs::create_dir_all(home.join(".codex")).unwrap();
        assert_eq!(codex_config(&env), Some(home.join(".codex").join("config.toml")));
        let custom = root.join("custom-codex");
        std::fs::create_dir_all(&custom).unwrap();
        let env = Env { codex_home: Some(custom.clone()), ..env };
        assert_eq!(codex_config(&env), Some(custom.join("config.toml")));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_packaged_process_registers_the_app_execution_alias() {
        let env = Env { local_app_data: Some(PathBuf::from("/L")), app_data: None, home: Some(PathBuf::from("/H")), codex_home: None, packaged: true };
        assert_eq!(fixed_exe(&env).unwrap(), PathBuf::from("/L").join("Microsoft").join("WindowsApps").join("gil.exe"));
        let unpackaged = Env { packaged: false, ..env };
        assert_ne!(fixed_exe(&unpackaged).unwrap(), fixed_exe(&Env { packaged: true, ..unpackaged.clone() }).unwrap());
    }
}
