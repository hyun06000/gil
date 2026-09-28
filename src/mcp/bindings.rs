//! Local, durable Monitor consent. One immutable file per explicitly prepared scope.
//! Never stored in the Project, plugin cache, tool result, or iframe storage.
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use super::monitor::Refusal;

const SCHEMA: u32 = 1;
const MAX_BYTES: u64 = 16_384;
static SERIAL: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Identity { device: u64, inode: u64, created_ns: Option<u128> }

impl Identity {
    fn at(root: &Path) -> Result<Self, Refusal> {
        let meta = fs::metadata(root.join(".gil")).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => Refusal::new("project_missing", "선택했던 Project 폴더를 찾을 수 없다"),
            _ => Refusal::new("unreadable", "선택했던 Project 를 확인할 수 없다"),
        })?;
        if !meta.is_dir() { return Err(moved()); }
        // state.yaml is atomically replaced on every commit. Identify the stable .gil
        // directory instead. A copied/reinitialized Project at the same path is not consent.
        #[cfg(unix)] {
            use std::os::unix::fs::MetadataExt;
            Ok(Self { device: meta.dev(), inode: meta.ino(),
                created_ns: meta.created().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_nanos()) })
        }
        #[cfg(not(unix))] {
            // Do not substitute a timestamp/label for an OS identity. Windows identity
            // and installation acceptance remain a separate distribution checkpoint.
            Err(Refusal::new("resume_unavailable", "이 환경의 안전한 Project 자동 복원은 아직 지원하지 않는다 — Companion 을 사용한다"))
        }
    }
}

fn moved() -> Refusal { Refusal::new("project_moved", "선택했던 자리의 Project 가 바뀌었다 — 폴더를 다시 선택한다") }
fn damaged() -> Refusal { Refusal::new("settings_damaged", "Monitor 연결 설정을 검증하지 못했다 — 기존 설정은 보존했다") }
fn unreadable() -> Refusal { Refusal::new("settings_unreadable", "Monitor 연결 설정을 읽지 못했다") }
fn unwritable() -> Refusal { Refusal::new("settings_unwritable", "Monitor 연결 설정을 저장하지 못했다 — 자동 복원이 준비되지 않았다") }
pub(super) fn unknown() -> Refusal { Refusal::new("reconnect_required", "저장된 Project 연결이 없다 — 이 Project 의 Monitor 를 한 번 열어 주세요") }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Binding { schema_version: u32, pub root: PathBuf, identity: Identity }

impl Binding {
    pub fn capture(root: PathBuf) -> Result<Self, Refusal> {
        let identity = Identity::at(&root)?;
        Ok(Self { schema_version: SCHEMA, root, identity })
    }
    pub fn scope(&self) -> String {
        // Identity participates in the opaque address: explicitly selecting a replacement
        // Project must never rebind already-open old App instances to the replacement.
        let mut hash = Sha256::new();
        hash.update(b"gil-monitor-binding-v1\0");
        hash.update(self.root.as_os_str().as_encoded_bytes());
        hash.update([0]);
        hash.update(self.identity.device.to_le_bytes()); hash.update(self.identity.inode.to_le_bytes());
        hash.update(format!("{:?}", self.identity.created_ns));
        format!("project:{:x}", hash.finalize())
    }
    pub fn validate(&self) -> Result<(), Refusal> {
        let root = self.root.canonicalize().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => Refusal::new("project_missing", "선택했던 Project 폴더를 찾을 수 없다"),
            _ => Refusal::new("unreadable", "선택했던 Project 를 확인할 수 없다"),
        })?;
        if root != self.root || Identity::at(&root)? != self.identity { return Err(moved()); }
        Ok(())
    }
    pub fn label(&self) -> String { self.root.file_name().and_then(|s| s.to_str()).unwrap_or("GIL Project").to_owned() }
}

#[derive(Debug)]
pub(super) struct Store { dir: Option<PathBuf> }

impl Default for Store {
    fn default() -> Self {
        let dir = if let Some(at) = std::env::var_os("GIL_MONITOR_STATE_DIR") { Some(PathBuf::from(at)) }
        else if cfg!(target_os = "macos") {
            std::env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Application Support/GIL/monitor-bindings-v1"))
        } else if cfg!(windows) {
            std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("GIL/monitor-bindings-v1"))
        } else {
            std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))
                .map(|p| p.join("gil/monitor-bindings-v1"))
        };
        Self { dir: dir.filter(|p| p.is_absolute()) }
    }
}

