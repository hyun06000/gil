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
            let mut tools = super::migration::catalog(super::authoring::catalog(tools));
            if let Some(start) = tools.iter_mut().find(|t| t.name == "gil_start") {
                start.description = Some("Development image-report creation: creates format 5 on supported macOS Apple Silicon local storage. Requires the user's explicitly selected absolute project_root. Existing records/remnants are preserved, never migrated or retried automatically. Success commits one Interview, zero Steps; does not open or confirm the Monitor. Unsupported platforms refuse, never fall back to format 4.".into());
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

#[cfg(test)]
mod tests {
    use super::*;
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
