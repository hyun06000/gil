//! Read-only Report projection. No file reads, image decoding, scope authority or wire tool registration.
use serde::Serialize;

use super::VisualDeclaration;
use super::registry::{ImageKind, ReportOwner, ReportVisuals, SourceKind};
use crate::{NodeKind, Project};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "ref", rename_all = "snake_case")]
pub enum ReportOwnerV1 {
    Step(String),
    Cycle(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum VisualStateV1 {
    OpenReport,
    LegacyUnspecified,
    NotNeeded { reason: String },
    Missing { reason: String, follow_up: String },
    Attached,
    References,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VisualUseV1 {
    pub asset_id: String,
    pub caption: String,
    pub alt: String,
    pub purpose: String,
    pub origin_step_ref: String,
    pub source_kind: String,
    pub source_description: String,
    pub mime_type: String,
    pub byte_length: u64,
    pub width: u32,
    pub height: u32,
    pub sha256: String,
}

/// Local DTO; the future MCP layer must add/validate scope and bound the complete response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReportVisualsV1 {
    pub schema_version: u32,
    pub owner: ReportOwnerV1,
    #[serde(flatten)]
    pub state: VisualStateV1,
    pub uses: Vec<VisualUseV1>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisualDetailError {
    NotFound,
    InvalidReference,
}

impl Project {
    pub fn report_visuals_v1(
        &self,
        owner: ReportOwner,
    ) -> Result<ReportVisualsV1, VisualDetailError> {
        let cycle_ref = match owner {
            ReportOwner::Step(step) => step.cycle(),
            ReportOwner::Cycle(cycle) => cycle,
        };
        let cycle = self
            .cycles()
            .nodes()
            .iter()
            .find(|c| c.id().to_ref() == cycle_ref)
            .ok_or(VisualDetailError::NotFound)?;
        let (owner_dto, has_report) = match owner {
            ReportOwner::Cycle(reference) => (
                ReportOwnerV1::Cycle(reference.to_string()),
                cycle.report().is_some(),
            ),
            ReportOwner::Step(reference) => {
                let node = cycle
                    .steps()
                    .nodes()
                    .iter()
                    .find(|n| cycle.step_ref(n.id) == reference)
                    .ok_or(VisualDetailError::NotFound)?;
                if matches!(node.kind, NodeKind::CycleEntry | NodeKind::CycleExit) {
                    return Err(VisualDetailError::NotFound);
                }
                (
                    ReportOwnerV1::Step(reference.to_string()),
                    node.report.is_some(),
                )
            }
        };
        let mut result = ReportVisualsV1 {
            schema_version: 1,
            owner: owner_dto,
            state: VisualStateV1::OpenReport,
            uses: vec![],
        };
        if !has_report {
            return Ok(result);
        }
        result.state = VisualStateV1::LegacyUnspecified;
        let Some(registry) = self.visual_state().map(|s| s.registry()) else {
            return Ok(result);
        };
        let Some(visuals) = registry.report(owner) else {
            return Ok(result);
        };
        result.state = match visuals {
            ReportVisuals::References(_) => VisualStateV1::References,
            ReportVisuals::Verify(VisualDeclaration::Attached { .. }) => VisualStateV1::Attached,
            ReportVisuals::Verify(VisualDeclaration::NotNeeded { reason }) => {
                VisualStateV1::NotNeeded {
                    reason: reason.clone(),
                }
            }
            ReportVisuals::Verify(VisualDeclaration::Missing {
                reason, follow_up, ..
            }) => VisualStateV1::Missing {
                reason: reason.clone(),
                follow_up: follow_up.clone(),
            },
        };
        result.uses = visuals
            .uses()
            .iter()
            .map(|usage| {
                let asset = registry
                    .asset_for_report(owner, usage.asset)
                    .ok_or(VisualDetailError::InvalidReference)?;
                let meta = asset.metadata();
                Ok(VisualUseV1 {
                    asset_id: usage.asset.0.to_string(),
                    caption: usage.caption.clone(),
                    alt: usage.alt.clone(),
                    purpose: usage.purpose.clone(),
                    origin_step_ref: asset.origin().to_string(),
                    source_kind: match meta.source_kind {
                        SourceKind::Captured => "captured",
                        SourceKind::Generated => "generated",
                        SourceKind::Provided => "provided",
                    }
                    .into(),
                    source_description: meta.source_description.clone(),
                    mime_type: match meta.image_kind {
                        ImageKind::Png => "image/png",
                        ImageKind::Jpeg => "image/jpeg",
                    }
                    .into(),
                    byte_length: meta.byte_len.get(),
                    width: meta.width.get(),
                    height: meta.height.get(),
                    sha256: meta
                        .digest
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect(),
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(result)
    }
}
