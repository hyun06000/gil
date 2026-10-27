//! Read-only MCP Monitor adapter. Paths stay here; the App receives opaque scopes.
//! Watches carry hints only. No recovery, write action, UI layout, or HTTP server.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, atomic::{AtomicU64, Ordering}};
use std::time::{Duration, Instant};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use rmcp::model::CallToolResult;
use crate::{ProjectSession, ReadOnlySession, RuleSet, SessionError, StoreError};
use super::bindings::{Binding, Store};

const LEASE: Duration = Duration::from_secs(90);
const MAX_PROJECTS: usize = 32;

#[derive(Debug)]
struct Entry {
    binding: Binding,
    revision: Arc<AtomicU64>,
    watching: Option<crate::Watching>,
    touched: Instant,
    attempted: Option<Instant>,
}

#[derive(Debug)]
pub struct Monitor { entries: Mutex<BTreeMap<String, Entry>>, store: Store, epoch: String }

impl Default for Monitor {
    fn default() -> Self { Self::with_store(Store::default()) }
}

impl Entry {
    fn new(binding: Binding) -> Self {
        Self { binding, revision: Arc::new(AtomicU64::new(0)), watching: None,
            touched: Instant::now(), attempted: None }
    }
}

#[derive(Debug)]
pub struct Refusal { pub(super) code: &'static str, pub(super) said: &'static str }

impl Refusal {
    pub(super) fn new(code: &'static str, said: &'static str) -> Self { Self { code, said } }
}

fn refuse(err: SessionError) -> Refusal {
    match err {
        SessionError::Busy { .. } => Refusal::new("busy", "다른 GIL 명령이 Project 를 사용 중이다"),
        SessionError::NeedsRecovery { .. } => Refusal::new("needs_recovery", "끝나지 않은 복원이 남아 있다 — GIL 명령으로 복구한 뒤 다시 읽는다"),
        SessionError::LegacyFormat { .. } => Refusal::new("unsupported_format", "이 Monitor 가 읽지 않는 저장 판이다"),
        SessionError::Store(StoreError::UnknownFormat { .. } | StoreError::PreviousFormat { .. } | StoreError::LegacyFormat { .. }) => Refusal::new("unsupported_format", "이 Monitor 가 읽지 않는 저장 판이다"),
        SessionError::Store(StoreError::NotFound { .. }) | SessionError::NoProjectDir { .. } => Refusal::new("not_a_project", "이 자리에 GIL Project 가 없다"),
        SessionError::Unreadable { .. } | SessionError::LockUnavailable { .. } | SessionError::Store(StoreError::Read { .. }) => Refusal::new("unreadable", "Project 기록을 읽지 못했다"),
        SessionError::Observe { .. } | SessionError::Object { .. } => Refusal::new("observe_failed", "Project 세계를 관측하지 못했다"),
        _ => Refusal::new("damaged", "Project 기록을 검증하지 못했다"),
    }
}

fn opened(root: &Path) -> Result<ReadOnlySession, Refusal> {
    let state = crate::project_state::state_for_root(root)
        .map_err(|_| Refusal::new("unreadable", "Project 기록 위치를 확인하지 못했다"))?;
    if !state.is_file() { return Err(Refusal::new("not_a_project", "이 자리에 GIL Project 가 없다")); }
    let rules = RuleSet::builtin().map_err(|_| Refusal::new("damaged", "함께 실린 명세를 읽지 못했다"))?;
    #[cfg(windows)]
    let opened = if state == root.join(crate::STATE_PATH) {
        ProjectSession::open_read_only(rules, &state)
    } else {
        ProjectSession::open_read_only_at(rules, root, &state)
    };
    #[cfg(not(windows))]
    let opened = ProjectSession::open_read_only(rules, &state);
    opened.map_err(refuse)
}

impl Monitor {
    fn with_store(store: Store) -> Self {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
        let unique = format!("{}-{time}-{}", std::process::id(), SERIAL.fetch_add(1, Ordering::Relaxed));
        Self { entries: Mutex::new(BTreeMap::new()), store,
            epoch: format!("{:x}", Sha256::digest(unique.as_bytes())) }
    }

    pub fn prepare(&self, root: &str) -> Result<Value, Refusal> {
        let path = Path::new(root);
        if !path.is_absolute() { return Err(Refusal::new("invalid_root", "명시적인 Project 절대경로가 필요하다")); }
        let root = path.canonicalize().map_err(|_| Refusal::new("unreadable", "Project 자리를 읽지 못했다"))?;
        // Exact root; never an ancestor search, cwd guess, or Companion's selection.
        let session = opened(&root)?;
        let binding = Binding::capture(root)?;
        let scope = binding.scope();
        let label = binding.label();
        let mut entries = self.entries.lock().map_err(|_| Refusal::new("unavailable", "Monitor 등록부를 열지 못했다"))?;
        if let Some(old) = entries.get(&scope) {
            if old.binding != binding { return Err(Refusal::new("scope_collision", "서로 다른 Project 주소가 겹쳤다 — 섞지 않는다")); }
        } else if entries.len() >= MAX_PROJECTS {
            return Err(Refusal::new("limit", "이 연결의 Project 한도에 도달했다 — 새 연결에서 연다"));
        }
        // Only explicit preparation writes LOCAL consent settings. The session's Project
        // lock is released here; neither persistence nor restoration holds a long-lived lock.
        self.store.save(&binding)?;
        entries.entry(scope.clone()).or_insert_with(|| Entry::new(binding));
        drop(session);
        Ok(json!({ "scope_id": scope, "label": label }))
    }

