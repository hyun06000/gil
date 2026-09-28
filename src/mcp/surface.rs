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
    TOOLS.get_or_init(|| serde_json::from_str(&include_str!("tools.json").replace("RESOURCE_URI", uri()))
        .expect("checked-in public tool catalog"))
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
