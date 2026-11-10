//! Real Chain containment and approval-linked closure. Serial successors retain their parent and immutable handoff.
//! Cycle lineage lives inside the container; it is not Chain ownership.
use crate::{ChainRef, CycleKind, CycleRef, Cycles, ExistenceRef, Report, SnapshotRef, StepRef};

#[derive(Debug, Clone)]
pub struct Chain {
    id: ChainRef,
    existence: ExistenceRef,
    parent: Option<ChainRef>,
    entry: SnapshotRef,
    report: Option<Report>,
    pub(crate) cycles: Cycles,
    pub(crate) approved_plan: Option<crate::approved_plan::PlanState>,
}

impl Chain {
    pub(crate) fn start(cycles: Cycles) -> Self {
        let first = &cycles.nodes()[0];
        let mut chain = Self::restore(
            ChainRef::new(1).unwrap(),
            first.existence(),
            first.entry_snapshot(),
            None,
            cycles,
        );
        if cfg!(feature = "approved-plan") { chain.approved_plan = Some(Default::default()); }
        chain
    }
    pub(crate) fn restore(
        id: ChainRef,
        existence: ExistenceRef,
        entry: SnapshotRef,
        parent: Option<ChainRef>,
        cycles: Cycles,
    ) -> Self {
        Self {
            id,
            existence,
            parent,
            entry,
            report: None,
            approved_plan: None,
            cycles,
        }
    }
    pub fn id(&self) -> ChainRef {
        self.id
    }
    pub fn existence(&self) -> ExistenceRef {
        self.existence
    }
    pub fn entry_snapshot(&self) -> SnapshotRef {
        self.entry
    }
    pub fn exit_snapshot(&self) -> Option<SnapshotRef> {
        self.report
            .as_ref()
            .and(self.cycles.current().exit_snapshot())
    }
    pub fn journey(&self) -> Option<crate::JourneyRef> {
        self.report.as_ref().and(self.cycles.current().journey())
    }
    pub fn is_closed(&self) -> bool {
        self.report.is_some()
    }
    pub fn report(&self) -> Option<&Report> {
        self.report.as_ref()
    }

    pub fn awaiting_report(&self) -> bool {
        !self.is_closed()
            && self.cycles.current().is_closed()
            && self.cycles.current().declared_action() == Some("close_chain")
    }
    pub fn close_hint(&self) -> String {
        let cycle = self.cycles.current();
        let approved = cycle
            .report()
            .and_then(|report| cycle.closing_synthesis(report).ok());
        let (target, verdict) = approved
            .map(|(id, report)| (id.to_string(), report.get("chain_verdict").unwrap_or("")))
            .unwrap_or((
                crate::locale::text("chain.close_hint_target").into(),
                "<success|failure>",
            ));
        crate::tr!("chain.close_hint", marker = crate::locale::text("receipt.marker.chain_report"), verdict = verdict, target = target)
    }

    pub(crate) fn close(&mut self, report: Report) -> Result<(), ChainError> {
        if self.is_closed() {
            return Err(ChainError::AlreadyClosed);
        }
        self.check_report(&report)?;
        self.report = Some(report);
        Ok(())
    }

    fn check_report(&self, report: &Report) -> Result<(), ChainError> {
        let invalid = |why| ChainError::Invalid(why);
        let cycle = self.cycles.current();
        if !cycle.is_closed() || self.cycles.pending_revisit().is_some() {
            return Err(invalid("close the approved Closing Interview first"));
        }
        let (expected, synthesis) = cycle
            .closing_synthesis(
                cycle
                    .report()
                    .ok_or_else(|| invalid("missing Cycle Report"))?,
            )
            .map_err(|_| invalid("current Cycle is not a successful approved Closing Interview"))?;
        let target: StepRef = report
            .get("closing_synthesis_ref")
            .unwrap_or("")
            .parse()
            .map_err(|_| invalid("closing_synthesis_ref must be a canonical Step reference"))?;
        if target != expected || !self.owns(target.cycle()) {
            return Err(invalid(
                "closing_synthesis_ref must be the current Closing Interview's final approved Synthesis",
            ));
        }
        if report.get("verdict") != synthesis.get("chain_verdict") {
            return Err(invalid(
                "Chain verdict must match the approved Synthesis chain_verdict",
            ));
        }
        if report
            .get("handoff_summary")
            .is_none_or(|value| value.trim().is_empty())
        {
            return Err(invalid("handoff_summary must not be empty"));
        }
        if report
            .field_names()
            .any(|key| !["verdict", "closing_synthesis_ref", "handoff_summary"].contains(&key))
        {
            return Err(invalid(
                "Chain Report accepts only verdict, closing_synthesis_ref and handoff_summary",
            ));
        }
        Ok(())
    }

