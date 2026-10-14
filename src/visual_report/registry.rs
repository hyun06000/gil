//! 불변 자료/Report 연결의 메모리 registry. 파일을 읽거나 state를 저장하지 않는다.
//! prepare_close는 검증된 후보를 반환하고 원본은 변경하지 않는다. 이후 통합 계층은
//! Graph close와 이 후보를 같은 state commit에 포함해야 한다. 후보 생성은 durable close가 아니다.

use std::collections::BTreeMap;
use std::num::{NonZeroU32, NonZeroU64};

use super::{VisualAssetId, VisualDeclaration, VisualUse, VisualValidationError, validate_uses};
use crate::{CycleRef, NodeKind, StepRef};

pub mod codec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReportOwner {
    Step(StepRef),
    Cycle(CycleRef),
}

/// 실제 Graph에서 조회해야 하는 상태. 외부 요청이 주장하는 kind를 그대로 넘기면 안 된다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetState {
    OpenStep(NodeKind),
    OpenCycle,
    Closed,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    Png,
    Jpeg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Captured,
    Generated,
    Provided,
}

/// bytes/decoder 계층이 나중에 검증해야 할 메타데이터. 이 값만으로 안전한 이미지임을
/// 주장하지 않는다. 공개 파일 경로나 raw HTML은 저장하지 않는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewVisualAsset {
    pub digest: [u8; 32],
    pub image_kind: ImageKind,
    pub byte_len: NonZeroU64,
    pub width: NonZeroU32,
    pub height: NonZeroU32,
    pub source_kind: SourceKind,
    pub source_description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualAsset {
    origin: StepRef,
    metadata: NewVisualAsset,
}

