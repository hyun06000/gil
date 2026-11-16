//! M4-R approval mode for the recovery of a closed, failed Experiment Cycle.
//!
//! The recovery contract (spec/GIL_Revisit_Recovery_v0.1.md, "승인 모드") decided on 2026-11-15:
//! manual approval is the default; a user may allow automatic approval for one Project only;
//! a missing, unreadable or invalid setting is manual; reading never changes a setting or starts
//! a recovery; status surfaces show the current mode.
//!
//! What this module does **not** decide: the human write path of the setting. The contract
//! says that path and the storage form are defined before implementation. Until the write path
//! exists, no product profile enables `recovery-auto-approval`; without that feature the Core
//! never reads the setting file and reports `manual` with basis `unavailable`.
//! See spec/proposals/M4R_Recovery_Approval_Mode_Foundation_v0.1.md.
//!
//! Provisional storage: `<record .gil>/recovery-approval.json` beside `state.yaml`, like the
//! locale sidecar. It is not part of the record format and the format number does not change.
// Without the opt-in feature only the `unavailable` projection is reachable; the parser stays
// compiled and unit-tested so the setting form cannot drift unnoticed.
#![cfg_attr(not(feature = "recovery-auto-approval"), allow(dead_code))]
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Sidecar file name beside the Project record.
pub(crate) const SETTING_FILE: &str = "recovery-approval.json";
pub(crate) const SETTING_SCHEMA: u32 = 1;
/// Bound of the sidecar bytes; a larger file is unreadable, never partially trusted.
const SETTING_LIMIT: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingFile {
    schema_version: u32,
    /// `manual` or `automatic`. Anything else is unreadable.
    mode: String,
    /// The setting's edition. Every change is expected to raise it; an approval record names it.
    revision: u32,
    /// Free text a future write path may keep (who/why). Never interpreted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

/// The exact edition of an automatic setting an approval is bound to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AutomaticSetting {
    pub(crate) revision: u32,
    /// SHA-256 of the sidecar bytes that were read. A re-read must match before approving.
    pub(crate) sha256: String,
}

/// Why the Project is in its current mode. `automatic` is `Some` only for a valid automatic setting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApprovalMode {
    pub(crate) automatic: Option<AutomaticSetting>,
    /// `default` (no setting), `setting` (a valid setting file), `unreadable` (a file exists but
    /// is not a valid setting; treated as manual) or `unavailable` (this build never reads one).
    pub(crate) basis: &'static str,
    pub(crate) revision: Option<u32>,
    pub(crate) detail: Option<String>,
}

impl ApprovalMode {
    pub(crate) fn mode(&self) -> &'static str {
        if self.automatic.is_some() { "automatic" } else { "manual" }
    }
    /// Status projection for Monitor and tool responses. Contains no token or path.
    pub(crate) fn view(&self) -> Value {
        let mut v = json!({"mode": self.mode(), "basis": self.basis});
        if let Some(r) = self.revision { v["revision"] = json!(r); }
        if let Some(d) = &self.detail { v["detail"] = json!(d); }
        v
    }
    fn manual(basis: &'static str, detail: Option<String>) -> Self {
        Self { automatic: None, basis, revision: None, detail }
    }
}

/// Parse sidecar bytes. Pure; the caller decides how the bytes were obtained.
fn parse(bytes: &[u8]) -> Result<SettingFile, &'static str> {
    if bytes.len() > SETTING_LIMIT { return Err("설정 파일이 너무 크다"); }
    let setting: SettingFile = serde_json::from_slice(bytes).map_err(|_| "설정 파일을 읽을 수 없다")?;
    if setting.schema_version != SETTING_SCHEMA { return Err("알 수 없는 설정 판"); }
    if setting.revision == 0 { return Err("설정 revision은 1 이상이다"); }
    if setting.mode != "manual" && setting.mode != "automatic" { return Err("알 수 없는 승인 모드"); }
    Ok(setting)
}

fn classify(bytes: &[u8]) -> ApprovalMode {
    match parse(bytes) {
        Ok(setting) => ApprovalMode {
            automatic: (setting.mode == "automatic").then(|| AutomaticSetting {
                revision: setting.revision,
                sha256: format!("{:x}", <sha2::Sha256 as sha2::Digest>::digest(bytes)),
            }),
            basis: "setting",
            revision: Some(setting.revision),
            detail: None,
        },
        Err(why) => ApprovalMode::manual("unreadable", Some(format!("{why}. 수동 승인으로 처리한다"))),
    }
}

