//! Session-only IPC. This transport grants no installation, update or Project access.
//!
//! macOS: a persistent Unix stream, same-euid peer checks in both directions, a fresh
//! OS-random challenge per stream, bounded frames/peers and handshake deadlines. IDs
//! never cross the wire. A private endpoint directory is supplied explicitly by the
//! deployment adapter. Existing endpoints are never removed or taken over.
//! Windows uses a local named pipe and same-token-user checks. Native CI remains separate.
//! Same-user malicious code is outside this boundary; a release claim is not publisher
//! authentication. No Project path/content is transmitted.

use crate::companion_session::{Release, Snapshot};
use std::io;
use std::path::Path;

pub const ENV_ENDPOINT: &str = "GIL_COMPANION_SESSION_SOCKET";
pub const SESSION_ARG: &str = "--gil-session-socket";

#[cfg(target_os = "macos")]
pub(crate) fn random_nonce() -> io::Result<String> {
    wire::nonce()
}

#[cfg(windows)]
pub(crate) mod windows;
#[cfg(any(target_os = "macos", windows))]
mod wire;
#[cfg(any(target_os = "macos", windows))]
pub use wire::{Client, Server};

#[cfg(not(any(target_os = "macos", windows)))]
pub struct Client;
#[cfg(not(any(target_os = "macos", windows)))]
impl Client {
    pub fn connect(_: &Path, _: &Release) -> io::Result<Self> {
        Err(unsupported())
    }
    pub fn response(&self) -> bool {
        false
    }
    pub fn alive(&self) -> bool {
        false
    }
}
#[cfg(not(any(target_os = "macos", windows)))]
pub struct Server;
#[cfg(not(any(target_os = "macos", windows)))]
impl Server {
    pub fn bind(_: &Path) -> io::Result<Self> {
        Err(unsupported())
    }
    pub fn snapshot(&self) -> io::Result<Snapshot> {
        Err(unsupported())
    }
    pub fn finished(&self) -> bool {
        true
    }
}
#[cfg(not(any(target_os = "macos", windows)))]
fn unsupported() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "Companion session IPC is not implemented on this platform",
    )
}

/// MCP adapter: explicit endpoint, or an adjacent installer-provisioned cohort receipt.
/// Legacy Core installations with no receipt remain disabled (no directory/PATH search).
/// This owns one lease per MCP connection, not per tool or Project. No background reconnect:
/// after Companion quits, only a later explicit successful Project use can try again.
pub(crate) struct McpSession {
    endpoint: Option<std::path::PathBuf>,
    client: Option<Client>,
    release: Option<Release>,
    closed: bool,
}
impl std::fmt::Debug for McpSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpSession")
            .field("configured", &self.endpoint.is_some())
            .finish_non_exhaustive()
    }
}
impl McpSession {
    pub(crate) fn new(endpoint: Option<std::path::PathBuf>) -> Self {
        Self {
            endpoint,
            client: None,
            release: None,
            closed: false,
        }
    }
    pub(crate) fn used(&mut self) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        if self.client.as_ref().is_some_and(Client::alive) {
            if let Some(client) = &self.client {
                client.response();
            }
            return Ok(());
        }
        self.client = None;
        #[cfg(any(target_os = "macos", windows))]
        let mut installed = if self.endpoint.is_none() {
            crate::companion_discovery::discover_for_core(&std::env::current_exe()?)?
        } else {
            None
        };
        if self.endpoint.is_none() {
            #[cfg(any(target_os = "macos", windows))]
            if installed.is_none() {
                return Ok(());
            }
            #[cfg(not(any(target_os = "macos", windows)))]
            return Ok(());
        }
        // Observe this Core's executable path, not a candidate/version-directory label.
        // This is an identity claim, not proof of mapped bytes or publisher authenticity.
        if self.release.is_none() {
            use sha2::{Digest, Sha256};
            use std::io::Read;
            let mut file = std::fs::File::open(std::env::current_exe()?)?;
            let mut digest = Sha256::new();
            let mut buf = [0u8; 65536];
            loop {
                let n = file.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                digest.update(&buf[..n]);
            }
            self.release = Some(
                Release::new(
                    env!("CARGO_PKG_VERSION"),
                    &format!("{:x}", digest.finalize()),
                )
                .map_err(|_| io::Error::other("invalid running Core identity"))?,
            );
        }
        let release = self.release.as_ref().expect("release computed");
        let client = match &self.endpoint {
            Some(endpoint) => Client::connect(endpoint, release)?,
            #[cfg(any(target_os = "macos", windows))]
            None => installed
                .as_mut()
                .expect("installation discovered")
                .connect(release)?,
            #[cfg(not(any(target_os = "macos", windows)))]
            None => return Ok(()),
        };
        client.response();
        self.client = Some(client);
        Ok(())
    }
    pub(crate) fn close(&mut self) {
        self.closed = true;
        self.client = None;
    }
}
