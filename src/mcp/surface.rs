//! Public wire catalog and embedded, content-addressed MCP App. No runtime files.
use std::sync::OnceLock;
use rmcp::model::{Tool, Resource, ResourceContents};
use sha2::{Digest, Sha256};
use serde_json::{Value, json};

pub const HTML: &str = include_str!("../../plugins/gil-companion-prototype/assets/monitor.html");
pub const MIME: &str = "text/html;profile=mcp-app";

pub fn uri() -> &'static str {
    static URI: OnceLock<String> = OnceLock::new();
    URI.get_or_init(|| format!("ui://gil-monitor/{}.html", &format!("{:x}", Sha256::digest(HTML.as_bytes()))[..20]))
}

/// What a visual-format project started by this build holds. The number comes from the build;
/// no format this build cannot read is named.
#[cfg(feature = "visual-authoring")]
const START_SHAPE: &str = if cfg!(feature = "will-end-history") {
    "a Chain with the visual registry and the single Will end history"
} else if cfg!(feature = "chain-core") {
    "a Chain with the visual registry"
} else {
    "Cycles with the visual registry"
};

pub fn connection_tools(headless: bool) -> &'static [Tool] {
    if !headless { return tools(); }
    static HEADLESS: OnceLock<Vec<Tool>> = OnceLock::new();
    HEADLESS.get_or_init(|| {
        let mut catalog = tools().to_vec();
        for tool in &mut catalog {
            if !matches!(tool.name.as_ref(), "gil_start" | "gil_context") { continue; }
            if let Some(schema) = &mut tool.output_schema {
                let properties = std::sync::Arc::make_mut(schema).get_mut("properties").expect("receipt properties");
                properties["next_action"] = json!({"type":"string","description":"Recorded next step or wait in a headless session."});
            }
            if tool.name == "gil_start" {
                let kind = if super::start_kind::HAS_TRACKED && super::start_kind::HAS_IMAGES {
                    " Choose start_kind explicitly on a new project: tracked_scope prepares a scope requiring the human's approval; image_reports creates an image-report project."
                } else if super::start_kind::HAS_TRACKED {
                    " Prepares tracked scope; the human must approve before the first archive."
                } else { "" };
                tool.description = Some(format!("Start the selected absolute project_root. The runner selected headless display. Follow the recorded next step; an approval or answer must still be supplied by its authorized source.{kind}").into());
            }
        }
        catalog
    })
}

