//! Explicit opt-in session mode. No installed-app discovery, login item or updater changes.
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

pub struct Sessions {
    endpoint: Option<PathBuf>,
    server: Mutex<Option<gil::companion_ipc::Server>>,
}
impl Sessions {
    pub fn from_args(args: impl IntoIterator<Item = std::ffi::OsString>) -> Result<Self, String> {
        let args: Vec<_> = args.into_iter().skip(1).collect();
        let endpoint = match args
            .iter()
            .position(|a| a == gil::companion_ipc::SESSION_ARG)
        {
            None => None,
            Some(0) if args.len() == 2 => {
                let path = PathBuf::from(&args[1]);
                if !path.is_absolute() {
                    return Err("session endpoint must be absolute".into());
                }
                Some(path)
            }
            Some(_) => return Err("expected --gil-session-socket <absolute path> only".into()),
        };
        Ok(Self {
            endpoint,
            server: Mutex::new(None),
        })
    }
    pub fn enabled(&self) -> bool {
        self.endpoint.is_some()
    }
    pub fn start(&self, app: &tauri::AppHandle) -> std::io::Result<()> {
        let Some(endpoint) = &self.endpoint else {
            return Ok(());
        };
        *self
            .server
            .lock()
            .map_err(|_| std::io::Error::other("session state poisoned"))? =
            Some(gil::companion_ipc::Server::bind(endpoint)?);
        let app = app.clone();
        std::thread::Builder::new()
            .name("gil-session-exit".into())
            .spawn(move || {
                while !crate::lifetime::is_leaving() {
                    std::thread::sleep(std::time::Duration::from_millis(250));
                    let state = app.state::<Sessions>();
                    let finished = state.server.lock().map_or(true, |server| {
                        server
                            .as_ref()
                            .is_none_or(gil::companion_ipc::Server::finished)
                    });
                    if finished {
                        crate::lifetime::quit(&app);
                        break;
                    }
                }
            })?;
        Ok(())
    }
    pub fn stop(&self) {
        if let Ok(mut server) = self.server.lock() {
            server.take();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(args: &[&str]) -> Result<Sessions, String> {
        Sessions::from_args(args.iter().map(std::ffi::OsString::from))
    }
    #[test]
    fn existing_launches_are_not_silently_switched_to_session_mode() {
        assert!(!parse(&["app"]).unwrap().enabled());
        assert!(!parse(&["app", "--other"]).unwrap().enabled());
        assert!(parse(&["app", gil::companion_ipc::SESSION_ARG]).is_err());
        assert!(parse(&["app", gil::companion_ipc::SESSION_ARG, "relative"]).is_err());
        let absolute = std::env::temp_dir().join("gil-session.sock");
        assert!(
            parse(&[
                "app",
                gil::companion_ipc::SESSION_ARG,
                absolute.to_str().unwrap()
            ])
            .unwrap()
            .enabled()
        );
    }
}