    pub(crate) fn validate_state(
        &self,
        owner: ExistenceRef,
        journey: crate::JourneyRef,
    ) -> Result<(), ChainError> {
        if let Some(plan) = &self.approved_plan { plan.validate(self).map_err(ChainError::Plan)?; }
        else if self.cycles.nodes().iter().flat_map(|c| c.steps().nodes()).any(|n| n.report.as_ref().is_some_and(|r| r.has("plan_revision") || r.has("plan_digest"))) {
            return Err(ChainError::Invalid("approval ledger is missing; do not downgrade planned records"));
        }
        Self::validate(self.id, self.existence, self.entry, &self.cycles, owner)?;
        for cycle in self.cycles.nodes() {
            if cycle.declared_action() == Some("close_chain")
                && cycle.id() != self.cycles.current_id()
            {
                return Err(ChainError::Invalid(
                    "a Closing Interview cannot be left behind",
                ));
            }
        }
        if let Some(report) = &self.report {
            self.check_report(report)?;
            if self.journey().is_none_or(|at| at.existence() != journey.existence() || at.revision() > journey.revision()) {
                return Err(ChainError::Invalid(
                    "closed Chain Journey must belong to its owner and not be in the future",
                ));
            }
        }
        Ok(())
    }
    pub fn parent(&self) -> Option<ChainRef> {
        self.parent
    }
    pub fn cycles(&self) -> &Cycles {
        &self.cycles
    }
    pub fn current_cycle(&self) -> CycleRef {
        self.cycles.current_id().to_ref()
    }
    pub fn owns(&self, cycle: CycleRef) -> bool {
        self.cycles
            .nodes()
            .iter()
            .any(|node| node.id().to_ref() == cycle)
    }
    pub(crate) fn validate(
        id: ChainRef,
        owner: ExistenceRef,
        entry: SnapshotRef,
        cycles: &Cycles,
        current_owner: ExistenceRef,
    ) -> Result<(), ChainError> {
        let Some(first) = cycles.nodes().first() else {
            return Err(ChainError::Invalid("Chain has no opening Interview"));
        };
        if id.number() == 0 || first.kind() != CycleKind::Interview || first.parent().is_some() {
            return Err(ChainError::Invalid(
                "the root Chain must contain its opening Interview",
            ));
        }
        if owner != current_owner
            || cycles
                .nodes()
                .iter()
                .any(|cycle| cycle.existence() != owner)
        {
            return Err(ChainError::Invalid(
                "an open Chain cannot change its Existence",
            ));
        }
        if entry != first.entry_snapshot() {
            return Err(ChainError::Invalid(
                "Chain Entry must be the opening Interview Entry",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainError {
    Plan(String),
    AlreadyClosed,
    IdExhausted,
    ParentOpen(ChainRef),
    LegacyProject,
    Invalid(&'static str),
}
impl std::fmt::Display for ChainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Plan(reason) => write!(f, "{reason}"),
            Self::AlreadyClosed => f.write_str(crate::locale::text("chain.err.already_closed")),
            Self::IdExhausted => f.write_str(crate::locale::text("chain.err.id_exhausted")),
            Self::ParentOpen(id) => f.write_str(&crate::tr!("chain.err.parent_open", id = id)),
            Self::LegacyProject => f.write_str(crate::locale::text("chain.err.legacy")),
            Self::Invalid(why) => f.write_str(&crate::tr!("chain.err.invalid", why = why)),
        }
    }
}
impl std::error::Error for ChainError {}