pub fn tools() -> &'static [Tool] {
    static TOOLS: OnceLock<Vec<Tool>> = OnceLock::new();
    TOOLS.get_or_init(|| {
        let tools: Vec<Tool> = serde_json::from_str(&include_str!("tools.json").replace("RESOURCE_URI", uri()))
            .expect("checked-in public tool catalog");
        #[cfg(feature = "visual-authoring")]
        let tools = {
            let tools = super::migration::catalog(super::authoring::catalog(tools));
            #[cfg(windows)]
            let tools = super::workspace::catalog(tools);
            let mut tools = tools;
            if let Some(start) = tools.iter_mut().find(|t| t.name == "gil_start") {
                start.description = Some(format!("Development image-report creation: creates format {} ({}) {}", crate::store::newest_known_format(), START_SHAPE, "on verified private storage (macOS Apple Silicon; Linux x86-64 on a listed local filesystem: ext4, XFS, Btrfs, tmpfs, overlayfs; Windows private app data). Requires the user's explicitly selected absolute project_root. On Windows the working folder may remain in OneDrive: GIL records are kept separately under the current user's local app data, while file observation still targets project_root. An existing inline Windows record is copied byte-for-byte to that private store and verified before publication; the inline source is never changed or deleted. Success commits one Interview, zero Steps; does not open or confirm the Monitor. Unsupported platforms refuse, never fall back to format 4. Migrating existing format-4 records stays macOS-only.").into());
            }
            tools
        };
        #[cfg(feature = "approved-plan")]
        let tools = super::plans::catalog(tools);
        #[cfg(feature = "exploration-flow")]
        let tools = super::records::catalog(tools);
        #[cfg(feature = "product-safety")]
        let tools = super::recovery::catalog(tools);
        #[cfg(feature = "will-end-history")]
        let tools = super::transition::catalog(unachieved_close(tools));
        #[cfg(all(feature = "tracking-policy", unix))]
        let tools = super::scope::catalog(super::start::catalog(tools));
        // After both start descriptions: the selection input and, with two kinds, one description of both.
        let tools = super::start_kind::catalog(tools);
        let mut tools = tools;
        tools.push(serde_json::from_str(include_str!("handoff-tool.json")).expect("handoff tool"));
        for tool in &mut tools {
            let hint = match tool.name.as_ref() {
                "gil_context" => " Read the existing selected GIL project before file-producing tools in every new session, including requests to add charts to an existing report without saying GIL. Resume its actual plan and Step; do not append a new objective to an old Verify or repeat start. This read does not install project instructions or change Snapshots.",
                "gil_start" if super::start_kind::HAS_TRACKED && super::start_kind::HAS_IMAGES => " Either kind adds local Codex/Claude continuity instructions while preserving existing bytes (before any proposal, or before the initial Snapshot); no Host hooks.",
                "gil_start" if cfg!(all(feature = "tracking-policy", unix)) => " Preparation adds local Codex/Claude continuity instructions while preserving existing bytes, before any proposal; no Host hooks.",
                "gil_start" => " Start adds local Codex/Claude continuity instructions while preserving existing bytes, before the initial Snapshot; no Host hooks. It also prepares the Monitor; the receipt names the exact show_gil_monitor({scope_id}) call, which is the only next call on every Host, before the first gil_open. Never repeat start after display failure.",
                "gil_open" => " Choose one independently checkable objective and observable done_when. Perform it, then gil_close before another objective. Creating, modifying, deleting, moving or importing project Artifact files requires an actually open Experiment Verify, including DOCX/PDF/PNG generation and rendering. All other stages keep project files unchanged. GIL deliverables default to project_root during Verify, including scripts and renders; never choose scratchpad to evade an unfinished stage. Only when external creation is necessary, explain why and where before writing, explicitly disclose that GIL does not track those files, and do not claim them as GIL-verified delivery. Unrelated outside work and file-free web research remain allowed. An Interview Outcome judges the approved scope, not completion of the whole project.",
                "gil_close" => " Call when done_when is met. Verify additionally requires actual saved evidence: Report.evidence_files lists project-relative paths, one per line. All must be nonempty Snapshot files; at least one must be new or changed. Chat-only results and visual not_needed/missing do not waive this. Visual evidence is a PNG of the real result (code-drawn plot or diagram, headless/ordinary browser screenshot, before/after table); AI-generated pictures never count. In visual-format projects a Verify that saved an image as evidence cannot declare not_needed, a one-word reason is refused, and Analysis and the Experiment Cycle Report must cite at least one asset the Verify attached. Analysis requires a nonblank interpretation; Outcome a nonblank lesson. Core links them to their actual evidence lineage. Follow the next receipt before more work.",
                "gil_cycle" => " Legacy helper: do NOT use action=close; this tool cannot accept a Report. Close both Steps and Cycles with gil_close(project_root, report). Use gil_open for new Cycles.",
                _ => continue,
            };
            if tool.name == "gil_cycle" {
                tool.title = Some("Legacy GIL cycle helper".into());
                tool.description = Some(hint.trim().to_owned().into());
            } else {
                tool.description.get_or_insert_with(Default::default).to_mut().push_str(hint);
            }
        }
        tools
    })
}

