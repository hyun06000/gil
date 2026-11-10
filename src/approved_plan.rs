//! M6-D opt-in approval ledger. No filesystem writes or identity-authentication claims.
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
    pub done_when: String,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Execution {
    pub cycle_ref: String,
    pub revision: u32,
    pub item_id: String,
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
    pub proposals: Vec<Proposal>,
    pub executed_cycles: Vec<Execution>,
    pub reinterview_requests: Vec<ReinterviewRequest>,
}

fn bad(message: &str) -> String {
    crate::tr!("plan.err.prefix", message = message)
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
        for (field, stored, received) in [("question", &old.question, &new.question), ("done_when", &old.done_when, &new.done_when)] {
            if stored != received {
                differences.push(crate::tr!("plan.err.item_mismatch", id = preview(&old.id), field = field, stored = preview(stored), received = preview(received)));
            }
        }
    }
    if differences.is_empty() { return Ok(()); }
    let count = differences.len();
    differences.truncate(8);
    Err(bad(&crate::tr!("plan.err.retained", differences = differences.join("\n"),
        more = if count > 8 { crate::tr!("plan.err.more_differences", n = count - 8) } else { String::new() })))
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
    pub fn pending<'a>(&'a self, chain: &Chain) -> Vec<&'a Item> {
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
                .is_some_and(|r| r.source != "monitor_button")
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
            let expected = p.draft.items.iter().find(|i| seen.get(i.id.as_str())
                .is_none_or(|prior| failed_execution(chain, prior)));
            if expected.map(|i| i.id.as_str()) != Some(e.item_id.as_str()) {
                return Err(bad(crate::locale::text("plan.err.execution_order")));
            }
            let c = chain
                .cycles()
                .nodes()
                .iter()
                .find(|c| {
                    c.id().to_ref().to_string() == e.cycle_ref && c.kind() == CycleKind::Experiment
                })
                .ok_or_else(|| bad(crate::locale::text("plan.err.execution_cycle_missing")))?;
            if let Some(prior) = seen.get(e.item_id.as_str()) {
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
            source: "monitor_button".into(),
        });
        Ok(p.clone())
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
    pub(crate) fn next_plan_execution(&self) -> Result<Option<(u32, String)>, String> {
        let Some(s) = self.approved_plan() else {
            return Ok(None);
        };
        let c = self.current_chain().unwrap();
        if !s.awaiting_interview(c).is_empty() {
            return Err(bad(crate::locale::text("plan.err.reinterview_pending")));
        }
        let p = s
            .active(c)
            .ok_or_else(|| bad(crate::locale::text("plan.err.close_outcome_first")))?;
        let item = s
            .pending(c)
            .first()
            .copied()
            .ok_or_else(|| bad(crate::locale::text("plan.err.no_pending_items")))?;
        Ok(Some((p.revision, item.id.clone())))
    }
    pub(crate) fn bind_plan_execution(
        &mut self,
        revision: u32,
        item_id: String,
        cycle_ref: String,
    ) {
        self.plan_mut().unwrap().executed_cycles.push(Execution {
            cycle_ref,
            revision,
            item_id,
        });
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

#[cfg(all(test, feature = "approved-plan"))]
pub(crate) mod tests;

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
