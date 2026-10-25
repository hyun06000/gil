//! Opt-in testbed evidence only. Reads existing handles; never changes permissions or verdicts.
//! Samples are sequential, not an atomic snapshot or attribution of the changing actor.
use serde_json::{json, Value};
use std::{cell::RefCell, fs::File, rc::{Rc, Weak}};
#[cfg(windows)]
use crate::runtime_observe::platform::StampSize as _;

thread_local! { static ACTIVE: RefCell<Weak<RefCell<Vec<Value>>>> = RefCell::new(Weak::new()); }
#[cfg(test)]
thread_local! { static FAIL_READ: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
pub struct Session { records: Rc<RefCell<Vec<Value>>>, previous: Weak<RefCell<Vec<Value>>> }
impl Session {
    pub fn new() -> Self {
        let records = Rc::new(RefCell::new(Vec::new()));
        let previous = ACTIVE.with(|a| a.replace(Rc::downgrade(&records)));
        Self { records, previous }
    }
    pub fn snapshot(&self) -> Vec<Value> { self.records.borrow().clone() }
}
impl Drop for Session {
    fn drop(&mut self) { ACTIVE.with(|a| { a.replace(self.previous.clone()); }); }
}
fn handle_snapshot(file: &File) -> Value {
    let identity = crate::runtime_observe::platform::stamp_file(file)
        .map(|s| { let id = s.object_id(); json!({"state":"observed", "volume_or_device":id.0.to_string(), "file_id_or_inode":id.1.to_string(), "stamp":format!("{s:?}")}) })
        .unwrap_or_else(|e| json!({"state":"error", "error":e.to_string(), "os_error":e.raw_os_error()}));
    let protection = read_security(file).unwrap_or_else(|e| json!({"state":"error", "error":e.to_string(), "os_error":e.raw_os_error()}));
    json!({"identity":identity, "protection":protection})
}
/// Reopen only for this sample, using the observer's no-link walk. Do not retain a handle
/// across Host operations: that would change Windows rename/sharing behaviour under test.
pub fn root_snapshot(root: &std::path::Path) -> Value {
    let opened = if !root.is_absolute() || root.components().any(|c| matches!(c, std::path::Component::ParentDir | std::path::Component::CurDir)) {
        Err(std::io::Error::other("diagnostic root must be absolute without dot components"))
    } else { crate::runtime_observe::platform::walk(root) };
    let mut v = match opened {
        Ok(Some(dir)) => { let mut v = handle_snapshot(dir.as_file()); v["state"] = json!("opened"); v },
        Ok(None) => json!({"state":"absent"}),
        Err(e) => json!({"state":"error", "error":e.to_string(), "os_error":e.raw_os_error()}),
    };
    v["basis"] = json!("path_reopened_no_link_walk; handle closed before next operation");
    v["unix_ns"] = json!(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_nanos().to_string()));
    v
}
pub(crate) fn sample(stage: &str, object: &str, file: &File) {
    ACTIVE.with(|a| {
        let Some(records) = a.borrow().upgrade() else { return };
        let snapshot = handle_snapshot(file);
        let mut records = records.borrow_mut();
        let seq = records.len();
        records.push(json!({"sequence":seq, "unix_ns":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_nanos().to_string()),
            "stage":stage, "object":object, "basis":"held_handle", "identity":snapshot["identity"], "protection":snapshot["protection"]}));
    });
}
fn read_security(file: &File) -> std::io::Result<Value> {
    #[cfg(test)]
    if FAIL_READ.with(|f| f.get()) { return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "injected diagnostic read denial")); }
    security(file)
}

#[cfg(unix)]
fn security(file: &File) -> std::io::Result<Value> {
    use std::os::unix::fs::MetadataExt;
    let m = file.metadata()?;
    Ok(json!({"state":"observed", "uid":m.uid(), "gid":m.gid(), "mode":m.mode(), "acl":"not_collected_on_unix"}))
}

