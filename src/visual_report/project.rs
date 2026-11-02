//! 실제 Project Graph adapter와 시각 Report 상태의 원자적 close.
//! 이관 후보는 메모리에서만 만든다. backup/잠금/사용자 이관 명령은 별도 계층의 책임이다.

use std::collections::BTreeMap;

use super::VisualAssetId;
use super::registry::codec::{ClosedReportKind, VisualGraph};
use super::registry::{
    NewVisualAsset, RegistryError, ReportOwner, ReportVisuals, TargetState, VisualRegistry,
};
use crate::{ExistenceRef, JourneyRef, NodeKind, Project, StepRef};

/// 이 시점까지 닫힌 Verify만 구형 미기재로 취급한다. 열린 Verify는 여기에 포함되지 않는다.
/// 저장된 경계는 복원 시 검사하며 일반 호출자는 임의 heads를 만들 수 없다.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MigrationBoundary {
    pub(crate) heads: BTreeMap<ExistenceRef, u32>,
}

#[derive(Debug, Clone)]
pub struct VisualState {
    pub(crate) boundary: MigrationBoundary,
    pub(crate) registry: VisualRegistry,
}

impl VisualState {
    pub fn registry(&self) -> &VisualRegistry {
        &self.registry
    }
}

impl Project {
    pub fn visual_state(&self) -> Option<&VisualState> {
        self.visual_state.as_ref()
    }

    /// 명시적 opt-in 후보만 만든다. 백업·잠금·디스크 이관은 호출 계층의 별도 책임이다.
    /// 아직 사용자 CLI/MCP에 노출하지 않는다.
    pub fn prepare_visual_upgrade(&self) -> Result<Self, crate::ActionError> {
        if self.visual_state.is_some() {
            return Err(crate::ActionError::Visual(
                "이미 시각 Report 형식이다 — 이관 경계를 다시 잡지 않는다".into(),
            ));
        }
        let mut next = self.clone();
        next.visual_state = Some(VisualState {
            boundary: MigrationBoundary::capture(self),
            registry: VisualRegistry::default(),
        });
        Ok(next)
    }

    pub(crate) fn prepare_visual_close(
        &self,
        owner: ReportOwner,
        report: Option<ReportVisuals>,
    ) -> Result<Option<VisualRegistry>, crate::ActionError> {
        let Some(state) = &self.visual_state else {
            if report.is_some() {
                return Err(crate::ActionError::Visual(
                    "먼저 명시적 시각 Report 이관이 필요하다".into(),
                ));
            }
            return Ok(None);
        };
        let graph = ProjectVisualGraph::new(self, &state.boundary)
            .map_err(|err| crate::ActionError::Visual(format!("{err:?}")))?;
        let report = match report {
            Some(report) => report,
            None if graph.target_state(owner) == TargetState::OpenStep(NodeKind::Verify) => {
                return Err(crate::ActionError::Visual(
                    "Verify의 시각 자료 선언이 필요하다 — 첨부 또는 없음과 이유를 적는다".into(),
                ));
            }
            None => ReportVisuals::References(vec![]),
        };
        self.require_verify_citation(&state.registry, owner, &report)?;
        graph
            .prepare_close(&state.registry, owner, vec![], |_| report)
            .map(Some)
            .map_err(|err| crate::ActionError::Visual(err.to_string()))
    }

    /// Analysis 와 Experiment Cycle Report 는 **그 Cycle 의 Verify 가 첨부한 시각 자료**를
    /// 적어도 하나 인용해야 한다. 해석과 인계 문서가 증거를 가리키지 않으면 Monitor 의 그
    /// 노드는 그림 없는 글만 남기고, 다음 협력자는 Verify 노드를 따로 뒤져야 한다.
    ///
    /// Verify 가 아무것도 첨부하지 않았으면(`not_needed`·`missing` 의 빈 uses·구형 미기재)
    /// 요구하지 않는다. Outcome 은 인용할 수 있으나 요구하지 않는다 — 판정문은 짧고, 인계는
    /// Cycle Report 가 맡는다. 새 쓰기에만 적용되며 과거 기록은 다시 재지 않는다.
    fn require_verify_citation(
        &self,
        registry: &VisualRegistry,
        owner: ReportOwner,
        report: &ReportVisuals,
    ) -> Result<(), crate::ActionError> {
        let Some((verify, assets, subject)) = self.verify_citation_sources(registry, owner) else {
            return Ok(());
        };
        let cited = report
            .uses()
            .iter()
            .any(|usage| assets.iter().any(|(id, _)| *id == usage.asset));
        if cited {
            return Ok(());
        }
        let listed = assets
            .iter()
            .map(|(id, digest)| {
                let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
                format!("asset_id \"{}\" sha256 \"{hex}\"", id.0)
            })
            .collect::<Vec<_>>()
            .join(" · ");
        Err(crate::ActionError::Visual(format!(
            "[visual_citation_required] {subject}는 이 Cycle의 Verify {verify}가 첨부한 시각 자료를 \
             적어도 하나 인용해야 한다. 닫힌 것은 없다. 같은 gil_close를 expected_owner와 visuals를 \
             함께 넣어 다시 보낸다: attachments는 빈 배열, declaration은 생략, uses의 각 항목은 \
             source {{\"kind\":\"stored\",\"owner\":{{\"kind\":\"step\",\"ref\":\"{verify}\"}},\
             \"asset_id\":…,\"sha256\":…}}와 이 보고서의 caption·alt·purpose다. 인용 가능한 자료: {listed}"
        )))
    }

