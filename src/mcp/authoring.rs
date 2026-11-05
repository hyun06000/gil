//! Opt-in development surface. No migration, no UI writes, no implicit attachment parsing.
mod diagnostics;

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
    Attached {},
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
    fn valid(&self) -> bool { self.problem().is_none() }
    fn problem(&self) -> Option<String> {
        let fail = |path: &str, why: &str| Some(format!("{path}: {why}"));
        if !Path::new(&self.project_root).is_absolute() { return fail("$.project_root", "expected absolute path"); }
        if self.report.len() > 262144 { return fail("$.report", "maximum 256 KiB"); }
        if self.expected_owner.parsed().is_err() { return fail("$.expected_owner.ref", "expected canonical Step or Cycle ref; use the actual open owner from gil_status"); }
        if self.visuals.attachments.len() > 8 { return fail("$.visuals.attachments", "maximum 8 entries"); }
        if self.visuals.uses.len() > 8 { return fail("$.visuals.uses", "maximum 8 entries"); }
        let mut keys = BTreeSet::new();
        for (i, a) in self.visuals.attachments.iter().enumerate() {
            for (name, value) in [("key", &a.key), ("path", &a.path), ("source_description", &a.source_description)] {
                if !text(value) { return fail(&format!("$.visuals.attachments[{i}].{name}"), "expected nonblank text, at most 4096 UTF-8 bytes, no NUL"); }
            }
            if !keys.insert(a.key.as_str()) { return fail(&format!("$.visuals.attachments[{i}].key"), "duplicate attachment key"); }
        }
        let mut used = BTreeSet::new();
        for (i, u) in self.visuals.uses.iter().enumerate() {
            for (name, value) in [("caption", &u.caption), ("alt", &u.alt), ("purpose", &u.purpose)] {
                if !text(value) { return fail(&format!("$.visuals.uses[{i}].{name}"), "expected nonblank text, at most 4096 UTF-8 bytes, no NUL"); }
            }
            let path = format!("$.visuals.uses[{i}].source");
            match &u.source {
                Reference::New { key } => {
                    if !keys.contains(key.as_str()) || !used.insert(key.as_str()) { return fail(&format!("{path}.key"), "must reference one attachment key exactly once"); }
                }
                Reference::Stored { owner, asset_id, sha256 } => {
                    if owner.parsed().is_err() { return fail(&format!("{path}.owner.ref"), "expected canonical owner from the stored receipt"); }
                    if id(asset_id).is_none() { return fail(&format!("{path}.asset_id"), "expected canonical positive integer string from the receipt"); }
                    if !hash(sha256) { return fail(&format!("{path}.sha256"), "expected 64 lowercase hexadecimal characters from the receipt"); }
                }
            }
        }
        if used != keys { return fail("$.visuals.uses", "every new attachment needs exactly one source {kind: new, key}; stored does not consume an attachment key"); }
        match &self.visuals.declaration {
            Some(Declaration::NotNeeded { reason }) => {
                if !text(reason) { return fail("$.visuals.declaration.reason", "expected nonblank text, at most 4096 UTF-8 bytes, no NUL"); }
                if !self.visuals.uses.is_empty() { return fail("$.visuals.uses", "not_needed requires no uses; never change attached to not_needed to bypass an error"); }
            }
            Some(Declaration::Missing { reason, follow_up }) => {
                for (name, value) in [("reason", reason), ("follow_up", follow_up)] {
                    if !text(value) { return fail(&format!("$.visuals.declaration.{name}"), "expected nonblank text, at most 4096 UTF-8 bytes, no NUL"); }
                }
            }
            Some(Declaration::Attached {}) if self.visuals.uses.is_empty() => return fail("$.visuals.uses", "attached requires at least one use; uses is beside declaration, not inside it"),
            None if !self.visuals.attachments.is_empty() => return fail("$.visuals.declaration", "new attachments require a Verify declaration"),
            _ => {}
        }
        None
    }
}
/// Explains a rejected shape without reading any project or echoing supplied values.
pub(super) fn rejection(input: &Value) -> String {
    let problem = if serde_json::to_vec(input).map_or(true, |v| v.len() > 512 * 1024) {
        "$: request exceeds 512 KiB".to_string()
    } else {
        diagnostics::shape(input).or_else(|| serde_json::from_value::<Close>(input.clone()).ok().and_then(|r| r.problem()))
            .unwrap_or_else(|| "$.visuals: invalid typed close; use the connected schema".into())
    };
    format!("gil_close: {problem}. Nothing was closed. New PNG: attachments with key/path/source_kind/source_description, uses with source {{kind: new, key}} plus caption/alt/purpose, declaration {{state: attached}}. Stored citation: source {{kind: stored, owner: {{kind, ref}}, asset_id, sha256}} from a confirmed receipt, never a key. Read gil_help topic report/visuals for complete JSON and gil_status for the actual open owner; no example ref identifies your current step.")
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
/// A visual-contract refusal keeps Core's own words: the model must see which asset to cite or
/// which declaration to change. The code is the bracketed tag Core put at the front of its reason.
fn close_error(error: crate::SessionError) -> (String, Option<String>) {
    match error {
        crate::SessionError::Evidence(reason) => ("verify_evidence_required".into(), Some(reason)),
        crate::SessionError::Action(crate::ActionError::Visual(reason)) => {
            let code = reason.strip_prefix('[').and_then(|rest| rest.split_once(']')).map(|(code, _)| code.to_string())
                .filter(|code| !code.is_empty() && code.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'))
                .unwrap_or_else(|| "visual_rejected".into());
            (code, Some(reason))
        }
        other => ("close_not_confirmed".into(), Some(other.to_string())),
    }
}
fn fail(code: &str) -> CallToolResult {
    fail_with(code, None)
}
fn fail_with(code: &str, reason: Option<String>) -> CallToolResult {
    let said = if code == "verify_evidence_required" {
        "단계는 닫히지 않았다. Verify는 실제 저장한 비어 있지 않은 파일을 evidence_files에 상대 경로로 명시해야 하며, 적어도 하나는 이전 Snapshot보다 새롭거나 달라야 한다. 읽기만 한 작업도 관측 결과를 파일로 남긴다. Analysis의 interpretation과 Outcome의 lesson은 비어 있을 수 없고 실제 계보의 근거를 참조한다. gil_context로 현재 단계를 확인하고 누락된 증거·해석·배움을 보완한다."
    } else if code == "invalid_report" {
        "Report 문법 오류로 실행하지 않았고 아무 단계도 닫히지 않았다. report는 '이름: 값' 텍스트다. 한 파일은 'evidence_files: evidence/result.md'처럼 같은 줄에 적는다. 여러 파일은 'evidence_files: |\n  evidence/one.md\n  evidence/two.md'처럼 |와 들여쓰기를 쓴다. 실제 저장한 경로만 적는다. 현재 대상이 여전히 열려 있으면 Report 문법을 고쳐 같은 gil_close를 한 번 제출한다. typed 입력의 expected_owner와 visuals도 함께 유지한다."
    } else if code.starts_with("visual_") {
        "시각 자료 계약이 거절했다. 닫힌 것은 없다. 아래 이유대로 visuals(선언·첨부·인용)를 고쳐 같은 gil_close를 한 번 다시 보낸다. 규칙은 gil_help topic report/visuals 에 있다."
    } else {
        "시각 보고서 요청을 완료하지 못했다. gil_report_visuals로 예상 보고서의 확정 여부를 확인한다. close를 자동 반복하지 않는다."
    };
    let said = match reason {
        Some(reason) => format!("{said}\n{reason}"),
        None => said.to_string(),
    };
    let mut result = CallToolResult::structured(json!({"ok":false,"code":code,
        "said":said}));
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
        .all_cycles()
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
        Err((code, reason)) => fail_with(&code, reason),
    }
}
fn execute(name: &str, input: Value) -> Result<Value, (String, Option<String>)> {
    execute_inner(name, input).map_err(|(code, reason)| (code, reason))
}
fn plain(code: &'static str) -> (String, Option<String>) {
    (code.to_string(), None)
}
fn execute_inner(name: &str, input: Value) -> Result<Value, (String, Option<String>)> {
    let rules = RuleSet::builtin().map_err(|_| plain("rules_unavailable"))?;
    if name == "gil_report_visuals" {
        let r: Read = serde_json::from_value(input).map_err(|_| plain("invalid_request"))?;
        let root = Path::new(&r.project_root);
        let state = crate::project_state::state_for_root(root)
            .map_err(|_| plain("confirmation_unavailable"))?;
        #[cfg(windows)]
        let session = ProjectSession::open_read_only_at(rules, root, &state)
            .map_err(|_| plain("confirmation_unavailable"))?;
        #[cfg(not(windows))]
        let session = ProjectSession::open_read_only(rules, &state)
        .map_err(|_| plain("confirmation_unavailable"))?;
        return projection(session.project(), r.owner.parsed().map_err(plain)?).map_err(plain);
    }
    let r: Close = serde_json::from_value(input).map_err(|_| plain("invalid_request"))?;
    let owner = r.expected_owner.parsed().map_err(plain)?;
    let mut session = crate::open_project_root(rules, Path::new(&r.project_root))
        .map_err(|_| plain("project_unavailable"))?;
    if session.project().visual_state().is_none() {
        return Err(plain("visual_upgrade_required"));
    }
    let c = session.project().current_cycle();
    let actual = match c.step_now_open() {
        Some(n) => ReportOwner::Step(c.step_ref(n)),
        None => ReportOwner::Cycle(c.id().to_ref()),
    };
    if owner != actual || projection(session.project(), owner).map_err(plain)?["confirmed"] == true {
        let actual = match actual {
            ReportOwner::Step(r) => json!({"kind":"step","ref":r.to_string()}),
            ReportOwner::Cycle(r) => json!({"kind":"cycle","ref":r.to_string()}),
        };
        return Err(("owner_mismatch".into(), Some(format!("expected_owner가 현재 보고 대상과 다르거나 대상이 이미 닫혔다. 현재 대상: {actual}. gil_status와 gil_report_visuals로 열림·확정 여부를 확인하고 닫힌 보고서는 반복 제출하지 않는다."))));
    }
    let is_verify = c
        .step_now_open()
        .and_then(|n| c.steps().node(n))
        .is_some_and(|n| n.kind == crate::NodeKind::Verify);
    if is_verify != r.visuals.declaration.is_some()
        || (!is_verify && !r.visuals.attachments.is_empty())
    {
        return Err(plain("visual_stage_mismatch"));
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
                let asset = id(asset_id).ok_or(plain("invalid_asset"))?;
                let stored = session
                    .read_report_image(owner.parsed().map_err(plain)?, asset)
                    .map_err(|_| plain("visual_unavailable"))?;
                if format!("{:x}", Sha256::digest(stored.bytes())) != *sha256 {
                    return Err(plain("visual_unavailable"));
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
    let report = Report::parse(&r.report).map_err(|_| plain("invalid_report"))?;
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
                Declaration::Attached {} => VisualDeclaration::Attached {
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
            .map_err(close_error)?;
    } else {
        session
            .close_references_and_commit(report, make_uses(&[]))
            .map_err(close_error)?;
    }
    projection(session.project(), owner).map_err(plain)
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
        "Close the current Report. Two input shapes only. Ordinary close {project_root, report}: every step of a format-4 project (including its Verify), and format-5 non-Verify steps that cite no stored images. Typed close {project_root, report, expected_owner:{kind,ref}, visuals:{attachments,uses,declaration}} with expected_owner and visuals ALWAYS together: every format-5 Verify (with attachments/uses and declaration attached, or empty arrays and declaration not_needed (reason) / missing (reason, follow_up) when there are no images), and any format-5 non-Verify step or Cycle report that cites stored images (uses with source kind stored, empty attachments, no declaration). Sending expected_owner without visuals, or visuals without expected_owner, is rejected with -32602 naming the missing field; that call is not executed and nothing is closed — read gil_status/gil_context for the actual open step, then send one corrected call. New PNGs use attachments:[{key,path,source_kind,source_description}] and uses:[{source:{kind:new,key},caption,alt,purpose}], with declaration:{state:attached}; these three fields are siblings inside visuals. Stored reuse requires source:{kind:stored,owner:{kind,ref},asset_id,sha256} from the receipt, never a key. Complete JSON: gil_help topic report/visuals (CLI alias: gil help visuals). After an uncertain or lost response use gil_report_visuals; never automatically repeat close. Every Verify also requires evidence_files in report: 1–32 unique project-relative paths, all nonempty files in its Snapshot, at least one new or changed. No-image declarations do not waive file evidence. Visual evidence means a PNG that shows the real result: a plot or diagram drawn by code from the data, a headless or ordinary browser screenshot of the actual page/UI, a before/after comparison table. AI-generated or decorative pictures are never visual evidence. Contract checks: not_needed/missing reason must say what replaces the image (one-word or boilerplate reasons such as 없음/not needed are refused, code visual_reason_weak); a Verify whose evidence_files include an image file cannot close not_needed (visual_declaration_conflict); when the Verify attached assets, the Analysis and the Experiment Cycle Report must cite at least one of them with a stored source (visual_citation_required; the refusal lists asset_id and sha256). Analysis interpretation and Outcome lesson must be nonblank; Core links their actual evidence lineage. No migration."
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
    fn documented_close() -> Value {
        let manual = include_str!("../manual/topics/report/visuals.md");
        let mut input: Value = serde_json::from_str(manual.split("```json\n").nth(1).unwrap().split("```").next().unwrap()).unwrap();
        input["project_root"] = json!(if cfg!(windows) { "C:/synthetic" } else { "/synthetic" });
        input
    }
    #[test]
    fn documented_two_image_close_and_stored_citation_match_the_wire() {
        let mut input = documented_close();
        assert!(valid("gil_close", &input));
        let mut falsely_unneeded = input.clone();
        falsely_unneeded["visuals"]["declaration"] = json!({"state":"not_needed","reason":"synthetic byte comparison"});
        assert!(!valid("gil_close", &falsely_unneeded), "changing only the declaration cannot bypass the two attached images");
        let manual = include_str!("../manual/topics/report/visuals.md");
        input["visuals"] = serde_json::from_str(manual.split("```json\n").nth(2).unwrap().split("```").next().unwrap()).unwrap();
        input["expected_owner"] = json!({"kind":"step","ref":"step:C3/S4"});
        assert!(valid("gil_close", &input));
    }
    #[test]
    fn visual_errors_locate_fields_without_leaking_values_or_unknown_names() {
        let secret = "synthetic-secret-never-echo";
        for (path, value, expected) in [
            ("/visuals/uses/0/source", json!({"kind":"stored","key":secret}), "$.visuals.uses[0].source.owner"),
            ("/visuals/uses/0", json!({"key":secret,"source":{"kind":"new","key":"earnings"},"caption":"c","alt":"a","purpose":"p"}), "$.visuals.uses[0]"),
            ("/visuals/declaration", json!({"state":"attached","uses":[secret]}), "$.visuals.declaration"),
            ("/visuals/declaration", Value::Null, "$.visuals.declaration"),
            ("/visuals/declaration/state", json!(secret), "$.visuals.declaration.state"),
            ("/visuals/uses/0/source/kind", json!(secret), "$.visuals.uses[0].source.kind"),
            ("/visuals/uses/0/source/key", json!(secret), "$.visuals.uses[0].source.key"),
            ("/visuals/uses/0/caption", json!(12), "$.visuals.uses[0].caption"),
            ("/visuals/uses/0/alt", json!(" "), "$.visuals.uses[0].alt"),
            ("/visuals/attachments/1/source_kind", json!(secret), "$.visuals.attachments[1].source_kind"),
            ("/expected_owner/ref", json!(secret), "$.expected_owner.ref"),
        ] {
            let mut input = documented_close();
            *input.pointer_mut(path).unwrap() = value;
            assert!(!valid("gil_close", &input), "{path}");
            let error = rejection(&input);
            assert!(error.contains(expected), "{error}");
            assert!(!error.contains(secret) && !error.contains("step:C2/S3"), "{error}");
        }
        let mut input = documented_close();
        input["visuals"][secret] = json!(secret);
        assert!(!valid("gil_close", &input));
        assert!(!rejection(&input).contains(secret));
        let mut input = documented_close();
        input.as_object_mut().unwrap().remove("report");
        assert!(!valid("gil_close", &input));
        assert!(rejection(&input).contains("$.report"));
    }
    #[test]
    fn strict_typed_inputs_refuse_ambiguous_or_unbounded_authoring() {
        let root = if cfg!(windows) { "C:/synthetic" } else { "/synthetic" };
        let good = json!({"project_root":root,"report":"execution: test","expected_owner":{"kind":"step","ref":"step:C2/S3"},
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