impl Store {
    #[cfg(test)]
    pub fn at(dir: PathBuf) -> Self { Self { dir: Some(dir) } }
    fn path(&self, scope: &str) -> Result<PathBuf, Refusal> {
        let hex = scope.strip_prefix("project:").filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
            .ok_or_else(unknown)?;
        Ok(self.dir.as_ref().ok_or_else(unreadable)?.join(format!("{hex}.json")))
    }
    fn private(meta: &fs::Metadata) -> bool {
        #[cfg(unix)] { use std::os::unix::fs::MetadataExt;
            meta.uid() == unsafe { libc::geteuid() } && meta.mode() & 0o077 == 0
        }
        #[cfg(not(unix))] { let _ = meta; false }
    }
    fn check_dir(&self) -> Result<(), Refusal> {
        let meta = fs::symlink_metadata(self.dir.as_ref().ok_or_else(unreadable)?).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound { unknown() } else { unreadable() }
        })?;
        if !meta.is_dir() || !Self::private(&meta) { return Err(unreadable()); }
        Ok(())
    }
    pub fn load(&self, scope: &str) -> Result<Binding, Refusal> {
        let path = self.path(scope)?;
        self.check_dir()?;
        let mut options = OpenOptions::new(); options.read(true);
        #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.custom_flags(libc::O_NOFOLLOW); }
        let file = options.open(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound { unknown() } else { unreadable() }
        })?;
        let meta = file.metadata().map_err(|_| unreadable())?;
        if !meta.is_file() || !Self::private(&meta) || meta.len() > MAX_BYTES { return Err(damaged()); }
        let mut bytes = Vec::new(); file.take(MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|_| unreadable())?;
        if bytes.len() as u64 > MAX_BYTES { return Err(damaged()); }
        let peek: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| damaged())?;
        if peek.get("schema_version").and_then(|v| v.as_u64()) != Some(u64::from(SCHEMA)) {
            return Err(Refusal::new("settings_unsupported", "Monitor 연결 설정의 판을 지원하지 않는다 — 기존 설정은 보존했다"));
        }
        let binding: Binding = serde_json::from_slice(&bytes).map_err(|_| damaged())?;
        if !binding.root.is_absolute() || binding.scope() != scope { return Err(damaged()); }
        Ok(binding)
    }
    pub fn save(&self, binding: &Binding) -> Result<(), Refusal> {
        let scope = binding.scope();
        let path = self.path(&scope)?;
        let dir = self.dir.as_ref().ok_or_else(unwritable)?;
        // Resolve existing ancestors BEFORE mkdir, including symlinked parents. Even a
        // mistaken developer override must not create a settings directory in the Project.
        if dir.components().any(|c| matches!(c, std::path::Component::ParentDir | std::path::Component::CurDir)) { return Err(unwritable()); }
        let mut parent = dir.as_path();
        let mut suffix = Vec::new();
        while !parent.try_exists().map_err(|_| unwritable())? {
            suffix.push(parent.file_name().ok_or_else(unwritable)?.to_owned());
            parent = parent.parent().ok_or_else(unwritable)?;
        }
        let mut resolved = parent.canonicalize().map_err(|_| unwritable())?;
        for part in suffix.into_iter().rev() { resolved.push(part); }
        if resolved.starts_with(&binding.root) { return Err(unwritable()); }
        let mut builder = fs::DirBuilder::new(); builder.recursive(true);
        #[cfg(unix)] { use std::os::unix::fs::DirBuilderExt; builder.mode(0o700); }
        builder.create(dir).map_err(|_| unwritable())?;
        self.check_dir()?;
        // Recheck after creation in case an ancestor changed concurrently.
        if dir.canonicalize().map_err(|_| unwritable())?.starts_with(&binding.root) { return Err(unwritable()); }
        if path.try_exists().map_err(|_| unreadable())? {
            return if self.load(&scope)? == *binding { Ok(()) } else { Err(damaged()) };
        }
        let bytes = serde_json::to_vec(binding).map_err(|_| unwritable())?;
        if bytes.len() as u64 > MAX_BYTES { return Err(unwritable()); }
        let tmp = dir.join(format!(".binding-{}-{}.tmp", std::process::id(), SERIAL.fetch_add(1, Ordering::Relaxed)));
        let mut options = OpenOptions::new(); options.write(true).create_new(true);
        #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
        let mut file = options.open(&tmp).map_err(|_| unwritable())?;
        let result = (|| {
            file.write_all(&bytes).and_then(|_| file.sync_all()).map_err(|_| unwritable())?;
            // Publish atomically without overwriting a concurrent writer or damaged record.
            // Per-scope files mean parallel Codex/Cowork connections cannot lose each other's entries.
            match fs::hard_link(&tmp, &path) {
                Ok(()) => (),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    if self.load(&scope)? != *binding { return Err(damaged()); }
                },
                Err(_) => return Err(unwritable()),
            }
            File::open(dir).and_then(|f| f.sync_all()).map_err(|_| unwritable())
        })();
        drop(file);
        let _ = fs::remove_file(&tmp); // only the unique file created by this call
        result
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("gil-binding-{}-{}", std::process::id(), SERIAL.fetch_add(1, Ordering::Relaxed)));
            fs::create_dir(&dir).unwrap(); Self(dir)
        }
        fn project(&self, name: &str) -> Binding {
            let root = self.0.join(name); fs::create_dir_all(root.join(".gil")).unwrap();
            Binding::capture(root.canonicalize().unwrap()).unwrap()
        }
        fn store(&self) -> Store { Store::at(self.0.join("settings")) }
    }
    impl Drop for Scratch { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }

    #[test]
    fn a_fresh_store_restores_only_the_requested_binding_with_private_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let s = Scratch::new(); let a = s.project("a/same-name"); let b = s.project("b/same-name");
        s.store().save(&a).unwrap(); s.store().save(&b).unwrap();
        assert_ne!(a.scope(), b.scope());
        let restored = s.store().load(&a.scope()).unwrap(); assert_eq!(restored, a);
        restored.validate().unwrap();
        assert_eq!(fs::metadata(s.store().dir.unwrap()).unwrap().permissions().mode() & 0o777, 0o700);
        assert_eq!(fs::metadata(s.store().path(&a.scope()).unwrap()).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(fs::read_dir(s.0.join("settings")).unwrap().count(), 2);
    }

    #[test]
    fn missing_or_replaced_project_never_rebinds_an_old_view() {
        let s = Scratch::new(); let original = s.project("project"); s.store().save(&original).unwrap();
        fs::rename(original.root.join(".gil"), s.0.join("old-records")).unwrap();
        assert_eq!(original.validate().unwrap_err().code, "project_missing");
        let replacement = s.project("project"); s.store().save(&replacement).unwrap();
        assert_ne!(original.scope(), replacement.scope());
        assert_eq!(s.store().load(&original.scope()).unwrap().validate().unwrap_err().code, "project_moved");
        s.store().load(&replacement.scope()).unwrap().validate().unwrap();
    }

    #[test]
    fn normal_state_file_replacement_does_not_change_the_project_identity() {
        let s = Scratch::new(); let binding = s.project("project");
        fs::write(binding.root.join(".gil/state.yaml"), "first").unwrap();
        fs::write(binding.root.join(".gil/next.yaml"), "second").unwrap();
        fs::rename(binding.root.join(".gil/next.yaml"), binding.root.join(".gil/state.yaml")).unwrap();
        binding.validate().unwrap(); assert_eq!(binding, Binding::capture(binding.root.clone()).unwrap());
    }

    #[test]
    fn corrupt_or_future_settings_are_preserved_not_overwritten() {
        let s = Scratch::new(); let binding = s.project("project"); s.store().save(&binding).unwrap();
        let path = s.store().path(&binding.scope()).unwrap();
        for (bytes, code) in [("{broken", "settings_damaged"), ("{\"schema_version\":99}", "settings_unsupported")] {
            fs::write(&path, bytes).unwrap();
            assert_eq!(s.store().load(&binding.scope()).unwrap_err().code, code);
            assert_eq!(s.store().save(&binding).unwrap_err().code, code);
            assert_eq!(fs::read_to_string(&path).unwrap(), bytes);
        }
    }

    #[test]
    fn scope_path_traversal_and_mismatched_binding_are_refused() {
        let s = Scratch::new(); let binding = s.project("project"); s.store().save(&binding).unwrap();
        for scope in ["project:../../private", "project:unknown", "", "project:/etc/passwd"] {
            assert_eq!(s.store().load(scope).unwrap_err().code, "reconnect_required");
        }
        let path = s.store().path(&binding.scope()).unwrap();
        let mut altered = binding.clone(); altered.root = s.0.join("other");
        fs::write(path, serde_json::to_vec(&altered).unwrap()).unwrap();
        assert_eq!(s.store().load(&binding.scope()).unwrap_err().code, "settings_damaged");
    }

    #[test]
    fn a_settings_override_inside_the_project_creates_nothing() {
        let s = Scratch::new(); let binding = s.project("project");
        let dir = binding.root.join("new/settings");
        assert_eq!(Store::at(dir.clone()).save(&binding).unwrap_err().code, "settings_unwritable");
        assert!(!binding.root.join("new").exists());
        std::os::unix::fs::symlink(&binding.root, s.0.join("alias")).unwrap();
        assert_eq!(Store::at(s.0.join("alias/other/settings")).save(&binding).unwrap_err().code, "settings_unwritable");
        assert!(!binding.root.join("other").exists());
    }

    #[test]
    fn concurrent_connections_publish_complete_records_without_lost_updates() {
        let s = Scratch::new(); let a = s.project("a"); let b = s.project("b");
        std::thread::scope(|threads| {
            for binding in [&a, &a, &b, &b] {
                threads.spawn(|| s.store().save(binding).unwrap());
            }
        });
        assert_eq!(s.store().load(&a.scope()).unwrap(), a);
        assert_eq!(s.store().load(&b.scope()).unwrap(), b);
        assert_eq!(fs::read_dir(s.0.join("settings")).unwrap().count(), 2);
    }

    #[test]
    fn symlinks_and_nonprivate_records_are_not_trusted() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let s = Scratch::new(); let binding = s.project("project"); s.store().save(&binding).unwrap();
        let path = s.store().path(&binding.scope()).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(s.store().load(&binding.scope()).unwrap_err().code, "settings_damaged");
        let held = s.0.join("held.json"); fs::rename(&path, &held).unwrap(); symlink(&held, &path).unwrap();
        assert_eq!(s.store().load(&binding.scope()).unwrap_err().code, "settings_unreadable");
    }
}
