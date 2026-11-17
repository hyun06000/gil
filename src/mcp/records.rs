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
//! # Every stored original can be read to its end
//!
//! An answer has a size limit, and a stored Report has none: a value full of quotes and
//! backslashes doubles when it is written as JSON and doubles again inside the text copy of the
//! answer. So no answer is ever "the whole thing or a refusal". Each one is assembled under a
//! byte budget counted the way it travels, and whatever it leaves out is named together with the
//! input that reads it:
//!
//! ```text
//! a list that did not fit      more: [{list, from}]      → the same call with list and from
//! a value or name left out     read: {field, part?}      → the same call with field (and offset)
//! a range that stops early     truncated, next_offset    → the same call with that offset
//! ```
//!
//! `truncated: true` says an answer left something out; `truncated: false` says it did not. (The
//! word is about the answer. Nothing here says a record or a requirement is complete.) A range
//! is cut on a character boundary, lists are cut between entries, and the pieces put back
//! together are the stored bytes — `report.sha256` and each range's `sha256` let a reader check.
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
/// Stored bytes one range carries: an evidence file, a field value or a field name. The rest
/// is read with `offset`.
pub(super) const CHUNK: usize = 16 * 1024;
/// A field value at most this long travels inside a Report overview or a `fields` page.
const INLINE_VALUE: usize = CHUNK;
/// Value bytes one overview or page carries in total; later fields are listed without values.
const INLINE_TOTAL: usize = 64 * 1024;
/// A field name, an evidence path or a requirement id at most this long is written out in full.
const INLINE_NAME: usize = 256;
/// Bytes the variable parts of one answer may take on the wire (structured and text copies).
/// Half the hard limit: the fixed parts are small, and the largest single entry — an inline
/// value of `INLINE_VALUE` bytes that are all control characters — stays well below it.
const WIRE_BUDGET: usize = 512 * 1024;
/// One answer never exceeds this, text and structured copies together. The budget above keeps
/// every answer inside it; the check in [`bounded`] is a backstop, not a way to answer.
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
    /// One page of a list this owner has, starting at `from`.
    #[serde(default)]
    list: Option<List>,
    #[serde(default)]
    from: Option<u64>,
    /// One field of the owner's Report by its `index`, read as a byte range from `offset`.
    #[serde(default)]
    field: Option<u64>,
    #[serde(default)]
    part: Option<Part>,
    #[serde(default)]
    offset: Option<u64>,
}
#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum List {
    Cycles,
    Steps,
    Fields,
    Files,
}
impl List {
    fn name(self) -> &'static str {
        match self {
            List::Cycles => "cycles",
            List::Steps => "steps",
            List::Fields => "fields",
            List::Files => "files",
        }
    }
}
#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Part {
    Value,
    Name,
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
        REPORT => serde_json::from_value::<ReadReport>(input.clone()).is_ok_and(|r| {
            Path::new(&r.project_root).is_absolute() && r.owner.asked().is_some()
                // An absent selector is "the overview"; `null` is not a spelling of absent.
                && !["list", "from", "field", "part", "offset"].iter().any(|key| input.get(key).is_some_and(Value::is_null))
                // Either a list page or a field range, never both; a position needs its selector.
                && !(r.list.is_some() && r.field.is_some())
                && (r.list.is_some() || r.from.is_none())
                && (r.field.is_some() || (r.part.is_none() && r.offset.is_none()))
        }),
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
        REPORT => Some("gil_report_read: expected {project_root (absolute string), owner: {kind: \"chain\" | \"cycle\" | \"step\", ref}} with a canonical ref such as chain:C1, cycle:C3 or step:C3/S3, and optionally ONE continuation copied from a previous answer: {list: \"cycles\" | \"steps\" | \"fields\" | \"files\", from (non-negative integer)} for the rest of a list, or {field (the field's index), part (\"value\" | \"name\"; omit for value), offset (non-negative integer; omit to start at 0)} for a byte range of one field. Not both, no from without list, no part or offset without field, no null. Nothing was read or changed. Take Cycle refs from gil_plan_read.requirements[].experiments[].cycle_ref or gil_report_read(owner: chain).cycles, and Step refs from gil_report_read(owner: cycle).steps."),
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

/// Where a Chain sits, as recorded.
fn chain_position(chain: &Chain) -> Value {
    json!({"ref":chain.id().to_string(), "state":state(chain.is_closed()),
        "parent":chain.parent().map(|parent| parent.to_string()),
        "entry_snapshot":chain.entry_snapshot().to_string(),
        "exit_snapshot":chain.exit_snapshot().map(|world| world.to_string())})
}

/// Where the Cycle sits, as recorded. `verdict` is the Cycle Report's own word, not a reading
/// of what it means for a requirement.
fn cycle_json(cycle: &Cycle) -> Value {
    json!({"ref":cycle.id().to_ref().to_string(), "kind":cycle.kind().as_str(), "state":state(cycle.is_closed()),
        "verdict":cycle.report().filter(|_| cycle.is_closed()).and_then(|report| report.get("verdict")).map(short),
        "parent":cycle.parent().map(|id| id.to_ref().to_string()),
        "revisit_from":cycle.revisit_from().map(|id| id.to_ref().to_string()),
        "entry_snapshot":cycle.entry_snapshot().to_string(),
        "exit_snapshot":cycle.exit_snapshot().map(|world| world.to_string())})
}

