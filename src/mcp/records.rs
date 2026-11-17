//! Model-visible, read-only path from an approved requirement to the original records.
//!
//! `gil_plan_read` already says which Experiments were linked to a requirement. What it could
//! not do was open them: the Step Reports of an earlier Cycle were reachable only through the
//! whole `gil_story`, the canonical reference of an earlier Verify was not listed anywhere a
//! model could read, and the files a Verify stored as evidence could not be read once a later
//! Verify or an approved revisit had changed the working folder.
//!
//! Two reads close that gap, and neither adds a judgment:
//!
//! ```text
//! gil_report_read    one Chain (its Cycles' references and links, its Chain Report),
//!                    one Cycle (where it sits, its Steps' references, its Cycle Report)
//!                    or one Step Report, fields exactly as stored
//! gil_evidence_read  the bytes of one file a closed Verify declared (evidence_files or
//!                    plan_document), from the Snapshot that Verify confirmed — never from
//!                    the working folder
//! ```
//!
//! Nothing here summarizes, merges, ranks or scores. A failed or revisited Experiment reads the
//! same way as a successful one, two records that disagree are each returned as written, and no
//! field says that a requirement is answered. The session is a [`ReadOnlySession`]: there is
//! no commit, no restore and no observation of the working folder behind these tools.
//!
//! Registered only in an `exploration-flow` build, where requirements and Experiments are
//! linked. They read any Chain that build can load, marked or not.
use crate::session::{EvidenceChunk, EvidenceIndex, StoredEvidenceError};
use crate::{Chain, ChainRef, Cycle, CycleRef, NodeKind, Project, ProjectSession, ReadOnlySession, Report, RuleSet, StepRef};
use base64::Engine;
use rmcp::model::{CallToolResult, Tool};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::Path;

pub(super) const REPORT: &str = "gil_report_read";
pub(super) const EVIDENCE: &str = "gil_evidence_read";
pub(super) const NAMES: [&str; 2] = [REPORT, EVIDENCE];
/// Stored evidence bytes carried by one answer. The rest is read with `offset`.
pub(super) const CHUNK: usize = 16 * 1024;
/// One answer never exceeds this, text and structured copies together.
const RESPONSE_LIMIT: usize = 1024 * 1024;
const SCHEMA_VERSION: u32 = 1;

