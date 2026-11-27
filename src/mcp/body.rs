//! The bodies of `gil_open` and `gil_close` on the MCP wire are objects of string fields.
//!
//! ```text
//! wire   {"contract": {"objective": "…", "next_action": "…", "done_when": "…"}}
//!        {"report":   {"execution": "…", "result": "…", "evidence_files": "evidence/a.md\nevidence/b.md"}}
//! Core   the same fields as Report text — one grammar, one parser, one stored form
//! ```
//!
//! The Core keeps its text grammar; this module is the only translation, in both directions. A
//! string body is refused here with one sentence: there is one form on the wire, never two. Field
//! names are the Report's own (dotted names such as `next_direction.action` are keys as they
//! stand); a value with several lines is a string with line breaks.
use rmcp::model::Tool;
use serde_json::{Map, Value, json};

/// The tool and its body field.
const BODIES: &[(&str, &str)] = &[("gil_open", "contract"), ("gil_close", "report")];

fn field_of(tool: &str) -> Option<&'static str> {
    BODIES.iter().find(|(name, _)| *name == tool).map(|(_, field)| *field)
}

/// A name the Report grammar can carry on one line.
fn name_ok(name: &str) -> bool {
    !name.is_empty() && !name.contains(char::is_whitespace) && !name.contains(':')
}

/// The Report text of an object of string fields. Every value is written as a block, so nothing
/// in a value — a colon, a leading space, a lone `|` — can be read as grammar.
pub(crate) fn to_text(fields: &Map<String, Value>) -> Result<String, &'static str> {
    let mut text = String::new();
    for (name, value) in fields {
        // A boolean answer (an Analysis's problem_solved, for one) is written as its word.
        let word = value.as_bool().map(|answer| if answer { "true" } else { "false" });
        let Some(value) = word.or(value.as_str()) else { return Err("every field value is a string or a boolean") };
        if !name_ok(name) { return Err("a field name has no space and no colon") }
        // Space around a value is not content in the Report grammar; a block's first line also sets its width.
        let value = value.trim();
        if value.contains('\n') || value.is_empty() || value == "|" {
            text.push_str(&format!("{name}: |\n"));
            for line in value.split('\n') { text.push_str(&format!("  {line}\n")); }
        } else {
            text.push_str(&format!("{name}: {value}\n"));
        }
    }
    Ok(text)
}

/// The object form of a Report text the Core produced (a skeleton, a close report to resend).
/// Text that is not a Report stays a string: the caller shows it as it is.
pub(crate) fn to_object(text: &str) -> Value {
    match crate::Report::parse(text) {
        Ok(report) => Value::Object(report.field_names().map(|name| (name.to_owned(), json!(report.get(name).unwrap_or_default()))).collect()),
        Err(_) => json!(text),
    }
}

/// Turns the object body of this call into the text the Core reads. `Err` is the wire refusal.
pub(super) fn accept(tool: &str, input: &mut Value) -> Result<(), String> {
    let Some(field) = field_of(tool) else { return Ok(()) };
    let Some(body) = input.get(field) else { return Ok(()) };
    let shape = match field {
        "contract" => "{\"objective\": \"…\", \"next_action\": \"…\", \"done_when\": \"…\"}",
        _ => "{\"<field>\": \"<value>\", …} with the fields of the current step's Report",
    };
    let wrong = |why: &str| format!("{tool}: '{field}' is an object of string fields, for example {shape}. {why} This call was not executed and nothing was {}. Send the same content as an object.",
        if tool == "gil_open" { "opened" } else { "closed" });
    let text = match body {
        Value::Object(fields) => to_text(fields).map_err(|why| wrong(&format!("Here {why}.")))?,
        Value::String(_) => return Err(wrong("A single text with `name: value` lines is not accepted.")),
        _ => return Err(wrong("")),
    };
    input[field] = json!(text);
    Ok(())
}