/// A label that is written out when it is short, and otherwise only measured: its length and
/// digest. Position data repeats in every answer, so it never carries an unbounded string.
fn short(text: &str) -> Value {
    if text.len() <= INLINE_NAME {
        json!(text)
    } else {
        json!({"omitted":true, "bytes":text.len(), "sha256":format!("{:x}", Sha256::digest(text.as_bytes()))})
    }
}

/// The approved revision and the requirement (or queue item) this Cycle was opened under.
/// The link is the one recorded when the Experiment opened; nothing is inferred from prose.
/// An id too long to repeat in every answer is measured here; its text is in `gil_plan_read`
/// and in the `plan_document` the Experiment's Verify stored.
fn plan_link(chain: Option<&Chain>, cycle: &Cycle) -> Value {
    let Some(plan) = chain.and_then(|chain| chain.approved_plan.as_ref()) else { return Value::Null; };
    let here = cycle.id().to_ref().to_string();
    let Some(link) = plan.executed_cycles.iter().find(|link| link.cycle_ref == here) else { return Value::Null; };
    let digest = plan.proposals.iter().find(|proposal| proposal.revision == link.revision).map(|proposal| proposal.digest.as_str());
    if plan.exploration() {
        json!({"contract":"exploration","plan_revision":link.revision,"plan_digest":digest,"requirement_id":short(&link.item_id)})
    } else {
        json!({"plan_revision":link.revision,"plan_digest":digest,"item_id":short(&link.item_id)})
    }
}

/// The same canonical field digest `gil_report_visuals` reports as `report_sha256`.
fn report_digest(report: &Report) -> String {
    let fields: BTreeMap<&str, &str> = report.field_names().map(|name| (name, report.get(name).unwrap_or_default())).collect();
    format!("{:x}", Sha256::digest(serde_json::to_vec(&fields).expect("string fields")))
}

/// The fields of a stored Report in their stored order. A field's position in this order is its
/// `index`; a closed Report never changes, so the index is a stable address.
fn fields_of(report: &Report) -> Vec<(&str, &str)> {
    report.field_names().map(|name| (name, report.get(name).unwrap_or_default())).collect()
}

/// What a stored Report is, without its fields. A Step, Cycle or Chain that has not written one
/// says so instead of showing an empty Report.
fn report_head(report: Option<&Report>) -> Value {
    match report {
        None => json!({"state":"not_written"}),
        Some(report) => json!({"state":"written", "sha256":report_digest(report), "field_count":report.field_names().count()}),
    }
}

fn is_boundary(kind: NodeKind) -> bool {
    matches!(kind, NodeKind::CycleEntry | NodeKind::CycleExit)
}

// ── the budget of one answer ───────────────────────────────────────────────

/// What one answer may still carry.
#[derive(Clone, Copy)]
struct Budget {
    /// Bytes on the wire for the variable parts.
    wire: usize,
    /// Field value bytes carried inline.
    inline: usize,
}
impl Budget {
    fn new() -> Budget {
        Budget { wire: WIRE_BUDGET, inline: INLINE_TOTAL }
    }
}

/// What a piece adds to an answer on the wire. The answer carries its JSON twice — once as
/// `structuredContent`, once as the text copy, where the same document is a JSON *string* and
/// every quote and backslash in it is escaped again. Both lengths add up piece by piece, so the
/// size of an answer is known before it is built.
fn wire(piece: &Value) -> usize {
    let document = piece.to_string();
    let as_text = serde_json::to_string(&document).map_or(usize::MAX / 4, |quoted| quoted.len());
    document.len().saturating_add(as_text)
}

/// One stretch of a list: the entries that fit, and where the rest starts.
struct Page {
    items: Vec<Value>,
    next: Option<usize>,
    total: usize,
    /// An entry left a value, a name or a path out.
    partial: bool,
}

/// The entries of `all` from `from` on that fit the budget. `forced` carries the first entry
/// even when it does not fit, so that a page asked for by position always advances. `Err` is
/// the list's length when `from` lies beyond it.
fn page<T>(
    all: &[T], from: usize, budget: &mut Budget, forced: bool,
    entry: impl Fn(usize, &T, &mut Budget) -> (Value, bool),
) -> Result<Page, usize> {
    if from > all.len() || (from == all.len() && from > 0) {
        return Err(all.len());
    }
    let (mut items, mut partial, mut at) = (Vec::new(), false, from);
    while at < all.len() {
        // The entry may spend the inline allowance; it counts only if the entry is taken.
        let mut after = *budget;
        let (item, left_out) = entry(at, &all[at], &mut after);
        let cost = wire(&item).saturating_add(2);
        if cost > after.wire && !(forced && items.is_empty()) {
            break;
        }
        after.wire = after.wire.saturating_sub(cost);
        *budget = after;
        partial |= left_out;
        items.push(item);
        at += 1;
    }
    Ok(Page { items, next: (at < all.len()).then_some(at), total: all.len(), partial })
}