/// Adds the unachieved marker to `gil_close` — only in a build that can record the result.
///
/// The base catalog (`tools.json`) and every build without `will-end-history` stay as they were:
/// a tool surface never offers an input its Core would have to refuse for every project. The
/// input is still refused, with an explicit-conversion notice, on a format 4–7 project.
#[cfg(feature = "will-end-history")]
fn unachieved_close(mut tools: Vec<Tool>) -> Vec<Tool> {
    let close = tools.iter_mut().find(|tool| tool.name == "gil_close").expect("close");
    let mut raw = serde_json::to_value(&*close).expect("tool");
    raw["inputSchema"]["properties"][super::actions::UNACHIEVED_WILL] = json!({
        "type": "string",
        "pattern": "^will:W[1-9][0-9]*$",
        "description": "Optional. Ends the open step's Will as UNACHIEVED instead of achieved: the canonical reference of the Will to end, exactly as gil_status/gil_context show it (for example will:W9). Use only when the contracted action of an Experiment step could not be carried out; the reason goes in the Report body, never here. Every ordinary close condition still applies (Report fields, approved plan, clean world, Verify evidence_files, visual declaration). Refused, closing nothing, when the reference is not the currently active Will (a stale resend cannot close a newer step), when that Will already ended (the answer names its step and result), in an Interview, or on a project whose storage format (4–7) does not record end results. After an unachieved end the Experiment closes only as failure: Outcome verdict failure with next_direction.action close_cycle, no new hypothesis and no in-Experiment revisit. A lost connection or a pending answer is not an end: leave the step open."
    });
    let description = raw["description"].as_str().unwrap_or_default().to_owned();
    raw["description"] = json!(format!("{description} Optional unachieved_will (a Will reference such as will:W9) ends the open Experiment step's Will as unachieved; omit it for the ordinary achieved close."));
    *close = serde_json::from_value(raw).expect("tool");
    tools
}

pub fn resource() -> Resource {
    Resource::new(uri(), "gil-monitor").with_title("GIL Monitor").with_mime_type(MIME)
}

pub fn contents() -> ResourceContents {
    serde_json::from_value(json!({"uri": uri(), "mimeType": MIME, "text": HTML,
        "_meta": {"ui": {"prefersBorder": true, "csp": {"connectDomains": [], "resourceDomains": []}}}}))
        .expect("embedded App resource")
}

