//! What a closed Verify stored as evidence, read back from **its own Snapshot**.
//!
//! A Verify Report names its evidence by project-relative path. The same path in the working
//! folder says nothing about that Verify: a later Verify may have rewritten the file and an
//! approved revisit may have returned the folder to an earlier world. The bytes the Verify
//! actually confirmed are the ones its Snapshot addresses, and they stay in the object store
//! whatever happens to the folder afterwards.
//!
//! This module reads exactly those bytes. It never looks at the working folder, never restores,
//! and has no door that writes: it hangs off [`ReadOnlySession`]. Only paths the Verify Report
//! itself declared can be asked for — its `evidence_files` and, where an approved plan was
//! bound, its `plan_document` (the exact approved revision the Experiment ran under). This is
//! not a way to read an arbitrary file of an arbitrary old world.
//!
//! Every way of not having the bytes is a different fact and is kept apart: no such Step, a
//! Step that is not a Verify, a Verify that has not written its Report, a path the Report never
//! declared, a declared path the Snapshot does not hold, and a store that cannot be read.
use super::*;
use crate::refs::StepRef;
use std::io::{self, Write};

/// Why stored evidence could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StoredEvidenceError {
    /// This Project has no such Step. Nothing nearby is offered instead.
    NotFound,
    /// The Step exists and is not a Verify; only a Verify confirms a world.
    NotAVerify { kind: NodeKind },
    /// The Verify is still open: no Report and no Snapshot yet.
    ReportNotWritten,
    /// The asked path is not a safe project-relative path.
    InvalidPath,
    /// The Verify Report names this path neither in `evidence_files` nor as `plan_document`.
    NotDeclared { declared: Vec<String> },
    /// The Report names the path, but the Verify's Snapshot holds no such file.
    NotInSnapshot { snapshot: SnapshotRef },
    /// The Snapshot or one of its objects cannot be read (missing, damaged, refused).
    Unavailable { snapshot: Option<SnapshotRef>, said: String },
    /// The offset lies at or beyond the end of the stored file.
    Offset { total: u64 },
}

/// How the latest confirmed world holds the same path. A record-to-record comparison: the
/// working folder is not observed, so an uncommitted edit there is invisible here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InCurrentWorld {
    Same,
    Different,
    Absent,
    /// The current world's manifest could not be read.
    Unknown,
}

impl InCurrentWorld {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            InCurrentWorld::Same => "same",
            InCurrentWorld::Different => "different",
            InCurrentWorld::Absent => "absent",
            InCurrentWorld::Unknown => "unknown",
        }
    }
}

/// One path a Verify Report declared, as its Snapshot holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeclaredEvidence {
    /// The path as the Report wrote it.
    pub(crate) path: String,
    /// The Report field that names it: `evidence_files` or `plan_document`.
    pub(crate) declared_in: &'static str,
    /// SHA-256 of the stored bytes; `None` when the Snapshot holds no such file.
    pub(crate) sha256: Option<String>,
    pub(crate) in_current_world: InCurrentWorld,
}

/// The evidence a Verify declared, located in the Snapshot it confirmed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EvidenceIndex {
    pub(crate) snapshot: SnapshotRef,
    /// The latest confirmed world, which `in_current_world` compares against.
    pub(crate) current_world: SnapshotRef,
    pub(crate) files: Vec<DeclaredEvidence>,
}

/// A bounded window of one stored evidence file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EvidenceChunk {
    pub(crate) snapshot: SnapshotRef,
    pub(crate) current_world: SnapshotRef,
    pub(crate) sha256: String,
    pub(crate) total_bytes: u64,
    pub(crate) offset: u64,
    pub(crate) bytes: Vec<u8>,
    pub(crate) in_current_world: InCurrentWorld,
}

/// Keeps only `[skip, skip + keep)` of a stream while counting all of it, so the store can
/// re-check the whole file's digest without the whole file being held in memory.
struct Window {
    skip: u64,
    keep: usize,
    seen: u64,
    held: Vec<u8>,
}