/// The catalog as a client sees it: the two bodies are objects. Validation and dispatch keep the
/// internal catalog, which they read after `accept` turned the body into text.
pub(super) fn present(tools: &[Tool]) -> Vec<Tool> {
    tools.iter().map(|tool| {
        let Some(field) = field_of(tool.name.as_ref()) else { return tool.clone() };
        let mut wire = serde_json::to_value(tool).expect("tool");
        if wire["inputSchema"]["properties"].get(field).is_none() { return tool.clone() }
        // The tool's own description spoke of a string body; the wire has one form.
        if let Some(description) = wire["description"].as_str().map(|text| text.replace(" STRING", " OBJECT")) {
            wire["description"] = json!(description);
        }
        let property = wire["inputSchema"]["properties"].get_mut(field).expect("checked above");
        *property = match field {
            "contract" => json!({"type":"object", "additionalProperties":{"type":"string"},
                "description":"The action contract of the step to open, as an object of string fields: {\"objective\": \"…\", \"next_action\": \"…\", \"done_when\": \"…\"}. For kind experiment in an exploration Chain it is the requirement link object gil_plan_read.experiment_link shows. Never one text with `name: value` lines. GIL validates the fields."}),
            _ => json!({"type":"object", "additionalProperties":{"type":["string","boolean"]},
                "description":"The Report of the step or Cycle to close, as an object of string fields (a field that asks for true or false takes a JSON boolean) named as the current close contract names them (gil_help topic close, or the skeleton a refusal shows), for example {\"execution\": \"…\", \"result\": \"…\", \"evidence_files\": \"evidence/a.md\\nevidence/b.md\"}. A value with several lines is one string with line breaks; a dotted name such as next_direction.action is a key as it stands. Never one text with `name: value` lines."}),
        };
        serde_json::from_value(wire).expect("tool")
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_object_reads_back_as_the_same_fields_whatever_its_values_hold() {
        let fields = json!({"objective":"keep: the colon", "next_action":" leading space", "done_when":"first\n\n  third: line\n", "next_direction.action":"|", "empty":""});
        let text = to_text(fields.as_object().unwrap()).unwrap();
        let report = crate::Report::parse(&text).unwrap();
        assert_eq!(report.get("objective"), Some("keep: the colon"));
        assert_eq!(report.get("next_action"), Some("leading space"), "space around a value is not content");
        assert_eq!(report.get("done_when"), Some("first\n\n  third: line"));
        assert_eq!(report.get("next_direction.action"), Some("|"));
        assert_eq!(report.get("empty"), Some(""));
        assert_eq!(to_object(&text)["objective"], "keep: the colon");
    }

    #[test]
    fn a_text_body_and_a_bad_name_are_refused_without_echoing_the_body() {
        let mut text = json!({"project_root":"/p", "contract":"objective: private words\n done_when: x\n"});
        let said = accept("gil_open", &mut text).unwrap_err();
        assert!(said.contains("object of string fields") && said.contains("nothing was opened") && !said.contains("private words"), "{said}");
        for bad in [json!({"a b":"x"}), json!({"a:":"x"}), json!({"":"x"}), json!({"a":1}), json!({"a":{"b":"c"}})] {
            let mut input = json!({"project_root":"/p", "report":bad});
            assert!(accept("gil_close", &mut input).unwrap_err().contains("nothing was closed"));
        }
        let mut ok = json!({"project_root":"/p", "report":{"verdict":"success", "problem_solved":true, "success_condition_met":false}});
        accept("gil_close", &mut ok).unwrap();
        assert_eq!(ok["report"], "problem_solved: true\nsuccess_condition_met: false\nverdict: success\n", "a boolean is written as its word");
        let mut other = json!({"project_root":"/p", "draft":"untouched"});
        accept("gil_plan_propose", &mut other).unwrap();
        assert_eq!(other["draft"], "untouched");
    }
}
