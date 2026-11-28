//! Test callers write `gil_open` contracts and `gil_close` Reports as Report text, the form the
//! Core keeps. The MCP wire takes them as objects of string fields (src/mcp/body.rs), so the test
//! clients translate here, in one place. A text the Report grammar refuses is sent as it stands:
//! the wire then refuses it as a string, which is what an agent sending such a text would meet.
use serde_json::{Value, json};

#[allow(dead_code)]
pub fn objects(name: &str, mut arguments: Value) -> Value {
    if name == "gil_plan_propose" {
        // Fixtures name an item's deciding observation `done_when`, as the record stores it.
        if let Some(items) = arguments.pointer_mut("/draft/items").and_then(Value::as_array_mut) {
            for item in items.iter_mut().filter_map(Value::as_object_mut) {
                if let Some(observation) = item.remove("done_when") {
                    item.insert("observation".into(), observation);
                    item.entry("source").or_insert_with(|| json!("the synthetic request of this test"));
                    item.entry("command").or_insert_with(|| json!("cat evidence/result.md"));
                    item.entry("part_of").or_insert_with(|| json!("M1"));
                }
            }
        }
        return arguments;
    }
    let field = match name { "gil_open" => "contract", "gil_close" => "report", _ => return arguments };
    if let Some(text) = arguments.get(field).and_then(Value::as_str) {
        if let Ok(report) = gil::Report::parse(text) {
            let fields: serde_json::Map<String, Value> = report.field_names()
                .map(|name| (name.to_owned(), json!(report.get(name).unwrap_or_default()))).collect();
            // Fixtures answer an Analysis with yes or no; an exploration Chain takes true or false.
            let mut fields = fields;
            for answer in ["problem_solved", "success_condition_met"] {
                match fields.get(answer).and_then(Value::as_str) {
                    Some("yes") => { fields.insert(answer.into(), json!(true)); }
                    Some("no") => { fields.insert(answer.into(), json!(false)); }
                    _ => {}
                }
            }
            // The exploration contract asks a Verify what it ran and saw, and an Analysis what it left unverified.
            if fields.contains_key("execution") && fields.contains_key("evidence_files") {
                fields.entry("command").or_insert_with(|| json!("the fixture's write"));
                fields.entry("observed").or_insert_with(|| json!("the fixture's file"));
            }
            // A Hypothesis names its change, and a failed Experiment's Outcome what it changed.
            // An Interpretation names what judges the work, its hardest part and what cannot be observed here.
            if fields.contains_key("interpretation") && !fields.contains_key("hypothesis_fit") && !fields.contains_key("evidence.verify_ref") {
                for (field, value) in [("major_problems", "M1: the fixture's result — judged by the fixture's check"), ("anti_goals", "none in a fixture"),
                    ("edge_cases", "none in a fixture"), ("hardest", "nothing in a fixture"), ("not_observable_here", "none")] {
                    fields.entry(field).or_insert_with(|| json!(value));
                }
            }
            if fields.contains_key("hypothesis") {
                fields.entry("change").or_insert_with(|| json!("write the fixture's files"));
            }
            if fields.get("verdict").and_then(Value::as_str) == Some("failure") && fields.contains_key("lesson") && !fields.contains_key("outcome_ref") {
                fields.entry("changed").or_insert_with(|| json!("the fixture's files"));
            }
            // An Experiment's Cycle Report and a Chain Report say what was established and why.
            if fields.contains_key("outcome_ref") || fields.contains_key("closing_synthesis_ref") {
                fields.entry("analysis").or_insert_with(|| json!("the fixture's result"));
                fields.entry("reflection").or_insert_with(|| json!("nothing to change in a fixture"));
            }
            if fields.contains_key("closing_synthesis_ref") {
                fields.entry("next_direction.action").or_insert_with(|| json!("end"));
                fields.entry("next_direction.reason").or_insert_with(|| json!("the fixture ends here"));
            }
            if fields.contains_key("problem_solved") {
                fields.entry("unverified").or_insert_with(|| json!("none"));
            }
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

/// A model reads an item's deciding observation as `observation` (and its `source`); fixtures and
/// the App's view name it `done_when`. This gives a model-read value the fixture's names back, so
/// the two can be compared field for field.
#[allow(dead_code)]
pub fn as_stored(mut value: Value) -> Value {
    fn walk(value: &mut Value) {
        match value {
            Value::Object(fields) => {
                if fields.contains_key("id") && fields.contains_key("question") {
                    if let Some(observation) = fields.remove("observation") { fields.insert("done_when".into(), observation); }
                    fields.remove("source");
                    fields.remove("command");
                    fields.remove("part_of");
                }
                fields.values_mut().for_each(walk);
            }
            Value::Array(values) => values.iter_mut().for_each(walk),
            _ => {}
        }
    }
    walk(&mut value);
    value
}