/// The current mode of the Project whose record is `state_path`. Never writes, repairs or deletes.
#[cfg(feature = "recovery-auto-approval")]
pub(crate) fn read(state_path: &std::path::Path) -> ApprovalMode {
    use std::io::Read;
    let Some(dir) = state_path.parent() else { return ApprovalMode::manual("default", None) };
    let path = dir.join(SETTING_FILE);
    // Single regular, unlinked control file below the record's `.gil`; links are refused, not followed.
    #[cfg(unix)]
    let opened = crate::internal_fs::file(&path, false).map(|(_, file)| file);
    #[cfg(windows)]
    let opened = crate::windows_fs::control_file(&path, false).map(|(_, file)| file);
    #[cfg(not(any(unix, windows)))]
    let opened: std::io::Result<std::fs::File> = Err(std::io::Error::other("unsupported"));
    let file = match opened {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return ApprovalMode::manual("default", None),
        Err(_) => return ApprovalMode::manual("unreadable", Some("설정 파일을 안전하게 열 수 없다. 수동 승인으로 처리한다".into())),
    };
    let mut bytes = Vec::new();
    if file.take(SETTING_LIMIT as u64 + 1).read_to_end(&mut bytes).is_err() {
        return ApprovalMode::manual("unreadable", Some("설정 파일을 읽지 못했다. 수동 승인으로 처리한다".into()));
    }
    classify(&bytes)
}

/// Builds without the opt-in feature never consult a setting: manual, basis `unavailable`.
#[cfg(not(feature = "recovery-auto-approval"))]
pub(crate) fn read(_state_path: &std::path::Path) -> ApprovalMode {
    ApprovalMode::manual("unavailable", Some("이 빌드는 자동 승인 설정을 읽지 않는다. Monitor 승인만 복구를 실행한다".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(v: Value) -> Vec<u8> { serde_json::to_vec(&v).unwrap() }

    #[test]
    fn only_an_exact_valid_setting_turns_automatic_on() {
        let auto = classify(&bytes(json!({"schema_version":1,"mode":"automatic","revision":3})));
        assert_eq!(auto.mode(), "automatic");
        assert_eq!(auto.basis, "setting");
        assert_eq!(auto.automatic.as_ref().unwrap().revision, 3);
        assert_eq!(auto.automatic.as_ref().unwrap().sha256.len(), 64);
        let manual = classify(&bytes(json!({"schema_version":1,"mode":"manual","revision":1,"note":"사용자 선택"})));
        assert_eq!(manual.mode(), "manual");
        assert_eq!(manual.basis, "setting");
        assert_eq!(manual.revision, Some(1));
        assert!(manual.automatic.is_none());
    }

    #[test]
    fn anything_else_is_manual_and_says_why() {
        for bad in [
            bytes(json!({"schema_version":2,"mode":"automatic","revision":1})),
            bytes(json!({"schema_version":1,"mode":"auto","revision":1})),
            bytes(json!({"schema_version":1,"mode":"automatic","revision":0})),
            bytes(json!({"schema_version":1,"mode":"automatic","revision":1,"approved":"yes"})),
            bytes(json!({"mode":"automatic"})),
            bytes(json!("automatic")),
            b"not json".to_vec(),
            vec![b' '; SETTING_LIMIT + 1],
        ] {
            let mode = classify(&bad);
            assert_eq!(mode.mode(), "manual", "{}", String::from_utf8_lossy(&bad[..bad.len().min(80)]));
            assert_eq!(mode.basis, "unreadable");
            assert!(mode.automatic.is_none());
            assert!(mode.detail.as_deref().is_some_and(|d| d.contains("수동 승인")));
        }
    }

    #[test]
    fn the_view_names_mode_and_basis_only() {
        let v = classify(&bytes(json!({"schema_version":1,"mode":"automatic","revision":2}))).view();
        assert_eq!(v, json!({"mode":"automatic","basis":"setting","revision":2}));
        let v = ApprovalMode::manual("default", None).view();
        assert_eq!(v, json!({"mode":"manual","basis":"default"}));
    }

    #[cfg(all(feature = "recovery-auto-approval", any(unix, windows)))]
    #[test]
    fn reading_never_writes_and_a_missing_or_linked_file_is_manual() {
        let root = std::env::temp_dir().join(format!("gil-recovery-mode-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".gil")).unwrap();
        let state = root.join(".gil").join("state.yaml");
        std::fs::write(&state, b"format: 5\n").unwrap();
        assert_eq!(read(&state).basis, "default");
        let setting = root.join(".gil").join(SETTING_FILE);
        std::fs::write(&setting, bytes(json!({"schema_version":1,"mode":"automatic","revision":1}))).unwrap();
        let before = std::fs::read(&setting).unwrap();
        assert_eq!(read(&state).mode(), "automatic");
        assert_eq!(std::fs::read(&setting).unwrap(), before, "reading rewrote the setting");
        std::fs::write(&setting, b"{").unwrap();
        assert_eq!(read(&state).basis, "unreadable");
        std::fs::remove_file(&setting).unwrap();
        let valid = root.join("elsewhere.json");
        std::fs::write(&valid, bytes(json!({"schema_version":1,"mode":"automatic","revision":1}))).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&valid, &setting).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&valid, &setting).unwrap();
        let linked = read(&state);
        assert_eq!(linked.mode(), "manual", "a link was followed into automatic mode");
        assert_eq!(linked.basis, "unreadable");
        assert!(std::fs::symlink_metadata(&setting).unwrap().file_type().is_symlink());
        let _ = std::fs::remove_dir_all(&root);
    }
}
