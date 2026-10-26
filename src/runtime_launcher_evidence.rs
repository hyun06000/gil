//! Testbed-only, explicitly requested side-channel evidence, never MCP stdout.
//! The harness creates the destination beside (not inside) the protected store.
use crate::{runtime_diagnostics::{root_snapshot, Session}, runtime_launch as launch};
use serde_json::{json, Value};
use std::{fs::{self, File, OpenOptions}, io::{self, Read, Write}, path::Path};

struct Journal { file: File, sequence: usize, failed: bool }
impl Journal {
    fn new(dir: &Path) -> io::Result<Self> {
        if !dir.is_absolute() || !fs::symlink_metadata(dir)?.is_dir() {
            return Err(io::Error::other("evidence destination must be an existing absolute directory"));
        }
        let ns = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(io::Error::other)?.as_nanos();
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let name = format!("launcher-{}-{ns}-{}.jsonl", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
        Ok(Self { file: OpenOptions::new().write(true).create_new(true).open(dir.join(name))?, sequence: 0, failed: false })
    }
    fn event(&mut self, stage: &str, data: Value) -> bool {
        let record = json!({"schema":1, "pid":std::process::id(), "sequence":self.sequence, "stage":stage,
            "unix_ns":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_nanos().to_string()), "data":data});
        self.sequence += 1;
        if let Err(e) = writeln!(self.file, "{record}").and_then(|()| self.file.sync_all()) {
            self.failed = true;
            eprintln!("LAUNCHER EVIDENCE NOT WRITTEN: {e}; record={record}");
            return false;
        }
        true
    }
}

pub fn launcher_with_evidence(root: &Path, bundled: &Path, dir: &Path) -> i32 {
    let mut journal = match Journal::new(dir) {
        Ok(j) => j,
        Err(e) => { eprintln!("LAUNCHER EVIDENCE NOT WRITTEN: {e}; no Core started"); return 6; }
    };
    let session = Session::new();
    if !journal.event("start", json!({"root":root_snapshot(root)})) { return 6; }
    let target = match launch::prepare_target(root, bundled) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("gil-runtime-testbed launcher: refused at {:?}: {}; verdict {:?}; spawned {:?}; child {:?}", e.stage, e.reason, e.verdict, e.spawned, e.child);
            journal.event("refused", json!({"stage":format!("{:?}",e.stage), "reason":e.reason, "verdict":format!("{:?}",e.verdict), "spawned":format!("{:?}",e.spawned), "child":format!("{:?}",e.child), "held_root_samples":session.snapshot(), "root_after":root_snapshot(root)}));
            return if journal.failed { 6 } else { 3 };
        }
    };
    if !journal.event("decision", json!({"target":format!("{:?}",target.kind), "verdict":format!("{:?}",target.verdict), "held_root_samples":session.snapshot(), "verified_sha256":target.verified_sha256})) { return 6; }
    let child = match launch::spawn(target, launch::FixedArgs::McpServe, launch::IoMode::Inherit) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("gil-runtime-testbed launcher: refused at {:?}: {}; verdict {:?}; spawned {:?}; child {:?}", e.stage, e.reason, e.verdict, e.spawned, e.child);
            journal.event("refused", json!({"stage":format!("{:?}",e.stage), "reason":e.reason, "verdict":format!("{:?}",e.verdict), "spawned":format!("{:?}",e.spawned), "child":format!("{:?}",e.child), "root_after":root_snapshot(root)}));
            return if journal.failed { 6 } else { 3 };
        }
    };
    journal.event("spawned", json!({"child_pid":child.id(), "root":root_snapshot(root)}));
    // Even a journal failure after spawn must keep the normal child wait/stdio lifetime.
    let report = child.send_eof_and_finish(&[], None);
    journal.event("exit", json!({"child_pid":report.pid, "exit":format!("{:?}",report.exit), "root":root_snapshot(root)}));
    if journal.failed { return 6; }
    match report.exit { launch::Exit::Code(c) => c, launch::Exit::Signal(_) => 4, launch::Exit::NotConfirmed { .. } => 5 }
}

/// Snapshot bounded raw records before temporary fixtures are removed. Partial/unreadable
/// journals are errors, not empty success. Files can be incomplete if a launcher is still alive.
pub fn read_launcher_evidence(dir: &Path) -> Value {
    let mut records = Vec::new(); let mut errors = Vec::new();
    let result = (|| -> io::Result<()> {
        let mut files = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with("launcher-") || !name.ends_with(".jsonl") { continue; }
            if files.len() >= 128 { return Err(io::Error::other("too many launcher journals")); }
            files.push(entry.path());
        }
        files.sort();
        for path in files {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let read = (|| -> io::Result<String> {
                if !fs::symlink_metadata(&path)?.is_file() { return Err(io::Error::other("journal is not a regular file")); }
                let mut bytes = Vec::new(); File::open(&path)?.take(2 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
                if bytes.len() > 2 * 1024 * 1024 { return Err(io::Error::other("journal exceeds size limit")); }
                String::from_utf8(bytes).map_err(io::Error::other)
            })();
            match read {
                Err(e) => errors.push(format!("{name}: {e}")),
                Ok(raw) => {
                    let mut events = Vec::new();
                    if !raw.ends_with('\n') { errors.push(format!("{name}: empty or incomplete journal")); }
                    for (n, line) in raw.lines().enumerate() {
                        match serde_json::from_str::<Value>(line) {
                            Ok(v) if v["schema"] == 1 && v["sequence"] == n => events.push(v),
                            other => errors.push(format!("{name}: invalid event at line {}: {other:?}", n+1)),
                        }
                    }
                    let has = |stage: &str| events.iter().any(|e| e["stage"] == stage);
                    if !has("start") || !(has("refused") || (has("decision") && has("spawned"))) {
                        errors.push(format!("{name}: missing launch decision/spawn or refusal evidence"));
                    }
                    // The host can close a launcher without letting it append an exit record.
                    // This is not proof of natural exit; process termination is checked separately.
                    let completion = if has("exit") { "exit_recorded" } else if has("refused") { "refusal_recorded" } else { "exit_not_recorded" };
                    records.push(json!({"file":name, "events":events, "raw":raw, "completion":completion}));
                }
            }
        }
        Ok(())
    })();
    if let Err(e) = result { errors.push(e.to_string()); }
    json!({"records":records, "errors":errors, "limits":"local testbed journal, not authenticated; sequential snapshots; no actor attribution; may contain personal SIDs/paths"})
}
