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
}

impl Env {
    pub fn from_process() -> Env {
        let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
        Env {
            local_app_data: var("LOCALAPPDATA"),
            app_data: var("APPDATA"),
            home: if cfg!(windows) { var("USERPROFILE") } else { var("HOME") },
            codex_home: var("CODEX_HOME"),
        }
    }
}

/// `%LOCALAPPDATA%\Programs\gil\gil.exe` — the one path Hosts register. macOS gets the
/// equivalent under Application Support so the same code can be exercised there; the macOS
/// user channel itself is still the Plugin/archive described in the install guide.
pub fn fixed_exe(env: &Env) -> Result<PathBuf, String> {
    if cfg!(windows) {
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
        let env = Env { local_app_data: Some(local.clone()), app_data: Some(roaming.clone()), home: None, codex_home: None };
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
        let env = Env { local_app_data: None, app_data: None, home: Some(home.clone()), codex_home: None };
        assert_eq!(codex_config(&env), None, "no .codex folder → not installed");
        std::fs::create_dir_all(home.join(".codex")).unwrap();
        assert_eq!(codex_config(&env), Some(home.join(".codex").join("config.toml")));
        let custom = root.join("custom-codex");
        std::fs::create_dir_all(&custom).unwrap();
        let env = Env { codex_home: Some(custom.clone()), ..env };
        assert_eq!(codex_config(&env), Some(custom.join("config.toml")));
        let _ = std::fs::remove_dir_all(root);
    }
}
