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
        // Exploration contract: a Verify says what it ran and what that showed. The Core cannot check
        // that either is true; it makes the Analysis answer against a stated observation.
        #[cfg(feature = "approved-plan")]
        if self.project.exploration_contract() {
            for field in ["command", "observed"] {
                if report.get(field).is_none_or(|s| s.trim().is_empty()) {
                    return Err(refused(crate::tr!("evidence.verify_ran", field = field)));
                }
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
        let Some(here) = cycle.step_now_open() else { return Ok(report); };
        let node = cycle.steps().node(here).expect("open step");
        // Check new writes here so older Reports remain readable. The grammar reports absent fields.
        let required_text: &[&str] = match node.kind {
            NodeKind::Question => &["response"],
            NodeKind::Synthesis => &["statement"],
            NodeKind::Define => &["problem", "success_condition"],
            NodeKind::Hypothesis => &["hypothesis"],
            _ => &[],
        };
        for &field in required_text {
            if report.get(field).is_some_and(|value| value.trim().is_empty()) {
                return Err(SessionError::Action(ActionError::Cycle(crate::CycleError::Grammar(
                    crate::GrammarError::EmptyReportField { subject: crate::Subject::Step(node.kind), field: field.into() },
                ))));
            }
        }
        // Exploration contract: the Interview that draws the first plan lays the work out from the
        // top before anything is listed. Its Interpretation names the major problems (each with
        // what judges it), what would make the work a failure although it runs, the inputs and
        // conditions the request implies without spelling out, the part most likely to fail, and
        // what cannot be observed here together with how the means to observe it will be made.
        // A plan drawn straight from the request listed what was easy to check and left out what
        // the work is judged by (2026-11-27).
        #[cfg(feature = "approved-plan")]
        if node.kind == NodeKind::Interpretation && self.project.exploration_contract() && !self.project.plan_approved_in_chain() {
            for field in ["major_problems", "anti_goals", "edge_cases", "hardest", "not_observable_here"] {
                if report.get(field).is_none_or(|value| value.trim().is_empty()) {
                    return Err(SessionError::Evidence(crate::tr!("evidence.interpretation_field", field = field)));
                }
            }
            if crate::approved_plan::major_problems(report.get("major_problems").unwrap_or("")).is_empty() {
                return Err(SessionError::Evidence(crate::locale::text("evidence.major_problems_shape").into()));
            }
        }
        if cycle.kind() != CycleKind::Experiment { return Ok(report); }
        // Exploration contract: the Analysis answers the Define's one observation with a boolean,
        // true or false, and an Outcome is a success only when neither answer was false. The Core
        // reads the two declared values; whether they are honest is not something it can check.
        #[cfg(feature = "approved-plan")]
        if self.project.exploration_contract() {
            const ANSWERS: [&str; 2] = ["problem_solved", "success_condition_met"];
            fn plain(value: Option<&str>) -> Option<&str> { value.map(str::trim).filter(|answer| matches!(*answer, "true" | "false")) }
            if node.kind == NodeKind::Analysis {
                for field in ANSWERS {
                    if plain(report.get(field)).is_none() {
                        return Err(refused(crate::tr!("evidence.analysis_yes_no", field = field)));
                    }
                }
                // What the Define's condition asked for and was not seen. Saying that something is
                // unverified and answering true is a contradiction the Core can read.
                let unverified = report.get("unverified").map(str::trim).unwrap_or("");
                if unverified.is_empty() {
                    return Err(refused(crate::locale::text("evidence.unverified_required")));
                }
                if !unverified.eq_ignore_ascii_case("none") && ANSWERS.iter().any(|field| plain(report.get(field)) == Some("true")) {
                    return Err(refused(crate::locale::text("evidence.unverified_but_true")));
                }
            }
            // A Hypothesis names the change it will make: an Experiment brings a state about, it does
            // not only check whether the state already holds.
            if node.kind == NodeKind::Hypothesis && report.get("change").is_none_or(|change| change.trim().is_empty()) {
                return Err(refused(crate::locale::text("evidence.hypothesis_change")));
            }
            // Going back returns the files. What this Experiment changed is kept in words, so the
            // next Hypothesis can build on it instead of finding it again.
            if node.kind == NodeKind::Outcome && report.get("verdict") == Some("failure")
                && report.get("changed").is_none_or(|changed| changed.trim().is_empty()) {
                return Err(refused(crate::locale::text("evidence.outcome_changed")));
            }
            if node.kind == NodeKind::Outcome && report.get("verdict") == Some("success") {
                let analysis = cycle.steps().lineage(here).map_err(|e| refused(e.to_string()))?
                    .into_iter().rev().find(|n| n.kind == NodeKind::Analysis && n.is_closed()).and_then(|n| n.report.as_ref());
                if analysis.is_some_and(|analysis| ANSWERS.iter().any(|field| plain(analysis.get(field)) == Some("false"))) {
                    return Err(refused(crate::locale::text("evidence.success_needs_yes")));
                }
            }
        }
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
