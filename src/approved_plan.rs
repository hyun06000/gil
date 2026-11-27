//! M6-D opt-in approval ledger. No filesystem writes or identity-authentication claims.
//!
//! # Two execution contracts, one per Chain
//!
//! A ledger without `contract` is the original **item queue**: opening an Experiment binds the
//! first unexecuted item, a success consumes it and a failure re-binds the same item from its
//! revisit lineage. Nothing in that contract changed.
//!
//! A ledger with `contract: exploration` (Chains opened by an `exploration-flow` build) keeps
//! the approved items as stable **requirements**. Every Experiment names the requirement it
//! contributes to and says why this small question is next; several Experiments may serve one
//! requirement, none of them consumes it, and after a failure's explicit revisit a different
//! small question may follow. The Core checks the reference, never the meaning of that prose.
use crate::{Chain, CycleKind, NodeKind, Project, Report, StepRef};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub goal: String,
    pub in_scope: String,
    pub out_of_scope: String,
    pub constraints: String,
    pub success_criteria: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub id: String,
    pub question: String,
    /// The one observation that decides the question. On the MCP wire it is named `observation`.
    pub done_when: String,
    /// The sentence of the user's request this problem comes from, quoted. Absent in plans written
    /// before the field existed; an empty source is not stored, so their digests are unchanged.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
    /// The command that makes the observation in this environment. Absent in older plans.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub command: String,
    /// Where this problem hangs in the tree the Interview drew: the major problem it belongs to and
    /// any branches under it, written as a path (`M2 > timestamps > rollover`). Absent in older plans.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub part_of: String,
}