impl VisualAsset {
    pub fn origin(&self) -> StepRef {
        self.origin
    }
    pub fn metadata(&self) -> &NewVisualAsset {
        &self.metadata
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReportVisuals {
    Verify(VisualDeclaration),
    References(Vec<VisualUse>),
}

impl ReportVisuals {
    pub fn uses(&self) -> &[VisualUse] {
        match self {
            Self::Verify(declaration) => declaration.uses(),
            Self::References(uses) => uses,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    TargetNotOpen,
    TargetKindMismatch,
    ReportAlreadyRecorded,
    CreationOutsideVerify,
    VerifyDeclarationRequired,
    DeclarationOutsideVerify,
    BlankSource,
    IdExhausted,
    UnreferencedCreation(VisualAssetId),
    InvalidReport(VisualValidationError),
}

/// 하나의 Project가 소유해야 한다. 이 타입 자체는 Host scope 인증 경계가 아니다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualRegistry {
    next_id: u64,
    assets: BTreeMap<VisualAssetId, VisualAsset>,
    reports: BTreeMap<ReportOwner, ReportVisuals>,
}

impl Default for VisualRegistry {
    fn default() -> Self {
        Self {
            next_id: 1,
            assets: BTreeMap::new(),
            reports: BTreeMap::new(),
        }
    }
}

impl VisualRegistry {
    /// 먼저 모든 변경을 후보 안에서 검증한다. 실패하거나 후보를 버리면 이름 발급까지 불변.
    /// callback의 상태는 실제 Graph에서, 자료 메타데이터는 검증된 object 계층에서 와야 한다.
    pub fn prepare_close(
        &self,
        owner: ReportOwner,
        state: TargetState,
        creations: Vec<NewVisualAsset>,
        report: impl FnOnce(&[VisualAssetId]) -> ReportVisuals,
    ) -> Result<Self, RegistryError> {
        if self.reports.contains_key(&owner) {
            return Err(RegistryError::ReportAlreadyRecorded);
        }
        let verify = match (owner, state) {
            (_, TargetState::Closed | TargetState::Unknown) => {
                return Err(RegistryError::TargetNotOpen);
            }
            (
                ReportOwner::Step(_),
                TargetState::OpenStep(NodeKind::CycleEntry | NodeKind::CycleExit),
            ) => return Err(RegistryError::TargetKindMismatch),
            (ReportOwner::Step(_), TargetState::OpenStep(kind)) => kind == NodeKind::Verify,
            (ReportOwner::Cycle(_), TargetState::OpenCycle) => false,
            _ => return Err(RegistryError::TargetKindMismatch),
        };
        if !verify && !creations.is_empty() {
            return Err(RegistryError::CreationOutsideVerify);
        }
        let mut candidate = self.clone();
        let mut ids = Vec::with_capacity(creations.len());
        for metadata in creations {
            if metadata.source_description.trim().is_empty() {
                return Err(RegistryError::BlankSource);
            }
            let id = NonZeroU64::new(candidate.next_id).ok_or(RegistryError::IdExhausted)?;
            candidate.next_id = candidate
                .next_id
                .checked_add(1)
                .ok_or(RegistryError::IdExhausted)?;
            let id = VisualAssetId::new(id);
            let ReportOwner::Step(origin) = owner else {
                return Err(RegistryError::CreationOutsideVerify);
            };
            candidate
                .assets
                .insert(id, VisualAsset { origin, metadata });
            ids.push(id);
        }
        let visuals = report(&ids);
        let available = |id| candidate.assets.contains_key(&id);
        match (&visuals, verify) {
            (ReportVisuals::Verify(declaration), true) => declaration.validate(available),
            (ReportVisuals::References(uses), false) => validate_uses(uses, available),
            (ReportVisuals::References(_), true) => {
                return Err(RegistryError::VerifyDeclarationRequired);
            }
            (ReportVisuals::Verify(_), false) => {
                return Err(RegistryError::DeclarationOutsideVerify);
            }
        }
        .map_err(RegistryError::InvalidReport)?;
        for id in ids {
            if !visuals.uses().iter().any(|usage| usage.asset == id) {
                return Err(RegistryError::UnreferencedCreation(id));
            }
        }
        candidate.reports.insert(owner, visuals);
        Ok(candidate)
    }

    pub fn report(&self, owner: ReportOwner) -> Option<&ReportVisuals> {
        self.reports.get(&owner)
    }

    /// 등록 ID 자체로 자료를 열지 않는다. 이 Report에 실제 연결된 자료만 반환한다.
    /// 호출자는 별도로 scope가 이 Project를 명시적으로 열었는지 확인해야 한다.
    pub fn asset_for_report(&self, owner: ReportOwner, id: VisualAssetId) -> Option<&VisualAsset> {
        let report = self.reports.get(&owner)?;
        report
            .uses()
            .iter()
            .any(|usage| usage.asset == id)
            .then(|| self.assets.get(&id))
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(cycle: u32, step: u32) -> ReportOwner {
        ReportOwner::Step(StepRef::new(CycleRef::new(cycle).unwrap(), step).unwrap())
    }
    fn metadata() -> NewVisualAsset {
        NewVisualAsset {
            digest: [42; 32],
            image_kind: ImageKind::Png,
            byte_len: NonZeroU64::new(50).unwrap(),
            width: NonZeroU32::new(2).unwrap(),
            height: NonZeroU32::new(2).unwrap(),
            source_kind: SourceKind::Captured,
            source_description: "합성 시험 메타데이터".into(),
        }
    }
    fn usage(id: VisualAssetId) -> VisualUse {
        VisualUse {
            asset: id,
            caption: "화면".into(),
            alt: "계산기".into(),
            purpose: "관측".into(),
        }
    }
    fn id(number: u64) -> VisualAssetId {
        VisualAssetId::new(NonZeroU64::new(number).unwrap())
    }
    fn create(registry: &VisualRegistry, owner: ReportOwner) -> VisualRegistry {
        registry
            .prepare_close(
                owner,
                TargetState::OpenStep(NodeKind::Verify),
                vec![metadata()],
                |ids| {
                    ReportVisuals::Verify(VisualDeclaration::Attached {
                        uses: vec![usage(ids[0])],
                    })
                },
            )
            .unwrap()
    }

    #[test]
    fn preparing_is_not_committing_and_dropping_candidate_does_not_consume_id() {
        let original = VisualRegistry::default();
        let candidate = create(&original, step(1, 3));
        assert_eq!(original, VisualRegistry::default());
        assert!(original.report(step(1, 3)).is_none());
        assert_eq!(candidate, create(&original, step(1, 3)));
        assert!(candidate.asset_for_report(step(1, 3), id(1)).is_some());
    }

    #[test]
    fn failed_report_leaves_no_assets_or_ids() {
        let original = VisualRegistry::default();
        let result = original.prepare_close(
            step(1, 3),
            TargetState::OpenStep(NodeKind::Verify),
            vec![metadata()],
            |_| ReportVisuals::Verify(VisualDeclaration::Attached { uses: vec![] }),
        );
        assert_eq!(
            result,
            Err(RegistryError::InvalidReport(
                VisualValidationError::EmptyAttachments
            ))
        );
        assert_eq!(original, VisualRegistry::default());
    }

    #[test]
    fn verify_creation_can_be_reused_in_outcome_cycle_and_next_define() {
        let origin = step(1, 3);
        let mut registry = create(&VisualRegistry::default(), origin);
        let before = registry.asset_for_report(origin, id(1)).unwrap().clone();
        for (owner, state) in [
            (step(1, 5), TargetState::OpenStep(NodeKind::Outcome)),
            (
                ReportOwner::Cycle(CycleRef::new(1).unwrap()),
                TargetState::OpenCycle,
            ),
            (step(2, 1), TargetState::OpenStep(NodeKind::Define)),
        ] {
            registry = registry
                .prepare_close(owner, state, vec![], |_| {
                    let mut reference = usage(id(1));
                    reference.purpose = "이전 관측 인용".into();
                    ReportVisuals::References(vec![reference])
                })
                .unwrap();
            assert_eq!(registry.asset_for_report(owner, id(1)), Some(&before));
        }
        assert_eq!(registry.asset_for_report(origin, id(1)), Some(&before));
    }

    #[test]
    fn same_bytes_from_new_verify_keep_distinct_provenance() {
        let registry = create(&create(&VisualRegistry::default(), step(1, 3)), step(2, 3));
        let first = registry.asset_for_report(step(1, 3), id(1)).unwrap();
        let second = registry.asset_for_report(step(2, 3), id(2)).unwrap();
        assert_eq!(first.metadata(), second.metadata());
        assert_ne!(first.origin(), second.origin());
        assert!(registry.asset_for_report(step(2, 3), id(1)).is_none());
    }

    #[test]
    fn knowing_id_does_not_grant_unreferenced_report_access() {
        let registry = create(&VisualRegistry::default(), step(1, 3));
        let registry = registry
            .prepare_close(
                step(2, 1),
                TargetState::OpenStep(NodeKind::Define),
                vec![],
                |_| ReportVisuals::References(vec![]),
            )
            .unwrap();
        assert!(registry.asset_for_report(step(2, 1), id(1)).is_none());
        assert!(registry.asset_for_report(step(99, 1), id(1)).is_none());
        assert!(registry.asset_for_report(step(1, 3), id(2)).is_none());
    }

    #[test]
    fn cannot_reopen_recorded_report_even_with_open_state_claim() {
        let registry = create(&VisualRegistry::default(), step(1, 3));
        let before = registry.clone();
        assert_eq!(
            registry.prepare_close(
                step(1, 3),
                TargetState::OpenStep(NodeKind::Verify),
                vec![],
                |_| panic!()
            ),
            Err(RegistryError::ReportAlreadyRecorded)
        );
        assert_eq!(registry, before);
    }

    #[test]
    fn closed_legacy_and_unknown_targets_are_rejected() {
        for state in [TargetState::Closed, TargetState::Unknown] {
            assert_eq!(
                VisualRegistry::default().prepare_close(step(1, 3), state, vec![], |_| panic!()),
                Err(RegistryError::TargetNotOpen)
            );
        }
    }

    #[test]
    fn only_verify_can_create() {
        for kind in NodeKind::ALL {
            if matches!(
                kind,
                NodeKind::Verify | NodeKind::CycleEntry | NodeKind::CycleExit
            ) {
                continue;
            }
            assert_eq!(
                VisualRegistry::default().prepare_close(
                    step(1, 1),
                    TargetState::OpenStep(kind),
                    vec![metadata()],
                    |_| panic!()
                ),
                Err(RegistryError::CreationOutsideVerify)
            );
        }
    }

    #[test]
    fn wrong_owner_kind_and_boundary_nodes_are_rejected() {
        for (owner, state) in [
            (step(1, 1), TargetState::OpenCycle),
            (
                ReportOwner::Cycle(CycleRef::new(1).unwrap()),
                TargetState::OpenStep(NodeKind::Verify),
            ),
            (step(1, 1), TargetState::OpenStep(NodeKind::CycleEntry)),
            (step(1, 1), TargetState::OpenStep(NodeKind::CycleExit)),
        ] {
            assert_eq!(
                VisualRegistry::default().prepare_close(owner, state, vec![], |_| panic!()),
                Err(RegistryError::TargetKindMismatch)
            );
        }
    }

    #[test]
    fn creation_must_be_referenced_and_source_must_be_nonblank() {
        let registry = VisualRegistry::default();
        assert_eq!(
            registry.prepare_close(
                step(1, 3),
                TargetState::OpenStep(NodeKind::Verify),
                vec![metadata()],
                |_| ReportVisuals::Verify(VisualDeclaration::NotNeeded {
                    reason: "없음".into()
                })
            ),
            Err(RegistryError::UnreferencedCreation(id(1)))
        );
        let mut blank = metadata();
        blank.source_description = "\n ".into();
        assert_eq!(
            registry.prepare_close(
                step(1, 3),
                TargetState::OpenStep(NodeKind::Verify),
                vec![blank],
                |_| panic!()
            ),
            Err(RegistryError::BlankSource)
        );
    }

    #[test]
    fn id_exhaustion_is_atomic() {
        let registry = VisualRegistry {
            next_id: u64::MAX,
            ..VisualRegistry::default()
        };
        let before = registry.clone();
        assert_eq!(
            registry.prepare_close(
                step(1, 3),
                TargetState::OpenStep(NodeKind::Verify),
                vec![metadata()],
                |_| panic!()
            ),
            Err(RegistryError::IdExhausted)
        );
        assert_eq!(registry, before);
    }

    #[test]
    fn declarations_belong_to_verify() {
        let registry = VisualRegistry::default();
        assert_eq!(
            registry.prepare_close(
                step(1, 3),
                TargetState::OpenStep(NodeKind::Verify),
                vec![],
                |_| ReportVisuals::References(vec![])
            ),
            Err(RegistryError::VerifyDeclarationRequired)
        );
        assert_eq!(
            registry.prepare_close(
                step(1, 1),
                TargetState::OpenStep(NodeKind::Define),
                vec![],
                |_| ReportVisuals::Verify(VisualDeclaration::NotNeeded {
                    reason: "자료 없음".into()
                })
            ),
            Err(RegistryError::DeclarationOutsideVerify)
        );
    }

    #[test]
    fn cycle_report_can_reference_but_cannot_create_assets() {
        let owner = ReportOwner::Cycle(CycleRef::new(1).unwrap());
        assert_eq!(
            VisualRegistry::default().prepare_close(
                owner,
                TargetState::OpenCycle,
                vec![metadata()],
                |_| panic!()
            ),
            Err(RegistryError::CreationOutsideVerify)
        );
    }

    #[test]
    fn later_bad_creation_does_not_leave_earlier_creation_registered() {
        let original = VisualRegistry::default();
        let mut bad = metadata();
        bad.source_description.clear();
        assert_eq!(
            original.prepare_close(
                step(1, 3),
                TargetState::OpenStep(NodeKind::Verify),
                vec![metadata(), bad],
                |_| panic!()
            ),
            Err(RegistryError::BlankSource)
        );
        assert_eq!(original, VisualRegistry::default());
        assert!(
            create(&original, step(1, 3))
                .asset_for_report(step(1, 3), id(1))
                .is_some()
        );
    }

    #[test]
    fn unknown_reference_and_partial_missing_are_distinct() {
        let original = VisualRegistry::default();
        assert_eq!(
            original.prepare_close(
                step(1, 1),
                TargetState::OpenStep(NodeKind::Define),
                vec![],
                |_| ReportVisuals::References(vec![usage(id(99))])
            ),
            Err(RegistryError::InvalidReport(
                VisualValidationError::UnavailableAsset { asset: id(99) }
            ))
        );
        let candidate = original
            .prepare_close(
                step(1, 3),
                TargetState::OpenStep(NodeKind::Verify),
                vec![metadata()],
                |ids| {
                    ReportVisuals::Verify(VisualDeclaration::Missing {
                        reason: "두 번째 화면 캡처 누락".into(),
                        follow_up: "후속 실험에서 생성".into(),
                        uses: vec![usage(ids[0])],
                    })
                },
            )
            .unwrap();
        assert!(candidate.asset_for_report(step(1, 3), id(1)).is_some());
        assert!(matches!(
            candidate.report(step(1, 3)),
            Some(ReportVisuals::Verify(VisualDeclaration::Missing { .. }))
        ));
    }

    #[test]
    fn not_needed_verify_records_no_asset_but_is_still_immutable() {
        let owner = step(1, 3);
        let registry = VisualRegistry::default()
            .prepare_close(
                owner,
                TargetState::OpenStep(NodeKind::Verify),
                vec![],
                |_| {
                    ReportVisuals::Verify(VisualDeclaration::NotNeeded {
                        reason: "문자열 동일성 시험".into(),
                    })
                },
            )
            .unwrap();
        assert!(registry.asset_for_report(owner, id(1)).is_none());
        assert_eq!(
            registry.prepare_close(
                owner,
                TargetState::OpenStep(NodeKind::Verify),
                vec![metadata()],
                |_| panic!()
            ),
            Err(RegistryError::ReportAlreadyRecorded)
        );
    }
}