/// One field as a list entry. Its value travels when it is short and the answer still has room;
/// otherwise the entry says so and names the input that reads it. Nothing is cut inside an entry.
fn field_entry(index: usize, &(name, value): &(&str, &str), budget: &mut Budget) -> (Value, bool) {
    let mut entry = json!({"index":index, "bytes":value.len()});
    let mut left_out = false;
    if name.len() <= INLINE_NAME {
        entry["name"] = json!(name);
    } else {
        entry["name_omitted"] = json!(true);
        entry["name_bytes"] = json!(name.len());
        entry["read_name"] = json!({"field":index, "part":"name"});
        left_out = true;
    }
    let carried = json!(value);
    let fits = value.len() <= INLINE_VALUE && value.len() <= budget.inline
        && wire(&entry).saturating_add(wire(&carried)).saturating_add(64) <= budget.wire;
    if fits {
        entry["value"] = carried;
        budget.inline -= value.len();
    } else {
        entry["value_omitted"] = json!(true);
        entry["read"] = json!({"field":index});
        left_out = true;
    }
    (entry, left_out)
}

/// One Cycle of a Chain as an address, with the link it was opened under.
fn cycle_entry(chain: &Chain, cycle: &Cycle) -> (Value, bool) {
    let mut entry = cycle_json(cycle);
    entry["plan_link"] = plan_link(Some(chain), cycle);
    (entry, false)
}

/// One Step of a Cycle as an address only — which Report to open next is the reader's choice.
fn step_json(cycle: &Cycle, node: &crate::walk::StepNode) -> Value {
    let mut step = json!({"ref":cycle.step_ref(node.id).to_string(), "kind":node.kind.as_str(), "state":state(node.is_closed())});
    if node.kind == NodeKind::Verify { step["snapshot"] = json!(node.snapshot.map(|world| world.to_string())); }
    if let Some(from) = node.revisit_from { step["revisit_from"] = json!(cycle.step_ref(from).to_string()); }
    step
}
fn step_entry(cycle: &Cycle, node: &crate::walk::StepNode) -> (Value, bool) {
    let mut entry = step_json(cycle, node);
    entry["report"] = json!(if node.report.is_some() { "written" } else { "not_written" });
    (entry, false)
}

/// One path a Verify declared, as its Snapshot holds it. A path too long to be a file's name is
/// measured, not written out: its text is the Report's own field.
fn file_entry(file: &crate::session::DeclaredEvidence) -> (Value, bool) {
    let mut entry = json!({"declared_in":file.declared_in});
    let long = file.path.len() > 4096;
    if long {
        entry["path_omitted"] = json!(true);
        entry["path_bytes"] = json!(file.path.len());
    } else {
        entry["path"] = json!(file.path);
    }
    match &file.sha256 {
        Some(sha256) => {
            entry["stored"] = json!("present");
            entry["sha256"] = json!(sha256);
            entry["in_current_world"] = json!(file.in_current_world.as_str());
        }
        None => entry["stored"] = json!("not_in_snapshot"),
    }
    (entry, long)
}

fn evidence_head(index: &EvidenceIndex) -> Value {
    json!({"state":"indexed", "source":"stored_snapshot", "snapshot":index.snapshot.to_string(),
        "current_world":index.current_world.to_string(), "read_with":EVIDENCE, "file_count":index.files.len()})
}

fn not_found(owner: &Owner) -> CallToolResult {
    refusal("not_found", crate::tr!("record_tool.not_found", owner = owner.text()), json!({"owner":owner.json()}))
}

/// The last line of defence. Every answer is built under [`WIRE_BUDGET`], so this refusal is
/// not a path a reader is expected to meet; it exists so that a mistake in the accounting can
/// never send an oversized frame.
fn bounded(value: Value) -> CallToolResult {
    let result = CallToolResult::structured(value);
    if serde_json::to_vec(&result).map_or(true, |bytes| bytes.len() > RESPONSE_LIMIT) {
        return refusal("response_too_large", crate::locale::text("record_tool.too_large").into(), json!({}));
    }
    result
}

/// Closes an answer: says whether it left anything out, and how the rest is read.
fn finish(mut body: Value, more: Vec<Value>, partial: bool) -> CallToolResult {
    let truncated = !more.is_empty() || partial;
    let note = crate::locale::text("record_tool.note_report");
    body["truncated"] = json!(truncated);
    if !more.is_empty() { body["more"] = json!(more); }
    body["note"] = json!(if truncated { format!("{note} {}", crate::locale::text("record_tool.note_partial")) } else { note.to_string() });
    bounded(body)
}

/// "The rest of this list starts here" — the inputs to add to the same call.
fn rest(list: List, page: &Page) -> Option<Value> {
    page.next.map(|from| json!({"list":list.name(), "from":from, "total":page.total}))
}

// ── what a reference names ─────────────────────────────────────────────────

