//! Ephemeral App observations, scoped to one MCP connection and one Project.
//! Never infer display from a View, a filesystem watcher or native Companion IPC.
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use serde_json::{Value, json};

const FRESH: Duration = Duration::from_secs(15);
const OPEN_GRACE: Duration = Duration::from_secs(30);
const MAX_INSTANCES: usize = 32;

#[derive(Debug)]
struct Seen { sequence: u64, state: String, at: Instant }
#[derive(Debug, Default)]
pub struct Presence {
    instances: BTreeMap<String, Seen>,
    requested: Option<Instant>,
    unanswered: u8,
}

pub fn valid(input: &Value) -> bool {
    let fields = ["instance_id", "presentation", "sequence"];
    if fields.iter().all(|key| input.get(key).is_none()) { return true; }
    let Some(id) = input["instance_id"].as_str() else { return false; };
    let Some(sequence) = input["sequence"].as_str() else { return false; };
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && sequence.len() <= 20 && sequence.bytes().all(|b| b.is_ascii_digit()) && !sequence.starts_with('0') && sequence.parse::<u64>().is_ok()
        && matches!(input["presentation"].as_str(), Some("fullscreen" | "inline" | "hidden" | "closed" | "loading" | "error"))
}

impl Presence {
    pub fn observe(&mut self, input: &Value, now: Instant) {
        let Some(id) = input["instance_id"].as_str() else { return; };
        let sequence = input["sequence"].as_str().unwrap().parse::<u64>().unwrap();
        if let Some(old) = self.instances.get(id) {
            // A late response cannot resurrect a torn-down instance or renew its lease.
            if old.state == "closed" || sequence <= old.sequence { return; }
        } else if self.instances.len() >= MAX_INSTANCES {
            // Keep tombstones longer than the App's bounded request timeout, then reclaim.
            self.instances.retain(|_, s| now.saturating_duration_since(s.at) < OPEN_GRACE);
            if self.instances.len() >= MAX_INSTANCES { return; }
        }
        if matches!(input["presentation"].as_str(), Some("fullscreen" | "inline")) { self.unanswered = 0; }
        self.instances.insert(id.to_owned(), Seen { sequence,
            state: input["presentation"].as_str().unwrap().to_owned(), at: now });
    }
    pub fn requested(&mut self, now: Instant) { self.requested = Some(now); self.unanswered = self.unanswered.saturating_add(1); }
    pub fn status(&self, scope: &str, now: Instant) -> Value {
        let fresh = |s: &&Seen| now.saturating_duration_since(s.at) <= FRESH;
        let visible = self.instances.values().filter(fresh)
            .filter(|s| matches!(s.state.as_str(), "fullscreen" | "inline"))
            .max_by_key(|s| (s.state == "fullscreen", s.at));
        let latest = self.instances.values().filter(fresh).max_by_key(|s| s.at);
        let pending = self.requested.is_some_and(|at| now.saturating_duration_since(at) < OPEN_GRACE
            && latest.is_none_or(|s| s.at < at || s.state == "loading"));
        let (state, mode) = if let Some(s) = visible { ("visible", Some(s.state.as_str())) }
            else if pending { ("opening", None) }
            else if self.unanswered >= 2 { ("unconfirmed", None) }
            else { (latest.map_or("unknown", |s| s.state.as_str()), None) };
        json!({"scope_id":scope,"state":state,"display_mode":mode,
            "should_open": visible.is_none() && !pending && self.unanswered < 2,
            "last_seen_ms":self.instances.values().map(|s| now.saturating_duration_since(s.at).as_millis() as u64).min(),
            "evidence":"app_report", "scope":"this_mcp_connection"})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn observation(id: &str, sequence: u64, state: &str) -> Value {
        json!({"instance_id":id,"sequence":sequence.to_string(),"presentation":state})
    }
    #[test]
    fn only_fresh_app_visibility_counts_and_open_requests_have_a_bounded_grace() {
        let mut p = Presence::default(); let now = Instant::now();
        assert_eq!(p.status("s",now)["should_open"],true);
        p.requested(now);
        assert_eq!(p.status("s",now)["state"],"opening");
        assert_eq!(p.status("s",now+OPEN_GRACE)["should_open"],true);
        p.observe(&observation("a",1,"fullscreen"),now);
        assert_eq!(p.status("s",now)["state"],"visible");
        assert_eq!(p.status("s",now)["display_mode"],"fullscreen");
        assert_eq!(p.status("s",now+OPEN_GRACE)["state"],"unknown");
        p.observe(&observation("a",2,"inline"),now+OPEN_GRACE);
        assert_eq!(p.status("s",now+OPEN_GRACE)["should_open"],false);
    }
    #[test]
    fn hidden_closed_late_events_and_multiple_instances_do_not_lie_about_display() {
        let mut p=Presence::default();let now=Instant::now();
        p.observe(&observation("a",2,"hidden"),now);
        p.observe(&observation("a",1,"fullscreen"),now);
        assert_eq!(p.status("s",now)["state"],"hidden");
        p.observe(&observation("b",1,"fullscreen"),now);
        assert_eq!(p.status("s",now)["should_open"],false);
        p.observe(&observation("b",2,"closed"),now);
        p.observe(&observation("b",3,"fullscreen"),now);
        assert_eq!(p.status("s",now)["should_open"],true);
        assert_ne!(p.status("s",now)["state"],"visible");
    }
    #[test]
    fn failed_display_is_bounded_and_a_real_visible_report_restores_the_budget() {
        let mut p=Presence::default();let now=Instant::now();
        p.requested(now);
        assert_eq!(p.status("s",now+OPEN_GRACE)["should_open"],true);
        p.requested(now+OPEN_GRACE);
        let later=now+OPEN_GRACE+OPEN_GRACE;
        assert_eq!(p.status("s",later)["state"],"unconfirmed");
        assert_eq!(p.status("s",later)["should_open"],false);
        p.observe(&observation("new",1,"fullscreen"),later);
        p.observe(&observation("new",2,"hidden"),later);
        assert_eq!(p.status("s",later)["should_open"],true);
    }
    #[test]
    fn instance_registry_is_bounded_but_can_recover_after_many_closed_views() {
        let mut p=Presence::default();let now=Instant::now();
        for n in 0..MAX_INSTANCES {p.observe(&observation(&format!("app-{n}"),1,"closed"),now);}
        p.observe(&observation("overflow",1,"fullscreen"),now);
        assert_eq!(p.instances.len(),MAX_INSTANCES);
        assert_ne!(p.status("s",now)["state"],"visible");
        p.observe(&observation("new",1,"fullscreen"),now+OPEN_GRACE);
        assert_eq!(p.instances.len(),1);
        assert_eq!(p.status("s",now+OPEN_GRACE)["state"],"visible");
    }
    #[test]
    fn partial_or_unbounded_observations_are_rejected() {
        assert!(valid(&json!({"scope_id":"s"})));
        assert!(valid(&observation("app-1",1,"loading")));
        for input in [json!({"instance_id":"x"}),observation("../private",1,"hidden"),
            observation("x",0,"hidden"),observation("x",1,"pretend-visible")] {
            assert!(!valid(&input));
        }
    }
}
