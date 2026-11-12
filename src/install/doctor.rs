//! `gil doctor` — one line per Host in a fixed form an agent can read mechanically:
//!
//! ```text
//! binary: OK C:\Users\name\AppData\Local\Programs\gil\gil.exe (v0.3.0)
//! claude-desktop: OK
//! codex: OK
//! ```
//!
//! `OK` the entry exists and points at this executable · `NOT_INSTALLED` the Host is not on
//! this PC · `MISSING` the Host is here but has no entry · `STALE` the entry points somewhere
//! else or at a file that does not exist · `ERROR` the file could not be read or parsed.
//! The exit code is non-zero when any `MISSING`, `STALE` or `ERROR` appears.

use super::{Registration, claude, codex, paths, paths::Env, same_path};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Ok,
    NotInstalled,
    Missing,
    Stale(String),
    Error(String),
}

impl Status {
    fn healthy(&self) -> bool {
        matches!(self, Status::Ok | Status::NotInstalled)
    }

    fn render(&self) -> String {
        match self {
            Status::Ok => "OK".into(),
            Status::NotInstalled => "NOT_INSTALLED".into(),
            Status::Missing => "MISSING".into(),
            Status::Stale(detail) => format!("STALE {detail}"),
            Status::Error(detail) => format!("ERROR {detail}"),
        }
    }
}

pub struct Report {
    pub binary: Status,
    pub binary_path: Option<PathBuf>,
    pub version: String,
    pub claude: Status,
    pub codex: Status,
}

impl Report {
    pub fn healthy(&self) -> bool {
        self.binary.healthy() && self.claude.healthy() && self.codex.healthy()
    }

    pub fn render(&self) -> String {
        let path = self.binary_path.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
        format!(
            "binary: {} {} (v{})\nclaude-desktop: {}\ncodex: {}\n",
            self.binary.render(), path, self.version, self.claude.render(), self.codex.render()
        )
    }
}

pub fn report(env: &Env) -> Report {
    let current = std::env::current_exe().ok();
    let (binary, binary_path) = match paths::fixed_exe(env) {
        Err(why) => (Status::Error(why), current.clone()),
        Ok(fixed) => {
            if !fixed.is_file() {
                (Status::Stale(format!("not installed at {}", fixed.display())), current.clone())
            } else if current.as_deref().is_some_and(|me| same_path(&me.to_string_lossy(), &fixed.to_string_lossy())) {
                (Status::Ok, Some(fixed))
            } else {
                (Status::Stale(format!("running from {} instead of", current.as_ref().map(|p| p.display().to_string()).unwrap_or_default())), Some(fixed))
            }
        }
    };
    let exe = paths::fixed_exe(env).unwrap_or_default();
    Report {
        binary,
        binary_path,
        version: crate::CORE_VERSION.to_owned(),
        claude: claude_status(env, &exe),
        codex: codex_status(env, &exe),
    }
}

fn claude_status(env: &Env, exe: &Path) -> Status {
    let paths = paths::claude_configs(env);
    if paths.is_empty() { return Status::NotInstalled; }
    let mut worst = Status::Ok;
    for path in &paths {
        let status = match claude::load(path) {
            Err(why) => Status::Error(format!("{why} ({})", path.display())),
            Ok(loaded) => match claude::classify(&loaded.root, exe) {
                Registration::Ours => Status::Ok,
                Registration::Missing => Status::Missing,
                Registration::Stale | Registration::Foreign => Status::Stale(format!(
                    "{} runs {:?}", path.display(), claude::entry(&loaded.root).and_then(claude::command_of).unwrap_or("<no command>")
                )),
            },
        };
        worst = worse(worst, status);
    }
    worst
}

fn codex_status(env: &Env, exe: &Path) -> Status {
    let Some(path) = paths::codex_config(env) else { return Status::NotInstalled };
    match codex::load(&path) {
        Err(why) => Status::Error(format!("{why} ({})", path.display())),
        Ok(loaded) => match codex::classify(&loaded.doc, exe) {
            Registration::Ours => Status::Ok,
            Registration::Missing => Status::Missing,
            Registration::Stale | Registration::Foreign => Status::Stale(format!(
                "{} runs {:?}", path.display(), codex::command_of(&loaded.doc).unwrap_or("<no command>")
            )),
        },
    }
}

fn rank(status: &Status) -> u8 {
    match status { Status::Ok => 0, Status::NotInstalled => 0, Status::Missing => 1, Status::Stale(_) => 2, Status::Error(_) => 3 }
}

fn worse(a: Status, b: Status) -> Status {
    if rank(&b) > rank(&a) { b } else { a }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_and_exit_follow_the_fixed_form() {
        let report = Report {
            binary: Status::Ok, binary_path: Some(PathBuf::from("/p/gil")), version: "0.3.0".into(),
            claude: Status::NotInstalled, codex: Status::Ok,
        };
        assert_eq!(report.render(), "binary: OK /p/gil (v0.3.0)\nclaude-desktop: NOT_INSTALLED\ncodex: OK\n");
        assert!(report.healthy());
        let report = Report { claude: Status::Missing, ..report };
        assert!(!report.healthy());
        assert!(report.render().contains("claude-desktop: MISSING\n"));
        let report = Report { codex: Status::Stale("x runs \"node\"".into()), ..report };
        assert!(report.render().contains("codex: STALE x runs \"node\"\n"));
    }

    #[test]
    fn host_statuses_come_from_the_files() {
        let root = super::super::tests::scratch("doctor");
        let home = root.join("home");
        let local = root.join("Local");
        std::fs::create_dir_all(home.join(".codex")).unwrap();
        std::fs::create_dir_all(local.join("Packages").join("Claude_x").join("LocalCache").join("Roaming").join("Claude")).unwrap();
        let env = Env { local_app_data: Some(local.clone()), app_data: None, home: Some(home.clone()), codex_home: None };
        let exe = PathBuf::from("/fixed/gil");
        assert_eq!(codex_status(&env, &exe), Status::Missing, "folder exists, file does not");
        assert_eq!(claude_status(&env, &exe), Status::Missing);
        codex::register(&home.join(".codex").join("config.toml"), &exe, false).unwrap();
        assert_eq!(codex_status(&env, &exe), Status::Ok);
        let claude_path = paths::claude_configs(&env)[0].clone();
        claude::register(&claude_path, Path::new("/elsewhere/gil"), false).unwrap();
        assert!(matches!(claude_status(&env, &exe), Status::Stale(_)));
        std::fs::write(&claude_path, "{ broken").unwrap();
        assert!(matches!(claude_status(&env, &exe), Status::Error(_)));
        assert_eq!(worse(Status::Missing, Status::Ok), Status::Missing);
        let _ = std::fs::remove_dir_all(root);
    }
}
