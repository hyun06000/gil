//! Opt-in development surface. No migration, no UI writes, no implicit attachment parsing.
use crate::visual_report::{
    VisualAssetId, VisualDeclaration, VisualUse,
    image::VisualAttachment,
    registry::{ReportOwner, SourceKind},
};
use crate::{Project, ProjectSession, Report, RuleSet};
use rmcp::model::{CallToolResult, Tool};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, num::NonZeroU64, path::Path};

#[derive(Clone, Deserialize)]
#[serde(
    tag = "kind",
    content = "ref",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum Owner {
    Step(String),
    Cycle(String),
}
impl Owner {
    fn parsed(&self) -> Result<ReportOwner, &'static str> {
        match self {
            Self::Step(s) => {
                let r: crate::StepRef = s.parse().map_err(|_| "invalid_owner")?;
                if r.to_string() != *s {
                    return Err("invalid_owner");
                }
                Ok(ReportOwner::Step(r))
            }
            Self::Cycle(s) => {
                let r: crate::CycleRef = s.parse().map_err(|_| "invalid_owner")?;
                if r.to_string() != *s {
                    return Err("invalid_owner");
                }
                Ok(ReportOwner::Cycle(r))
            }
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Read {
    project_root: String,
    owner: Owner,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Close {
    project_root: String,
    report: String,
    expected_owner: Owner,
    visuals: Visuals,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Visuals {
    attachments: Vec<Attachment>,
    uses: Vec<Usage>,
    declaration: Option<Declaration>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Attachment {
    key: String,
    path: String,
    source_kind: Source,
    source_description: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Source {
    Captured,
    Generated,
    Provided,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Usage {
    source: Reference,
    caption: String,
    alt: String,
    purpose: String,
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Reference {
    New {
        key: String,
    },
    Stored {
        owner: Owner,
        asset_id: String,
        sha256: String,
    },
}
#[derive(Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
enum Declaration {
    Attached,
    NotNeeded { reason: String },
    Missing { reason: String, follow_up: String },
}
fn text(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 4096 && !s.contains('\0')
}
fn id(s: &str) -> Option<VisualAssetId> {
    let n: NonZeroU64 = s.parse().ok()?;
    (n.to_string() == s).then(|| VisualAssetId::new(n))
}
fn hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
impl Close {
    fn valid(&self) -> bool {
        if !Path::new(&self.project_root).is_absolute()
            || self.report.len() > 262144
            || self.expected_owner.parsed().is_err()
            || self.visuals.attachments.len() > 8
            || self.visuals.uses.len() > 8
        {
            return false;
        }
        let mut keys = BTreeSet::new();
        for a in &self.visuals.attachments {
            if !text(&a.key)
                || !text(&a.path)
                || !text(&a.source_description)
                || !keys.insert(a.key.as_str())
            {
                return false;
            }
        }
        let mut used = BTreeSet::new();
        for u in &self.visuals.uses {
            if !text(&u.caption) || !text(&u.alt) || !text(&u.purpose) {
                return false;
            }
            match &u.source {
                Reference::New { key } => {
                    if !keys.contains(key.as_str()) || !used.insert(key.as_str()) {
                        return false;
                    }
                }
                Reference::Stored {
                    owner,
                    asset_id,
                    sha256,
                } => {
                    if owner.parsed().is_err() || id(asset_id).is_none() || !hash(sha256) {
                        return false;
                    }
                }
            }
        }
        if used != keys {
            return false;
        } // No orphan creation hidden from its own Report.
        match &self.visuals.declaration {
            Some(Declaration::NotNeeded { reason }) => text(reason) && self.visuals.uses.is_empty(),
            Some(Declaration::Missing { reason, follow_up }) => text(reason) && text(follow_up),
            Some(Declaration::Attached) => !self.visuals.uses.is_empty(),
            None => self.visuals.attachments.is_empty(),
        }
    }
}
pub fn valid(name: &str, input: &Value) -> bool {
    if serde_json::to_vec(input).map_or(true, |v| v.len() > 512 * 1024) {
        return false;
    }
    if input
        .pointer("/visuals/declaration")
        .is_some_and(Value::is_null)
    {
        return false;
    }
    if name == "gil_report_visuals" {
        serde_json::from_value::<Read>(input.clone())
            .is_ok_and(|r| Path::new(&r.project_root).is_absolute() && r.owner.parsed().is_ok())
    } else {
        serde_json::from_value::<Close>(input.clone()).is_ok_and(|r| r.valid())
    }
}
fn fail(code: &str) -> CallToolResult {
    let mut result = CallToolResult::structured(json!({"ok":false,"code":code,
        "said":"시각 보고서 요청을 완료하지 못했다. gil_report_visuals로 예상 보고서의 확정 여부를 확인한다. close를 자동 반복하지 않는다."}));
    result.is_error = Some(true);
    result
}
fn projection(p: &Project, owner: ReportOwner) -> Result<Value, &'static str> {
    let view = p.report_visuals_v1(owner).map_err(|_| "report_not_found")?;
    let cycle_ref = match owner {
        ReportOwner::Step(r) => r.cycle(),
        ReportOwner::Cycle(r) => r,
    };
    let c = p
        .cycles()
        .nodes()
        .iter()
        .find(|c| c.id().to_ref() == cycle_ref)
        .ok_or("report_not_found")?;
    let report = match owner {
        ReportOwner::Step(r) => c
            .steps()
            .nodes()
            .iter()
            .find(|n| c.step_ref(n.id) == r)
            .and_then(|n| n.report.as_ref()),
        ReportOwner::Cycle(_) => c.report(),
    };
    let digest = report.map(|r| {
        let fields: std::collections::BTreeMap<_, _> =
            r.field_names().map(|k| (k, r.get(k).unwrap())).collect();
        format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&fields).expect("string fields"))
        )
    });
    Ok(
        json!({"ok":true,"confirmed":report.is_some(),"report_sha256":digest,"visuals":view,
        "display_confirmed":false}),
    )
}
pub fn call(name: &str, input: Value) -> CallToolResult {
    if !valid(name, &input) {
        return fail("invalid_request");
    }
    match execute(name, input) {
        Ok(value) => super::visuals::bounded(CallToolResult::structured(value)),
        Err(code) => fail(code),
    }
}
fn execute(name: &str, input: Value) -> Result<Value, &'static str> {
    let rules = RuleSet::builtin().map_err(|_| "rules_unavailable")?;
    if name == "gil_report_visuals" {
        let r: Read = serde_json::from_value(input).map_err(|_| "invalid_request")?;
        let session = ProjectSession::open_read_only(
            rules,
            Path::new(&r.project_root).join(crate::STATE_PATH),
        )
        .map_err(|_| "confirmation_unavailable")?;
        return projection(session.project(), r.owner.parsed()?);
    }
    let r: Close = serde_json::from_value(input).map_err(|_| "invalid_request")?;
    let owner = r.expected_owner.parsed()?;
    let mut session = crate::open_project_root(rules, Path::new(&r.project_root))
        .map_err(|_| "project_unavailable")?;
    if session.project().visual_state().is_none() {
        return Err("visual_upgrade_required");
    }
    let c = session.project().current_cycle();
    let actual = match c.step_now_open() {
        Some(n) => ReportOwner::Step(c.step_ref(n)),
        None => ReportOwner::Cycle(c.id().to_ref()),
    };
    if owner != actual || projection(session.project(), owner)?["confirmed"] == true {
        return Err("owner_mismatch");
    }
    let is_verify = c
        .step_now_open()
        .and_then(|n| c.steps().node(n))
        .is_some_and(|n| n.kind == crate::NodeKind::Verify);
    if is_verify != r.visuals.declaration.is_some()
        || (!is_verify && !r.visuals.attachments.is_empty())
    {
        return Err("visual_stage_mismatch");
    }
    // Resolve only references already linked to a Report in this exact Project.
    let mut resolved = Vec::new();
    for u in &r.visuals.uses {
        resolved.push(match &u.source {
            Reference::New { .. } => None,
            Reference::Stored {
                owner,
                asset_id,
                sha256,
            } => {
                let asset = id(asset_id).ok_or("invalid_asset")?;
                let stored = session
                    .read_report_image(owner.parsed()?, asset)
                    .map_err(|_| "visual_unavailable")?;
                if format!("{:x}", Sha256::digest(stored.bytes())) != *sha256 {
                    return Err("visual_unavailable");
                }
                Some(asset)
            }
        });
    }
    let make_uses = |ids: &[VisualAssetId]| {
        r.visuals
            .uses
            .iter()
            .zip(&resolved)
            .map(|(u, stored)| {
                let asset = match &u.source {
                    Reference::New { key } => {
                        ids[r
                            .visuals
                            .attachments
                            .iter()
                            .position(|a| a.key == *key)
                            .expect("validated key")]
                    }
                    Reference::Stored { .. } => stored.expect("resolved asset"),
                };
                VisualUse {
                    asset,
                    caption: u.caption.clone(),
                    alt: u.alt.clone(),
                    purpose: u.purpose.clone(),
                }
            })
            .collect::<Vec<_>>()
    };
    let report = Report::parse(&r.report).map_err(|_| "invalid_report")?;
    if let Some(declaration) = &r.visuals.declaration {
        let attachments = r
            .visuals
            .attachments
            .iter()
            .map(|a| VisualAttachment {
                path: a.path.clone(),
                source_kind: match a.source_kind {
                    Source::Captured => SourceKind::Captured,
                    Source::Generated => SourceKind::Generated,
                    Source::Provided => SourceKind::Provided,
                },
                source_description: a.source_description.clone(),
            })
            .collect();
        session
            .close_verify_with_images_and_commit(report, attachments, |ids| match declaration {
                Declaration::Attached => VisualDeclaration::Attached {
                    uses: make_uses(ids),
                },
                Declaration::NotNeeded { reason } => VisualDeclaration::NotNeeded {
                    reason: reason.clone(),
                },
                Declaration::Missing { reason, follow_up } => VisualDeclaration::Missing {
                    reason: reason.clone(),
                    follow_up: follow_up.clone(),
                    uses: make_uses(ids),
                },
            })
            .map_err(|_| "close_not_confirmed")?;
    } else {
        session
            .close_references_and_commit(report, make_uses(&[]))
            .map_err(|_| "close_not_confirmed")?;
    }
    projection(session.project(), owner)
}

pub fn catalog(mut tools: Vec<Tool>) -> Vec<Tool> {
    let contract: Value =
        serde_json::from_str(include_str!("authoring-schema.json")).expect("authoring schema");
    let close = tools
        .iter_mut()
        .find(|t| t.name == "gil_close")
        .expect("close");
    let mut raw = serde_json::to_value(&*close).unwrap();
    raw["inputSchema"] = contract["close"].clone();
    // Legacy receipts remain unchanged; typed close returns its durable Report projection.
    raw.as_object_mut().unwrap().remove("outputSchema");
    raw["description"] = json!(
        "Close the current Report. Development visual-authoring accepts typed visuals and expected_owner together, only on explicit format-5 projects. After an uncertain response use gil_report_visuals; never automatically repeat close. No migration."
    );
    *close = serde_json::from_value(raw).unwrap();
    tools.push(serde_json::from_value(json!({"name":"gil_report_visuals",
        "description":"Read confirmation, canonical report-field digest and visual reference metadata for one Report in an explicit project. No image bytes, file changes or recovery. Use after an uncertain visual close and to reuse stored assets; confirmed does not prove a particular caller performed the close or that a screen was displayed.",
        "inputSchema":contract["read"],"annotations":{"readOnlyHint":true,"destructiveHint":false,"openWorldHint":false},
        "_meta":{"ui":{"visibility":["model"]}}})).unwrap());
    tools
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_typed_inputs_refuse_ambiguous_or_unbounded_authoring() {
        let good = json!({"project_root":"/synthetic","report":"execution: test","expected_owner":{"kind":"step","ref":"step:C2/S3"},
            "visuals":{"attachments":[],"uses":[],"declaration":{"state":"not_needed","reason":"numbers only"}}});
        assert!(valid("gil_close", &good));
        for (path, value) in [
            ("/visuals/declaration", Value::Null),
            (
                "/visuals/declaration",
                json!({"state":"not_needed","reason":" "}),
            ),
            (
                "/visuals/declaration",
                json!({"state":"missing","reason":"absent"}),
            ),
            ("/visuals/declaration", json!({"state":"attached"})),
            ("/expected_owner/ref", json!("step:C02/S3")),
            ("/project_root", json!("relative")),
            ("/report", json!("x".repeat(262145))),
        ] {
            let mut bad = good.clone();
            *bad.pointer_mut(path).unwrap() = value;
            assert!(!valid("gil_close", &bad), "{path}");
        }
        let mut bad = good.clone();
        bad["expected_owner"]["path"] = json!("unexpected");
        assert!(!valid("gil_close", &bad));
        let mut bad = good.clone();
        bad["visuals"]["attachments"] = json!([{"key":"orphan","path":"a.png","source_kind":"generated","source_description":"test"}]);
        assert!(!valid("gil_close", &bad));
    }
}
