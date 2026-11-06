//! Read-only wire diagnostics. Paths contain only schema-owned names and array indices;
//! never include input values, unknown property names or serde's raw error text.
use serde_json::Value;

pub(super) fn shape(input: &Value) -> Option<String> {
    for name in ["project_root", "report", "expected_owner", "visuals"] {
        if input.get(name).is_none() { return Some(format!("$.{name}: required field is missing")); }
    }
    let schema: Value = serde_json::from_str(include_str!("../authoring-schema.json")).expect("authoring schema");
    at(input, &schema["close"], &schema["close"], "$")
}

fn at(value: &Value, schema: &Value, root: &Value, path: &str) -> Option<String> {
    let fail = |reason: &str| Some(format!("{path}: {reason}"));
    if let Some(reference) = schema["$ref"].as_str() {
        return at(value, root.pointer(reference.strip_prefix('#')?)?, root, path);
    }
    if let Some(variants) = schema["oneOf"].as_array() {
        // The authoring unions are tagged by schema-owned `kind` or `state` constants.
        for variant in variants {
            for tag in ["kind", "state"] {
                if let Some(expected) = variant.pointer(&format!("/properties/{tag}/const")) {
                    if value.get(tag) == Some(expected) { return at(value, variant, root, path); }
                }
            }
        }
        if value.is_object() {
            for tag in ["kind", "state"] {
                if variants.iter().any(|v| v.pointer(&format!("/properties/{tag}/const")).is_some()) {
                    return Some(format!("{path}.{tag}: expected a documented variant tag"));
                }
            }
        }
        return fail("expected a documented kind/state variant; declaration is omitted outside Verify, never null");
    }
    if let Some(options) = schema["enum"].as_array() {
        if !options.contains(value) { return fail("unsupported enum value; use the connected schema"); }
    }
    match schema["type"].as_str() {
        Some("object") => {
            let Some(object) = value.as_object() else { return fail("expected object"); };
            if let Some(required) = schema["required"].as_array() {
                for name in required.iter().filter_map(Value::as_str) {
                    if !object.contains_key(name) { return Some(format!("{path}.{name}: required field is missing")); }
                }
            }
            let properties = schema["properties"].as_object()?;
            if schema["additionalProperties"] == false && object.keys().any(|k| !properties.contains_key(k)) {
                return fail("unexpected field; only documented properties are allowed");
            }
            for (name, child) in properties {
                if let Some(v) = object.get(name) {
                    if let Some(error) = at(v, child, root, &format!("{path}.{name}")) { return Some(error); }
                }
            }
        }
        Some("array") => {
            let Some(array) = value.as_array() else { return fail("expected array"); };
            if schema["maxItems"].as_u64().is_some_and(|max| array.len() as u64 > max) {
                return fail("too many entries (maximum 8)");
            }
            for (index, v) in array.iter().enumerate() {
                if let Some(error) = at(v, &schema["items"], root, &format!("{path}[{index}]")) { return Some(error); }
            }
        }
        Some("string") if !value.is_string() => return fail("expected string"),
        _ => {}
    }
    None
}
