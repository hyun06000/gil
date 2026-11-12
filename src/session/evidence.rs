//! New writes must leave inspectable evidence in the observed world. Old records remain readable.
//! This is the filesystem transaction boundary shared by CLI, ordinary MCP and typed visual MCP.
use super::*;
use std::collections::BTreeSet;

fn refused(reason: impl Into<String>) -> SessionError {
    SessionError::Evidence(reason.into())
}

impl ProjectSession {
    pub(super) fn require_verify_evidence(
        &self, report: &Report, world: &artifact::ManifestAddress,
    ) -> Result<(), SessionError> {
        for field in ["execution", "result"] {
            if report.get(field).is_none_or(|s| s.trim().is_empty()) {
                return Err(refused(crate::tr!("evidence.field_required", field = field)));
            }
        }
        let declared = report.get("evidence_files").unwrap_or("");
        let mut paths = BTreeSet::new();
        for line in declared.lines().filter(|s| !s.trim().is_empty()) {
            let path = crate::visual_report::file::relative(line)
                .map_err(|_| refused(crate::locale::text("evidence.paths_shape")))?;
            if !paths.insert(path) || paths.len() > 32 {
                return Err(refused(crate::locale::text("evidence.paths_count")));
            }
        }
        if paths.is_empty() {
            return Err(refused(crate::locale::text("evidence.files_required")));
        }
        let objects = self.store()?;
        let found = objects.read_manifest(world)
            .map_err(|e| SessionError::Object { said: e.to_string() })?;
        let baseline = self.project.world_snapshot();
        let address = self.project.world_manifest(baseline)
            .ok_or(SessionError::UnknownWorld { world: baseline })?;
        let before = objects.read_manifest(address)
            .map_err(|e| SessionError::Manifest { world: baseline, said: e.to_string() })?;
        let empty = artifact::digest_of(std::io::empty()).expect("empty reader");
        let plan_path = if let Some(document) = self.project.plan_document() {
            let path = crate::visual_report::file::relative(report.get("plan_document").unwrap_or(""))
                .map_err(|_| refused(crate::locale::text("evidence.plan_path")))?;
            let expected = artifact::digest_of(std::io::Cursor::new(document.as_bytes())).expect("document bytes");
            if found.get(&path) != Some(&expected) { return Err(refused(crate::locale::text("evidence.plan_mismatch"))); }
            Some(path)
        } else { None };
        let mut fresh = false;
        for path in paths {
            let digest = found.get(&path)
                .ok_or_else(|| refused(crate::tr!("evidence.not_in_snapshot", path = path)))?;
            if digest == &empty { return Err(refused(crate::tr!("evidence.empty_file", path = path))); }
            fresh |= plan_path.as_ref() != Some(&path) && before.get(&path) != Some(digest);
        }
        if !fresh {
            return Err(refused(crate::locale::text("evidence.not_fresh")));
        }
        Ok(())
    }

    /// These references are structural facts, not invented learning or retrospective reports.
    pub(super) fn link_learning(&self, mut report: Report) -> Result<Report, SessionError> {
        let cycle = self.project.current_cycle();
        if cycle.kind() != CycleKind::Experiment { return Ok(report); }
        let Some(here) = cycle.step_now_open() else { return Ok(report); };
        let node = cycle.steps().node(here).expect("open step");
        let (text, field, source_kind) = match node.kind {
            NodeKind::Analysis => ("interpretation", "evidence.verify_ref", NodeKind::Verify),
            NodeKind::Outcome => ("lesson", "learning.analysis_ref", NodeKind::Analysis),
            _ => return Ok(report),
        };
        if report.get(text).is_none_or(|s| s.trim().is_empty()) {
            return Err(refused(crate::tr!("evidence.lesson_required", text = text)));
        }
        let lineage = cycle.steps().lineage(here).map_err(|e| refused(e.to_string()))?;
        let source = lineage.iter().rev().find(|n| n.kind == source_kind && n.is_closed())
            .ok_or_else(|| refused(crate::locale::text("evidence.no_basis_step")))?;
        let expected = cycle.step_ref(source.id).to_string();
        if report.get(field).is_some_and(|given| given != expected) {
            return Err(refused(crate::tr!("evidence.basis_ref", field = field, expected = expected)));
        }
        report.insert(field, expected);
        Ok(report)
    }
}