impl Write for Window {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let start = self.seen;
        let end = start + buf.len() as u64;
        let from = self.skip.max(start);
        let to = self.skip.saturating_add(self.keep as u64).min(end);
        if from < to {
            self.held.extend_from_slice(&buf[(from - start) as usize..(to - start) as usize]);
        }
        self.seen = end;
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// The Verify a canonical reference names, with its Report and the world it confirmed.
struct ClosedVerify<'a> {
    report: &'a Report,
    snapshot: SnapshotRef,
}

impl ReadOnlySession {
    fn closed_verify(&self, step: StepRef) -> Result<ClosedVerify<'_>, StoredEvidenceError> {
        let project = self.project();
        let cycle = project.all_cycles().find(|cycle| cycle.id().to_ref() == step.cycle())
            .ok_or(StoredEvidenceError::NotFound)?;
        let node = cycle.steps().nodes().iter().find(|node| cycle.step_ref(node.id) == step)
            .ok_or(StoredEvidenceError::NotFound)?;
        if matches!(node.kind, NodeKind::CycleEntry | NodeKind::CycleExit) {
            return Err(StoredEvidenceError::NotFound);
        }
        if node.kind != NodeKind::Verify {
            return Err(StoredEvidenceError::NotAVerify { kind: node.kind });
        }
        let report = node.report.as_ref().ok_or(StoredEvidenceError::ReportNotWritten)?;
        let snapshot = node.snapshot.ok_or_else(|| StoredEvidenceError::Unavailable {
            snapshot: None,
            said: crate::locale::text("record_tool.no_snapshot").into(),
        })?;
        Ok(ClosedVerify { report, snapshot })
    }

    fn manifest_of(&self, store: &ObjectStore, world: SnapshotRef) -> Result<Manifest, String> {
        let address = self.project().world_manifest(world)
            .ok_or_else(|| crate::tr!("record_tool.unknown_snapshot", snapshot = world))?;
        store.read_manifest(address).map_err(|error| error.to_string())
    }

    fn evidence_store(&self) -> Result<ObjectStore, StoredEvidenceError> {
        self.0.store().map_err(|error| StoredEvidenceError::Unavailable { snapshot: None, said: error.to_string() })
    }

    /// Every path the Verify Report declared, with what its Snapshot holds there.
    ///
    /// No blob is opened: the manifest already carries each file's content address.
    pub(crate) fn verify_evidence_index(&self, step: StepRef) -> Result<EvidenceIndex, StoredEvidenceError> {
        let verify = self.closed_verify(step)?;
        let store = self.evidence_store()?;
        let stored = self.manifest_of(&store, verify.snapshot)
            .map_err(|said| StoredEvidenceError::Unavailable { snapshot: Some(verify.snapshot), said })?;
        let current_world = self.project().world_snapshot();
        let current = self.manifest_of(&store, current_world).ok();
        let files = declared_paths(verify.report).into_iter().map(|(path, declared_in)| {
            let entry = crate::visual_report::file::relative(&path).ok();
            let digest = entry.as_ref().and_then(|entry| stored.get(entry));
            DeclaredEvidence {
                sha256: digest.map(|digest| digest.hex()),
                in_current_world: match (digest, entry.as_ref(), current.as_ref()) {
                    (Some(digest), Some(entry), Some(current)) => compare(digest, current.get(entry)),
                    _ => InCurrentWorld::Unknown,
                },
                path,
                declared_in,
            }
        }).collect();
        Ok(EvidenceIndex { snapshot: verify.snapshot, current_world, files })
    }

    /// At most `limit` bytes of one declared evidence file, starting at `offset`, as the
    /// Verify's Snapshot holds them. The store re-checks the whole file's digest while reading.
    pub(crate) fn read_verify_evidence(
        &self, step: StepRef, path: &str, offset: u64, limit: usize,
    ) -> Result<EvidenceChunk, StoredEvidenceError> {
        let verify = self.closed_verify(step)?;
        let asked = crate::visual_report::file::relative(path).map_err(|_| StoredEvidenceError::InvalidPath)?;
        let declared: Vec<String> = declared_paths(verify.report).into_iter().map(|(path, _)| path).collect();
        if !declared.iter().any(|line| crate::visual_report::file::relative(line).ok().as_ref() == Some(&asked)) {
            return Err(StoredEvidenceError::NotDeclared { declared });
        }
        let snapshot = verify.snapshot;
        let unavailable = |said: String| StoredEvidenceError::Unavailable { snapshot: Some(snapshot), said };
        let store = self.evidence_store()?;
        let stored = self.manifest_of(&store, snapshot).map_err(unavailable)?;
        let digest = stored.get(&asked).ok_or(StoredEvidenceError::NotInSnapshot { snapshot })?;
        let mut window = Window { skip: offset, keep: limit, seen: 0, held: Vec::new() };
        store.read_blob(digest, &mut window).map_err(|error| unavailable(error.to_string()))?;
        if offset > 0 && offset >= window.seen {
            return Err(StoredEvidenceError::Offset { total: window.seen });
        }
        let current_world = self.project().world_snapshot();
        let in_current_world = match self.manifest_of(&store, current_world) {
            Ok(current) => compare(digest, current.get(&asked)),
            Err(_) => InCurrentWorld::Unknown,
        };
        Ok(EvidenceChunk {
            snapshot, current_world, sha256: digest.hex(), total_bytes: window.seen, offset,
            bytes: window.held, in_current_world,
        })
    }
}