#[cfg(windows)]
fn security(file: &File) -> std::io::Result<Value> {
    use std::{os::windows::io::AsRawHandle, ptr::null_mut};
    use windows_sys::Win32::{Foundation::LocalFree, Security::{*, Authorization::*}};
    struct Allocation(*mut std::ffi::c_void);
    impl Drop for Allocation { fn drop(&mut self) { unsafe { LocalFree(self.0); } } }
    unsafe {
        let (mut owner, mut acl, mut sd) = (null_mut(), null_mut(), null_mut());
        let flags = OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION;
        let code = GetSecurityInfo(file.as_raw_handle(), SE_FILE_OBJECT, flags, &mut owner, null_mut(), &mut acl, null_mut(), &mut sd);
        if code != 0 { return Err(std::io::Error::from_raw_os_error(code as i32)); }
        let _sd = Allocation(sd);
        let (mut control, mut revision) = (0, 0);
        if GetSecurityDescriptorControl(sd, &mut control, &mut revision) == 0 { return Err(std::io::Error::last_os_error()); }
        let mut text = null_mut();
        if ConvertSecurityDescriptorToStringSecurityDescriptorW(sd, 1, flags, &mut text, null_mut()) == 0 { return Err(std::io::Error::last_os_error()); }
        let _text = Allocation(text.cast());
        let mut n = 0; while *text.add(n) != 0 { n += 1; }
        let sddl = String::from_utf16_lossy(std::slice::from_raw_parts(text, n));
        Ok(json!({"state":"observed", "owner_sid":if owner.is_null() { None } else { Some(crate::windows_fs::sid_text(owner)?) },
            "control":control, "dacl_protected": control & SE_DACL_PROTECTED != 0,
            "ace_count":if acl.is_null() { None } else { Some((*acl).AceCount) },
            "sddl":sddl, "sddl_fields":"owner and DACL; ACE type, flags, rights and trustee; no SACL requested"}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn root_samples_distinguish_absence_errors_and_replaced_objects() {
        let base = std::fs::canonicalize(std::env::temp_dir()).unwrap().join(format!("gil-root-sample-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&base).unwrap();
        let root = base.join("root");
        assert_eq!(root_snapshot(&root)["state"], "absent");
        assert_eq!(root_snapshot(std::path::Path::new("relative"))["state"], "error");
        std::fs::create_dir(&root).unwrap();
        let before = root_snapshot(&root);
        // No diagnostic handle survives this call, including on Windows.
        std::fs::rename(&root, base.join("old")).unwrap();
        std::fs::create_dir(&root).unwrap();
        let after = root_snapshot(&root);
        assert_eq!(before["state"], "opened"); assert_eq!(after["state"], "opened");
        assert_ne!(before["identity"]["file_id_or_inode"], after["identity"]["file_id_or_inode"]);
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o755)).unwrap();
            let sample = root_snapshot(&root);
            assert_eq!(sample["identity"]["file_id_or_inode"], after["identity"]["file_id_or_inode"]);
            assert_eq!(sample["protection"]["mode"].as_u64().unwrap() & 0o777, 0o755);
        }
        std::fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn read_errors_are_not_absence_and_nested_sessions_are_isolated() {
        let f = File::open(std::env::current_exe().unwrap()).unwrap();
        let outer = Session::new();
        sample("outer", "fixture", &f);
        {
            let inner = Session::new();
            FAIL_READ.with(|f| f.set(true));
            sample("denied", "fixture", &f);
            FAIL_READ.with(|f| f.set(false));
            assert_eq!(inner.snapshot()[0]["protection"]["state"], "error");
            assert_eq!(outer.snapshot().len(), 1);
        }
        sample("restored", "fixture", &f);
        assert_eq!(outer.snapshot().len(), 2);
    }
    #[test]
    fn samples_keep_identity_and_permissions_without_writing() {
        let p = std::env::temp_dir().join(format!("gil-diagnostic-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::write(&p, b"unchanged").unwrap();
        let f = File::open(&p).unwrap();
        let session = Session::new();
        sample("before", "fixture", &f);
        sample("after", "fixture", &f);
        let v = session.snapshot();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0]["identity"], v[1]["identity"]);
        assert_eq!(v[0]["protection"], v[1]["protection"]);
        assert_eq!(v[0]["protection"]["state"], "observed");
        assert_eq!(std::fs::read(&p).unwrap(), b"unchanged");
        drop(f); std::fs::remove_file(p).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn a_fixture_mode_change_is_visible_without_repair() {
        use std::os::unix::fs::PermissionsExt;
        let p = std::env::temp_dir().join(format!("gil-diagnostic-mode-{}", std::process::id()));
        std::fs::write(&p, b"fixture").unwrap();
        let f = File::open(&p).unwrap(); let session = Session::new();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).unwrap();
        sample("before", "fixture", &f);
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o400)).unwrap();
        sample("after", "fixture", &f);
        let v = session.snapshot();
        assert_ne!(v[0]["protection"]["mode"], v[1]["protection"]["mode"]);
        assert_eq!(f.metadata().unwrap().permissions().mode() & 0o777, 0o400);
        drop(f); std::fs::remove_file(p).unwrap();
    }
}