/// The frozen inputs contain only string properties. Domain grammar stays in Core.
/// Honor additionalProperties:false consistently for text and specialized tools.
pub fn valid_input(tool: &Tool, input: &Value) -> bool {
    // Enforce the catalog's top-level boundary before specialized dispatch too. Ordinary
    // text actions previously ignored misspelled keys despite additionalProperties:false.
    if tool.input_schema.get("additionalProperties") == Some(&Value::Bool(false)) {
        let Some(object) = input.as_object() else { return false; };
        let Some(properties) = tool.input_schema.get("properties").and_then(Value::as_object) else { return false; };
        if object.keys().any(|key| !properties.contains_key(key)) { return false; }
    }
    // The marker names one Will. Anything but a canonical reference — `null` included, which is
    // not an omission — is a wire error, never a close. This runs before every specialized input
    // (the typed visual close among them) so both close shapes refuse the same values.
    if tool.name == "gil_close" {
        if let Some(value) = input.get(super::actions::UNACHIEVED_WILL) {
            let canonical = value.as_str().is_some_and(|text| text.parse::<crate::WillRef>().is_ok_and(|will| will.to_string() == text));
            if !canonical { return false; }
        }
    }
    if super::unattended::NAMES.contains(&tool.name.as_ref()) { return super::unattended::valid(tool.name.as_ref(), input); }
    #[cfg(feature = "product-safety")]
    if tool.name.starts_with("gil_revisit_") { return super::recovery::valid(tool.name.as_ref(), input); }
    #[cfg(all(feature = "tracking-policy", unix))]
    if super::start::NAMES.contains(&tool.name.as_ref()) { return super::start::valid(tool.name.as_ref(), input); }
    #[cfg(all(feature = "tracking-policy", unix))]
    if super::scope::NAMES.contains(&tool.name.as_ref()) { return super::scope::valid(tool.name.as_ref(), input); }
    #[cfg(feature = "approved-plan")]
    if tool.name.starts_with("gil_plan_") { return super::plans::valid(tool.name.as_ref(), input); }
    #[cfg(feature = "exploration-flow")]
    if super::records::NAMES.contains(&tool.name.as_ref()) { return super::records::valid(tool.name.as_ref(), input); }
    #[cfg(feature = "will-end-history")]
    if super::transition::NAMES.contains(&tool.name.as_ref()) {
        return super::transition::valid(tool.name.as_ref(), input);
    }
    #[cfg(feature = "visual-authoring")]
    if tool.name.starts_with("gil_migration_") {
        return super::migration::valid(tool.name.as_ref(), input);
    }
    #[cfg(feature = "visual-authoring")]
    if tool.name == "gil_report_visuals" || (tool.name == "gil_close" && input.get("visuals").is_some()) {
        return super::authoring::valid(tool.name.as_ref(), input);
    }
    #[cfg(all(feature = "visual-authoring", windows))]
    if tool.name == super::workspace::NAME {
        return super::workspace::valid(input);
    }
    // Older/default builds must never silently discard a visual close request.
    if tool.name == "gil_close" && (input.get("visuals").is_some() || input.get("expected_owner").is_some()) {
        return false;
    }
    if matches!(tool.name.as_ref(), "gil_monitor_report_visuals" | "gil_monitor_report_image") {
        return super::visuals::valid(input, tool.name == "gil_monitor_report_image");
    }
    if tool.name == "gil_await_user" && input.as_object().is_none_or(|v| v.keys().any(|k| !matches!(k.as_str(), "project_root" | "question"))) { return false; }
    if tool.name == "gil_monitor_poll" && !super::presence::valid(input) { return false; }
    // The optional language tag has the same bounds everywhere it travels.
    if tool.name == "gil_start" && !input.get("locale").is_none_or(super::presence::locale_tag_valid) { return false; }
    let Some(object) = input.as_object() else { return false; };
    let schema = &tool.input_schema;
    let Some(properties) = schema.get("properties").and_then(Value::as_object) else { return false; };
    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        if required.iter().filter_map(Value::as_str).any(|key| !object.contains_key(key)) { return false; }
    }
    for (key, value) in object {
        if let Some(property) = properties.get(key) {
            if property.get("type").and_then(Value::as_str) != Some("string") || !value.is_string() { return false; }
            if key == "scope_id" {
                let value = value.as_str().unwrap_or_default();
                let Some(digest) = value.strip_prefix("project:") else { return false; };
                if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) { return false; }
            }
        } else if tool.name.starts_with("gil_monitor_") || tool.name == "show_gil_monitor" { return false; }
    }
    true
}

/// Static, safe explanation for a schema rejection (-32602). It names the missing or misplaced
/// field so the caller can correct the call, never echoes input values, and states that nothing
/// was closed. Field grammar itself stays with Core; this only describes the wire shape.
pub fn rejection(tool: &Tool, input: &Value) -> String {
    if tool.name == "gil_close" && input.get("contract").is_some() {
        return "gil_close: unknown field 'contract'. Use 'report' for the Report object of string fields; 'contract' belongs to gil_open. Keep the same body in report, preserving expected_owner/visuals when required. Nothing was closed. Read gil_help({project_root, topic: \"close\"}) for the current Report fields.".into();
    }
    if tool.name == "gil_open" && input.get("report").is_some() {
        return "gil_open: unknown field 'report'. Use 'contract' for the action contract object {objective, next_action, done_when}; 'report' belongs to gil_close. Nothing was opened.".into();
    }
    if tool.name == "gil_close" {
        if let Some(hint) = unachieved_rejection(input) { return hint.into(); }
    }
    // The outer argument is missing, not a field inside the body: say so before Core's Report
    // grammar would describe it as missing Report fields.
    if tool.name == "gil_close" && input.as_object().is_some_and(|object| !object.contains_key("report")) {
        return "gil_close: 'report' is missing. Send project_root and the Report object (string fields) for the current Step or Cycle in report. Include expected_owner and visuals when a typed close is required. This call was not executed. Nothing was closed. Read gil_help({project_root, topic: \"close\"}) for the current Report fields, then send one corrected call.".into();
    }
    #[cfg(feature = "visual-authoring")]
    if tool.name == "gil_close" && input.get("expected_owner").is_some() && input.get("visuals").is_some() {
        return super::authoring::rejection(input);
    }
    rejection_hint(tool, input).to_string()
}