#[derive(Deserialize)]
#[serde(tag = "kind", content = "ref", rename_all = "snake_case", deny_unknown_fields)]
enum Owner {
    Step(String),
    Cycle(String),
    Chain(String),
}
enum Asked {
    Step(StepRef),
    Cycle(CycleRef),
    Chain(ChainRef),
}
impl Owner {
    /// Only the canonical spelling: a reference that merely parses is not the stored address.
    fn asked(&self) -> Option<Asked> {
        match self {
            Owner::Step(text) => text.parse::<StepRef>().ok().filter(|r| r.to_string() == *text).map(Asked::Step),
            Owner::Cycle(text) => text.parse::<CycleRef>().ok().filter(|r| r.to_string() == *text).map(Asked::Cycle),
            Owner::Chain(text) => text.parse::<ChainRef>().ok().filter(|r| r.to_string() == *text).map(Asked::Chain),
        }
    }
    fn json(&self) -> Value {
        match self {
            Owner::Step(text) => json!({"kind":"step","ref":text}),
            Owner::Cycle(text) => json!({"kind":"cycle","ref":text}),
            Owner::Chain(text) => json!({"kind":"chain","ref":text}),
        }
    }
    fn text(&self) -> &str {
        match self {
            Owner::Step(text) | Owner::Cycle(text) | Owner::Chain(text) => text,
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadReport {
    project_root: String,
    owner: Owner,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadEvidence {
    project_root: String,
    owner: Owner,
    path: String,
    #[serde(default)]
    offset: Option<u64>,
}

pub(super) fn valid(name: &str, input: &Value) -> bool {
    if serde_json::to_vec(input).map_or(true, |bytes| bytes.len() > 16 * 1024) {
        return false;
    }
    match name {
        REPORT => serde_json::from_value::<ReadReport>(input.clone())
            .is_ok_and(|r| Path::new(&r.project_root).is_absolute() && r.owner.asked().is_some()),
        EVIDENCE => serde_json::from_value::<ReadEvidence>(input.clone()).is_ok_and(|r| {
            Path::new(&r.project_root).is_absolute()
                && matches!(r.owner.asked(), Some(Asked::Step(_)))
                && !r.path.trim().is_empty() && r.path.len() <= 4096 && !r.path.contains('\0')
                // `offset: null` is not "from the start"; leave the field out for that.
                && !input.get("offset").is_some_and(Value::is_null)
        }),
        _ => false,
    }
}

/// Static explanation of a schema rejection: names the shape, echoes no input.
pub(super) fn rejection(name: &str) -> Option<&'static str> {
    match name {
        REPORT => Some("gil_report_read: expected exactly {project_root (absolute string), owner: {kind: \"chain\" | \"cycle\" | \"step\", ref}} with a canonical ref such as chain:C1, cycle:C3 or step:C3/S3. Nothing was read or changed. Take Cycle refs from gil_plan_read.requirements[].experiments[].cycle_ref or gil_report_read(owner: chain).cycles, and Step refs from gil_report_read(owner: cycle).steps."),
        EVIDENCE => Some("gil_evidence_read: expected exactly {project_root (absolute string), owner: {kind: \"step\", ref: <a Verify's canonical step ref>}, path (one project-relative path that Verify declared), offset (optional non-negative integer; omit it to start at 0)}. Nothing was read or changed. Read gil_report_read(owner: that Verify) for evidence.files."),
        _ => None,
    }
}

fn refusal(code: &str, said: String, extra: Value) -> CallToolResult {
    let mut body = json!({"ok":false,"code":code,"said":said});
    if let (Some(fields), Some(more)) = (body.as_object_mut(), extra.as_object()) {
        for (key, value) in more { fields.insert(key.clone(), value.clone()); }
    }
    let mut result = CallToolResult::structured(body);
    result.is_error = Some(true);
    result
}

fn open(root: &str) -> Result<ReadOnlySession, CallToolResult> {
    let unavailable = |why: String| refusal("record_unavailable", crate::tr!("record_tool.unavailable", why = why), json!({}));
    let root = Path::new(root);
    let rules = RuleSet::builtin().map_err(|error| unavailable(error.to_string()))?;
    let state = crate::project_state::state_for_root(root).map_err(unavailable)?;
    ProjectSession::open_read_only_at(rules, root, &state).map_err(|error| unavailable(error.to_string()))
}

/// The Cycle a canonical reference names, and the Chain that owns it (none in a legacy record).
/// Compared by canonical address only: no nearest number, no other Chain's Cycle of the same rank.
fn locate(project: &Project, asked: CycleRef) -> Option<(Option<&Chain>, &Cycle)> {
    if project.chains().is_empty() {
        return project.cycles().nodes().iter().find(|cycle| cycle.id().to_ref() == asked).map(|cycle| (None, cycle));
    }
    project.chains().iter().find_map(|chain| {
        chain.cycles().nodes().iter().find(|cycle| cycle.id().to_ref() == asked).map(|cycle| (Some(chain), cycle))
    })
}

fn state(closed: bool) -> &'static str {
    if closed { "closed" } else { "open" }
}

fn chain_json(chain: Option<&Chain>) -> Value {
    chain.map_or(Value::Null, |chain| json!({"ref":chain.id().to_string(), "state":state(chain.is_closed())}))
}

/// One Chain as addresses: where it sits, each of its Cycles with the link it was opened under,
/// and its Chain Report. The approved requirements' own text is not re-projected here — the
/// current Chain's is in `gil_plan_read`, and any Chain's is in the `plan_document` its Verifies
/// stored.
fn chain_index(chain: &Chain) -> (Value, Vec<Value>) {
    let position = json!({"ref":chain.id().to_string(), "state":state(chain.is_closed()),
        "parent":chain.parent().map(|parent| parent.to_string()),
        "entry_snapshot":chain.entry_snapshot().to_string(),
        "exit_snapshot":chain.exit_snapshot().map(|world| world.to_string())});
    let cycles = chain.cycles().nodes().iter().map(|cycle| {
        let mut entry = cycle_json(cycle);
        entry["plan_link"] = plan_link(Some(chain), cycle);
        entry
    }).collect();
    (position, cycles)
}

/// Where the Cycle sits, as recorded. `verdict` is the Cycle Report's own word, not a reading
/// of what it means for a requirement.
fn cycle_json(cycle: &Cycle) -> Value {
    json!({"ref":cycle.id().to_ref().to_string(), "kind":cycle.kind().as_str(), "state":state(cycle.is_closed()),
        "verdict":cycle.report().filter(|_| cycle.is_closed()).and_then(|report| report.get("verdict")),
        "parent":cycle.parent().map(|id| id.to_ref().to_string()),
        "revisit_from":cycle.revisit_from().map(|id| id.to_ref().to_string()),
        "entry_snapshot":cycle.entry_snapshot().to_string(),
        "exit_snapshot":cycle.exit_snapshot().map(|world| world.to_string())})
}

/// The approved revision and the requirement (or queue item) this Cycle was opened under.
/// The link is the one recorded when the Experiment opened; nothing is inferred from prose.
fn plan_link(chain: Option<&Chain>, cycle: &Cycle) -> Value {
    let Some(plan) = chain.and_then(|chain| chain.approved_plan.as_ref()) else { return Value::Null; };
    let here = cycle.id().to_ref().to_string();
    let Some(link) = plan.executed_cycles.iter().find(|link| link.cycle_ref == here) else { return Value::Null; };
    let digest = plan.proposals.iter().find(|proposal| proposal.revision == link.revision).map(|proposal| proposal.digest.as_str());
    if plan.exploration() {
        json!({"contract":"exploration","plan_revision":link.revision,"plan_digest":digest,"requirement_id":link.item_id})
    } else {
        json!({"plan_revision":link.revision,"plan_digest":digest,"item_id":link.item_id})
    }
}

/// The same canonical field digest `gil_report_visuals` reports as `report_sha256`.
fn report_digest(report: &Report) -> String {
    let fields: BTreeMap<&str, &str> = report.field_names().map(|name| (name, report.get(name).unwrap_or_default())).collect();
    format!("{:x}", Sha256::digest(serde_json::to_vec(&fields).expect("string fields")))
}

/// A stored Report, every field by its own name and with its whole value. A Step or Cycle that
/// has not written one says so instead of showing an empty Report.
fn report_json(report: Option<&Report>) -> Value {
    match report {
        None => json!({"state":"not_written"}),
        Some(report) => json!({"state":"written", "sha256":report_digest(report),
            "fields":report.field_names().map(|name| json!({"name":name,"value":report.get(name).unwrap_or_default()})).collect::<Vec<_>>()}),
    }
}

fn is_boundary(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::CycleEntry | NodeKind::CycleExit)
}

/// The Steps of one Cycle as addresses only — which Report to open next is the reader's choice.
fn steps_json(cycle: &Cycle) -> Vec<Value> {
    cycle.steps().nodes().iter().filter(|node| !is_boundary(node.kind)).map(|node| {
        let mut step = json!({"ref":cycle.step_ref(node.id).to_string(), "kind":node.kind.as_str(),
            "state":state(node.is_closed()), "report":if node.report.is_some() { "written" } else { "not_written" }});
        if node.kind == NodeKind::Verify { step["snapshot"] = json!(node.snapshot.map(|world| world.to_string())); }
        if let Some(from) = node.revisit_from { step["revisit_from"] = json!(cycle.step_ref(from).to_string()); }
        step
    }).collect()
}

fn evidence_json(index: Result<EvidenceIndex, StoredEvidenceError>) -> Value {
    match index {
        Ok(index) => json!({"state":"indexed", "source":"stored_snapshot", "snapshot":index.snapshot.to_string(),
            "current_world":index.current_world.to_string(), "read_with":EVIDENCE,
            "files":index.files.iter().map(|file| match &file.sha256 {
                Some(sha256) => json!({"path":file.path, "declared_in":file.declared_in, "stored":"present", "sha256":sha256, "in_current_world":file.in_current_world.as_str()}),
                None => json!({"path":file.path, "declared_in":file.declared_in, "stored":"not_in_snapshot"}),
            }).collect::<Vec<_>>()}),
        Err(StoredEvidenceError::Unavailable { snapshot, said }) =>
            json!({"state":"unavailable", "snapshot":snapshot.map(|world| world.to_string()), "reason":said}),
        Err(_) => Value::Null,
    }
}

fn not_found(owner: &Owner) -> CallToolResult {
    refusal("not_found", crate::tr!("record_tool.not_found", owner = owner.text()), json!({"owner":owner.json()}))
}

fn bounded(value: Value) -> CallToolResult {
    let result = CallToolResult::structured(value);
    if serde_json::to_vec(&result).map_or(true, |bytes| bytes.len() > RESPONSE_LIMIT) {
        return refusal("response_too_large", crate::locale::text("record_tool.too_large").into(), json!({}));
    }
    result
}

fn read_report(request: ReadReport) -> CallToolResult {
    let Some(asked) = request.owner.asked() else { return not_found(&request.owner); };
    let session = match open(&request.project_root) { Ok(session) => session, Err(refused) => return refused };
    let project = session.project();
    let cycle_ref = match &asked {
        Asked::Step(step) => step.cycle(),
        Asked::Cycle(cycle) => *cycle,
        Asked::Chain(asked) => {
            let Some(chain) = project.chains().iter().find(|chain| chain.id() == *asked) else { return not_found(&request.owner); };
            let (position, cycles) = chain_index(chain);
            return bounded(json!({"ok":true, "schema_version":SCHEMA_VERSION, "source":"stored_record", "owner":request.owner.json(),
                "chain":position, "cycles":cycles, "report":report_json(chain.report()),
                "note":crate::locale::text("record_tool.note_report")}));
        }
    };
    let Some((chain, cycle)) = locate(project, cycle_ref) else { return not_found(&request.owner); };
    let mut body = json!({"ok":true, "schema_version":SCHEMA_VERSION, "source":"stored_record", "owner":request.owner.json(),
        "chain":chain_json(chain), "cycle":cycle_json(cycle), "plan_link":plan_link(chain, cycle),
        "note":crate::locale::text("record_tool.note_report")});
    let Asked::Step(step) = asked else {
        // A Cycle: its Steps as addresses and its own Report. No Step Report body travels here.
        body["steps"] = json!(steps_json(cycle));
        body["report"] = report_json(cycle.report());
        return bounded(body);
    };
    let Some(node) = cycle.steps().nodes().iter().find(|node| cycle.step_ref(node.id) == step && !is_boundary(node.kind))
        else { return not_found(&request.owner); };
    let mut described = json!({"ref":cycle.step_ref(node.id).to_string(), "kind":node.kind.as_str(), "state":state(node.is_closed())});
    if node.kind == NodeKind::Verify { described["snapshot"] = json!(node.snapshot.map(|world| world.to_string())); }
    if let Some(from) = node.revisit_from { described["revisit_from"] = json!(cycle.step_ref(from).to_string()); }
    body["step"] = described;
    body["report"] = report_json(node.report.as_ref());
    if node.kind == NodeKind::Verify && node.report.is_some() {
        body["evidence"] = evidence_json(session.verify_evidence_index(step));
    }
    bounded(body)
}

/// Text when the window is valid UTF-8 (a character cut by the window's end is left for the
/// next call), otherwise the same bytes in base64. Returns how many bytes the answer carries.
fn encoded(bytes: &[u8], more_follows: bool) -> (usize, &'static str, &'static str, String) {
    match std::str::from_utf8(bytes) {
        Ok(text) => (bytes.len(), "utf8", "text", text.to_owned()),
        Err(error) if more_follows && error.error_len().is_none() && error.valid_up_to() > 0 => {
            let kept = error.valid_up_to();
            (kept, "utf8", "text", String::from_utf8_lossy(&bytes[..kept]).into_owned())
        }
        Err(_) => (bytes.len(), "base64", "data_base64", base64::engine::general_purpose::STANDARD.encode(bytes)),
    }
}

fn read_evidence(request: ReadEvidence) -> CallToolResult {
    let Some(Asked::Step(step)) = request.owner.asked() else { return not_found(&request.owner); };
    let session = match open(&request.project_root) { Ok(session) => session, Err(refused) => return refused };
    let owner = request.owner.text();
    let offset = request.offset.unwrap_or(0);
    let chunk: EvidenceChunk = match session.read_verify_evidence(step, request.path.trim(), offset, CHUNK) {
        Ok(chunk) => chunk,
        Err(error) => return match error {
            StoredEvidenceError::NotFound => not_found(&request.owner),
            StoredEvidenceError::NotAVerify { kind } =>
                refusal("not_a_verify", crate::tr!("record_tool.not_a_verify", owner = owner, kind = kind.as_str()), json!({"owner":request.owner.json(),"kind":kind.as_str()})),
            StoredEvidenceError::ReportNotWritten =>
                refusal("report_not_written", crate::tr!("record_tool.report_not_written", owner = owner), json!({"owner":request.owner.json()})),
            StoredEvidenceError::InvalidPath =>
                refusal("invalid_path", crate::locale::text("record_tool.invalid_path").into(), json!({})),
            StoredEvidenceError::NotDeclared { declared } =>
                refusal("not_declared", crate::tr!("record_tool.not_declared", owner = owner), json!({"owner":request.owner.json(),"declared":declared})),
            StoredEvidenceError::NotInSnapshot { snapshot } =>
                refusal("not_in_snapshot", crate::tr!("record_tool.not_in_snapshot", snapshot = snapshot), json!({"owner":request.owner.json(),"snapshot":snapshot.to_string()})),
            StoredEvidenceError::Unavailable { snapshot, said } =>
                refusal("evidence_unavailable", crate::tr!("record_tool.evidence_unavailable", why = said), json!({"owner":request.owner.json(),"snapshot":snapshot.map(|world| world.to_string())})),
            StoredEvidenceError::Offset { total } =>
                refusal("invalid_offset", crate::tr!("record_tool.offset", total = total), json!({"total_bytes":total})),
        },
    };
    let project = session.project();
    let Some((chain, cycle)) = locate(project, step.cycle()) else { return not_found(&request.owner); };
    let more_follows = offset + (chunk.bytes.len() as u64) < chunk.total_bytes;
    let (carried, encoding, field, data) = encoded(&chunk.bytes, more_follows);
    let end = offset + carried as u64;
    let truncated = end < chunk.total_bytes;
    let mut note = crate::locale::text("record_tool.note_evidence").to_string();
    if truncated {
        note.push(' ');
        note.push_str(&crate::tr!("record_tool.note_more", from = offset, to = end, total = chunk.total_bytes, next = end));
    }
    let mut body = json!({"ok":true, "schema_version":SCHEMA_VERSION, "source":"stored_snapshot", "owner":request.owner.json(),
        "chain":chain_json(chain), "cycle":cycle_json(cycle), "plan_link":plan_link(chain, cycle),
        "snapshot":chunk.snapshot.to_string(), "path":request.path.trim(), "sha256":chunk.sha256, "total_bytes":chunk.total_bytes,
        "offset":offset, "returned_bytes":carried, "truncated":truncated,
        "next_offset":if truncated { Some(end) } else { None }, "remaining_bytes":chunk.total_bytes - end,
        "encoding":encoding, "current_world":chunk.current_world.to_string(),
        "in_current_world":chunk.in_current_world.as_str(), "note":note});
    body[field] = json!(data);
    bounded(body)
}

pub(super) fn call(name: &str, input: Value) -> CallToolResult {
    let bad = || refusal("invalid_request", rejection(name).unwrap_or("Invalid GIL tool arguments").to_string(), json!({}));
    if !valid(name, &input) { return bad(); }
    match name {
        REPORT => serde_json::from_value(input).map_or_else(|_| bad(), read_report),
        EVIDENCE => serde_json::from_value(input).map_or_else(|_| bad(), read_evidence),
        _ => bad(),
    }
}

pub(super) fn catalog(mut tools: Vec<Tool>) -> Vec<Tool> {
    let owner = |kinds: Value| json!({"type":"object","additionalProperties":false,"required":["kind","ref"],
        "properties":{"kind":{"enum":kinds},"ref":{"type":"string","description":"Canonical reference exactly as GIL printed it: chain:C<n>, cycle:C<n> or step:C<n>/S<m>."}}});
    let root = json!({"type":"string","description":"Exact absolute project root explicitly selected by the user."});
    let visuals = if cfg!(feature = "visual-authoring") {
        " Pixels of visual assets stay with the Monitor; gil_report_visuals with the same owner lists their metadata."
    } else { "" };
    for (name, description, schema) in [
        (REPORT,
            "Read one stored Report verbatim by canonical reference. Changes nothing. owner {kind: cycle, ref} returns where that Cycle sits (Chain, parent, revisit source, entry/exit Snapshot, and the approved revision and requirement it was opened under), the references, kinds and states of its Steps without their bodies, and its Cycle Report; owner {kind: step, ref} returns that one Step Report and, for a closed Verify, each file it declared (evidence_files, plan_document) as its own Snapshot holds it; owner {kind: chain, ref} returns that Chain's Cycles as references with their links, and its Chain Report, which is how an earlier Chain's Experiments are found. Start from gil_plan_read.requirements[].experiments[].cycle_ref, read the Cycle for its Step references, then open only the Reports the next decision needs. report.fields are the stored original, every field with its own name and whole value: nothing is summarized, merged, ranked or scored. Failed and revisited Experiments read exactly like successful ones, and records that disagree are each returned as written; this tool never picks one. A record existing is not a finding that it is true or still valid, and no field says a requirement is answered: that is decided only in the user-approved Closing Interview. An open Step or Cycle answers report.state not_written. An unknown reference is refused (not_found) and no nearby record is substituted.".to_string(),
            json!({"type":"object","additionalProperties":false,"required":["project_root","owner"],
                "properties":{"project_root":root,"owner":owner(json!(["step","cycle","chain"]))}})),
        (EVIDENCE,
            format!("Read the stored bytes of one file that a closed Verify declared in evidence_files (or as plan_document, the exact approved revision it ran under), from the Snapshot that Verify confirmed. Changes nothing and restores nothing. owner is that Verify {{kind: step, ref}}; path is one of gil_report_read(owner: that Verify).evidence.files[].path. It never reads the working folder: a file of the same name there may hold other content after a later Verify or an approved revisit, so a failed Experiment's evidence stays readable here after the folder was restored. in_current_world (same | different | absent | unknown) compares the stored bytes with the latest confirmed world only; the working folder is not observed. Each call carries at most 16 KiB, as text (encoding utf8) or data_base64; sha256 and total_bytes describe the whole stored file, and when truncated is true the rest is read by calling again with offset: next_offset. Refusals keep different facts apart: report_not_written (the Verify is still open), not_a_verify, not_declared (the Report never named that path; declared lists what it named), not_in_snapshot, evidence_unavailable (the store could not be read) and not_found. None of them means the evidence was judged absent or false.{visuals}"),
            json!({"type":"object","additionalProperties":false,"required":["project_root","owner","path"],
                "properties":{"project_root":root,"owner":owner(json!(["step"])),
                    "path":{"type":"string","description":"One project-relative path the Verify Report declared in evidence_files or as plan_document."},
                    "offset":{"type":"integer","minimum":0,"description":"Byte offset to continue from; omit to start at 0. Use next_offset from the previous answer."}}})),
    ] {
        tools.push(serde_json::from_value(json!({"name":name, "description":description, "inputSchema":schema,
            "annotations":{"readOnlyHint":true,"destructiveHint":false,"openWorldHint":false},
            "_meta":{"ui":{"visibility":["model"]}}})).expect("record read catalog"));
    }
    tools
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> &'static str {
        if cfg!(windows) { "C:/synthetic" } else { "/synthetic" }
    }

