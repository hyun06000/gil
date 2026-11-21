//! Test-only observation of the registration lock attempt in `store` (compiled only under
//! `cfg(test)`, never into a product build).
//!
//! `store` maps every `try_lock` error to one user message. This probe reads the value that
//! `try_lock` already returned, before that mapping, and keeps what the standard library carried.
//! It never retries, waits, locks again, unlocks, deletes the lock file or asks the operating
//! system again: an error number that `TryLockError::WouldBlock` does not carry is reported as
//! not provided, never reconstructed from `last_os_error` or guessed.
//!
//! Every attempt bumps two counters; only a failed attempt is formatted, kept in memory and
//! written once to the test's captured stderr (shown by the test harness when that test fails).
//! Paths are written relative to the system temporary folder; settings contents are never read.

use std::cell::Cell;
use std::fs::TryLockError;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Lock attempts made by `store` in this test process, success or failure.
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
/// Failed attempts observed in this test process, for the control test.
static FAILURES: Mutex<Vec<Record>> = Mutex::new(Vec::new());

thread_local! {
    /// Attempts on this thread and when the previous one was made.
    static ON_THREAD: Cell<(u32, Option<Instant>)> = const { Cell::new((0, None)) };
}

/// One failed attempt, as observed.
#[derive(Clone, Debug)]
pub(crate) struct Record {
    /// 1-based attempt number within this test process (all threads).
    pub sequence: u64,
    /// 1-based attempt number on this thread. libtest runs each parallel test on its own thread.
    pub thread_attempt: u32,
    /// Microseconds since the previous attempt on this thread, if there was one.
    pub since_previous_on_thread_us: Option<u128>,
    pub pid: u32,
    pub thread_name: String,
    pub thread_id: String,
    /// Lock path relative to the system temporary folder.
    pub lock: String,
    /// `WouldBlock` or `Error`.
    pub variant: &'static str,
    pub debug: String,
    pub display: String,
    /// Only for `TryLockError::Error`: the inner `std::io::Error`'s kind and OS error number.
    pub kind: Option<String>,
    pub raw_os_error: Option<i32>,
}

/// Observe one `try_lock` result. Never changes it and never panics past this call.
pub(crate) fn observe(lock_path: &Path, attempt: &Result<(), TryLockError>) {
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed) + 1;
    let now = Instant::now();
    let (thread_attempt, previous) = ON_THREAD.with(|cell| {
        let (count, previous) = cell.get();
        cell.set((count.saturating_add(1), Some(now)));
        (count.saturating_add(1), previous)
    });
    let Err(error) = attempt else { return };
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let current = std::thread::current();
        let record = Record {
            sequence,
            thread_attempt,
            since_previous_on_thread_us: previous.map(|at| now.duration_since(at).as_micros()),
            pid: std::process::id(),
            thread_name: current.name().unwrap_or("<unnamed>").to_string(),
            thread_id: format!("{:?}", current.id()),
            lock: relative_to_temp(lock_path),
            ..describe(error)
        };
        let line = format!("{}\n", render(&record));
        if let Ok(mut failures) = FAILURES.lock() {
            failures.push(record);
        }
        eprint!("{line}");
    }));
}

/// The fields of a `TryLockError` itself; identity fields are left empty.
pub(crate) fn describe(error: &TryLockError) -> Record {
    let (variant, kind, raw_os_error) = match error {
        TryLockError::WouldBlock => ("WouldBlock", None, None),
        TryLockError::Error(inner) => ("Error", Some(format!("{:?}", inner.kind())), inner.raw_os_error()),
    };
    Record {
        sequence: 0,
        thread_attempt: 0,
        since_previous_on_thread_us: None,
        pid: 0,
        thread_name: String::new(),
        thread_id: String::new(),
        lock: String::new(),
        variant,
        debug: format!("{error:?}"),
        display: error.to_string(),
        kind,
        raw_os_error,
    }
}

/// One line: the attempt, the error as returned and the test run's settings.
pub(crate) fn render(record: &Record) -> String {
    let not_provided = || match record.variant {
        "WouldBlock" => "not provided (TryLockError::WouldBlock carries no io::Error)".to_string(),
        _ => "not provided by the inner io::Error".to_string(),
    };
    format!(
        "GIL-LOCK-PROBE sequence={} thread_attempt={} since_previous_on_thread_us={} pid={} thread_name={:?} thread_id={} \
         lock={:?} variant={} debug={:?} display={:?} kind={} raw_os_error={} env={}",
        record.sequence,
        record.thread_attempt,
        record.since_previous_on_thread_us.map_or("none".to_string(), |us| us.to_string()),
        record.pid,
        record.thread_name,
        record.thread_id,
        record.lock,
        record.variant,
        record.debug,
        record.display,
        record.kind.clone().unwrap_or_else(not_provided),
        record.raw_os_error.map_or_else(not_provided, |code| code.to_string()),
        environment(),
    )
}

/// Failed attempts observed so far in this process.
pub(crate) fn failures() -> Vec<Record> {
    FAILURES.lock().map(|failures| failures.clone()).unwrap_or_default()
}

pub(crate) fn relative_to_temp(path: &Path) -> String {
    match path.strip_prefix(std::env::temp_dir()) {
        Ok(relative) => format!("$TMPDIR/{}", relative.display()),
        Err(_) => format!(
            "<outside the temporary folder>/{}",
            path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()
        ),
    }
}

/// The run's settings that bear on parallelism, read without changing anything.
fn environment() -> String {
    let rust_test_threads = std::env::var("RUST_TEST_THREADS").unwrap_or_else(|_| "unset".into());
    let mut args = std::env::args().skip(1);
    let mut test_threads_arg = "absent".to_string();
    while let Some(arg) = args.next() {
        if let Some(value) = arg.strip_prefix("--test-threads=") {
            test_threads_arg = value.to_string();
        } else if arg == "--test-threads" {
            test_threads_arg = args.next().unwrap_or_default();
        }
    }
    let parallelism = std::thread::available_parallelism().map_or("unknown".to_string(), |n| n.to_string());
    let features: Vec<&str> = [
        (cfg!(feature = "chain-core"), "chain-core"),
        (cfg!(feature = "approved-plan"), "approved-plan"),
        (cfg!(feature = "exploration-flow"), "exploration-flow"),
        (cfg!(feature = "will-end-history"), "will-end-history"),
        (cfg!(feature = "visual-authoring"), "visual-authoring"),
        (cfg!(feature = "product-safety"), "product-safety"),
        (cfg!(feature = "recovery-auto-approval"), "recovery-auto-approval"),
        (cfg!(feature = "runtime-testbed"), "runtime-testbed"),
    ]
    .into_iter()
    .filter_map(|(on, name)| on.then_some(name))
    .collect();
    format!(
        "os={} arch={} RUST_TEST_THREADS={} --test-threads={} available_parallelism={} features=[{}]",
        std::env::consts::OS,
        std::env::consts::ARCH,
        rust_test_threads,
        test_threads_arg,
        parallelism,
        features.join(",")
    )
}