    fn root(&self, scope: &str) -> Result<PathBuf, Refusal> {
        let mut entries = self.entries.lock().map_err(|_| Refusal::new("unavailable", "Monitor 등록부를 열지 못했다"))?;
        if !entries.contains_key(scope) {
            // Load ONLY this App's saved consent, never enumerate projects or infer cwd.
            let binding = self.store.load(scope)?;
            binding.validate()?;
            if entries.len() >= MAX_PROJECTS { return Err(Refusal::new("limit", "이 연결의 Project 한도에 도달했다 — 새 연결에서 연다")); }
            entries.insert(scope.to_owned(), Entry::new(binding));
        }
        let e = &entries[scope];
        e.binding.validate()?;
        Ok(e.binding.root.clone())
    }

    fn session(&self, scope: &str) -> Result<ReadOnlySession, Refusal> {
        let root = self.root(scope)?;
        let session = opened(&root)?;
        // Check again under the domain lock before returning any freshly read facts.
        self.root(scope)?;
        Ok(session)
    }

    /// No project scan here. A quiet App checks only this counter every two seconds.
    pub fn poll(&self, scope: &str) -> Result<Value, Refusal> {
        let root = self.root(scope)?;
        let mut entries = self.entries.lock().map_err(|_| Refusal::new("unavailable", "Monitor 등록부를 열지 못했다"))?;
        let e = entries.get_mut(scope).ok_or_else(super::bindings::unknown)?;
        e.touched = Instant::now();
        // Retry a failed watcher at most once a minute, not every UI heartbeat.
        if e.watching.is_none() && e.attempted.is_none_or(|at| at.elapsed() >= Duration::from_secs(60)) {
            e.attempted = Some(Instant::now());
            let revision = e.revision.clone();
            e.watching = crate::watch_project_hints(&root, move || {
                revision.fetch_add(1, Ordering::Relaxed);
            }).ok();
            // A renewed watch may have missed events while its lease was absent.
            e.revision.fetch_add(1, Ordering::Relaxed);
        }
        // A restarted watcher can have the same numeric counter as the old one. The
        // process epoch forces a complete refresh instead of silently waiting five minutes.
        Ok(json!({ "scope_id": scope, "revision": format!("{}:{}", self.epoch, e.revision.load(Ordering::Relaxed)), "watching": e.watching.is_some() }))
    }

    pub fn view(&self, scope: &str) -> Result<Value, Refusal> {
        // Capture cursor BEFORE reading, so changes during the read cause another refresh.
        let hint = self.poll(scope)?;
        let session = self.session(scope)?;
        let snapshot = session.monitor().map_err(refuse)?;
        let view = crate::monitor_view_v1(&snapshot).map_err(|_| Refusal::new("unsupported_vocabulary", "이 Monitor 가 모르는 판정·종류·시각이 있다"))?;
        let label = self.entries.lock().map_err(|_| Refusal::new("unavailable", "Monitor 등록부를 열지 못했다"))?[scope].binding.label();
        Ok(json!({ "scope_id": scope, "label": label, "view": view,
            "revision": hint["revision"], "watching": hint["watching"] }))
    }

    pub fn detail(&self, scope: &str, step: &str) -> Result<Value, Refusal> {
        let step = step.parse::<crate::StepRef>().map_err(|_| Refusal::new("not_found", "정확한 Step 주소가 필요하다"))?;
        let session = self.session(scope)?;
        let detail = session.node_detail_v1(step).map_err(|_| Refusal::new("not_found", "그 Step 은 이 Project 에 없다"))?;
        Ok(json!({ "scope_id": scope, "detail": detail }))
    }

    pub(crate) fn report_visuals(&self, scope: &str, owner: crate::visual_report::registry::ReportOwner) -> Result<Value, Refusal> {
        let session = self.session(scope)?;
        let detail = session.project().report_visuals_v1(owner)
            .map_err(|_| Refusal::new("not_found", "그 Report는 이 Project에 없다"))?;
        let mut value = serde_json::to_value(detail).map_err(|_| Refusal::new("unavailable", "보고서를 표시하지 못했다"))?;
        value["scope_id"] = json!(scope);
        if serde_json::to_vec(&value).map_err(|_| Refusal::new("unavailable", "보고서를 표시하지 못했다"))?.len() > 1024*1024 {
            return Err(Refusal::new("response_too_large", "시각 자료 목록이 응답 한도를 넘었다"));
        }
        self.root(scope)?;
        Ok(value)
    }

