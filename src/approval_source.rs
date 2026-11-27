//! Who stood in for a user's approval. The mark names a channel, never a person.
//!
//! `monitor_button` is the ordinary product path: a button in the Monitor App. `unattended_runner`
//! is written only by a server its runner started in the unattended mode (see `mcp::unattended`):
//! no screen and no person took part, and whoever ran the session answered through tools the model
//! is not given. A record keeps the mark it was written with; neither is ever rewritten as the other.
pub(crate) const MONITOR: &str = "monitor_button";
pub(crate) const UNATTENDED: &str = "unattended_runner";

/// A source this build can have written. Anything else in a record is refused as before.
pub(crate) fn known(source: &str) -> bool {
    source == MONITOR || source == UNATTENDED
}
