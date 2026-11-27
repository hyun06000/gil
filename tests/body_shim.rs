//! Test callers write `gil_open` contracts and `gil_close` Reports as Report text, the form the
//! Core keeps. The MCP wire takes them as objects of string fields (src/mcp/body.rs), so the test
//! clients translate here, in one place. A text the Report grammar refuses is sent as it stands:
//! the wire then refuses it as a string, which is what an agent sending such a text would meet.
use serde_json::{Value, json};

#[allow(dead_code)]
pub fn objects(name: &str, mut arguments: Value) -> Value {
    let field = match name { "gil_open" => "contract", "gil_close" => "report", _ => return arguments };
    if let Some(text) = arguments.get(field).and_then(Value::as_str) {
        if let Ok(report) = gil::Report::parse(text) {
            let fields: serde_json::Map<String, Value> = report.field_names()
                .map(|name| (name.to_owned(), json!(report.get(name).unwrap_or_default()))).collect();
            arguments[field] = Value::Object(fields);
        }
    }
    arguments
}

/// The same translation for a whole request: only `tools/call` carries a body.
#[allow(dead_code)]
pub fn tool_call(method: &str, mut params: Value) -> Value {
    if method == "tools/call" {
        if let Some(name) = params.get("name").and_then(Value::as_str).map(str::to_owned) {
            params["arguments"] = objects(&name, params["arguments"].take());
        }
    }
    params
}
