//! Session-owned Companion policy, not an updater or an IPC authentication boundary.
//!
//! The native adapter supplies a monotonic clock and registers a connection only after a
//! Project has opened successfully. MCP startup alone is not a registration. A worker
//! reports update completion only after independently verifying and publishing a release.
//! This module performs no network, filesystem, process, model, or Project operations.
//! See `spec/GIL_Companion_Install_Update_v0.1.md` for the unimplemented adapters.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde::Serialize;

pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);
pub const LEASE_TIMEOUT: Duration = Duration::from_secs(60);
pub const STARTUP_GRACE: Duration = Duration::from_secs(30);
const MAX_CONNECTIONS: usize = 128;

/// Process-local IDs are not bearer credentials and must never be accepted from arbitrary IPC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConnectionId(u64);

/// Exact release identity; version labels are not ordered to guess a newer release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Release {
    version: String,
    sha256: String,
}

impl Release {
    pub fn new(version: &str, sha256: &str) -> Result<Self, Error> {
        if version.is_empty()
            || version.len() > 128
            || !version
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".-+_".contains(&b))
            || sha256.len() != 64
            || !sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Error::InvalidRelease);
        }
        Ok(Self {
            version: version.into(),
            sha256: sha256.into(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    InvalidRelease,
    Capacity,
    IdExhausted,
    ClockWentBackwards,
    Busy,
    NoOffer,
    OfferChanged,
    WrongOperation,
    ReceiptMismatch,
    ShuttingDown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum UpdateState {
    Idle,
    Offered {
        release: Release,
    },
    Running {
        release: Release,
    },
    AwaitingConnection {
        release: Release,
    },
    Applied {
        release: Release,
    },
    /// Failure never implies that disk selection was unchanged. The worker's durable report
    /// owns that fact; this policy neither rolls back nor retries.
    Failed {
        release: Release,
    },
}

/// A UI operation correlation token, not proof of human approval or publisher authenticity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateTicket {
    id: u64,
    release: Release,
}

impl UpdateTicket {
    pub fn release(&self) -> &Release {
        &self.release
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub active_connections: usize,
    pub update: UpdateState,
    pub exit_requested: bool,
    pub should_exit: bool,
}

struct Connection {
    last_seen: Instant,
    release: Release,
}

/// Owned by the one native Companion instance. Call through one serialized event loop.
pub struct Controller {
    last_now: Instant,
    started: Instant,
    had_connection: bool,
    exit_requested: bool,
    next_id: u64,
    connections: BTreeMap<ConnectionId, Connection>,
    update: UpdateState,
    operation: Option<UpdateTicket>,
    // Only a connection registered after successful publication can confirm application.
    first_new_connection_id: u64,
}

impl Controller {
    pub fn new(now: Instant) -> Self {
        Self {
            last_now: now,
            started: now,
            had_connection: false,
            exit_requested: false,
            next_id: 1,
            connections: BTreeMap::new(),
            update: UpdateState::Idle,
            operation: None,
            first_new_connection_id: 1,
        }
    }

    fn tick(&mut self, now: Instant) -> Result<(), Error> {
        if now < self.last_now {
            return Err(Error::ClockWentBackwards);
        }
        self.last_now = now;
        self.connections
            .retain(|_, c| now.duration_since(c.last_seen) < LEASE_TIMEOUT);
        Ok(())
    }

    fn allocate(&mut self) -> Result<u64, Error> {
        let id = self.next_id;
        self.next_id = id.checked_add(1).ok_or(Error::IdExhausted)?;
        Ok(id)
    }

    /// Native adapter only: a Project has actually been opened on this MCP connection.
    /// The release comes from the running Core, not the latest manifest or directory name.
    pub fn project_connected(
        &mut self,
        release: Release,
        now: Instant,
    ) -> Result<ConnectionId, Error> {
        if self.exit_requested {
            return Err(Error::ShuttingDown);
        }
        self.tick(now)?;
        if self.connections.len() >= MAX_CONNECTIONS {
            return Err(Error::Capacity);
        }
        let id = ConnectionId(self.allocate()?);
        self.connections.insert(
            id,
            Connection {
                last_seen: now,
                release,
            },
        );
        self.had_connection = true;
        Ok(id)
    }

    /// Expired registrations cannot be resurrected by a delayed heartbeat.
    pub fn heartbeat(&mut self, id: ConnectionId, now: Instant) -> Result<bool, Error> {
        self.tick(now)?;
        let Some(connection) = self.connections.get_mut(&id) else {
            return Ok(false);
        };
        connection.last_seen = now;
        Ok(true)
    }

    pub fn disconnected(&mut self, id: ConnectionId, now: Instant) -> Result<(), Error> {
        self.tick(now)?;
        self.connections.remove(&id);
        Ok(())
    }

    /// Discovery is informational only. Does not download, execute, or select anything.
    pub fn offer(&mut self, release: Release) -> Result<(), Error> {
        if self.exit_requested {
            return Err(Error::ShuttingDown);
        }
        if self.operation.is_some() {
            return Err(Error::Busy);
        }
        self.update = UpdateState::Offered { release };
        Ok(())
    }

    /// Called only from the native user-confirmation handler. Do not expose through MCP,
    /// agent instructions, arbitrary URLs or an unauthenticated local socket.
    pub fn user_confirmed(&mut self, displayed: &Release) -> Result<UpdateTicket, Error> {
        if self.exit_requested {
            return Err(Error::ShuttingDown);
        }
        if self.operation.is_some() {
            return Err(Error::Busy);
        }
        let UpdateState::Offered { release } = &self.update else {
            return Err(Error::NoOffer);
        };
        if release != displayed {
            return Err(Error::OfferChanged);
        }
        let ticket = UpdateTicket {
            id: self.allocate()?,
            release: displayed.clone(),
        };
        self.update = UpdateState::Running {
            release: displayed.clone(),
        };
        self.operation = Some(ticket.clone());
        Ok(ticket)
    }

    /// The worker must persist its result before this callback: completing a worker can
    /// immediately allow shutdown. A publication receipt is not yet a new-Core response.
    pub fn published(&mut self, ticket: &UpdateTicket, receipt: &Release) -> Result<(), Error> {
        if self.operation.as_ref() != Some(ticket) {
            return Err(Error::WrongOperation);
        }
        if ticket.release != *receipt {
            return Err(Error::ReceiptMismatch);
        }
        self.first_new_connection_id = self.next_id;
        self.update = UpdateState::AwaitingConnection {
            release: receipt.clone(),
        };
        self.operation = None;
        Ok(())
    }

    /// Report a durable failure, release the shutdown hold, and never auto-retry/rollback.
    pub fn failed(&mut self, ticket: &UpdateTicket) -> Result<(), Error> {
        if self.operation.as_ref() != Some(ticket) {
            return Err(Error::WrongOperation);
        }
        self.update = UpdateState::Failed {
            release: ticket.release.clone(),
        };
        self.operation = None;
        Ok(())
    }

    /// Native adapter reports a successful tool response, in addition to connection identity.
    /// Merely starting a process, reloading MCP or selecting a version is not sufficient.
    pub fn response_confirmed(&mut self, id: ConnectionId, now: Instant) -> Result<bool, Error> {
        self.tick(now)?;
        let UpdateState::AwaitingConnection { release } = &self.update else {
            return Ok(false);
        };
        let Some(connection) = self.connections.get(&id) else {
            return Ok(false);
        };
        if id.0 < self.first_new_connection_id || connection.release != *release {
            return Ok(false);
        }
        self.update = UpdateState::Applied {
            release: release.clone(),
        };
        Ok(true)
    }

    /// A normal UI Quit is deferred only while the user-requested update is running.
    /// OS shutdown is not prevented by this in-memory policy.
    pub fn request_exit(&mut self) {
        self.exit_requested = true;
    }

    pub fn snapshot(&mut self, now: Instant) -> Result<Snapshot, Error> {
        self.tick(now)?;
        let idle = self.connections.is_empty()
            && (self.had_connection || now.duration_since(self.started) >= STARTUP_GRACE);
        Ok(Snapshot {
            active_connections: self.connections.len(),
            update: self.update.clone(),
            exit_requested: self.exit_requested,
            should_exit: self.operation.is_none() && (self.exit_requested || idle),
        })
    }
}

#[cfg(test)]
mod tests;
