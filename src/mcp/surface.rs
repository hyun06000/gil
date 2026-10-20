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
                start.description = Some("Development image-report creation: creates format 5 on verified local storage (macOS Apple Silicon; Windows local fixed NTFS). Requires the user's explicitly selected absolute project_root. Existing records/remnants are preserved, never migrated or retried automatically. Success commits one Interview, zero Steps; does not open or confirm the Monitor. Unsupported platforms/volumes refuse, never fall back to format 4. On Windows the selected location is checked before anything is created: OneDrive/cloud-sync roots, network or removable drives, non-NTFS volumes, reparse points and unverifiable locations refuse with code unsupported_location and nothing is created; see gil_prepare_workspace. Migrating existing format-4 records stays macOS-only.".into());
            }
            tools
        };
        tools
    })
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
/// Monitor inputs are strict, as in the previous JS adapter. Other inputs ignore extras.
pub fn valid_input(tool: &Tool, input: &Value) -> bool {
    #[cfg(feature = "visual-authoring")]
    if tool.name.starts_with("gil_migration_") {
        return super::migration::valid(tool.name.as_ref(), input);
    }
    #[cfg(feature = "visual-authoring")]
    if tool.name == "gil_report_visuals" || (tool.name == "gil_close" && input.get("visuals").is_some()) {
        return super::authoring::valid(tool.name.as_ref(), input);
    }
    // Older/default builds must never silently discard a visual close request.
    if tool.name == "gil_close" && (input.get("visuals").is_some() || input.get("expected_owner").is_some()) {
        return false;
    }
    if matches!(tool.name.as_ref(), "gil_monitor_report_visuals" | "gil_monitor_report_image") {
        return super::visuals::valid(input, tool.name == "gil_monitor_report_image");
    }
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
pub fn rejection(tool: &Tool, input: &Value) -> &'static str {
    if tool.name != "gil_close" {
        return "Invalid GIL tool arguments";
    }
    let owner = input.get("expected_owner").is_some();
    let visuals = input.get("visuals").is_some();
    if !cfg!(feature = "visual-authoring") && (owner || visuals) {
        return "gil_close: this GIL accepts only the text contract {project_root, report}; 'visuals' and 'expected_owner' are not supported here and were not silently dropped. Nothing was closed. Send the ordinary close, or use an image-capable GIL; never migrate or convert records to work around this.";
    }
    match (owner, visuals) {
        (true, false) => "gil_close: 'expected_owner' was given without 'visuals'. This call was not executed. Nothing was closed. Ordinary close takes exactly {project_root, report}: every step of a format-4 project including its Verify, and format-5 non-Verify steps that cite no stored images. Typed close takes {project_root, report, expected_owner:{kind,ref}, visuals:{attachments,uses,declaration}} together: every format-5 Verify (empty arrays and declaration not_needed/missing when there are no images) and any format-5 report that cites stored images. Read gil_status/gil_context for the actual open step, then send one corrected call in the shape that step requires.",
        (false, true) => "gil_close: 'visuals' was given without 'expected_owner' {kind, ref}. This call was not executed. Nothing was closed. Ordinary close takes exactly {project_root, report} (all format-4 steps including Verify; format-5 non-Verify steps without stored-image citations); typed close takes {project_root, report, expected_owner, visuals} together (every format-5 Verify, even without images, and any format-5 report citing stored images). Read gil_status/gil_context for the actual open step, then send one corrected call.",
        (true, true) => "gil_close: 'expected_owner' and 'visuals' are present but malformed. Owner ref looks like step:C2/S3 or cycle:C2; declaration must not be null; a Verify needs exactly one declaration (attached with uses, not_needed with a reason and no uses, missing with reason and follow_up); at most 8 attachments/uses with unique keys; report up to 256 KiB. Nothing was closed; correct the call instead of repeating it.",
        (false, false) => "gil_close: invalid arguments. Expected {project_root (absolute string), report (string)}. Nothing was closed.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
