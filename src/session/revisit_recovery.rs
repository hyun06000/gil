//! Explicitly approved, conservative recovery of legacy whole-tree snapshots.
//! No policy is retroactively assigned to a snapshot. Protected paths must be byte-identical;
//! all other changes are previewed and the original bytes are durably retained before movement.
//!
//! M4-R approval modes: every approval record names its source — the Monitor button (manual) or
//! a valid per-Project automatic setting (`crate::recovery_mode`, opt-in build). The automatic
//! path runs the same plan, refusals, preservation and resumption as the manual one; it only
//! replaces the click. A released setting never cancels or re-approves a started recovery.
use super::*;
use serde_json::{Value, json};
#[cfg(any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows))]
use sha2::{Digest, Sha256};

#[cfg(any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows))]
fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}
#[cfg(any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows))]
fn hash(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

impl ProjectSession {
    pub(crate) fn revisit_review(&self) -> Result<Value, String> {
        // 형식 10/11: 최신 승인 정책 기준 복원(구현 순서 6번)이 없으므로 whole-tree 복구 계획을 만들지 않는다.
        #[cfg(feature = "tracking-policy")]
        if self.project.tracking().is_some() {
            return Ok(json!({"supported":false,"reason":crate::locale::text("session.tracking_unsupported.revisit")}));
        }
        if self.project.cycles().pending_revisit().is_none() && !self.project.cycles().can_revisit()
        {
            return Ok(json!({"supported":true,"state":"not_needed"}));
        }
        #[cfg(not(any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows)))]
        return Ok(
            json!({"supported":false,"reason":"이 플랫폼의 승인 복원은 아직 제공하지 않는다. 기존 파일과 기록을 보존한다."}),
        );
        #[cfg(any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows))]
        {
            // Linux: a place whose filesystem is not on the reviewed list gets no plan and no
            // approval capability. The answer names the reason; nothing was read into a plan.
            if let Err(reason) = self.recovery_place() {
                return Ok(json!({"supported":false,"reason":reason}));
            }
            let plan = self.recovery_plan()?;
            // Shown beside the plan, never inside the digest or the stored approval.
            let mut view = plan.view;
            view["approval_mode"] = self.recovery_approval_view();
            Ok(view)
        }
    }

    /// Current approval mode of this Project (spec "승인 모드"): manual unless a valid automatic
    /// setting is readable in an opt-in build. Reading never writes or starts a recovery.
    pub(crate) fn recovery_approval_view(&self) -> Value {
        crate::recovery_mode::read(&self.state_path).view()
    }

    pub(crate) fn approve_revisit(&mut self, digest: &str) -> Result<Value, String> {
        self.approve_revisit_from(digest, ApprovalSource::MonitorButton)
    }

    /// The same approval with the channel it came through.
    #[cfg_attr(not(any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows)), allow(unused_variables))]
    pub(crate) fn approve_revisit_from(&mut self, digest: &str, source: ApprovalSource) -> Result<Value, String> {
        #[cfg(feature = "tracking-policy")]
        if self.project.tracking().is_some() {
            return Err(crate::locale::text("session.tracking_unsupported.revisit").into());
        }
        #[cfg(not(any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows)))]
        {
            let _ = digest;
            Err("이 플랫폼의 승인 복원은 아직 제공하지 않는다".into())
        }
        #[cfg(any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows))]
        {
            self.recovery_place()?;
            let plan = self.recovery_plan()?;
            if plan.view["digest"].as_str() != Some(digest) {
                return Err("복원 계획이 바뀌었다. 변경 목록을 다시 확인하고 승인한다".into());
            }
            self.apply_recovery(plan, source)
        }
    }

    /// A model/CLI call may only resume a durable approval, never create one — unless this
    /// Project carries a valid automatic setting (opt-in build): then the Core itself approves a
    /// freshly checked plan and records that setting as the source. A pending recovery is always
    /// resumed from its stored approval, whatever the setting says now.
    pub(crate) fn resume_revisit(&mut self) -> Result<Value, String> {
        #[cfg(feature = "tracking-policy")]
        if self.project.tracking().is_some() {
            return Err(crate::locale::text("session.tracking_unsupported.revisit").into());
        }
        #[cfg(not(any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows)))]
        return Err(
            "이 플랫폼의 승인 복원은 아직 제공하지 않는다. 기존 파일과 기록을 보존한다".into(),
        );
        #[cfg(any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows))]
        {
            self.recovery_place()?;
            if self.project.cycles().pending_revisit().is_none() {
                #[cfg(feature = "recovery-auto-approval")]
                if let Some(setting) = crate::recovery_mode::read(&self.state_path).automatic {
                    return self.automatic_revisit(setting);
                }
                return Err("Monitor의 되돌아가기 변경 목록을 확인하고 ‘보존 후 되돌아가기’를 눌러야 한다. 파일과 기록은 바꾸지 않았다".into());
            }
            let plan = self.recovery_plan()?;
            let dir = archive(self, &plan.key, false).map_err(error)?;
            verify_approval(&dir, &plan)?;
            self.apply_recovery(plan, ApprovalSource::Resumed)
        }
    }

    /// Automatic approval: the same plan and refusals as the Monitor path; the setting that
    /// stood in for the click is re-read right before the approval record is written and must
    /// be byte-identical. No token exists on this path and none is minted.
    #[cfg(all(feature = "recovery-auto-approval", any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows)))]
    fn automatic_revisit(
        &mut self,
        setting: crate::recovery_mode::AutomaticSetting,
    ) -> Result<Value, String> {
        let plan = self.recovery_plan()?;
        if crate::recovery_mode::read(&self.state_path).automatic.as_ref() != Some(&setting) {
            return Err("자동 복구 설정이 바뀌어 승인하지 않았다. 파일과 기록은 바꾸지 않았다. 설정을 확인한 뒤 다시 요청한다".into());
        }
        self.apply_recovery(plan, ApprovalSource::AutomaticSetting(setting))
    }
}

/// Who stood in for the approval of one recovery. Recorded once, beside the plan; a resumption
/// follows the stored approval and never rewrites it.
#[cfg_attr(not(any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows)), allow(dead_code))]
pub(crate) enum ApprovalSource {
    /// The user's Monitor button through the App-only `gil_revisit_apply`.
    MonitorButton,
    /// The runner of an unattended session, through the tool the model is not given.
    UnattendedRunner,
    /// A valid per-Project automatic setting of this exact edition (opt-in build).
    #[cfg(feature = "recovery-auto-approval")]
    AutomaticSetting(crate::recovery_mode::AutomaticSetting),
    /// `gil_revisit` continuing a durable approval; creates no approval and no source record.
    Resumed,
}

#[cfg(any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows))]
impl ApprovalSource {
    /// Deterministic bytes for `source.json`. No timestamp, token, person or path inside.
    fn record(&self, plan: &Recovery) -> Option<Value> {
        let base = |source: &str| {
            json!({"schema_version":1,"approval_source":source,
                "pending_state_sha256":plan.key,"digest":plan.view["digest"],
                "from":plan.view["from"],"target":plan.view["target"],
                "human_click":source == "monitor_button",
                "notice":"승인 출처 기록이다. 복구 계획은 approval.json, 원본은 files.json이 가리킨다. 사람의 신원 인증을 뜻하지 않는다."})
        };
        match self {
            ApprovalSource::MonitorButton => Some(base(crate::approval_source::MONITOR)),
            ApprovalSource::UnattendedRunner => Some(base(crate::approval_source::UNATTENDED)),
            #[cfg(feature = "recovery-auto-approval")]
            ApprovalSource::AutomaticSetting(setting) => {
                let mut v = base("automatic_setting");
                v["setting"] = json!({"file":crate::recovery_mode::SETTING_FILE,
                    "schema_version":crate::recovery_mode::SETTING_SCHEMA,
                    "revision":setting.revision,"sha256":setting.sha256});
                Some(v)
            }
            ApprovalSource::Resumed => None,
        }
    }
}

#[cfg(any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows))]
use crate::{
    artifact::{IgnoreSource, PolicyVersion, TrackingPolicy},
    restore::preview,
};
#[cfg(any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows))]
use std::{
    ffi::CString,
    fs::File,
    io::{Read, Write},
};

#[cfg(any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows))]
struct Recovery {
    next: Project,
    before: Manifest,
    target: Manifest,
    world: SnapshotRef,
    key: String,
    view: Value,
}

#[cfg(any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows))]
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

#[cfg(any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows))]
impl ProjectSession {
    /// Where the approved recovery may run at all. macOS and Windows keep their existing checks
    /// (the visual creation's local-storage rule, the Windows recovery check inside the plan).
    /// Linux has no local-mount flag, so both the working folder and the record folder must sit on
    /// one of the filesystems the image-report creation itself accepts — the ones whose in-folder
    /// rename, directory `fsync` and no-follow opens the preservation and the restore rely on.
    /// Read-only: it opens the two folders without following a link and asks their type.
    fn recovery_place(&self) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        {
            let unlisted = |what: &str, e: std::io::Error| format!(
                "{what}: {e}. 승인 복원은 검수 대상 로컬 파일 시스템(ext4·XFS·Btrfs·tmpfs·overlayfs)에서만 진행한다. 파일과 기록은 바꾸지 않았다");
            let root = self.root().map_err(error)?;
            let work = crate::internal_fs::dir_nofollow(&root).map_err(|e| unlisted("작업 폴더", e))?;
            crate::internal_fs::listed_local_filesystem(&work).map_err(|e| unlisted("작업 폴더", e))?;
            let gil = crate::internal_fs::dir_nofollow(&gil_dir(&self.state_path).map_err(error)?).map_err(|e| unlisted("기록 폴더", e))?;
            crate::internal_fs::listed_local_filesystem(&gil).map_err(|e| unlisted("기록 폴더", e))?;
        }
        Ok(())
    }

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
        #[cfg(windows)]
        restore::check_windows_recovery(&root, &gil_dir(&self.state_path).map_err(error)?)
            .map_err(error)?;
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
                && defaults.includes_file(&path.to_ascii_lowercase())?
                && old_policy.includes_file(path)?
                && target_policy.includes_file(path)?)
        };
        let inspection = preview::preview(
            &preview::Request {
                before: &observed,
                target: &target,
                before_policy: None,
                target_policy: None,
                os: recovery_os(),
            },
            |_| Ok(true),
            &preview::LstatProbe::at(&root),
        );
        if !matches!(inspection.probe(), preview::ProbeStatus::Checked { .. })
            || inspection
                .entries()
                .iter()
                .any(|e| e.blocked(recovery_os()))
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
            "archive":gil_dir(&self.state_path).map_err(error)?.canonicalize().map_err(error)?.join("revisit-preserved").join(&key).to_string_lossy(),"pending_state_sha256":key,
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

    fn apply_recovery(&mut self, plan: Recovery, source: ApprovalSource) -> Result<Value, String> {
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
        // Only the call that creates the approval records its source. A duplicate or resumed
        // call finds the approval already stored and leaves both records as they are.
        let fresh = match guarded::regular_at(&dir, c"approval.json") {
            Ok(_) => false,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
            Err(e) => return Err(error(e)),
        };
        write_once(
            &dir,
            c"approval.json",
            &serde_json::to_vec_pretty(&approval).map_err(error)?,
        )?;
        if fresh {
            match source.record(&plan) {
                Some(record) => write_once(
                    &dir,
                    c"source.json",
                    &serde_json::to_vec_pretty(&record).map_err(error)?,
                )?,
                None => return Err("저장된 승인이 없어 재개할 수 없다. 새 승인은 Monitor 또는 유효한 자동 설정에서만 만든다".into()),
            }
        }
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
        result["approval_source"] = json!(read_source(&dir));
        result["next"] = json!(
            "gil_open으로 새 Experiment 또는 Interview를 연다. 실패 계보와 승인 항목은 유지된다."
        );
        Ok(result)
    }
}

