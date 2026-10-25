#![cfg(all(feature = "runtime-testbed", any(target_os = "macos", windows)))]
use gil::companion_discovery::{APP_FILE, RECEIPT, SCHEMA, discover_for_core};
use gil::companion_session::Release;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

struct Fixture {
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        #[cfg(target_os = "macos")]
        let base = PathBuf::from("/private/tmp");
        #[cfg(windows)]
        let base = std::env::temp_dir();
        let root = base.join(format!(
            "gcd-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        fs::copy(
            env!("CARGO_BIN_EXE_gil-runtime-testbed"),
            root.join(APP_FILE),
        )
        .unwrap();
        let fixture = Self { root };
        fixture.receipt(1, APP_FILE, &fixture.hash());
        fixture
    }
    fn core(&self) -> PathBuf {
        self.root.join("gil")
    }
    fn hash(&self) -> String {
        format!(
            "{:x}",
            Sha256::digest(fs::read(self.root.join(APP_FILE)).unwrap())
        )
    }
    fn receipt(&self, protocol: u32, name: &str, hash: &str) {
        fs::write(self.root.join(RECEIPT),serde_json::json!({"schema":SCHEMA,"protocol":protocol,"companion_file":name,"companion_sha256":hash}).to_string()).unwrap();
    }
    fn namespace(&self) -> PathBuf {
        #[cfg(target_os = "macos")]
        {
            self.root.join("ipc")
        }
        #[cfg(windows)]
        {
            PathBuf::from(format!(
                r"\\.\pipe\gil-companion-v1-{}",
                self.root.file_name().unwrap().to_str().unwrap()
            ))
        }
    }
    fn count(&self, prefix: &str) -> usize {
        fs::read_dir(&self.root)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with(prefix))
            .count()
    }
}
// Keep isolated evidence on failure; never delete a directory while its fixture may still live.
impl Drop for Fixture {
    fn drop(&mut self) {
        if !std::thread::panicking() && self.count("started-") == self.count("exited-") {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
fn until(mut f: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(12);
    while !f() {
        assert!(Instant::now() < end, "fixture timed out");
        std::thread::sleep(Duration::from_millis(25));
    }
}
fn release() -> Release {
    Release::new("fixture.1", &"a".repeat(64)).unwrap()
}

#[test]
fn isolated_installed_companion_is_started_once_shared_and_restarted_after_exit() {
    let f = Fixture::new();
    let mut installed = discover_for_core(&f.core()).unwrap().unwrap();
    let namespace = f.namespace();
    let (a, b) = std::thread::scope(|scope| {
        let a = scope.spawn(|| installed.connect_at(&release(), &namespace).unwrap());
        let b = scope.spawn(|| {
            discover_for_core(&f.core())
                .unwrap()
                .unwrap()
                .connect_at(&release(), &namespace)
                .unwrap()
        });
        (a.join().unwrap(), b.join().unwrap())
    });
    assert_eq!(f.count("started-"), 1);
    drop(a);
    std::thread::sleep(Duration::from_millis(250));
    assert_eq!(f.count("exited-"), 0);
    assert!(b.response());
    drop(b);
    until(|| f.count("exited-") == 1);
    let c = installed.connect_at(&release(), &namespace).unwrap();
    assert_eq!(f.count("started-"), 2);
    drop(c);
    until(|| f.count("exited-") == 2);
}
#[test]
fn absent_invalid_or_changed_installation_never_starts_anything() {
    let f = Fixture::new();
    fs::remove_file(f.root.join(RECEIPT)).unwrap();
    assert!(discover_for_core(&f.core()).unwrap().is_none());
    for (protocol, name, hash) in [
        (2, APP_FILE, f.hash()),
        (1, "../other", f.hash()),
        (1, APP_FILE, "0".repeat(64)),
    ] {
        f.receipt(protocol, name, &hash);
        assert!(discover_for_core(&f.core()).is_err());
    }
    f.receipt(1, APP_FILE, &f.hash());
    #[allow(unused_mut)] // Unix mutates the pinned file; Windows refuses that write.
    let mut installed = discover_for_core(&f.core()).unwrap().unwrap();
    #[cfg(unix)]
    {
        fs::write(f.root.join(APP_FILE), b"not a native executable").unwrap();
        assert!(installed.connect_at(&release(), &f.namespace()).is_err());
    }
    #[cfg(windows)]
    {
        assert!(
            fs::write(f.root.join(APP_FILE), b"replacement").is_err(),
            "pinned image denies write"
        );
        drop(installed);
    }
    assert_eq!(f.count("started-"), 0);
}

#[test]
fn linked_or_oversized_receipts_are_refused_without_repair_or_execution() {
    let f = Fixture::new();
    let original = fs::read(f.root.join(RECEIPT)).unwrap();
    let link = f.root.join("receipt-alias");
    fs::hard_link(f.root.join(RECEIPT), &link).unwrap();
    assert!(discover_for_core(&f.core()).is_err());
    assert_eq!(fs::read(&link).unwrap(), original);
    fs::remove_file(&link).unwrap(); // only our fixture link
    fs::write(f.root.join(RECEIPT), vec![b' '; 4097]).unwrap();
    assert!(discover_for_core(&f.core()).is_err());
    assert_eq!(fs::metadata(f.root.join(RECEIPT)).unwrap().len(), 4097);
    #[cfg(unix)]
    {
        fs::remove_file(f.root.join(RECEIPT)).unwrap();
        fs::write(&link, &original).unwrap();
        std::os::unix::fs::symlink(&link, f.root.join(RECEIPT)).unwrap();
        assert!(discover_for_core(&f.core()).is_err());
        assert_eq!(fs::read(&link).unwrap(), original);
    }
    assert_eq!(f.count("started-"), 0);
}