/// The record a canonical reference names. Found by canonical address only: no nearest number
/// and no other Cycle's Step of the same rank.
enum Target<'a> {
    Chain(&'a Chain),
    Cycle(Option<&'a Chain>, &'a Cycle),
    Step(Option<&'a Chain>, &'a Cycle, &'a crate::walk::StepNode, StepRef),
}

impl<'a> Target<'a> {
    fn find(project: &'a Project, asked: &Asked) -> Option<Target<'a>> {
        match asked {
            Asked::Chain(asked) => project.chains().iter().find(|chain| chain.id() == *asked).map(Target::Chain),
            Asked::Cycle(asked) => locate(project, *asked).map(|(chain, cycle)| Target::Cycle(chain, cycle)),
            Asked::Step(step) => {
                let (chain, cycle) = locate(project, step.cycle())?;
                let node = cycle.steps().nodes().iter().find(|node| cycle.step_ref(node.id) == *step && !is_boundary(node.kind))?;
                Some(Target::Step(chain, cycle, node, *step))
            }
        }
    }
    fn report(&self) -> Option<&'a Report> {
        match self {
            Target::Chain(chain) => chain.report(),
            Target::Cycle(_, cycle) => cycle.report(),
            Target::Step(_, _, node, _) => node.report.as_ref(),
        }
    }
    /// The lists this kind of record has.
    fn lists(&self) -> &'static [List] {
        match self {
            Target::Chain(_) => &[List::Cycles, List::Fields],
            Target::Cycle(..) => &[List::Steps, List::Fields],
            Target::Step(..) => &[List::Fields, List::Files],
        }
    }
    /// Which record this is and where it sits — the part every answer about it carries.
    fn position(&self, owner: &Owner) -> Value {
        let mut body = json!({"ok":true, "schema_version":SCHEMA_VERSION, "source":"stored_record", "owner":owner.json()});
        match self {
            Target::Chain(chain) => body["chain"] = chain_position(chain),
            Target::Cycle(chain, cycle) | Target::Step(chain, cycle, _, _) => {
                body["chain"] = chain_json(*chain);
                body["cycle"] = cycle_json(cycle);
                body["plan_link"] = plan_link(*chain, cycle);
            }
        }
        if let Target::Step(_, cycle, node, _) = self {
            body["step"] = step_json(cycle, node);
        }
        body
    }
    fn steps(&self) -> Vec<&'a crate::walk::StepNode> {
        match self {
            Target::Cycle(_, cycle) => cycle.steps().nodes().iter().filter(|node| !is_boundary(node.kind)).collect(),
            _ => Vec::new(),
        }
    }
}

/// The whole record in one answer when it fits: where it sits, its index list, its Report and,
/// for a closed Verify, what it declared. What does not fit is named in `more` or on the entry.
fn overview(session: &ReadOnlySession, owner: &Owner, target: &Target<'_>) -> CallToolResult {
    let mut body = target.position(owner);
    let mut budget = Budget::new();
    let (mut more, mut partial) = (Vec::new(), false);
    match target {
        Target::Chain(chain) => {
            let cycles = page(chain.cycles().nodes(), 0, &mut budget, false, |_, cycle, _| cycle_entry(chain, cycle)).expect("a list starts at 0");
            more.extend(rest(List::Cycles, &cycles));
            body["cycles"] = json!(cycles.items);
        }
        Target::Cycle(_, cycle) => {
            let steps = page(&target.steps(), 0, &mut budget, false, |_, node, _| step_entry(cycle, node)).expect("a list starts at 0");
            more.extend(rest(List::Steps, &steps));
            body["steps"] = json!(steps.items);
        }
        Target::Step(..) => {}
    }
    let mut report = report_head(target.report());
    if let Some(stored) = target.report() {
        let fields = page(&fields_of(stored), 0, &mut budget, false, field_entry).expect("a list starts at 0");
        more.extend(rest(List::Fields, &fields));
        partial |= fields.partial;
        report["fields"] = json!(fields.items);
    }
    body["report"] = report;
    if let Target::Step(_, _, node, step) = target {
        if node.kind == NodeKind::Verify && node.report.is_some() {
            body["evidence"] = match session.verify_evidence_index(*step) {
                Ok(index) => {
                    let files = page(&index.files, 0, &mut budget, false, |_, file, _| file_entry(file)).expect("a list starts at 0");
                    more.extend(rest(List::Files, &files));
                    partial |= files.partial;
                    let mut evidence = evidence_head(&index);
                    evidence["files"] = json!(files.items);
                    evidence
                }
                Err(StoredEvidenceError::Unavailable { snapshot, said }) =>
                    json!({"state":"unavailable", "snapshot":snapshot.map(|world| world.to_string()), "reason":said}),
                Err(_) => Value::Null,
            };
        }
    }
    finish(body, more, partial)
}