/// Why an `unachieved_will` input was refused at the wire: unsupported here, or not a reference.
/// A well-formed marker in a supporting build is not the reason and yields `None`.
fn unachieved_rejection(input: &Value) -> Option<&'static str> {
    let value = input.get(super::actions::UNACHIEVED_WILL)?;
    if !cfg!(feature = "will-end-history") {
        return Some("gil_close: 'unachieved_will' is not supported by this GIL; it was not silently dropped and nothing was closed. This build records every closed step's Will as achieved and cannot record an unachieved end. Leave the step open, or close it without the field only if the contracted action was actually carried out. Never convert or edit records to work around this.");
    }
    let canonical = value.as_str().is_some_and(|text| text.parse::<crate::WillRef>().is_ok_and(|will| will.to_string() == text));
    (!canonical).then_some("gil_close: 'unachieved_will' must be the canonical reference of the Will to end, for example will:W9, exactly as gil_status or gil_context show it; null is not an omission and is refused too. Nothing was closed. For an ordinary achieved close leave the field out; otherwise read gil_status for the active Will and send one corrected call.")
}

fn rejection_hint(tool: &Tool, input: &Value) -> &'static str {
    if tool.name == "gil_plan_propose" {
        return if input.get("draft").is_none() {
            "gil_plan_propose: 'draft' is missing. The call takes exactly {project_root, draft}; scope, items and synthesis belong inside draft, never at the top level, and exploration_checklist is a string field inside synthesis (item IDs, one per line). Nothing was recorded; wrap the same content in draft and send one corrected call."
        } else {
            "gil_plan_propose: invalid arguments with 'draft' present. The call takes exactly {project_root, draft}; project_root must be an absolute string. draft takes exactly {scope, items, synthesis}. scope has five string fields: goal, in_scope, out_of_scope, constraints, success_criteria. items is an array (1–64) of {id, question, observation, source}, all nonblank strings. No extra fields at the top level, in draft, scope or an item. synthesis is a map of string-valued Report fields: statement, basis_refs, exploration_checklist, plus chain_verdict (success or failure) for Closing; omit approved, plan_revision and plan_digest. The call must fit within 72 KiB. Nothing was recorded; correct the call instead of repeating it."
        };
    }
    #[cfg(feature = "exploration-flow")]
    if let Some(hint) = super::records::rejection(tool.name.as_ref()) {
        return hint;
    }
    #[cfg(feature = "will-end-history")]
    if let Some(hint) = super::transition::rejection(tool.name.as_ref()) {
        return hint;
    }
    if tool.name != "gil_close" {
        return "Invalid GIL tool arguments";
    }
    let owner = input.get("expected_owner").is_some();
    let visuals = input.get("visuals").is_some();
    if !cfg!(feature = "visual-authoring") && (owner || visuals) {
        return "gil_close: this GIL accepts only the text contract {project_root, report}; 'visuals' and 'expected_owner' are not supported here and were not silently dropped. Nothing was closed. Send the ordinary close, or use an image-capable GIL; never migrate or convert records to work around this.";
    }
    match (owner, visuals) {
        (true, false) => "gil_close: 'expected_owner' was given without 'visuals'. This call was not executed. Nothing was closed. Ordinary close takes exactly {project_root, report}: every step, Verify included, of a project whose storage format has no visual registry (an even format number), and non-Verify steps of a visual-format project (an odd format number) that cite no stored images. Typed close takes {project_root, report, expected_owner:{kind,ref}, visuals:{attachments,uses,declaration}} together: every visual-format Verify (empty arrays and declaration not_needed/missing when there are no images) and any visual-format report that cites stored images. Read gil_status/gil_context for the actual open step, then send one corrected call in the shape that step requires.",
        (false, true) => "gil_close: 'visuals' was given without 'expected_owner' {kind, ref}. This call was not executed. Nothing was closed. Ordinary close takes exactly {project_root, report} (every step, Verify included, of an even-format project without the visual registry; non-Verify steps of an odd visual-format project without stored-image citations); typed close takes {project_root, report, expected_owner, visuals} together (every visual-format Verify, even without images, and any visual-format report citing stored images). Read gil_status/gil_context for the actual open step, then send one corrected call.",
        (true, true) => "gil_close: 'expected_owner' and 'visuals' are present but malformed. Read gil_status for the actual open owner; declaration must not be null; a Verify needs exactly one declaration (attached with uses, not_needed with a reason and no uses, missing with reason and follow_up); at most 8 attachments/uses with unique keys; report up to 256 KiB. Nothing was closed; correct the call instead of repeating it.",
        (false, false) => "gil_close: invalid arguments. Expected {project_root (absolute string), report (object of string fields)}; expected_owner and visuals only together and only in a project that keeps image reports. Nothing was closed.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn action_extras_are_rejected_before_reports_or_typed_visuals_are_dispatched() {
        for (name, wrong, right) in [("gil_close", "contract", "report"), ("gil_open", "report", "contract")] {
            let tool = tools().iter().find(|t| t.name == name).unwrap();
            let mut input = json!({"project_root":"/synthetic"});
            input[wrong] = json!("private body text");
            assert!(!valid_input(tool, &input));
            let message = rejection(tool, &input);
            assert!(message.contains(wrong) && message.contains(right), "{message}");
            assert!(!message.contains("private body text"));
            input[right] = json!("correct body");
            assert!(!valid_input(tool, &input), "both keys is still invalid");
            input.as_object_mut().unwrap().remove(wrong);
            assert!(valid_input(tool, &input));
            input["typo"] = json!(true);
            assert!(!valid_input(tool, &input));
        }
        let close = tools().iter().find(|t| t.name == "gil_close").unwrap();
        let typed = json!({"project_root":"/synthetic","report":"body","contract":"private body text",
            "expected_owner":{"kind":"step","ref":"step:C1/S1"},
            "visuals":{"attachments":[],"uses":[],"declaration":{"state":"not_needed","reason":"byte comparison"}}});
        assert!(!valid_input(close, &typed));
        assert!(rejection(close, &typed).contains("unknown field 'contract'"));
    }
    /// The unachieved marker is offered only by a build that can record the result. Everywhere
    /// else it is a wire error that says so — never silently dropped into an achieved close.
    #[test]
    fn the_unachieved_marker_is_offered_only_where_the_result_can_be_recorded() {
        let close = tools().iter().find(|t| t.name == "gil_close").unwrap();
        let offered = close.input_schema.get("properties").and_then(Value::as_object)
            .is_some_and(|properties| properties.contains_key(crate::mcp::actions::UNACHIEVED_WILL));
        assert_eq!(offered, cfg!(feature = "will-end-history"));
        let named = json!({"project_root":"/synthetic","report":"private body text","unachieved_will":"will:W9"});
        assert_eq!(valid_input(close, &named), cfg!(feature = "will-end-history"));
        if !cfg!(feature = "will-end-history") {
            let said = rejection(close, &named);
            assert!(said.contains("not supported by this GIL") && said.contains("nothing was closed"), "{said}");
            assert!(!said.contains("private body text") && !said.contains("will:W9"), "{said}");
        }
        // Anything but the canonical reference of one Will is refused at the wire in every build,
        // in the ordinary shape and in the typed visual shape alike. `null` is not an omission.
        // The typed input requires an absolute root on this platform; the plain shape checks it later.
        let root = if cfg!(windows) { "C:/synthetic" } else { "/synthetic" };
        let typed = json!({"project_root":root,"report":"private body text",
            "expected_owner":{"kind":"step","ref":"step:C2/S3"},
            "visuals":{"attachments":[],"uses":[],"declaration":{"state":"not_needed","reason":"byte comparison only"}}});
        for shape in [json!({"project_root":root,"report":"private body text"}), typed.clone()] {
            for malformed in [json!("W9"), json!("will:W09"), json!("will:w9"), json!(" will:W9"), json!(9), json!(null), json!(true), json!({}), json!(["will:W9"])] {
                let mut input = shape.clone();
                input["unachieved_will"] = malformed.clone();
                assert!(!valid_input(close, &input), "{malformed} in {shape}");
                let said = rejection(close, &input);
                assert!(said.contains("unachieved_will") && said.contains("othing was closed"), "{malformed}: {said}");
                assert!(!said.contains("private body text"), "{said}");
            }
        }
        // The typed shape with a well-formed marker passes the wire only where it can be recorded.
        let mut marked = typed;
        marked["unachieved_will"] = json!("will:W9");
        assert_eq!(valid_input(close, &marked), cfg!(all(feature = "will-end-history", feature = "visual-authoring")));
    }
    #[test]
    fn rejections_name_the_missing_field_without_echoing_input() {
        let close=tools().iter().find(|t| t.name=="gil_close").unwrap();
        let secret="/synthetic/someone/private";
        let owner_only=json!({"project_root":secret,"report":"x","expected_owner":{"kind":"step","ref":"step:C1/S1"}});
        let visuals_only=json!({"project_root":secret,"report":"x","visuals":{}});
        for input in [&owner_only,&visuals_only] {
            assert!(!valid_input(close,input));
            let said=rejection(close,input);
            assert!(said.contains("visuals") && said.contains("expected_owner") && said.contains("Nothing was closed"),"{said}");
            assert!(!said.contains(secret));
        }
        if cfg!(feature="visual-authoring") {
            assert!(rejection(close,&owner_only).contains("without 'visuals'"));
            assert!(rejection(close,&visuals_only).contains("without 'expected_owner'"));
        } else {
            assert!(rejection(close,&owner_only).contains("text contract"));
        }
        assert_eq!(rejection(tools().iter().find(|t| t.name=="gil_status").unwrap(),&json!({})),"Invalid GIL tool arguments");
    }
    #[cfg(feature = "approved-plan")]
    #[test]
    fn plan_wire_shape_keeps_report_strings_open_and_rejects_structural_extras_safely() {
        let propose = tools().iter().find(|t| t.name == "gil_plan_propose").unwrap();
        let secret = "/synthetic/private-plan-secret";
        let valid = json!({"project_root":secret,"draft":{
            "scope":{"goal":"g","in_scope":"i","out_of_scope":"o","constraints":"c","success_criteria":"s"},
            "items":[{"id":"A","question":"q","done_when":"d"}],
            "synthesis":{"statement":"s","basis_refs":"step:C1/S1","exploration_checklist":"A","chain_verdict":"success"}}});
        assert!(valid_input(propose, &valid));
        let mut report_extension = valid.clone();
        report_extension["draft"]["synthesis"]["other_report_field"] = json!("string");
        assert!(valid_input(propose, &report_extension), "the wire accepts Report string fields; Core validates semantics");
        assert_eq!(propose.input_schema["properties"]["draft"]["properties"]["synthesis"]["additionalProperties"], json!({"type":"string"}));
        let flat = json!({"project_root":secret,"scope":valid["draft"]["scope"],"items":valid["draft"]["items"],"synthesis":valid["draft"]["synthesis"]});
        assert!(!valid_input(propose, &flat));
        assert!(rejection(propose, &flat).contains("'draft' is missing"));
        assert!(!rejection(propose, &flat).contains(secret));
        for pointer in ["", "/draft", "/draft/scope", "/draft/items/0"] {
            let mut input = valid.clone();
            input.pointer_mut(pointer).unwrap().as_object_mut().unwrap().insert(secret.into(), json!(secret));
            assert!(!valid_input(propose, &input), "extra field at {pointer}");
            let said = rejection(propose, &input);
            assert!(said.contains("invalid arguments with 'draft' present") && said.contains("chain_verdict"));
            assert!(!said.contains(secret));
        }
        for pointer in ["/project_root", "/draft/scope/goal", "/draft/items/0/id", "/draft/synthesis/statement"] {
            let mut input = valid.clone();
            *input.pointer_mut(pointer).unwrap() = json!([secret]);
            assert!(!valid_input(propose, &input), "non-string at {pointer}");
            assert!(!rejection(propose, &input).contains(secret));
        }
        for (pointer, field) in [("", "project_root"), ("/draft", "items"), ("/draft/scope", "goal"), ("/draft/items/0", "done_when")] {
            let mut input = valid.clone();
            input.pointer_mut(pointer).unwrap().as_object_mut().unwrap().remove(field);
            assert!(!valid_input(propose, &input), "missing {pointer}/{field}");
        }
    }
    /// `report` is the one body every close carries. The catalog says so, the wire refuses a call
    /// without it, and the refusal names the outer argument instead of Report fields.
    #[test]
    fn close_declares_report_required_and_names_it_when_missing() {
        let close = tools().iter().find(|t| t.name == "gil_close").unwrap();
        let required: Vec<&str> = close.input_schema.get("required").and_then(Value::as_array).unwrap()
            .iter().filter_map(Value::as_str).collect();
        assert!(required.contains(&"project_root") && required.contains(&"report"), "{required:?}");
        let secret = if cfg!(windows) { "C:/synthetic/private" } else { "/synthetic/private" };
        let missing = json!({"project_root": secret});
        assert!(!valid_input(close, &missing));
        let said = rejection(close, &missing);
        assert!(said.contains("'report' is missing") && said.contains("Nothing was closed"), "{said}");
        assert!(said.contains("Step or Cycle") && !said.contains(secret), "{said}");
        assert!(valid_input(close, &json!({"project_root": secret, "report": ""})), "an empty body is Core's to judge");
    }
    #[test]
    fn visual_authoring_is_explicit_and_never_silently_dropped() {
        let close=tools().iter().find(|t| t.name=="gil_close").unwrap();
        assert!(!valid_input(close,&json!({"project_root":"/synthetic","expected_owner":{"kind":"step","ref":"step:C2/S3"}})));
        assert!(!valid_input(close,&json!({"project_root":"/synthetic","visuals":{}})));
        assert_eq!(tools().iter().any(|t| t.name=="gil_report_visuals"),cfg!(feature="visual-authoring"));
    }
    #[test]
    fn embedded_resource_matches_the_built_shared_bundle() {
        let manifest: Value = serde_json::from_str(include_str!("../../plugins/gil-companion-prototype/assets/monitor.json")).unwrap();
        assert_eq!(manifest["resource_uri"], uri());
        assert_eq!(manifest["sha256"], format!("{:x}", Sha256::digest(HTML.as_bytes())));
        let show = tools().iter().find(|t| t.name == "show_gil_monitor").unwrap();
        let tool = serde_json::to_value(show).unwrap();
        assert_eq!(tool["_meta"]["ui"]["resourceUri"], uri());
        assert_eq!(tool["_meta"]["ui/resourceUri"], uri());
    }
    #[test]
    fn app_can_only_call_monitor_reads_or_the_explicit_native_fallback() {
        for tool in tools() {
            let value = serde_json::to_value(tool).unwrap();
            let visibility = value["_meta"]["ui"]["visibility"].as_array().unwrap();
            if super::super::actions::NAMES.contains(&tool.name.as_ref()) {
                assert_eq!(visibility, &[json!("model")]);
            }
        }
    }
}