fn compare(stored: &artifact::ContentDigest, current: Option<&artifact::ContentDigest>) -> InCurrentWorld {
    match current {
        Some(current) if current == stored => InCurrentWorld::Same,
        Some(_) => InCurrentWorld::Different,
        None => InCurrentWorld::Absent,
    }
}

/// The nonblank lines of `evidence_files`, in the Report's order and spelling, then
/// `plan_document` when the Report names one that is not already among them.
fn declared_paths(report: &Report) -> Vec<(String, &'static str)> {
    let mut paths: Vec<(String, &'static str)> = report.get("evidence_files").unwrap_or("").lines().map(str::trim)
        .filter(|line| !line.is_empty()).map(|line| (line.to_owned(), "evidence_files")).collect();
    if let Some(plan) = report.get("plan_document").map(str::trim).filter(|path| !path.is_empty()) {
        if !paths.iter().any(|(path, _)| path == plan) {
            paths.push((plan.to_owned(), "plan_document"));
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_keeps_only_the_asked_range_and_counts_everything() {
        let mut window = Window { skip: 5, keep: 4, seen: 0, held: Vec::new() };
        for part in [&b"0123"[..], b"4567", b"89ab", b"cdef"] {
            window.write_all(part).unwrap();
        }
        assert_eq!(window.held, b"5678");
        assert_eq!(window.seen, 16);
        let mut past = Window { skip: 40, keep: 4, seen: 0, held: Vec::new() };
        past.write_all(b"short").unwrap();
        assert!(past.held.is_empty());
        assert_eq!(past.seen, 5);
        let mut all = Window { skip: 0, keep: usize::MAX, seen: 0, held: Vec::new() };
        all.write_all(b"whole").unwrap();
        assert_eq!(all.held, b"whole");
    }

    #[test]
    fn declared_paths_keep_the_reports_order_and_spelling() {
        let report = Report::parse("execution: x\nresult: y\nplan_document: plan/p.md\nevidence_files: |\n  evidence/b.md\n\n  evidence/a.md\n").unwrap();
        assert_eq!(declared_paths(&report), [("evidence/b.md".to_string(), "evidence_files"), ("evidence/a.md".to_string(), "evidence_files"), ("plan/p.md".to_string(), "plan_document")]);
        let listed_twice = Report::parse("plan_document: plan/p.md\nevidence_files: plan/p.md\n").unwrap();
        assert_eq!(declared_paths(&listed_twice), [("plan/p.md".to_string(), "evidence_files")]);
        assert!(declared_paths(&Report::parse("execution: x\n").unwrap()).is_empty());
    }
}