    #[test]
    fn both_reads_are_model_visible_read_only_and_strict() {
        let tools = catalog(vec![]);
        assert_eq!(tools.iter().map(|tool| tool.name.as_ref()).collect::<Vec<_>>(), NAMES);
        for tool in &tools {
            let wire = serde_json::to_value(tool).unwrap();
            assert_eq!(wire["_meta"]["ui"]["visibility"], json!(["model"]), "{}", tool.name);
            assert_eq!(wire["annotations"]["readOnlyHint"], true, "{}", tool.name);
            assert_eq!(wire["inputSchema"]["additionalProperties"], false, "{}", tool.name);
        }
    }

    #[test]
    fn only_canonical_references_and_declared_shapes_are_accepted() {
        let report = |owner: Value| json!({"project_root":root(),"owner":owner});
        assert!(valid(REPORT, &report(json!({"kind":"cycle","ref":"cycle:C3"}))));
        assert!(valid(REPORT, &report(json!({"kind":"step","ref":"step:C3/S4"}))));
        assert!(valid(REPORT, &report(json!({"kind":"chain","ref":"chain:C1"}))));
        for owner in [json!({"kind":"cycle","ref":"cycle:C03"}), json!({"kind":"cycle","ref":"step:C3/S4"}),
            json!({"kind":"step","ref":"cycle:C3"}), json!({"kind":"chain","ref":"cycle:C1"}), json!({"kind":"snapshot","ref":"snapshot:A1"}),
            json!({"kind":"step","ref":"step:C3/S4","path":"x"}), json!("cycle:C3")] {
            assert!(!valid(REPORT, &report(owner.clone())), "{owner}");
        }
        assert!(!valid(REPORT, &json!({"project_root":"relative","owner":{"kind":"cycle","ref":"cycle:C3"}})));
        assert!(!valid(REPORT, &json!({"project_root":root(),"owner":{"kind":"cycle","ref":"cycle:C3"},"path":"evidence/a.md"})));

        let evidence = |extra: Value| {
            let mut input = json!({"project_root":root(),"owner":{"kind":"step","ref":"step:C3/S3"},"path":"evidence/a.md"});
            for (key, value) in extra.as_object().unwrap() { input[key] = value.clone(); }
            input
        };
        assert!(valid(EVIDENCE, &evidence(json!({}))));
        assert!(valid(EVIDENCE, &evidence(json!({"offset":16384}))));
        for extra in [json!({"offset":-1}), json!({"offset":null}), json!({"offset":"0"}), json!({"path":" "}),
            json!({"owner":{"kind":"cycle","ref":"cycle:C3"}}), json!({"owner":{"kind":"chain","ref":"chain:C1"}}), json!({"sha256":"x"})] {
            assert!(!valid(EVIDENCE, &evidence(extra.clone())), "{extra}");
        }
        for name in NAMES { assert!(rejection(name).unwrap().contains("Nothing was read or changed")); }
    }

    #[test]
    fn a_window_cut_inside_a_character_leaves_that_character_for_the_next_call() {
        let text = "가나다".as_bytes();
        // Seven bytes: two whole characters and the first byte of the third.
        let (carried, encoding, field, data) = encoded(&text[..7], true);
        assert_eq!((carried, encoding, field, data.as_str()), (6, "utf8", "text", "가나"));
        // The same cut at the very end of a file is not text.
        assert_eq!(encoded(&text[..7], false).1, "base64");
        // A window that starts inside a character is not text either; nothing is guessed.
        assert_eq!(encoded(&text[1..], true).1, "base64");
        let (carried, encoding, _, data) = encoded(&[0x89, b'P', b'N', b'G'], false);
        assert_eq!((carried, encoding), (4, "base64"));
        assert_eq!(base64::engine::general_purpose::STANDARD.decode(data).unwrap(), [0x89, b'P', b'N', b'G']);
        assert_eq!(encoded(b"", false), (0, "utf8", "text", String::new()));
    }
}
