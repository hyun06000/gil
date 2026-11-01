//! Explicitly approved, conservative recovery of legacy whole-tree snapshots.
//! No policy is retroactively assigned to a snapshot. Protected paths must be byte-identical;
//! all other changes are previewed and the original bytes are durably retained before movement.
use super::*;
use serde_json::{Value, json};
#[cfg(target_os = "macos")]
use sha2::{Digest, Sha256};

#[cfg(target_os = "macos")]
fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}
#[cfg(target_os = "macos")]
fn hash(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

impl ProjectSession {
    pub(crate) fn revisit_review(&self) -> Result<Value, String> {
        if self.project.cycles().pending_revisit().is_none() && !self.project.cycles().can_revisit()
        {
            return Ok(json!({"supported":true,"state":"not_needed"}));
        }
        #[cfg(not(target_os = "macos"))]
        return Ok(
            json!({"supported":false,"reason":"이 플랫폼의 승인 복원은 아직 제공하지 않는다. 기존 파일과 기록을 보존한다."}),
        );
        #[cfg(target_os = "macos")]
        {
            let plan = self.recovery_plan()?;
            Ok(plan.view)
        }
    }

    pub(crate) fn approve_revisit(&mut self, digest: &str) -> Result<Value, String> {
        #[cfg(not(target_os = "macos"))]
        {
            let _ = digest;
            Err("이 플랫폼의 승인 복원은 아직 제공하지 않는다".into())
        }
        #[cfg(target_os = "macos")]
        {
            let plan = self.recovery_plan()?;
            if plan.view["digest"].as_str() != Some(digest) {
                return Err("복원 계획이 바뀌었다. 변경 목록을 다시 확인하고 승인한다".into());
            }
            self.apply_recovery(plan)
        }
    }

    /// A model/CLI call may only resume a durable approval, never create one.
    pub(crate) fn resume_revisit(&mut self) -> Result<Value, String> {
        #[cfg(not(target_os = "macos"))]
        return Err(
            "이 플랫폼의 승인 복원은 아직 제공하지 않는다. 기존 파일과 기록을 보존한다".into(),
        );
        #[cfg(target_os = "macos")]
        {
            if self.project.cycles().pending_revisit().is_none() {
                return Err("Monitor의 되돌아가기 변경 목록을 확인하고 ‘보존 후 되돌아가기’를 눌러야 한다. 파일과 기록은 바꾸지 않았다".into());
            }
            let plan = self.recovery_plan()?;
            let dir = archive(self, &plan.key, false).map_err(error)?;
            verify_approval(&dir, &plan)?;
            self.apply_recovery(plan)
        }
    }
}

#[cfg(target_os = "macos")]
use crate::{
    artifact::{IgnoreSource, PolicyVersion, TrackingPolicy},
    internal_fs as guarded,
    restore::preview,
};
#[cfg(target_os = "macos")]
use std::{
    ffi::CString,
    fs::File,
    io::{Read, Write},
};

#[cfg(target_os = "macos")]
struct Recovery {
    next: Project,
    before: Manifest,
    target: Manifest,
    world: SnapshotRef,
    key: String,
    view: Value,
}

#[cfg(target_os = "macos")]
fn policy(store: &ObjectStore, manifest: &Manifest) -> Result<TrackingPolicy, String> {
    let mut sources = Vec::new();
    for entry in manifest.entries() {
        let path = entry.path.as_str();
        if path.rsplit('/').next() == Some(".gitignore") {
            let mut bytes = Vec::new();
            store.read_blob(&entry.content, &mut bytes).map_err(error)?;
            if bytes.len() > 1024 * 1024 {
                return Err("ignore 원문 크기 한도".into());
            }
            sources.push(IgnoreSource::new(
                path.rsplit_once('/').map_or("", |(dir, _)| dir),
                String::from_utf8(bytes).map_err(error)?,
            ));
        }
    }
    TrackingPolicy::from_sources(PolicyVersion::V1, sources).map_err(error)
}

#[cfg(target_os = "macos")]
impl ProjectSession {
    fn recovery_plan(&self) -> Result<Recovery, String> {
        let mut next = self.project.clone();
        let from = if let Some(from) = next.cycles().pending_revisit() {
            from
        } else {
            let from = next.current_cycle().id();
            next.revisit_cycle().map_err(error)?;
            from
        };
        let world = next.world_snapshot();
        let before_world = next
            .cycles()
            .node(from)
            .and_then(|c| c.exit_snapshot())
            .ok_or("실패 Snapshot 없음")?;
        let store = self.store().map_err(error)?;
        let read = |world| {
            store
                .read_manifest(next.world_manifest(world).ok_or("Snapshot 주소 없음")?)
                .map_err(error)
        };
        let before = read(before_world)?;
        let target = read(world)?;
        let root = self.root().map_err(error)?;
        let observed = artifact::observe(&root).map_err(error)?;
        let resumed = self.project.cycles().pending_revisit().is_some();
        if observed != before && !(resumed && observed == target) {
            return Err(
                "기록되지 않은 파일 변경이 있어 복원을 거절했다. 현재 파일은 보존한다".into(),
            );
        }
        // Full legacy worlds are compared, never filtered or re-labelled. If the protected
        // subset differs, there is no safe projection consistent with the old snapshot identity.
        let defaults = TrackingPolicy::from_sources(PolicyVersion::V1, vec![IgnoreSource::at_root(
            ".env\n.env.*\n*.pem\n*.key\n.ssh/\n.aws/\nnode_modules/\ntarget/\ndist/\nbuild/\n.cache/\n.venv/\n__pycache__/\n"
        )]).map_err(error)?;
        let old_policy = policy(&store, &before)?;
        let target_policy = policy(&store, &target)?;
        let includes = |path: &str| -> Result<bool, String> {
            Ok(defaults.includes_file(path)?
                && old_policy.includes_file(path)?
                && target_policy.includes_file(path)?)
        };
        let inspection = preview::preview(
            &preview::Request {
                before: &observed,
                target: &target,
                before_policy: None,
                target_policy: None,
                os: preview::TargetOs::MacOs,
            },
            |_| Ok(true),
            &preview::LstatProbe::at(&root),
        );
        if !matches!(inspection.probe(), preview::ProbeStatus::Checked { .. })
            || inspection
                .entries()
                .iter()
                .any(|e| e.blocked(preview::TargetOs::MacOs))
        {
            return Err("복원 경로에 충돌·링크·미확인 경계가 있어 거절했다".into());
        }
        let mut changes = Vec::new();
        let mut paths: Vec<_> = before.paths().chain(target.paths()).collect();
        paths.sort_by_key(|p| p.as_str());
        paths.dedup();
        for path in paths {
            if before.get(path) == target.get(path) {
                continue;
            }
            if !includes(path.as_str())? {
                return Err(format!(
                    "보호 대상이 달라 복원할 수 없다: {}. 원본과 Snapshot을 그대로 보존한다",
                    path.as_str()
                ));
            }
            if preview::inspect_text(path.as_str()).iter().any(|r| {
                matches!(
                    r.severity(preview::TargetOs::Windows),
                    preview::Severity::Blocks | preview::Severity::Unverified
                )
            }) {
                return Err(format!("안전하지 않은 복원 경로: {}", path.as_str()));
            }
            changes.push(
                json!({"path":path.as_str(),"action":match (before.get(path),target.get(path)) {
                    (Some(_),None)=>"preserve", (None,Some(_))=>"create", _=>"replace"
                }}),
            );
        }
        // All referenced evidence and destination bytes must be readable before any write.
        for entry in before.entries().iter().chain(target.entries()) {
            store
                .read_blob(&entry.content, &mut std::io::sink())
                .map_err(error)?;
        }
        if artifact::observe(&root).map_err(error)? != observed {
            return Err("검사 중 파일이 바뀌었다".into());
        }
        let encoded = store::encode(&next).map_err(error)?;
        let key = hash(&encoded);
        let mut view = json!({"supported":true,"state":if resumed && observed == target {"completed"} else if resumed {"resumable"} else {"awaiting_approval"},
            "from":from.to_ref().to_string(),"target":next.current_cycle().id().to_ref().to_string(),
            "before_world":before_world.to_string(),"target_world":world.to_string(),"changes":changes,
            "archive":format!(".gil/revisit-preserved/{key}"),"pending_state_sha256":key,
            "policy":"legacy-whole-tree-protected-difference-v1",
            "notice":"실패 기록·Snapshot·배움은 유지한다. 교체·제외되는 기존 파일은 보존본을 만든 뒤 대상 세계로 복원한다. 새 Cycle은 별도로 연다."});
        // Resumption is the same approved operation, not a new proposal.
        let mut bound = view.clone();
        bound["state"] = json!("awaiting_approval");
        view["digest"] = json!(hash(serde_json::to_vec(&bound).map_err(error)?));
        Ok(Recovery {
            next,
            before,
            target,
            world,
            key,
            view,
        })
    }

    fn apply_recovery(&mut self, plan: Recovery) -> Result<Value, String> {
        let store = self.store().map_err(error)?;
        let dir = archive(self, &plan.key, true).map_err(error)?;
        // Save every before-world file under opaque ordinals, with an explicit path mapping.
        // These are retained, not the transient restore rollback backups that get reaped.
        let mut mapping = Vec::new();
        for (i, entry) in plan.before.entries().iter().enumerate() {
            let name = CString::new(format!("file-{i}")).unwrap();
            match guarded::regular_at(&dir, &name) {
                Ok(f) => {
                    if artifact::digest_of(f).map_err(error)? != entry.content {
                        return Err("보존본이 불완전하다. 기존 파일은 변경하지 않는다".into());
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    publish(&dir, &name, |f| {
                        store.read_blob(&entry.content, f).map_err(error)
                    })?;
                }
                Err(e) => return Err(error(e)),
            }
            mapping.push(json!({"path":entry.path.as_str(),"saved_as":format!("file-{i}"),"sha256":entry.content.hex()}));
        }
        write_once(
            &dir,
            c"files.json",
            &serde_json::to_vec_pretty(&mapping).map_err(error)?,
        )?;
        let mut approval = plan.view.clone();
        approval["state"] = json!("approved");
        write_once(
            &dir,
            c"approval.json",
            &serde_json::to_vec_pretty(&approval).map_err(error)?,
        )?;
        guarded::fsync(&dir).map_err(error)?;
        verify_approval(&dir, &plan)?;
        let root = self.root().map_err(error)?;
        let now = artifact::observe(&root).map_err(error)?;
        let pending = self.project.cycles().pending_revisit().is_some();
        if now != plan.before && !(pending && now == plan.target) {
            return Err("승인 후 파일이 바뀌었다. 현재 파일을 보존한다".into());
        }
        if !pending {
            store::save_within(&self._lock, &plan.next, &self.state_path).map_err(error)?;
            self.project = plan.next;
        }
        super::failpoint::fail_at("recovery-after-state-save").map_err(error)?;
        restore::run_with_expected(
            &root,
            &gil_dir(&self.state_path).map_err(error)?,
            &store,
            plan.world,
            &plan.target,
            Some(&now),
        )
        .map_err(|e| {
            format!("승인과 보존본은 저장됐다. gil_revisit으로 같은 복원을 재개한다: {e}")
        })?;
        let mut result = plan.view;
        result["state"] = json!("completed");
        result["next"] = json!(
            "gil_open으로 새 Experiment 또는 Interview를 연다. 실패 계보와 승인 항목은 유지된다."
        );
        Ok(result)
    }
}

#[cfg(target_os = "macos")]
fn archive(s: &ProjectSession, key: &str, create: bool) -> std::io::Result<File> {
    let gil = guarded::directory(s.state_path.parent().unwrap(), false)?;
    fn child(parent: &File, name: &std::ffi::CStr, create: bool) -> std::io::Result<File> {
        if create {
            match guarded::create_dir_at(parent, name, 0o700) {
                Ok(dir) => {
                    guarded::fsync(parent)?;
                    return Ok(dir);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e),
            }
        }
        guarded::child_dir(parent, name)
    }
    let base = child(&gil, c"revisit-preserved", create)?;
    child(&base, &CString::new(key).unwrap(), create)
}
#[cfg(target_os = "macos")]
fn write_once(dir: &File, name: &std::ffi::CStr, bytes: &[u8]) -> Result<(), String> {
    match guarded::regular_at(dir, name) {
        Ok(mut f) => {
            let mut old = Vec::new();
            f.read_to_end(&mut old).map_err(error)?;
            if old == bytes {
                Ok(())
            } else {
                Err("기존 보존 receipt가 다르거나 불완전하다".into())
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            publish(dir, name, |f| f.write_all(bytes).map_err(error))
        }
        Err(e) => Err(error(e)),
    }
}
#[cfg(target_os = "macos")]
fn verify_approval(dir: &File, plan: &Recovery) -> Result<(), String> {
    let f = guarded::regular_at(dir, c"approval.json").map_err(error)?;
    let got: Value = serde_json::from_reader(f.take(4 * 1024 * 1024)).map_err(error)?;
    let mut expected = plan.view.clone();
    expected["state"] = json!("approved");
    if got != expected {
        return Err("저장된 복원 승인이 현재 상태와 다르다".into());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn publish(
    dir: &File,
    name: &std::ffi::CStr,
    write: impl FnOnce(&mut File) -> Result<(), String>,
) -> Result<(), String> {
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(error)?;
    let tmp = CString::new(format!("part-{}", hash(random))).unwrap();
    let mut f = guarded::create_file_at(dir, &tmp, 0o600).map_err(error)?;
    write(&mut f)?;
    f.sync_all().map_err(error)?;
    guarded::rename_at(dir, &tmp, name).map_err(error)?;
    guarded::fsync(dir).map_err(error)
}