    /// 인용 대상이 있는 자리인가 — (Verify 주소, 그 Verify 가 첨부한 자료, 이 보고서의 이름).
    fn verify_citation_sources(
        &self,
        registry: &VisualRegistry,
        owner: ReportOwner,
    ) -> Option<(StepRef, Vec<(VisualAssetId, [u8; 32])>, &'static str)> {
        let cycle_ref = match owner {
            ReportOwner::Step(step) => step.cycle(),
            ReportOwner::Cycle(cycle) => cycle,
        };
        let cycle = self
            .all_cycles()
            .find(|cycle| cycle.id().to_ref() == cycle_ref)?;
        if cycle.kind() != crate::CycleKind::Experiment {
            return None;
        }
        let subject = match owner {
            ReportOwner::Cycle(_) => "Experiment Cycle Report",
            ReportOwner::Step(step) => {
                let node = cycle
                    .steps()
                    .nodes()
                    .iter()
                    .find(|node| cycle.step_ref(node.id) == step)?;
                if node.kind != NodeKind::Analysis {
                    return None;
                }
                "Analysis"
            }
        };
        // 이 Cycle 에서 **가장 최근에 닫힌** Verify 가운데 실제로 자료를 첨부한 것.
        let verify = cycle
            .steps()
            .nodes()
            .iter()
            .rev()
            .filter(|node| node.kind == NodeKind::Verify && node.is_closed())
            .map(|node| cycle.step_ref(node.id))
            .find_map(|step| {
                let visuals = registry.report(ReportOwner::Step(step))?;
                let uses = match visuals {
                    ReportVisuals::Verify(declaration) => declaration.uses(),
                    ReportVisuals::References(_) => return None,
                };
                if uses.is_empty() {
                    return None;
                }
                let assets = uses
                    .iter()
                    .filter_map(|usage| {
                        let asset = registry.asset_for_report(ReportOwner::Step(step), usage.asset)?;
                        Some((usage.asset, asset.metadata().digest))
                    })
                    .collect::<Vec<_>>();
                Some((step, assets))
            })?;
        Some((verify.0, verify.1, subject))
    }

    pub(crate) fn apply_visual_close(
        &mut self,
        registry: Option<VisualRegistry>,
    ) -> Result<(), crate::ActionError> {
        if let Some(registry) = registry {
            let state = self
                .visual_state
                .as_ref()
                .expect("후보는 활성 형식에서만 만든다");
            let graph = ProjectVisualGraph::new(self, &state.boundary)
                .map_err(|err| crate::ActionError::Visual(format!("{err:?}")))?;
            super::registry::codec::encode(&registry, &graph)
                .map_err(|err| crate::ActionError::Visual(format!("{err:?}")))?;
            self.visual_state.as_mut().expect("방금 검사했다").registry = registry;
        }
        Ok(())
    }
}