#[cfg(any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows))]
fn archive(s: &ProjectSession, key: &str, create: bool) -> std::io::Result<ArchiveDir> {
    let gil = guarded::directory(s.state_path.parent().unwrap(), false)?;
    fn child(
        parent: &ArchiveDir,
        name: &std::ffi::CStr,
        create: bool,
    ) -> std::io::Result<ArchiveDir> {
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
#[cfg(any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows))]
fn write_once(dir: &ArchiveDir, name: &std::ffi::CStr, bytes: &[u8]) -> Result<(), String> {
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
/// The stored approval source, or `unrecorded` for an approval saved before source records
/// existed or interrupted before its source was written. Never invents a click.
#[cfg(any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows))]
fn read_source(dir: &ArchiveDir) -> String {
    guarded::regular_at(dir, c"source.json")
        .ok()
        .and_then(|f| serde_json::from_reader::<_, Value>(f.take(64 * 1024)).ok())
        .and_then(|v| v["approval_source"].as_str().map(str::to_owned))
        .unwrap_or_else(|| "unrecorded".into())
}
#[cfg(any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows))]
fn verify_approval(dir: &ArchiveDir, plan: &Recovery) -> Result<(), String> {
    let f = guarded::regular_at(dir, c"approval.json").map_err(error)?;
    let got: Value = serde_json::from_reader(f.take(4 * 1024 * 1024)).map_err(error)?;
    let mut expected = plan.view.clone();
    expected["state"] = json!("approved");
    if got != expected {
        return Err("저장된 복원 승인이 현재 상태와 다르다".into());
    }
    Ok(())
}

#[cfg(any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows))]
fn publish(
    dir: &ArchiveDir,
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

// macOS and Linux share the descriptor-relative, no-follow file access (`internal_fs`): exclusive
// creation, in-folder rename and directory `fsync` are the same POSIX calls on both.
#[cfg(any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64")))]
use crate::internal_fs as guarded;
#[cfg(any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64")))]
type ArchiveDir = File;
#[cfg(windows)]
type ArchiveDir = crate::recovery_fs_windows::Dir;
#[cfg(any(target_os = "macos", all(target_os = "linux", target_arch = "x86_64"), windows))]
fn recovery_os() -> preview::TargetOs {
    if cfg!(windows) {
        preview::TargetOs::Windows
    } else if cfg!(target_os = "macos") {
        preview::TargetOs::MacOs
    } else {
        // Linux: names are compared as bytes (no case or normalization folding), so two paths
        // that differ only by case are two files and are previewed as such.
        preview::TargetOs::Unix
    }
}
#[cfg(windows)]
mod guarded {
    use super::ArchiveDir;
    use std::{ffi::CStr, fs::File, io, path::Path};
    fn text(n: &CStr) -> io::Result<&str> {
        n.to_str().map_err(io::Error::other)
    }
    pub(super) fn directory(p: &Path, _: bool) -> io::Result<ArchiveDir> {
        ArchiveDir::open(p)
    }
    pub(super) fn child_dir(d: &ArchiveDir, n: &CStr) -> io::Result<ArchiveDir> {
        d.child(text(n)?, false)
    }
    pub(super) fn create_dir_at(d: &ArchiveDir, n: &CStr, _: u32) -> io::Result<ArchiveDir> {
        d.private_child(text(n)?)
    }
    pub(super) fn regular_at(d: &ArchiveDir, n: &CStr) -> io::Result<File> {
        d.read(text(n)?)
    }
    pub(super) fn create_file_at(d: &ArchiveDir, n: &CStr, _: u32) -> io::Result<File> {
        d.create(text(n)?)
    }
    pub(super) fn rename_at(d: &ArchiveDir, from: &CStr, to: &CStr) -> io::Result<()> {
        d.rename_file(text(from)?, text(to)?)
    }
    pub(super) fn fsync(d: &ArchiveDir) -> io::Result<()> {
        d.sync()
    }
}