/// One page of one list of the record, starting where a previous answer said the rest begins.
fn list_page(session: &ReadOnlySession, owner: &Owner, target: &Target<'_>, list: List, from: u64) -> CallToolResult {
    if !target.lists().contains(&list) {
        let lists: Vec<&str> = target.lists().iter().map(|list| list.name()).collect();
        return refusal("invalid_list", crate::tr!("record_tool.invalid_list", owner = owner.text(), lists = lists.join(", ")),
            json!({"owner":owner.json(), "lists":lists}));
    }
    let from = usize::try_from(from).unwrap_or(usize::MAX);
    let mut body = target.position(owner);
    let mut budget = Budget::new();
    let found = match (list, target) {
        (List::Cycles, Target::Chain(chain)) => page(chain.cycles().nodes(), from, &mut budget, true, |_, cycle, _| cycle_entry(chain, cycle)),
        (List::Steps, Target::Cycle(_, cycle)) => page(&target.steps(), from, &mut budget, true, |_, node, _| step_entry(cycle, node)),
        (List::Files, Target::Step(_, _, node, step)) => {
            if node.kind != NodeKind::Verify {
                return refusal("not_a_verify", crate::tr!("record_tool.not_a_verify", owner = owner.text(), kind = node.kind.as_str()),
                    json!({"owner":owner.json(), "kind":node.kind.as_str()}));
            }
            match session.verify_evidence_index(*step) {
                Ok(index) => {
                    body["evidence"] = evidence_head(&index);
                    page(&index.files, from, &mut budget, true, |_, file, _| file_entry(file))
                }
                Err(StoredEvidenceError::Unavailable { snapshot, said }) => return refusal("evidence_unavailable",
                    crate::tr!("record_tool.evidence_unavailable", why = said), json!({"owner":owner.json(), "snapshot":snapshot.map(|world| world.to_string())})),
                Err(_) => return refusal("report_not_written", crate::tr!("record_tool.report_not_written", owner = owner.text()), json!({"owner":owner.json()})),
            }
        }
        _ => {
            // `fields` of any record: an unwritten Report is an empty list of a Report that says so.
            body["report"] = report_head(target.report());
            let fields = target.report().map(fields_of).unwrap_or_default();
            page(&fields, from, &mut budget, true, field_entry)
        }
    };
    let found = match found {
        Ok(found) => found,
        Err(total) => return refusal("invalid_from", crate::tr!("record_tool.invalid_from", total = total), json!({"list":list.name(), "total":total})),
    };
    body["list"] = json!(list.name());
    body["from"] = json!(from);
    body["total"] = json!(found.total);
    body["returned"] = json!(found.items.len());
    body["next_from"] = json!(found.next);
    let more: Vec<Value> = rest(list, &found).into_iter().collect();
    body["items"] = json!(found.items);
    finish(body, more, found.partial)
}

/// The stretch of `whole` that one answer carries from `offset`: at most [`CHUNK`] bytes, ended
/// on a character boundary. `None` when the offset is beyond the end or inside a character.
fn stretch(whole: &str, offset: usize) -> Option<&str> {
    if offset > whole.len() || (offset == whole.len() && offset > 0) || !whole.is_char_boundary(offset) {
        return None;
    }
    let mut end = offset.saturating_add(CHUNK).min(whole.len());
    while !whole.is_char_boundary(end) {
        end -= 1;
    }
    Some(&whole[offset..end])
}

/// Why a field range cannot be answered.
enum RangeError {
    /// No field has that index; the Report has this many.
    Field { count: usize },
    /// The offset is at or beyond the end, or inside a character; the part is this long.
    Offset { total: usize },
}

/// A byte range of one field's value (or of its name), as stored: which field, which bytes of
/// how many, the digest of the whole part, and where the next range starts.
fn range_of(report: &Report, index: u64, part: Part, offset: u64) -> Result<Value, RangeError> {
    let fields = fields_of(report);
    let &(name, value) = usize::try_from(index).ok().and_then(|index| fields.get(index))
        .ok_or(RangeError::Field { count: fields.len() })?;
    let whole = match part { Part::Value => value, Part::Name => name };
    let text = usize::try_from(offset).ok().and_then(|offset| stretch(whole, offset))
        .ok_or(RangeError::Offset { total: whole.len() })?;
    let offset = offset as usize;
    let end = offset + text.len();
    let truncated = end < whole.len();
    let mut field = json!({"index":index, "bytes":value.len()});
    if name.len() <= INLINE_NAME {
        field["name"] = json!(name);
    } else {
        field["name_omitted"] = json!(true);
        field["name_bytes"] = json!(name.len());
    }
    Ok(json!({"report":report_head(Some(report)), "field":field,
        "part":match part { Part::Value => "value", Part::Name => "name" },
        "sha256":format!("{:x}", Sha256::digest(whole.as_bytes())), "total_bytes":whole.len(),
        "offset":offset, "returned_bytes":text.len(), "truncated":truncated,
        "next_offset":truncated.then_some(end), "remaining_bytes":whole.len() - end,
        "encoding":"utf8", "text":text}))
}

fn field_range(owner: &Owner, target: &Target<'_>, index: u64, part: Part, offset: u64) -> CallToolResult {
    let Some(report) = target.report() else {
        return refusal("report_not_written", crate::tr!("record_tool.no_report_yet", owner = owner.text()), json!({"owner":owner.json()}));
    };
    let range = match range_of(report, index, part, offset) {
        Ok(range) => range,
        Err(RangeError::Field { count }) => return refusal("invalid_field",
            crate::tr!("record_tool.invalid_field", count = count), json!({"owner":owner.json(), "field_count":count})),
        Err(RangeError::Offset { total }) => return refusal("invalid_offset",
            crate::tr!("record_tool.field_offset", total = total), json!({"total_bytes":total})),
    };
    let mut note = crate::locale::text("record_tool.note_report").to_string();
    if let Some(next) = range["next_offset"].as_u64() {
        note.push(' ');
        note.push_str(&crate::tr!("record_tool.note_more", from = range["offset"], to = next, total = range["total_bytes"], next = next));
    }
    let mut body = target.position(owner);
    if let (Some(into), Some(from)) = (body.as_object_mut(), range.as_object()) {
        for (key, value) in from { into.insert(key.clone(), value.clone()); }
    }
    body["note"] = json!(note);
    bounded(body)
}

