//! M6-A: a real, open container. Closing and subsequent Chains are M6-B/C.
//! Cycle lineage lives inside the container; it is not Chain ownership.
use crate::{ChainRef, CycleKind, CycleRef, Cycles, ExistenceRef, SnapshotRef};

#[derive(Debug, Clone)]
pub struct Chain {
    id: ChainRef,
    existence: ExistenceRef,
    entry: SnapshotRef,
    pub(crate) cycles: Cycles,
}

impl Chain {
    pub(crate) fn start(cycles: Cycles) -> Self {
        let first = &cycles.nodes()[0];
        Self::restore(
            ChainRef::new(1).unwrap(),
            first.existence(),
            first.entry_snapshot(),
            cycles,
        )
    }
    pub(crate) fn restore(
        id: ChainRef,
        existence: ExistenceRef,
        entry: SnapshotRef,
        cycles: Cycles,
    ) -> Self {
        Self {
            id,
            existence,
            entry,
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
        None
    }
    pub fn is_closed(&self) -> bool {
        false
    }
    pub fn parent(&self) -> Option<ChainRef> {
        None
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
        if id.number() != 1 || first.kind() != CycleKind::Interview || first.parent().is_some() {
            return Err(ChainError::Invalid(
                "M6-A requires the root Chain and its opening Interview",
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
    ParentOpen(ChainRef),
    LegacyProject,
    Invalid(&'static str),
}
impl std::fmt::Display for ChainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParentOpen(id) => write!(
                f,
                "{id}이 열려 있어 자식 Chain을 만들 수 없다. Chain 종료는 M6-B에서 지원한다"
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
