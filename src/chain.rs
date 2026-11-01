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
}

impl Chain {
    pub(crate) fn start(cycles: Cycles) -> Self {
        let first = &cycles.nodes()[0];
        Self::restore(
            ChainRef::new(1).unwrap(),
            first.existence(),
            first.entry_snapshot(),
            None,
            cycles,
        )
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
                "<승인된 종료 Synthesis StepRef>".into(),
                "<success|failure>",
            ));
        format!(
            "Chain Report를 gil close로 제출한다.\nverdict: {verdict}\nclosing_synthesis_ref: {target}\nhandoff_summary: <탐색한 경로·실패·남은 질문 요약>\n"
        )
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
    AlreadyClosed,
    IdExhausted,
    ParentOpen(ChainRef),
    LegacyProject,
    Invalid(&'static str),
}
impl std::fmt::Display for ChainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyClosed => {
                write!(f, "이 Chain은 이미 닫혔다. 기존 Report와 기록을 보존한다")
            }
            Self::IdExhausted => {
                write!(f, "Chain 또는 Cycle 식별자 공간이 소진되었다. 기록을 보존한다")
            }
            Self::ParentOpen(id) => write!(
                f,
                "{id}이 열려 있어 자식 Chain을 만들 수 없다. 승인된 Closing Interview와 Chain Report로 먼저 닫는다"
            ),
            Self::LegacyProject => write!(
                f,
                "기존 format 4/5에는 Chain이 없다. 기록을 보존하며 자동 이전하지 않는다"
            ),
            Self::Invalid(why) => write!(f, "잘못된 Chain: {why}"),
        }
    }
}
impl std::error::Error for ChainError {}