/// The names of the major problems an Interpretation listed, one per line as `M<number>: …`.
pub fn major_problems(listed: &str) -> Vec<String> {
    listed.lines().filter_map(|line| {
        let name = line.trim_start_matches(|c: char| c == '-' || c == '*' || c.is_whitespace()).split(':').next()?.trim();
        (name.len() > 1 && name.starts_with('M') && name[1..].chars().all(|c| c.is_ascii_digit())).then(|| name.to_owned())
    }).collect()
}
/// The major problem a path names: its first part.
pub fn major_of(part_of: &str) -> &str {
    part_of.split(['>', '/']).next().unwrap_or("").trim()
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Draft {
    pub scope: Scope,
    pub items: Vec<Item>,
    pub synthesis: BTreeMap<String, String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Approve,
    RequestChanges,
}
impl Decision {
    fn approved(&self) -> &'static str {
        match self {
            Self::Approve => "yes",
            Self::RequestChanges => "no",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub decision: Decision,
    pub source: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    pub revision: u32,
    pub base_revision: Option<u32>,
    pub synthesis_ref: String,
    pub draft: Draft,
    pub digest: String,
    pub receipt: Option<Receipt>,
}
/// The stored marker of a Chain's execution contract. Absent means the original item queue;
/// `exploration` is the only value a record may carry, so an explicit legacy spelling cannot
/// appear and be silently rewritten.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Contract {
    Exploration,
}
/// Why this small question is the next one, in the Agent's words. The Core requires the three
/// statements to exist and stores them verbatim; it does not judge whether they are true.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub relevance: String,
    pub readiness: String,
    pub uncertainty: String,
}
impl Selection {
    fn validate(&self) -> Result<(), String> {
        if [&self.relevance, &self.readiness, &self.uncertainty]
            .iter()
            .any(|s| s.trim().is_empty() || s.len() > 4096)
        {
            return Err(link_bad(crate::locale::text("plan.err.selection_fields")));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Execution {
    pub cycle_ref: String,
    pub revision: u32,
    pub item_id: String,
    /// Exploration contract only: present on every link, absent in every item-queue binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<Selection>,
}
/// The explicit link an Experiment carries when it opens in an exploration Chain: the exact
/// approved revision (number and digest, as `gil_plan_read.active_plan` shows them), one of its
/// requirement ids, and the selection statements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExperimentLink {
    pub revision: u32,
    pub digest: String,
    pub requirement_id: String,
    pub selection: Selection,
}
impl ExperimentLink {
    pub const FIELDS: [&'static str; 6] =
        ["plan_revision", "plan_digest", "requirement_id", "relevance", "readiness", "uncertainty"];
    /// Reads the link from the same field grammar an action contract uses. Every field is
    /// required and nothing else is accepted, so a misspelled name is never dropped silently.
    pub fn from_report(report: &Report) -> Result<Self, String> {
        let unknown: Vec<&str> = report.field_names().filter(|k| !Self::FIELDS.contains(k)).collect();
        let missing: Vec<&str> = Self::FIELDS.iter().copied()
            .filter(|k| report.get(k).is_none_or(|v| v.trim().is_empty())).collect();
        if !unknown.is_empty() || !missing.is_empty() {
            return Err(link_bad(&crate::tr!("plan.err.link_fields",
                missing = if missing.is_empty() { "-".to_string() } else { missing.join(", ") },
                unknown = if unknown.is_empty() { "-".to_string() } else { unknown.join(", ") })));
        }
        let field = |k: &str| report.get(k).unwrap().trim().to_string();
        let link = Self {
            revision: field("plan_revision").parse().map_err(|_| link_bad(crate::locale::text("plan.err.link_revision")))?,
            digest: field("plan_digest"),
            requirement_id: field("requirement_id"),
            selection: Selection {
                relevance: field("relevance"),
                readiness: field("readiness"),
                uncertainty: field("uncertainty"),
            },
        };
        link.selection.validate()?;
        Ok(link)
    }
    /// The body an Agent sends with `gil open experiment`, with this Project's real values where
    /// they are known. Shown in refusals and in `gil_plan_read`, never stored.
    pub fn skeleton(active: Option<&Proposal>) -> String {
        let (revision, digest, ids) = match active {
            Some(p) => (p.revision.to_string(), p.digest.clone(),
                p.draft.items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>().join(" | ")),
            None => ("…".into(), "…".into(), "…".into()),
        };
        format!("plan_revision: {revision}\nplan_digest: {digest}\nrequirement_id: {ids}\nrelevance: …\nreadiness: …\nuncertainty: …")
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReinterviewRequest {
    pub cycle_ref: String,
    pub reason: String,
}
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanState {
    /// Absent: the original item queue, read and written exactly as before. An older writer
    /// meets an unknown field here and refuses the whole record instead of applying queue rules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract: Option<Contract>,
    pub proposals: Vec<Proposal>,
    pub executed_cycles: Vec<Execution>,
    pub reinterview_requests: Vec<ReinterviewRequest>,
}

fn bad(message: &str) -> String {
    crate::tr!("plan.err.prefix", message = message)
}
/// A refused requirement link is an input the Agent can correct from `gil_plan_read`, unlike an
/// approval-state refusal. It carries its own marker so receipts and guidance tell them apart.
fn link_bad(message: &str) -> String {
    crate::tr!("plan.err.link_prefix", message = message)
}
/// A refusal about the link itself (absent, unreadable), marked like every other link refusal.
pub fn link_refusal(message: &str) -> String {
    link_bad(message)
}
/// Whether a refusal text is a requirement-link refusal, in the current locale.
pub fn is_link_refusal(problem: &str) -> bool {
    let prefix = crate::locale::text("plan.err.link_prefix");
    let marker = prefix.split("{message}").next().unwrap_or(prefix).trim_end();
    !marker.is_empty() && problem.contains(marker)
}
// Diagnostics are bounded and escaped; full, exact text is available in active_plan.
// Validation remains atomic and compares definitions, not just IDs or executed status.
fn retained_items(previous: &[Item], proposed: &[Item]) -> Result<(), String> {
    fn preview(value: &str) -> String {
        let text: String = value.chars().take(120).collect();
        format!("{}{}", serde_json::to_string(&text).unwrap(), if text.len() < value.len() { crate::locale::text("plan.err.omitted") } else { "" })
    }
    let mut differences = Vec::new();
    for old in previous {
        let Some(new) = proposed.iter().find(|i| i.id == old.id) else {
            differences.push(crate::tr!("plan.err.item_missing", id = preview(&old.id)));
            continue;
        };
        for (field, stored, received) in [("question", &old.question, &new.question), ("done_when", &old.done_when, &new.done_when), ("source", &old.source, &new.source), ("command", &old.command, &new.command)] {
            if stored != received {
                differences.push(crate::tr!("plan.err.item_mismatch", id = preview(&old.id), field = field, stored = preview(stored), received = preview(received)));
            }
        }
    }
    if differences.is_empty() { return Ok(()); }
    let count = differences.len();
    differences.truncate(8);
    // Cancel/replace/redefine stay an M6-E decision; the refusal says so instead of only "missing".
    Err(bad(&format!("{}\n{}", crate::tr!("plan.err.retained", differences = differences.join("\n"),
        more = if count > 8 { crate::tr!("plan.err.more_differences", n = count - 8) } else { String::new() }),
        crate::locale::text("plan.err.no_cancel"))))
}
fn failed_execution(chain: &Chain, e: &Execution) -> bool {
    chain.cycles().nodes().iter().find(|c| c.id().to_ref().to_string() == e.cycle_ref)
        .is_some_and(|c| c.is_closed() && c.report().is_some_and(|r| r.get("verdict") == Some("failure")))
}
impl Proposal {
    fn hash(&self) -> String {
        let bytes = serde_json::to_vec(&(
            self.revision,
            self.base_revision,
            &self.synthesis_ref,
            &self.draft,
        ))
        .expect("serializable proposal");
        format!("{:x}", Sha256::digest(bytes))
    }
    pub fn report(&self) -> Result<Report, String> {
        let receipt = self
            .receipt
            .as_ref()
            .ok_or_else(|| bad(crate::locale::text("plan.err.decide_in_monitor")))?;
        let mut report: Report = self.draft.synthesis.clone().into_iter().collect();
        report.insert("approved", receipt.decision.approved());
        report.insert("plan_revision", self.revision.to_string());
        report.insert("plan_digest", &self.digest);
        Ok(report)
    }
    pub fn document(&self) -> String {
        // JSON fences make arbitrary user prose unambiguous; this same complete content is reviewed in UI.
        format!(
            "# GIL approved plan\n\nRevision: {}\n\nSynthesis: {}\n\nSHA-256: {}\n\n```json\n{}\n```\n",
            self.revision,
            self.synthesis_ref,
            self.digest,
            serde_json::to_string_pretty(&self.draft).unwrap()
        )
    }
}
impl PlanState {
    /// The ledger a Chain opened by this build starts with. Only an `exploration-flow` build
    /// marks the new contract; every other build keeps creating the original item queue.
    pub(crate) fn for_new_chain() -> Self {
        Self {
            contract: cfg!(feature = "exploration-flow").then_some(Contract::Exploration),
            ..Self::default()
        }
    }
    /// Whether this Chain links Experiments to requirements instead of consuming an item queue.
    pub fn exploration(&self) -> bool {
        self.contract == Some(Contract::Exploration)
    }
    /// The Experiments linked to one requirement, in the order they were opened.
    pub fn linked<'a>(&'a self, requirement_id: &'a str) -> impl Iterator<Item = &'a Execution> + 'a {
        self.executed_cycles.iter().filter(move |e| e.item_id == requirement_id)
    }
    pub fn active<'a>(&'a self, chain: &Chain) -> Option<&'a Proposal> {
        self.proposals
            .iter()
            .rev()
            .find(|p| self.activated(chain, p))
    }
    fn activated(&self, chain: &Chain, p: &Proposal) -> bool {
        let Ok(target) = p.synthesis_ref.parse::<StepRef>() else {
            return false;
        };
        chain
            .cycles()
            .nodes()
            .iter()
            .find(|c| c.id().to_ref() == target.cycle())
            .is_some_and(|c| {
                c.is_closed()
                    && c.steps().nodes().iter().any(|n| {
                        c.step_ref(n.id) == target
                            && n.is_closed()
                            && p.report().ok().as_ref() == n.report.as_ref()
                    })
                    && c.report()
                        .is_some_and(|r| r.get("verdict") == Some("success"))
                    && c.steps().nodes().iter().any(|n| {
                        n.kind == NodeKind::Outcome
                            && n.is_closed()
                            && c.report().is_some_and(|r| {
                                r.get("outcome_ref") == Some(c.step_ref(n.id).to_string().as_str())
                            })
                            && n.report.as_ref().is_some_and(|r| {
                                r.get("synthesis_ref") == Some(p.synthesis_ref.as_str())
                            })
                    })
                    && p.receipt
                        .as_ref()
                        .is_some_and(|r| r.decision == Decision::Approve)
            })
    }
    /// Item queue: the items still to execute, a failed one included (its retry). Exploration:
    /// the requirements no Experiment has been linked to yet. A linked requirement is not
    /// thereby answered, and a successful Experiment consumes nothing.
    pub fn pending<'a>(&'a self, chain: &Chain) -> Vec<&'a Item> {
        if self.exploration() {
            return self.active(chain)
                .map(|p| p.draft.items.iter().filter(|i| self.linked(&i.id).next().is_none()).collect())
                .unwrap_or_default();
        }
        self.active(chain)
            .map(|p| {
                p.draft
                    .items
                    .iter()
                    .filter(|i| self.executed_cycles.iter().rev().find(|e| e.item_id == i.id)
                        .is_none_or(|e| failed_execution(chain, e)))
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn current<'a>(&'a self, project: &Project) -> Option<&'a Proposal> {
        let c = project.current_cycle();
        let target = c.step_ref(c.step_now_open()?).to_string();
        self.proposals.last().filter(|p| p.synthesis_ref == target)
    }
    pub fn awaiting_interview(&self, chain: &Chain) -> Vec<&ReinterviewRequest> {
        let active_cycle = self
            .active(chain)
            .and_then(|p| p.synthesis_ref.parse::<StepRef>().ok())
            .map(|r| r.cycle().number())
            .unwrap_or(0);
        self.reinterview_requests
            .iter()
            .filter(|r| {
                r.cycle_ref
                    .parse::<crate::CycleRef>()
                    .is_ok_and(|c| c.number() >= active_cycle)
            })
            .collect()
    }
    pub(crate) fn validate(&self, chain: &Chain) -> Result<(), String> {
        if !cfg!(feature = "approved-plan") {
            return Err(bad(crate::locale::text("plan.err.core_required")));
        }
        // A build that cannot honour requirement links must not read them as queue bindings.
        if self.exploration() && !cfg!(feature = "exploration-flow") {
            return Err(bad(crate::locale::text("plan.err.exploration_core_required")));
        }
        let exploration = self.exploration();
        for request in &self.reinterview_requests {
            if request.reason.trim().is_empty()
                || request.reason.len() > 4096
                || !chain.cycles().nodes().iter().any(|c| {
                    c.kind() == CycleKind::Experiment
                        && c.id().to_ref().to_string() == request.cycle_ref
                })
            {
                return Err(bad(crate::locale::text("plan.err.bad_reinterview")));
            }
        }
        let mut definitions = BTreeMap::<String, Item>::new();
        let mut prior_step = (0, 0);
        for (idx, p) in self.proposals.iter().enumerate() {
            if p.revision as usize != idx + 1 || p.digest != p.hash() {
                return Err(bad(crate::locale::text("plan.err.revision_or_digest")));
            }
            p.draft.validate()?;
            if let Some(base) = p.base_revision {
                let prev = self
                    .proposals
                    .get(
                        base.checked_sub(1)
                            .ok_or_else(|| bad(crate::locale::text("plan.err.base_from_one")))?
                            as usize,
                    )
                    .filter(|b| b.revision < p.revision && self.activated(chain, b))
                    .ok_or_else(|| bad(crate::locale::text("plan.err.base_missing")))?;
                retained_items(&prev.draft.items, &p.draft.items)?;
            }
            let target: StepRef = p
                .synthesis_ref
                .parse()
                .map_err(|_| bad(crate::locale::text("plan.err.synthesis_address")))?;
            if target.to_string() != p.synthesis_ref {
                return Err(bad(crate::locale::text("plan.err.synthesis_canonical")));
            }
            let order = (target.cycle().number(), target.step());
            if order < prior_step {
                return Err(bad(crate::locale::text("plan.err.synthesis_order")));
            }
            prior_step = order;
            let cycle = chain
                .cycles()
                .nodes()
                .iter()
                .find(|c| c.id().to_ref() == target.cycle())
                .ok_or_else(|| bad(crate::locale::text("plan.err.synthesis_cycle_missing")))?;
            let node = cycle
                .steps()
                .nodes()
                .iter()
                .find(|n| cycle.step_ref(n.id) == target)
                .ok_or_else(|| bad(crate::locale::text("plan.err.synthesis_missing")))?;
            if cycle.kind() != CycleKind::Interview || node.kind != NodeKind::Synthesis {
                return Err(bad(crate::locale::text("plan.err.interview_only")));
            }
            let expected_base = self.proposals[..idx]
                .iter()
                .rev()
                .find(|old| {
                    self.activated(chain, old)
                        && old
                            .synthesis_ref
                            .parse::<StepRef>()
                            .is_ok_and(|r| r.cycle().number() < target.cycle().number())
                })
                .map(|old| old.revision);
            if p.base_revision != expected_base {
                return Err(bad(crate::locale::text("plan.err.base_not_latest")));
            }
            if p.receipt
                .as_ref()
                .is_some_and(|r| !crate::approval_source::known(&r.source))
            {
                return Err(bad(crate::locale::text("plan.err.input_path")));
            }
            // Superseded decisions stay in history; only the latest proposal may close the Step.
            let last = !self.proposals[idx + 1..]
                .iter()
                .any(|new| new.synthesis_ref == p.synthesis_ref);
            if last && node.is_closed() && node.report.as_ref() != Some(&p.report()?) {
                return Err(bad(crate::locale::text("plan.err.report_mismatch")));
            }
            if self.activated(chain, p) {
                for item in &p.draft.items {
                    if definitions.get(&item.id).is_some_and(|old| old != item) {
                        return Err(bad(crate::locale::text("plan.err.item_changed")));
                    }
                    definitions.insert(item.id.clone(), item.clone());
                }
            }
        }
        for c in chain.cycles().nodes() {
            for n in c
                .steps()
                .nodes()
                .iter()
                .filter(|n| n.kind == NodeKind::Synthesis && n.is_closed())
            {
                if !self
                    .proposals
                    .iter()
                    .any(|p| p.synthesis_ref == c.step_ref(n.id).to_string())
                {
                    return Err(bad(crate::locale::text("plan.err.synthesis_without_proposal")));
                }
            }
        }
        let mut seen: BTreeMap<&str, &Execution> = BTreeMap::new();
        let mut prior_cycle = 0;
        for e in &self.executed_cycles {
            let p = self
                .proposals
                .iter()
                .find(|p| p.revision == e.revision && self.activated(chain, p))
                .ok_or_else(|| bad(crate::locale::text("plan.err.execution_revision_missing")))?;
            if exploration {
                // A link names an approved requirement of its own revision and says why. Order
                // of the list, earlier links and their verdicts place no constraint on it.
                if !p.draft.items.iter().any(|i| i.id == e.item_id) {
                    return Err(bad(crate::locale::text("plan.err.link_record_unapproved")));
                }
                e.selection.as_ref()
                    .ok_or_else(|| bad(crate::locale::text("plan.err.link_record_selection")))?
                    .validate()
                    .map_err(|_| bad(crate::locale::text("plan.err.link_record_selection")))?;
            } else {
                if e.selection.is_some() {
                    return Err(bad(crate::locale::text("plan.err.selection_outside_contract")));
                }
                let expected = p.draft.items.iter().find(|i| seen.get(i.id.as_str())
                    .is_none_or(|prior| failed_execution(chain, prior)));
                if expected.map(|i| i.id.as_str()) != Some(e.item_id.as_str()) {
                    return Err(bad(crate::locale::text("plan.err.execution_order")));
                }
            }
            let c = chain
                .cycles()
                .nodes()
                .iter()
                .find(|c| {
                    c.id().to_ref().to_string() == e.cycle_ref && c.kind() == CycleKind::Experiment
                })
                .ok_or_else(|| bad(crate::locale::text("plan.err.execution_cycle_missing")))?;
            if let Some(prior) = seen.get(e.item_id.as_str()).filter(|_| !exploration) {
                // A retry must descend from the declared revisit of the last failed attempt.
                // A fresh Interview may intervene, but cannot erase the origin of the retry.
                let mut todo = vec![c.id()];
                let mut visited = BTreeSet::new();
                let mut inherited = false;
                while let Some(id) = todo.pop() {
                    if !visited.insert(id) { continue; }
                    let node = chain.cycles().node(id).ok_or_else(|| bad(crate::locale::text("plan.err.retry_lineage_missing")))?;
                    if let Some(from) = node.revisit_from() {
                        if from.to_ref().to_string() == prior.cycle_ref { inherited = true; break; }
                        todo.push(from);
                    }
                    if let Some(parent) = node.parent() { todo.push(parent); }
                }
                if !inherited {
                    return Err(bad(crate::locale::text("plan.err.retry_lineage")));
                }
            }
            seen.insert(&e.item_id, e);
            if c.id().to_ref().number() <= prior_cycle {
                return Err(bad(crate::locale::text("plan.err.binding_order")));
            }
            prior_cycle = c.id().to_ref().number();
            let latest = self.proposals.iter().rev().find(|q| {
                self.activated(chain, q)
                    && q.synthesis_ref
                        .parse::<StepRef>()
                        .is_ok_and(|r| r.cycle().number() < c.id().to_ref().number())
            });
            if latest.map(|q| q.revision) != Some(e.revision) {
                return Err(bad(crate::locale::text("plan.err.stale_revision")));
            }
            let approved: StepRef = p.synthesis_ref.parse().unwrap();
            if c.id().to_ref().number() <= approved.cycle().number() {
                return Err(bad(crate::locale::text("plan.err.before_approval")));
            }
        }
        for c in chain
            .cycles()
            .nodes()
            .iter()
            .filter(|c| c.kind() == CycleKind::Experiment)
        {
            if self
                .executed_cycles
                .iter()
                .filter(|e| e.cycle_ref == c.id().to_ref().to_string())
                .count()
                != 1
            {
                return Err(bad(crate::locale::text("plan.err.one_binding")));
            }
        }
        Ok(())
    }
}
impl Draft {
    fn validate(&self) -> Result<(), String> {
        if [
            &self.scope.goal,
            &self.scope.in_scope,
            &self.scope.out_of_scope,
            &self.scope.constraints,
            &self.scope.success_criteria,
        ]
        .iter()
        .any(|s| s.trim().is_empty())
        {
            return Err(bad(crate::locale::text("plan.err.scope_fields")));
        }
        if self.items.is_empty()
            || self.items.len() > 64
            || serde_json::to_vec(self).unwrap().len() > 64 * 1024
        {
            return Err(bad(crate::locale::text("plan.err.items_bounds")));
        }
        let mut ids = BTreeSet::new();
        for i in &self.items {
            if i.id.is_empty()
                || i.id.len() > 64
                || !i
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                || !ids.insert(&i.id)
                || i.question.trim().is_empty()
                || i.done_when.trim().is_empty()
            {
                return Err(bad(crate::locale::text("plan.err.item_fields")));
            }
        }
        let report: Report = self.synthesis.clone().into_iter().collect();
        if Report::parse(&report_text(&report)).ok().as_ref() != Some(&report) {
            return Err(bad(crate::locale::text("plan.err.synthesis_shape")));
        }
        if self.synthesis.get("exploration_checklist")
            != Some(
                &self
                    .items
                    .iter()
                    .map(|i| i.id.as_str())
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
        {
            return Err(bad(crate::locale::text("plan.err.checklist")));
        }
        if ["approved", "plan_revision", "plan_digest"]
            .iter()
            .any(|k| self.synthesis.contains_key(*k))
        {
            return Err(bad(crate::locale::text("plan.err.core_fields")));
        }
        Ok(())
    }
}
impl Project {
    pub fn approved_plan(&self) -> Option<&PlanState> {
        self.current_chain()?.approved_plan.as_ref()
    }
    fn plan_mut(&mut self) -> Result<&mut PlanState, String> {
        let crate::project::WorldGraph::Chains(chains) = &mut self.world else {
            return Err(bad(crate::locale::text("plan.err.not_profile")));
        };
        chains
            .last_mut()
            .and_then(|c| c.approved_plan.as_mut())
            .ok_or_else(|| bad(crate::locale::text("plan.err.no_auto_migration")))
    }
    pub fn propose_plan(&mut self, draft: Draft) -> Result<Proposal, String> {
        draft.validate()?;
        let c = self.current_cycle();
        let at = c
            .step_now_open()
            .ok_or_else(|| bad(crate::locale::text("plan.err.open_synthesis_first")))?;
        if c.kind() != CycleKind::Interview
            || c.steps().node(at).unwrap().kind != NodeKind::Synthesis
        {
            return Err(bad(crate::locale::text("plan.err.open_interview_synthesis")));
        }
        let target = c.step_ref(at).to_string();
        let state = self
            .approved_plan()
            .ok_or_else(|| bad(crate::locale::text("plan.err.no_auto_migration")))?;
        // Exploration contract: a Chain is proposed closed as a success only when every problem of
        // the plan has an Experiment that closed as a success. Which Experiment served which
        // problem is already in the record; whether its success was deserved is not judged here.
        if state.exploration() && draft.synthesis.get("chain_verdict").map(String::as_str) == Some("success") {
            let chain = self.current_chain().expect("an approved plan lives in a Chain");
            let succeeded = |execution: &Execution| chain.cycles().nodes().iter()
                .find(|cycle| cycle.id().to_ref().to_string() == execution.cycle_ref)
                .is_some_and(|cycle| cycle.is_closed() && cycle.report().and_then(|report| report.get("verdict")) == Some("success"));
            let open: Vec<&str> = draft.items.iter().filter(|item| !state.linked(&item.id).any(succeeded)).map(|item| item.id.as_str()).collect();
            if !open.is_empty() {
                return Err(bad(&crate::tr!("plan.err.closing_success_needs_experiments", ids = open.join(", "))));
            }
        }
        if let Some(last) = state.proposals.last().filter(|p| p.synthesis_ref == target) {
            if last.draft == draft {
                return Ok(last.clone());
            }
        }
        let mut p = Proposal {
            revision: u32::try_from(state.proposals.len() + 1).map_err(|_| bad(crate::locale::text("plan.err.revision_limit")))?,
            base_revision: state
                .active(self.current_chain().unwrap())
                .map(|p| p.revision),
            synthesis_ref: target,
            draft,
            digest: String::new(),
            receipt: None,
        };
        p.digest = p.hash();
        let mut next = self.clone();
        next.plan_mut()?.proposals.push(p.clone());
        next.approved_plan()
            .unwrap()
            .validate(next.current_chain().unwrap())?;
        // Check ordinary Synthesis grammar and lineage before asking the person to approve it.
        let mut validation = next.clone();
        validation.plan_mut()?.proposals.last_mut().unwrap().receipt = Some(Receipt {
            decision: Decision::Approve,
            source: "monitor_button".into(),
        });
        let report = validation
            .approved_plan()
            .unwrap()
            .proposals
            .last()
            .unwrap()
            .report()?;
        validation
            .close_action_step(report)
            .map_err(|e| bad(&e.to_string()))?;
        *self = next;
        Ok(p)
    }
    // Only the capability-checked MCP App adapter may record human input. No public agent/CLI setter.
    #[cfg(feature = "approved-plan")]
    pub(crate) fn decide_plan(
        &mut self,
        revision: u32,
        digest: &str,
        decision: Decision,
    ) -> Result<Proposal, String> {
        self.decide_plan_from(revision, digest, decision, crate::approval_source::MONITOR)
    }
    /// The same decision with the channel it came through (`crate::approval_source`).
    #[cfg(feature = "approved-plan")]
    pub(crate) fn decide_plan_from(
        &mut self,
        revision: u32,
        digest: &str,
        decision: Decision,
        source: &str,
    ) -> Result<Proposal, String> {
        let p = self
            .approved_plan()
            .and_then(|s| s.current(self))
            .ok_or_else(|| bad(crate::locale::text("plan.err.no_synthesis_to_approve")))?;
        if p.revision != revision || p.digest != digest {
            return Err(bad(crate::locale::text("plan.err.proposal_changed")));
        }
        if let Some(r) = &p.receipt {
            if r.decision != decision {
                return Err(bad(crate::locale::text("plan.err.response_final")));
            }
            return Ok(p.clone());
        }
        let p = self.plan_mut()?.proposals.last_mut().unwrap();
        p.receipt = Some(Receipt {
            decision,
            source: source.into(),
        });
        Ok(p.clone())
    }
    /// Whether this Chain already has an approved plan: the Interview now open is then a later one.
    pub fn plan_approved_in_chain(&self) -> bool {
        self.approved_plan().zip(self.current_chain()).is_some_and(|(state, chain)| state.active(chain).is_some())
    }
    /// The major problems the nearest closed Interpretation of the current Cycle listed, as written.
    pub fn listed_major_problems(&self) -> Option<String> {
        let cycle = self.current_cycle();
        cycle.steps().nodes().iter().rev().filter(|node| node.kind == NodeKind::Interpretation)
            .find_map(|node| node.report.as_ref()?.get("major_problems").map(str::to_owned))
    }
    pub(crate) fn check_plan_close(&self, kind: NodeKind, report: &Report) -> Result<(), String> {
        let Some(s) = self.approved_plan() else {
            return Ok(());
        };
        if kind != NodeKind::Synthesis {
            return Ok(());
        }
        let p = s
            .current(self)
            .ok_or_else(|| bad(crate::locale::text("plan.err.propose_then_wait")))?;
        if &p.report()? != report {
            return Err(bad(crate::locale::text("plan.err.close_as_approved")));
        }
        Ok(())
    }
    /// What the Experiment about to open is bound to. The item queue picks its first pending
    /// item and takes no input. The exploration contract picks nothing: the caller names an
    /// approved requirement of the active revision, and that link is checked here.
    pub(crate) fn next_plan_execution(&self, link: Option<&ExperimentLink>) -> Result<Option<PlanBinding>, String> {
        let Some(s) = self.approved_plan() else {
            return match link {
                Some(_) => Err(link_bad(crate::locale::text("plan.err.link_outside_contract"))),
                None => Ok(None),
            };
        };
        let c = self.current_chain().unwrap();
        if !s.exploration() && link.is_some() {
            return Err(link_bad(crate::locale::text("plan.err.link_outside_contract")));
        }
        let p = self.plan_ready()?.expect("a ledger has an active revision once it is ready");
        if s.exploration() {
            let link = link.ok_or_else(|| link_bad(&crate::tr!("plan.err.link_required", skeleton = ExperimentLink::skeleton(Some(p)))))?;
            self.check_link(link, p)?;
            return Ok(Some(PlanBinding { revision: p.revision, item_id: link.requirement_id.clone(), selection: Some(link.selection.clone()) }));
        }
        let item = s
            .pending(c)
            .first()
            .copied()
            .ok_or_else(|| bad(crate::locale::text("plan.err.no_pending_items")))?;
        Ok(Some(PlanBinding { revision: p.revision, item_id: item.id.clone(), selection: None }))
    }
    /// The approval state an Experiment needs before anything else is looked at: no preserved
    /// re-interview request and an executable approved revision. `Ok(None)` without a ledger.
    /// Both contracts ask this first, so an approval-state refusal is never reported as a
    /// missing or wrong requirement link.
    pub(crate) fn plan_ready(&self) -> Result<Option<&Proposal>, String> {
        let Some(s) = self.approved_plan() else { return Ok(None); };
        let c = self.current_chain().unwrap();
        if !s.awaiting_interview(c).is_empty() {
            return Err(bad(crate::locale::text("plan.err.reinterview_pending")));
        }
        s.active(c).map(Some).ok_or_else(|| bad(crate::locale::text("plan.err.close_outcome_first")))
    }
    /// The link must name the active revision of **this** Chain exactly, and one of its
    /// requirements. A digest that belongs elsewhere is named as such: another Chain's approval
    /// is evidence to read, never permission to execute here, and an older revision of this
    /// Chain was superseded by a later approval.
    fn check_link(&self, link: &ExperimentLink, active: &Proposal) -> Result<(), String> {
        link.selection.validate()?;
        if link.revision != active.revision || link.digest != active.digest {
            let current = self.current_chain().map(|c| c.id());
            if let Some(other) = self.chains().iter().find(|chain| Some(chain.id()) != current
                && chain.approved_plan.as_ref().is_some_and(|s| s.proposals.iter().any(|p| p.digest == link.digest)))
            {
                return Err(link_bad(&crate::tr!("plan.err.link_other_chain", chain = other.id(), revision = active.revision)));
            }
            let state = self.approved_plan().unwrap();
            if state.proposals.iter().any(|p| p.revision != active.revision
                && (p.digest == link.digest || p.revision == link.revision))
            {
                return Err(link_bad(&crate::tr!("plan.err.link_stale", given = link.revision, revision = active.revision)));
            }
            return Err(link_bad(&crate::tr!("plan.err.link_mismatch", revision = active.revision)));
        }
        if !active.draft.items.iter().any(|i| i.id == link.requirement_id) {
            let ids = active.draft.items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>().join(", ");
            return Err(link_bad(&crate::tr!("plan.err.link_unapproved", id = format!("{:?}", link.requirement_id), revision = active.revision, ids = ids)));
        }
        Ok(())
    }
    pub(crate) fn bind_plan_execution(&mut self, binding: PlanBinding, cycle_ref: String) {
        self.plan_mut().unwrap().executed_cycles.push(Execution {
            cycle_ref,
            revision: binding.revision,
            item_id: binding.item_id,
            selection: binding.selection,
        });
    }
    /// Whether the current Chain links Experiments to requirements (exploration contract).
    pub fn exploration_contract(&self) -> bool {
        self.approved_plan().is_some_and(PlanState::exploration)
    }
    /// The selection statements recorded when the current Experiment was linked, if any.
    pub fn current_plan_selection(&self) -> Option<&Selection> {
        let cycle = self.current_cycle().id().to_ref().to_string();
        self.approved_plan()?.executed_cycles.iter().find(|e| e.cycle_ref == cycle)?.selection.as_ref()
    }
    pub fn request_plan_interview(&mut self, reason: String) -> Result<(), String> {
        if reason.trim().is_empty()
            || reason.len() > 4096
            || self.current_cycle().kind() != CycleKind::Experiment
        {
            return Err(bad(crate::locale::text("plan.err.reinterview_reason")));
        }
        let request = ReinterviewRequest {
            cycle_ref: self.current_cycle().id().to_ref().to_string(),
            reason,
        };
        let state = self.plan_mut()?;
        if !state.reinterview_requests.contains(&request) {
            state.reinterview_requests.push(request);
        }
        Ok(())
    }
    pub fn current_plan_item(&self) -> Option<&Item> {
        let state = self.approved_plan()?;
        let e = state
            .executed_cycles
            .iter()
            .find(|e| e.cycle_ref == self.current_cycle().id().to_ref().to_string())?;
        state
            .proposals
            .iter()
            .find(|p| p.revision == e.revision)?
            .draft
            .items
            .iter()
            .find(|i| i.id == e.item_id)
    }
    /// The documents of every plan revision this project has proposed. GIL saves the bound one when
    /// a Verify opens; a file holding one of the others is an earlier save of its own.
    pub(crate) fn plan_documents(&self) -> Vec<String> {
        self.approved_plan().map(|state| state.proposals.iter().map(Proposal::document).collect()).unwrap_or_default()
    }
    pub fn plan_document(&self) -> Option<String> {
        let state = self.approved_plan()?;
        let e = state
            .executed_cycles
            .iter()
            .find(|e| e.cycle_ref == self.current_cycle().id().to_ref().to_string())?;
        state
            .proposals
            .iter()
            .find(|p| p.revision == e.revision)
            .map(Proposal::document)
    }
}

#[cfg(test)]
impl Project {
    /// Test fixture only: the first Chain as a build without the exploration contract opened it.
    pub(crate) fn as_item_queue_for_test(mut self) -> Self {
        self.plan_mut().expect("approved-plan profile").contract = None;
        self
    }
}

/// A checked binding waiting for the Cycle it belongs to. Built only by `next_plan_execution`.
pub(crate) struct PlanBinding {
    revision: u32,
    item_id: String,
    selection: Option<Selection>,
}

#[cfg(all(test, feature = "approved-plan"))]
pub(crate) mod tests;
#[cfg(all(test, feature = "exploration-flow"))]
mod exploration_tests;

pub(crate) fn report_text(report: &Report) -> String {
    report
        .field_names()
        .map(|key| {
            format!(
                "{key}: |\n{}\n",
                report
                    .get(key)
                    .unwrap()
                    .split('\n')
                    .map(|line| format!("  {line}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        })
        .collect()
}