fn read_report(request: ReadReport) -> CallToolResult {
    let Some(asked) = request.owner.asked() else { return not_found(&request.owner); };
    let session = match open(&request.project_root) { Ok(session) => session, Err(refused) => return refused };
    let Some(target) = Target::find(session.project(), &asked) else { return not_found(&request.owner); };
    match (request.list, request.field) {
        (Some(list), _) => list_page(&session, &request.owner, &target, list, request.from.unwrap_or(0)),
        (None, Some(field)) => field_range(&request.owner, &target, field, request.part.unwrap_or(Part::Value), request.offset.unwrap_or(0)),
        (None, None) => overview(&session, &request.owner, &target),
    }
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
            StoredEvidenceError::NotDeclared { declared } => {
                // A line too long to be a file's name is counted, not echoed; its text is the Report's field.
                let listed: Vec<&String> = declared.iter().filter(|path| path.len() <= 4096).collect();
                refusal("not_declared", crate::tr!("record_tool.not_declared", owner = owner),
                    json!({"owner":request.owner.json(), "declared":listed, "declared_count":declared.len()}))
            }
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
            "Read one stored Report verbatim by canonical reference. Changes nothing. owner {kind: cycle, ref} returns where that Cycle sits (Chain, parent, revisit source, entry/exit Snapshot, and the approved revision and requirement it was opened under), the references, kinds and states of its Steps without their bodies, and its Cycle Report; owner {kind: step, ref} returns that one Step Report and, for a closed Verify, each file it declared (evidence_files, plan_document) as its own Snapshot holds it; owner {kind: chain, ref} returns that Chain's Cycles as references with their links, and its Chain Report, which is how an earlier Chain's Experiments are found. Start from gil_plan_read.requirements[].experiments[].cycle_ref, read the Cycle for its Step references, then open only the Reports the next decision needs. report.fields lists every stored field in stored order as {index, name, bytes, value}, the stored text unchanged: nothing is summarized, merged, ranked or scored. Every stored original can be read to its end, and an answer always says whether it left something out: truncated: false means it carries everything asked; truncated: true means part was left out to keep one answer small, and the answer names the input that reads it. A value longer than 16 KiB, or beyond the 64 KiB one answer carries, appears as value_omitted: true with read: {field: <index>} — call again with field (and offset: next_offset while truncated is true) to read it in ranges of at most 16 KiB cut on character boundaries; the range's sha256 and total_bytes describe the whole value. A list that did not fit appears in more as {list, from} — call again with list and from for the next page (next_from continues it). Send at most one continuation per call. Omitted never means absent or unreadable. Failed and revisited Experiments read exactly like successful ones, and records that disagree are each returned as written; this tool never picks one. A record existing is not a finding that it is true or still valid, and no field says a requirement is answered: that is decided only in the user-approved Closing Interview. An open Step or Cycle answers report.state not_written. An unknown reference is refused (not_found) and no nearby record is substituted.".to_string(),
            json!({"type":"object","additionalProperties":false,"required":["project_root","owner"],
                "properties":{"project_root":root,"owner":owner(json!(["step","cycle","chain"])),
                    "list":{"enum":["cycles","steps","fields","files"],"description":"Continuation: one page of this list of the owner (chain: cycles, fields; cycle: steps, fields; step: fields, files). Copy it from more in a previous answer."},
                    "from":{"type":"integer","minimum":0,"description":"With list: the entry index the page starts at; omit for 0. Use more[].from or next_from."},
                    "field":{"type":"integer","minimum":0,"description":"Continuation: the index of one field of the owner's Report, to read its value as a byte range. Copy it from that field's read."},
                    "part":{"enum":["value","name"],"description":"With field: value (default) or the field's name when the entry says name_omitted."},
                    "offset":{"type":"integer","minimum":0,"description":"With field: byte offset to continue from; omit to start at 0. Use next_offset from the previous answer."}}})),
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

    fn framed(body: &Value) -> usize {
        serde_json::to_vec(&CallToolResult::structured(body.clone())).unwrap().len()
    }

    /// The accounting is the size on the wire: a piece never adds more than `wire` says.
    #[test]
    fn the_wire_cost_of_a_piece_is_what_it_adds_to_the_framed_answer() {
        let empty = framed(&json!({"items":[]}));
        for nasty in ["\\\"".repeat(1000), "\u{1}".repeat(1000), "가나다".repeat(300), "plain ascii ".repeat(100), "\n\t\"\\".repeat(500), String::new()] {
            let piece = json!({"index":7,"name":"x","value":nasty});
            let added = framed(&json!({"items":[piece.clone()]})) - empty;
            assert!(added <= wire(&piece) && wire(&piece) <= added + 2, "{} bytes of {:?}…: added {added}, counted {}", nasty.len(), nasty.chars().take(4).collect::<String>(), wire(&piece));
        }
        // The reviewer's case: 200 KB of quotes and backslashes is four times that on the wire
        // inside the structured copy and the text copy together — over the limit as one piece.
        assert!(wire(&json!("\\\"".repeat(100_000))) > RESPONSE_LIMIT);
    }

    #[test]
    fn a_page_takes_what_fits_says_where_the_rest_starts_and_always_advances() {
        let all: Vec<String> = (0..10).map(|n| format!("entry-{n}")).collect();
        let entry = |_: usize, text: &String, _: &mut Budget| (json!({"text":text}), false);
        let one = wire(&json!({"text":"entry-0"})) + 2;
        let mut budget = Budget { wire: one * 3, inline: INLINE_TOTAL };
        let first = page(&all, 0, &mut budget, false, entry).unwrap();
        assert_eq!((first.items.len(), first.next, first.total, first.partial), (3, Some(3), 10, false));
        assert_eq!(budget.wire, 0);
        // Nothing fits: an unforced page is empty and still names its start; a forced one advances.
        let none = page(&all, 3, &mut Budget { wire: 0, inline: 0 }, false, entry).unwrap();
        assert_eq!((none.items.len(), none.next), (0, Some(3)));
        let forced = page(&all, 3, &mut Budget { wire: 0, inline: 0 }, true, entry).unwrap();
        assert_eq!((forced.items.len(), forced.next), (1, Some(4)));
        let last = page(&all, 9, &mut Budget::new(), true, entry).unwrap();
        assert_eq!((last.items, last.next), (vec![json!({"text":"entry-9"})], None));
        // A position at or beyond the end is not an empty page; an empty list read from 0 is.
        assert_eq!(page(&all, 10, &mut Budget::new(), true, entry).err(), Some(10));
        assert_eq!(page(&all, 11, &mut Budget::new(), true, entry).err(), Some(10));
        let empty: Vec<String> = Vec::new();
        let nothing = page(&empty, 0, &mut Budget::new(), true, entry).unwrap();
        assert_eq!((nothing.items.len(), nothing.next, nothing.total), (0, None, 0));
    }

    #[test]
    fn a_field_entry_carries_a_short_value_and_names_the_read_of_a_long_one() {
        let mut budget = Budget::new();
        let (short, left_out) = field_entry(0, &("result", "관측했다"), &mut budget);
        assert_eq!((short, left_out), (json!({"index":0,"name":"result","bytes":12,"value":"관측했다"}), false));
        assert_eq!(budget.inline, INLINE_TOTAL - 12);
        let long = "x".repeat(INLINE_VALUE + 1);
        let (entry, left_out) = field_entry(3, &("result", &long), &mut budget);
        assert_eq!((entry, left_out), (json!({"index":3,"name":"result","bytes":INLINE_VALUE + 1,"value_omitted":true,"read":{"field":3}}), true));
        assert_eq!(budget.inline, INLINE_TOTAL - 12, "an omitted value spends nothing");
        // The inline allowance of one answer runs out; the field is still listed, without its value.
        let mut spent = Budget { wire: WIRE_BUDGET, inline: 5 };
        assert_eq!(field_entry(1, &("a", "123456"), &mut spent).0["value_omitted"], true);
        assert_eq!(field_entry(1, &("a", "12345"), &mut spent).0["value"], "12345");
        // A name too long to write out is measured, and has its own read.
        let name = "n".repeat(INLINE_NAME + 1);
        let (entry, left_out) = field_entry(2, &(&name, "v"), &mut Budget::new());
        assert_eq!((entry, left_out), (json!({"index":2,"bytes":1,"name_omitted":true,"name_bytes":INLINE_NAME + 1,"read_name":{"field":2,"part":"name"},"value":"v"}), true));
    }

    #[test]
    fn a_stretch_ends_on_a_character_boundary_and_refuses_a_position_inside_one() {
        let whole = "가".repeat(CHUNK); // three bytes each
        let first = stretch(&whole, 0).unwrap();
        assert_eq!(first.len(), CHUNK - CHUNK % 3);
        assert!(first.chars().all(|c| c == '가'));
        assert_eq!(stretch(&whole, first.len()).unwrap().len(), CHUNK - CHUNK % 3);
        assert!(stretch(&whole, 1).is_none(), "inside a character");
        assert!(stretch(&whole, whole.len()).is_none() && stretch(&whole, whole.len() + 3).is_none());
        assert_eq!(stretch(&whole, whole.len() - 3), Some("가"));
        assert_eq!(stretch("", 0), Some(""));
    }

    /// Every byte of a stored Report is reachable inside the limit, whatever it holds: thousands
    /// of fields (more than one answer lists), values that grow fourfold or thirteenfold on the wire, and a name too long to
    /// list. Pages and ranges are followed exactly as the answers direct, and what they give
    /// back is the Report.
    #[test]
    fn a_report_of_any_shape_is_read_to_its_end_within_the_limit_and_reassembles() {
        let mut stored: BTreeMap<String, String> = (0..12_000).map(|n| (format!("k{n:05}"), format!("값 {n}"))).collect();
        stored.insert("escape_heavy".into(), "\\\"".repeat(150_000));
        stored.insert("control".into(), "\u{1}".repeat(40_000));
        stored.insert(format!("long_name_{}", "n".repeat(5000)), "긴 이름의 값\n둘째 줄".repeat(2000));
        for n in 0..10 { stored.insert(format!("worst{n}"), "\u{1}".repeat(INLINE_VALUE)); }
        stored.insert("empty".into(), String::new());
        let report: Report = stored.clone().into_iter().collect();
        let count = stored.len();
        let within = |body: Value| -> Value {
            let size = framed(&body);
            assert!(size <= RESPONSE_LIMIT, "an answer of {size} bytes");
            assert_ne!(bounded(body.clone()).is_error, Some(true), "the backstop was reached");
            body
        };
        let whole = |index: u64, part: Part| -> String {
            let (mut text, mut offset, mut calls) = (String::new(), 0u64, 0);
            loop {
                let Ok(range) = range_of(&report, index, part, offset) else { panic!("field {index} at {offset}") };
                let range = within(range);
                assert!(range["returned_bytes"].as_u64().unwrap() <= CHUNK as u64);
                text.push_str(range["text"].as_str().unwrap());
                calls += 1;
                assert_eq!(range["sha256"], json!(format!("{:x}", Sha256::digest(match part { Part::Value => fields_of(&report)[index as usize].1, Part::Name => fields_of(&report)[index as usize].0 }.as_bytes()))));
                match range["next_offset"].as_u64() {
                    Some(next) => { assert_eq!(range["truncated"], json!(true)); assert!(next > offset); offset = next; }
                    None => { assert_eq!((range["truncated"].clone(), range["remaining_bytes"].clone()), (json!(false), json!(0))); break; }
                }
                assert!(calls < 1000);
            }
            text
        };
        let (mut read, mut from, mut pages, mut omitted_values, mut omitted_names) = (BTreeMap::new(), 0usize, 0, 0, 0);
        loop {
            // The first answer is the overview's share of the budget; later ones are forced pages.
            let got = page(&fields_of(&report), from, &mut Budget::new(), pages > 0, field_entry).unwrap();
            assert!(!got.items.is_empty(), "a page that does not advance");
            within(json!({"ok":true, "items":got.items, "report":report_head(Some(&report))}));
            for item in &got.items {
                let index = item["index"].as_u64().unwrap();
                assert_eq!(index as usize, read.len(), "entries arrive in stored order, none skipped");
                let name = match item.get("name") {
                    Some(name) => name.as_str().unwrap().to_owned(),
                    None => { assert_eq!((item["name_omitted"].clone(), item["read_name"].clone()), (json!(true), json!({"field":index,"part":"name"}))); omitted_names += 1; whole(index, Part::Name) }
                };
                let value = match item.get("value") {
                    Some(value) => value.as_str().unwrap().to_owned(),
                    None => { assert_eq!((item["value_omitted"].clone(), item["read"].clone()), (json!(true), json!({"field":index}))); omitted_values += 1; whole(index, Part::Value) }
                };
                assert_eq!(item["bytes"].as_u64(), Some(value.len() as u64));
                assert!(read.insert(name, value).is_none());
            }
            pages += 1;
            match got.next {
                Some(next) => { assert_eq!((next, got.total), (read.len(), count)); from = next; }
                None => break,
            }
        }
        assert_eq!(read, stored, "the pieces put back together are the stored Report");
        assert!(pages > 1 && omitted_values >= 3 && omitted_names == 1, "{pages} pages, {omitted_values} values, {omitted_names} names read by range");
        let reassembled: Report = read.into_iter().collect();
        assert_eq!(report_digest(&reassembled), report_digest(&report));
        // A position past the end and a field that does not exist are refused, not answered empty.
        assert!(matches!(range_of(&report, count as u64, Part::Value, 0), Err(RangeError::Field { count: n }) if n == count));
        assert!(matches!(range_of(&report, 0, Part::Value, 1_000_000), Err(RangeError::Offset { .. })));
    }

    #[test]
    fn a_continuation_is_one_selector_with_its_own_position() {
        let base = |extra: Value| {
            let mut input = json!({"project_root":root(),"owner":{"kind":"step","ref":"step:C3/S3"}});
            for (key, value) in extra.as_object().unwrap() { input[key] = value.clone(); }
            input
        };
        for good in [json!({"list":"fields"}), json!({"list":"files","from":32}), json!({"list":"steps","from":0}), json!({"list":"cycles"}),
            json!({"field":0}), json!({"field":4,"offset":16384}), json!({"field":4,"part":"name"}), json!({"field":4,"part":"value","offset":3})] {
            assert!(valid(REPORT, &base(good.clone())), "{good}");
        }
        for bad in [json!({"from":3}), json!({"offset":3}), json!({"part":"name"}), json!({"list":"fields","field":0}), json!({"list":"fields","offset":0}),
            json!({"field":0,"from":0}), json!({"list":"reports"}), json!({"list":null}), json!({"field":null}), json!({"list":"fields","from":null}),
            json!({"field":-1}), json!({"field":"result"}), json!({"field":0,"part":"digest"}), json!({"list":"fields","from":-1}), json!({"list":"fields","from":"0"})] {
            assert!(!valid(REPORT, &base(bad.clone())), "{bad}");
        }
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