impl MigrationBoundary {
    pub fn capture(project: &Project) -> Self {
        let (_, existences, _, _) = project.parts();
        Self {
            heads: existences
                .values()
                .map(|existence| {
                    let journey = existence.current_journey();
                    (journey.existence(), journey.revision())
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryError {
    UnknownExistence,
    BeyondCurrentJourney,
}

pub struct ProjectVisualGraph<'a> {
    project: &'a Project,
    boundary: &'a MigrationBoundary,
}

impl<'a> ProjectVisualGraph<'a> {
    pub fn new(
        project: &'a Project,
        boundary: &'a MigrationBoundary,
    ) -> Result<Self, BoundaryError> {
        let (_, existences, _, _) = project.parts();
        for (owner, revision) in &boundary.heads {
            let current = existences
                .values()
                .map(|e| e.current_journey())
                .find(|journey| journey.existence() == *owner)
                .ok_or(BoundaryError::UnknownExistence)?;
            if *revision > current.revision() {
                return Err(BoundaryError::BeyondCurrentJourney);
            }
        }
        Ok(Self { project, boundary })
    }

    /// 현재 Project의 실제 상태를 사용한다. 호출자가 Step kind를 선언해서 우회하지 못한다.
    /// 후보는 실제 Project close와 함께 commit되기 전까지 저장된 사실이 아니다.
    pub fn prepare_close(
        &self,
        registry: &VisualRegistry,
        owner: ReportOwner,
        creations: Vec<NewVisualAsset>,
        report: impl FnOnce(&[VisualAssetId]) -> ReportVisuals,
    ) -> Result<VisualRegistry, RegistryError> {
        registry.prepare_close(owner, self.target_state(owner), creations, report)
    }

    pub fn target_state(&self, owner: ReportOwner) -> TargetState {
        let cycle_ref = match owner {
            ReportOwner::Step(step) => step.cycle(),
            ReportOwner::Cycle(cycle) => cycle,
        };
        let Some(cycle) = self
            .project
            .all_cycles()
            .find(|cycle| cycle.id().to_ref() == cycle_ref)
        else {
            return TargetState::Unknown;
        };
        match owner {
            ReportOwner::Cycle(_) => {
                if cycle.is_closed() {
                    TargetState::Closed
                } else {
                    TargetState::OpenCycle
                }
            }
            ReportOwner::Step(step) => {
                let Some(node) = cycle
                    .steps()
                    .nodes()
                    .iter()
                    .find(|node| cycle.step_ref(node.id) == step)
                else {
                    return TargetState::Unknown;
                };
                if node.is_closed() {
                    TargetState::Closed
                } else {
                    TargetState::OpenStep(node.kind)
                }
            }
        }
    }

    fn closed_fact(&self, owner: ReportOwner) -> Option<(ClosedReportKind, JourneyRef)> {
        let cycle_ref = match owner {
            ReportOwner::Step(step) => step.cycle(),
            ReportOwner::Cycle(cycle) => cycle,
        };
        let cycle = self
            .project
            .all_cycles()
            .find(|cycle| cycle.id().to_ref() == cycle_ref)?;
        match owner {
            ReportOwner::Cycle(_) => {
                if !cycle.is_closed() || cycle.report().is_none() {
                    return None;
                }
                Some((ClosedReportKind::Cycle, cycle.journey()?))
            }
            ReportOwner::Step(step) => {
                let node = cycle
                    .steps()
                    .nodes()
                    .iter()
                    .find(|node| cycle.step_ref(node.id) == step)?;
                if !node.is_closed()
                    || node.report.is_none()
                    || matches!(node.kind, NodeKind::CycleEntry | NodeKind::CycleExit)
                {
                    return None;
                }
                Some((ClosedReportKind::Step(node.kind), node.journey?))
            }
        }
    }
}

impl VisualGraph for ProjectVisualGraph<'_> {
    fn closed_report_kind(&self, owner: ReportOwner) -> Option<ClosedReportKind> {
        self.closed_fact(owner).map(|(kind, _)| kind)
    }

    fn required_verify_reports(&self) -> Vec<StepRef> {
        self.project
            .all_cycles()
            .flat_map(|cycle| {
                cycle.steps().nodes().iter().filter_map(|node| {
                    if node.kind != NodeKind::Verify || !node.is_closed() {
                        return None;
                    }
                    let journey = node.journey?;
                    let cutoff = self
                        .boundary
                        .heads
                        .get(&journey.existence())
                        .copied()
                        .unwrap_or(0);
                    (journey.revision() > cutoff).then(|| cycle.step_ref(node.id))
                })
            })
            .collect()
    }

    fn reference_was_available(&self, origin: StepRef, consumer: ReportOwner) -> bool {
        let Some((ClosedReportKind::Step(NodeKind::Verify), produced)) =
            self.closed_fact(ReportOwner::Step(origin))
        else {
            return false;
        };
        let Some((_, consumed)) = self.closed_fact(consumer) else {
            return false;
        };
        // 서로 다른 Existence의 Journey 번호는 비교할 수 없다. 전환 계약이 생기기 전 fail closed.
        produced.existence() == consumed.existence()
            && (consumer == ReportOwner::Step(origin) || produced.revision() < consumed.revision())
    }
}