    /// Verified object read used by the App-only chunk tool.
    /// Scope selects the Project; an asset ID alone never grants access to another Report.
    #[allow(dead_code)]
    pub(crate) fn report_image(
        &self,
        scope: &str,
        owner: crate::visual_report::registry::ReportOwner,
        asset: crate::visual_report::VisualAssetId,
    ) -> Result<crate::visual_report::image::VerifiedImage, Refusal> {
        let session = self.session(scope)?;
        let image = session.read_report_image(owner, asset)
            .map_err(|_| Refusal::new("visual_unavailable", "이 Report의 시각 자료를 검증하여 읽지 못했다"))?;
        // Keep the read-only lock through the final consent/identity check. Return no paths.
        self.root(scope)?;
        Ok(image)
    }

    fn expire(&self, now: Instant) {
        if let Ok(mut entries) = self.entries.lock() {
            for e in entries.values_mut() {
                if now.saturating_duration_since(e.touched) > LEASE {
                    e.watching = None;
                    e.attempted = None;
                }
            }
        }
    }

    pub fn reap(self: &Arc<Self>) {
        let weak = Arc::downgrade(self);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(15)).await;
                let Some(monitor) = weak.upgrade() else { break; };
                // Dropping OS watchers can block. Never do that on the stdio runtime thread.
                if tokio::task::spawn_blocking(move || monitor.expire(Instant::now())).await.is_err() { break; }
            }
        });
    }
}

pub async fn run(work: impl FnOnce() -> Result<Value, Refusal> + Send + 'static) -> CallToolResult {
    match tokio::task::spawn_blocking(work).await {
        Ok(Ok(value)) => CallToolResult::structured(value),
        other => {
            let err = match other { Ok(Err(e)) => e, _ => Refusal::new("unavailable", "Monitor 조회를 끝내지 못했다") };
            let mut result = CallToolResult::structured(json!({ "code": err.code, "said": err.said }));
            result.is_error = Some(true);
            result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_scope_and_relative_root_are_path_free() {
        let m = Monitor::default();
        assert_eq!(m.prepare("../private").unwrap_err().code, "invalid_root");
        assert_eq!(m.view("project:unknown").unwrap_err().code, "reconnect_required");
        assert_eq!(m.detail("project:unknown", "step:C1/S1").unwrap_err().code, "reconnect_required");
        let owner = crate::visual_report::registry::ReportOwner::Step("step:C1/S1".parse().unwrap());
        let asset = crate::visual_report::VisualAssetId::new(std::num::NonZeroU64::new(1).unwrap());
        assert_eq!(m.report_image("project:unknown", owner, asset).unwrap_err().code, "reconnect_required");
    }

    #[test]
    fn image_scope_rejects_unlinked_assets_and_replaced_project() {
        use crate::visual_report::{VisualAssetId, registry::ReportOwner};
        let at = std::env::temp_dir().join(format!("gil-image-scope-{}", std::process::id()));
        std::fs::create_dir(&at).unwrap();
        let root = at.join("project");
        std::fs::create_dir(&root).unwrap();
        let session = ProjectSession::start(RuleSet::builtin().unwrap(), root.join(crate::STATE_PATH)).unwrap();
        session.commit().unwrap();
        drop(session);
        let m = Monitor::with_store(Store::at(at.join("consent")));
        let prepared = m.prepare(root.to_str().unwrap()).unwrap();
        let scope = prepared["scope_id"].as_str().unwrap();
        let owner = ReportOwner::Step("step:C1/S1".parse().unwrap());
        let asset = VisualAssetId::new(std::num::NonZeroU64::new(1).unwrap());
        let before = std::fs::read(root.join(crate::STATE_PATH)).unwrap();
        let err = m.report_image(scope, owner, asset).unwrap_err();
        assert_eq!(err.code, "visual_unavailable");
        assert!(!err.said.contains(root.to_str().unwrap()));
        assert_eq!(std::fs::read(root.join(crate::STATE_PATH)).unwrap(), before);
        std::fs::rename(root.join(".gil"), at.join("original-gil")).unwrap();
        std::fs::create_dir(root.join(".gil")).unwrap();
        assert_eq!(m.report_image(scope, owner, asset).unwrap_err().code, "project_moved");
        // Synthetic test tree only.
        std::fs::remove_dir_all(at).unwrap();
    }
    #[test]
    fn lease_expiry_allows_a_fresh_watch_without_retaining_app_state() {
        let m = Monitor::default();
        let now = Instant::now();
        let at = std::env::temp_dir().join(format!("gil-monitor-lease-{}", std::process::id()));
        std::fs::create_dir_all(at.join(".gil")).unwrap();
        let mut e = Entry::new(Binding::capture(at.clone()).unwrap());
        e.touched = now; e.attempted = Some(now);
        m.entries.lock().unwrap().insert("s".into(), e);
        m.expire(now + LEASE + Duration::from_secs(1));
        assert!(m.entries.lock().unwrap()["s"].attempted.is_none());
        std::fs::remove_dir_all(at).unwrap();
    }
}
