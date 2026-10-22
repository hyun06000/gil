//! Runtime update — **pure** store-state judge (Runtime Update Implementation Plan §1.1).
//!
//! Decides, from observations only, which state the GIL-owned runtime store is in and what the
//! launcher may run. It performs no file access, no locking, no process execution; the caller
//! observes the store and hands the facts in. Every combination of inputs ends in exactly one
//! [`State`] (property-tested below).
//!
//! Decision rules (contract §3, plan §1.1, 2026-10-23):
//! - any access denial or protection failure → `Blocked`;
//! - no store root → `FirstUse` (bundled Core);
//! - root present but `store.json` absent/invalid → `Damaged` regardless of the other files;
//! - a valid selection with no intent is `Selected` only when its generation equals the
//!   recorded history `last_generation`;
//! - a valid intent allows `Selected` only when the selection matches the intent's generation,
//!   version and full digest and the intent stands in a defined normal-transition relation to
//!   the history: `last_generation + 1` (published, history update pending → `history_lagging`)
//!   or `last_generation` (history updated, intent cleanup pending); a mismatching or otherwise
//!   unexplained intent, selection or generation gap is `Damaged`;
//! - a published selection or an intent with generation 0 is never a selected state (generation
//!   0 means "never selected" and belongs to `store.last_generation` only); generation
//!   arithmetic uses checked operations, so `u64::MAX` histories cannot overflow;
//! - with no selection ever published (`last_generation == 0`, no intent, no selection) the
//!   bundled Core runs in `Initializing` / `Prepared`; a recorded intent without a matching
//!   published selection never falls back to the bundled Core.
#![cfg_attr(not(test), allow(dead_code))]

/// What the caller observed for a JSON record of the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Record<T> {
    Absent,
    /// Present but unparsable, wrong schema, or wrong owner.
    Invalid,
    Valid(T),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Initializing,
    Prepared,
}

/// `store.json`: the initialization record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Store {
    pub phase: Phase,
    /// Generation of the last selection whose publication **and** history update completed.
    pub last_generation: u64,
}

/// `intent.json`: the durable selection intent written before a selection is published.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Intent {
    pub generation: u64,
    pub version: String,
    pub sha256: String,
}

/// `selection.json`: the published selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub generation: u64,
    pub version: String,
    pub sha256: String,
    /// The referenced version directory exists with its `complete` marker and its digest matches.
    pub version_complete: bool,
}

/// Everything the judge needs; gathered read-only by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    /// Access denials or protection-check failures already found (empty = none).
    pub blocked: Vec<String>,
    pub root_exists: bool,
    pub store: Record<Store>,
    pub intent: Record<Intent>,
    pub selection: Record<Selection>,
    /// Interruption traces that do not change the state: `*.new` leftovers, incomplete version
    /// directories, and so on.
    pub traces: Vec<String>,
}

/// What the launcher may run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Run {
    /// The bundled Core, in-process.
    Bundled,
    /// The selected Core version.
    Selected,
    /// No GIL Core at all: diagnostic surface only.
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    Blocked { reasons: Vec<String> },
    FirstUse,
    Initializing,
    Prepared,
    Selected { generation: u64, version: String, history_lagging: bool },
    Damaged { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub state: State,
    pub run: Run,
    /// Diagnostic-only surface: `gil_runtime_status` and `gil_help`; recovery tools only when
    /// `recovery_allowed` (Damaged with protection checks passed), never when Blocked.
    pub diagnostic_only: bool,
    pub recovery_allowed: bool,
    pub interrupted: Vec<String>,
}

impl Verdict {
    fn new(state: State, run: Run, interrupted: Vec<String>) -> Self {
        let (diagnostic_only, recovery_allowed) = match &state {
            State::Blocked { .. } => (true, false),
            State::Damaged { .. } => (true, true),
            _ => (false, false),
        };
        Self { state, run, diagnostic_only, recovery_allowed, interrupted }
    }
}

/// The judge. Pure: no I/O, no side effects.
pub fn judge(o: &Observation) -> Verdict {
    let mut traces = o.traces.clone();
    let damaged = |reason: &str, traces: Vec<String>| Verdict::new(State::Damaged { reason: reason.to_owned() }, Run::None, traces);

    // 1. Access denial / protection failure outranks everything (no bypass, no recovery tools).
    if !o.blocked.is_empty() {
        return Verdict::new(State::Blocked { reasons: o.blocked.clone() }, Run::None, traces);
    }
    // 2. No root at all: verified first use.
    if !o.root_exists {
        return Verdict::new(State::FirstUse, Run::Bundled, traces);
    }
    // 3. Root without a valid initialization record is unclear, whatever else exists.
    let store = match &o.store {
        Record::Valid(s) => s,
        Record::Absent => return damaged("store.json absent under an existing root", traces),
        Record::Invalid => return damaged("store.json invalid", traces),
    };
    // 4. A damaged intent is never explained away.
    let intent = match &o.intent {
        Record::Valid(i) => Some(i),
        Record::Absent => None,
        Record::Invalid => return damaged("intent.json invalid", traces),
    };
    // 5. Selection cases.
    match &o.selection {
        Record::Invalid => return damaged("selection.json invalid", traces),
        Record::Valid(sel) if !sel.version_complete => {
            return damaged("selection points to a missing or incomplete version", traces);
        }
        Record::Valid(sel) if sel.generation == 0 => {
            return damaged("selection generation 0 is not a selectable generation", traces);
        }
        Record::Valid(sel) => {
            return match intent {
                // No intent: the published selection must agree with the completed history.
                None if sel.generation == store.last_generation && sel.generation >= 1 => {
                    Verdict::new(State::Selected { generation: sel.generation, version: sel.version.clone(), history_lagging: false }, Run::Selected, traces)
                }
                None if store.last_generation == 0 => damaged("selection present but no completed history and no intent", traces),
                None => damaged("selection generation differs from history without an intent", traces),
                // Valid intent: only the exact next transition, matching in every field, is
                // explained (publication done, history update not yet).
                Some(i) => {
                    if i.generation == 0 {
                        return damaged("intent generation 0 is not a selectable generation", traces);
                    }
                    let matches = i.generation == sel.generation && i.version == sel.version && i.sha256 == sel.sha256;
                    if !matches {
                        return damaged("selection does not match the recorded intent", traces);
                    }
                    // Overflow-free: a history at u64::MAX has no next transition.
                    if store.last_generation.checked_add(1) == Some(i.generation) {
                        traces.push("history_lagging: selection published, last_generation not yet updated, intent not yet cleared".to_owned());
                        return Verdict::new(State::Selected { generation: sel.generation, version: sel.version.clone(), history_lagging: true }, Run::Selected, traces);
                    }
                    if i.generation == store.last_generation {
                        traces.push("intent_pending_cleanup: history updated, intent.json not yet cleared".to_owned());
                        return Verdict::new(State::Selected { generation: sel.generation, version: sel.version.clone(), history_lagging: false }, Run::Selected, traces);
                    }
                    damaged("intent generation is neither the next transition nor the completed one", traces)
                }
            };
        }
        Record::Absent => {}
    }
    // 6. No published selection.
    if let Some(i) = intent {
        if i.generation == 0 {
            return damaged("intent generation 0 is not a selectable generation", traces);
        }
        return damaged("selection intent recorded but no valid selection published", traces);
    }
    if store.last_generation >= 1 {
        return damaged("past selection lost: history records a generation but no selection exists", traces);
    }
    // 7. Verified "never selected": the bundled Core may run.
    match store.phase {
        Phase::Initializing => Verdict::new(State::Initializing, Run::Bundled, traces),
        Phase::Prepared => Verdict::new(State::Prepared, Run::Bundled, traces),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sel(generation: u64, version: &str, sha: &str, complete: bool) -> Record<Selection> {
        Record::Valid(Selection { generation, version: version.into(), sha256: sha.into(), version_complete: complete })
    }
    fn intent(generation: u64, version: &str, sha: &str) -> Record<Intent> {
        Record::Valid(Intent { generation, version: version.into(), sha256: sha.into() })
    }
    fn store(phase: Phase, last_generation: u64) -> Record<Store> {
        Record::Valid(Store { phase, last_generation })
    }
    fn obs(root_exists: bool, store: Record<Store>, intent: Record<Intent>, selection: Record<Selection>) -> Observation {
        Observation { blocked: vec![], root_exists, store, intent, selection, traces: vec![] }
    }
    fn state(o: &Observation) -> State { judge(o).state }

    // ── table rows ──
    #[test]
    fn blocked_outranks_everything_and_offers_no_recovery() {
        let mut o = obs(true, store(Phase::Prepared, 1), Record::Absent, sel(1, "v1", "a", true));
        o.blocked = vec!["selection.json: access denied".into()];
        let v = judge(&o);
        assert!(matches!(v.state, State::Blocked { .. }));
        assert_eq!(v.run, Run::None);
        assert!(v.diagnostic_only && !v.recovery_allowed);
    }
    #[test]
    fn no_root_is_first_use_with_bundled_core() {
        let v = judge(&obs(false, Record::Absent, Record::Absent, Record::Absent));
        assert_eq!(v.state, State::FirstUse);
        assert_eq!(v.run, Run::Bundled);
        assert!(!v.diagnostic_only);
    }
    #[test]
    fn root_without_valid_store_is_damaged_whatever_else_exists() {
        // AT-B2a: store.json and selection both absent or damaged under an existing root.
        for st in [Record::Absent, Record::Invalid] {
            for s in [Record::Absent, Record::Invalid, sel(1, "v1", "a", true)] {
                for i in [Record::Absent, intent(1, "v1", "a")] {
                    let v = judge(&obs(true, st.clone(), i, s.clone()));
                    assert!(matches!(v.state, State::Damaged { .. }), "{v:?}");
                    assert_eq!(v.run, Run::None);
                    assert!(v.diagnostic_only && v.recovery_allowed);
                }
            }
        }
    }
    #[test]
    fn initializing_and_prepared_need_a_verified_never_selected_record() {
        assert_eq!(state(&obs(true, store(Phase::Initializing, 0), Record::Absent, Record::Absent)), State::Initializing);
        assert_eq!(state(&obs(true, store(Phase::Prepared, 0), Record::Absent, Record::Absent)), State::Prepared);
        assert_eq!(judge(&obs(true, store(Phase::Prepared, 0), Record::Absent, Record::Absent)).run, Run::Bundled);
    }
    #[test]
    fn selected_without_intent_requires_generation_equal_to_history() {
        let v = judge(&obs(true, store(Phase::Prepared, 3), Record::Absent, sel(3, "v3", "c", true)));
        assert_eq!(v.state, State::Selected { generation: 3, version: "v3".into(), history_lagging: false });
        assert_eq!(v.run, Run::Selected);
        // generation boundaries: lower or higher than history without an intent is unexplained
        for g in [2, 4] {
            assert!(matches!(state(&obs(true, store(Phase::Prepared, 3), Record::Absent, sel(g, "vx", "x", true))), State::Damaged { .. }));
        }
    }
    #[test]
    fn selection_with_zero_history_and_no_intent_is_damaged_but_matching_intent_is_the_exception() {
        // AT-B2b corrected: last_generation = 0 + selection present is damaged only without an
        // explaining intent; the first publication before the history update is Selected.
        assert!(matches!(state(&obs(true, store(Phase::Prepared, 0), Record::Absent, sel(1, "v1", "a", true))), State::Damaged { .. }));
        let v = judge(&obs(true, store(Phase::Prepared, 0), intent(1, "v1", "a"), sel(1, "v1", "a", true)));
        assert_eq!(v.state, State::Selected { generation: 1, version: "v1".into(), history_lagging: true });
        assert_eq!(v.run, Run::Selected);
        assert!(v.interrupted.iter().any(|t| t.starts_with("history_lagging")));
    }
    #[test]
    fn intent_mismatch_or_unexplained_gap_is_damaged() {
        let base = store(Phase::Prepared, 1);
        // version differs
        assert!(matches!(state(&obs(true, base.clone(), intent(2, "v2", "b"), sel(2, "v2-other", "b", true))), State::Damaged { .. }));
        // digest differs
        assert!(matches!(state(&obs(true, base.clone(), intent(2, "v2", "b"), sel(2, "v2", "bb", true))), State::Damaged { .. }));
        // generation differs
        assert!(matches!(state(&obs(true, base.clone(), intent(2, "v2", "b"), sel(3, "v2", "b", true))), State::Damaged { .. }));
        // intent is not the next transition (skips a generation, or stale)
        assert!(matches!(state(&obs(true, base.clone(), intent(3, "v3", "c"), sel(3, "v3", "c", true))), State::Damaged { .. }));
        // intent two steps ahead, or older than the history, is unexplained
        assert!(matches!(state(&obs(true, store(Phase::Prepared, 0), intent(2, "v2", "b"), sel(2, "v2", "b", true))), State::Damaged { .. }));
        assert!(matches!(state(&obs(true, store(Phase::Prepared, 3), intent(2, "v2", "b"), sel(2, "v2", "b", true))), State::Damaged { .. }));
        // intent damaged
        assert!(matches!(state(&obs(true, base.clone(), Record::Invalid, sel(1, "v1", "a", true))), State::Damaged { .. }));
        assert!(matches!(state(&obs(true, base, Record::Invalid, Record::Absent)), State::Damaged { .. }));
    }
    #[test]
    fn intent_without_a_published_selection_never_runs_the_bundled_core() {
        for st in [store(Phase::Prepared, 0), store(Phase::Prepared, 1), store(Phase::Initializing, 0)] {
            let v = judge(&obs(true, st, intent(1, "v1", "a"), Record::Absent));
            assert!(matches!(v.state, State::Damaged { .. }), "{v:?}");
            assert_eq!(v.run, Run::None);
            assert!(v.recovery_allowed);
        }
        // intent present, selection points to the previous generation (published not yet)
        assert!(matches!(state(&obs(true, store(Phase::Prepared, 1), intent(2, "v2", "b"), sel(1, "v1", "a", true))), State::Damaged { .. }));
    }
    #[test]
    fn lost_or_invalid_or_incomplete_selection_is_damaged() {
        assert!(matches!(state(&obs(true, store(Phase::Prepared, 1), Record::Absent, Record::Absent)), State::Damaged { .. }));
        assert!(matches!(state(&obs(true, store(Phase::Prepared, 1), Record::Absent, Record::Invalid)), State::Damaged { .. }));
        assert!(matches!(state(&obs(true, store(Phase::Prepared, 1), Record::Absent, sel(1, "v1", "a", false))), State::Damaged { .. }));
        assert!(matches!(state(&obs(true, store(Phase::Prepared, 0), Record::Absent, Record::Invalid)), State::Damaged { .. }));
    }

    // ── interruption points of the recording order (plan §1.1 step 2) ──
    #[test]
    fn select_interruption_points_judge_as_specified() {
        // Starting point: generation 1 selected, history 1.
        let before = obs(true, store(Phase::Prepared, 1), Record::Absent, sel(1, "v1", "a", true));
        assert!(matches!(state(&before), State::Selected { generation: 1, history_lagging: false, .. }));
        // (a) after intent durable, before publication: previous selection still there → damaged
        let after_intent = obs(true, store(Phase::Prepared, 1), intent(2, "v2", "b"), sel(1, "v1", "a", true));
        assert!(matches!(state(&after_intent), State::Damaged { .. }));
        assert_eq!(judge(&after_intent).run, Run::None);
        // (b) after publication, before history update → selected + lagging
        let after_publish = obs(true, store(Phase::Prepared, 1), intent(2, "v2", "b"), sel(2, "v2", "b", true));
        assert_eq!(state(&after_publish), State::Selected { generation: 2, version: "v2".into(), history_lagging: true });
        // (c) after history update, before intent cleared → selected (intent equals history, explained)
        let after_history = obs(true, store(Phase::Prepared, 2), intent(2, "v2", "b"), sel(2, "v2", "b", true));
        // intent generation == last_generation (not +1): the intent is already fulfilled; this is
        // the "history updated, intent not yet cleared" point and is reported, not damaged.
        let v = judge(&after_history);
        assert_eq!(v.state, State::Selected { generation: 2, version: "v2".into(), history_lagging: false }, "{v:?}");
        assert!(v.interrupted.iter().any(|t| t.starts_with("intent_pending_cleanup")));
        // (d) after intent cleared → selected
        assert_eq!(state(&obs(true, store(Phase::Prepared, 2), Record::Absent, sel(2, "v2", "b", true))), State::Selected { generation: 2, version: "v2".into(), history_lagging: false });
    }
    #[test]
    fn first_publication_then_selection_lost_before_history_update_is_damaged() {
        // AT-B6b
        let v = judge(&obs(true, store(Phase::Prepared, 0), intent(1, "v1", "a"), Record::Absent));
        assert!(matches!(v.state, State::Damaged { .. }));
        assert_eq!(v.run, Run::None);
    }
    #[test]
    fn prepare_interruption_points_before_store_json_are_damaged_after_are_bundled() {
        // root created, store.json not yet written
        assert!(matches!(state(&obs(true, Record::Absent, Record::Absent, Record::Absent)), State::Damaged { .. }));
        // store.json written, version dir / copy / complete / phase update pending
        assert_eq!(state(&obs(true, store(Phase::Initializing, 0), Record::Absent, Record::Absent)), State::Initializing);
        // phase updated, never selected, reconnect (AT-B5)
        assert_eq!(state(&obs(true, store(Phase::Prepared, 0), Record::Absent, Record::Absent)), State::Prepared);
    }

    // ── generation boundaries: zero and u64::MAX ──
    #[test]
    fn generation_zero_is_never_a_selected_state() {
        let v = judge(&obs(true, store(Phase::Prepared, 0), intent(0, "v0", "z"), sel(0, "v0", "z", true)));
        assert!(matches!(v.state, State::Damaged { .. }), "{v:?}");
        assert_eq!(v.run, Run::None);
        assert!(matches!(state(&obs(true, store(Phase::Prepared, 0), Record::Absent, sel(0, "v0", "z", true))), State::Damaged { .. }));
        assert!(matches!(state(&obs(true, store(Phase::Prepared, 0), intent(0, "v0", "z"), Record::Absent)), State::Damaged { .. }));
        // store.last_generation = 0 keeps its normal "never selected" meaning
        assert_eq!(state(&obs(true, store(Phase::Prepared, 0), Record::Absent, Record::Absent)), State::Prepared);
    }
    #[test]
    fn history_at_max_with_matching_selection_and_no_intent_is_selected() {
        let v = judge(&obs(true, store(Phase::Prepared, u64::MAX), Record::Absent, sel(u64::MAX, "vmax", "m", true)));
        assert_eq!(v.state, State::Selected { generation: u64::MAX, version: "vmax".into(), history_lagging: false });
        assert_eq!(v.run, Run::Selected);
    }
    #[test]
    fn history_at_max_with_matching_intent_is_pending_cleanup_without_panic() {
        let v = judge(&obs(true, store(Phase::Prepared, u64::MAX), intent(u64::MAX, "vmax", "m"), sel(u64::MAX, "vmax", "m", true)));
        assert_eq!(v.state, State::Selected { generation: u64::MAX, version: "vmax".into(), history_lagging: false });
        assert!(v.interrupted.iter().any(|t| t.starts_with("intent_pending_cleanup")));
    }
    #[test]
    fn wrap_around_from_max_to_zero_is_damaged() {
        let v = judge(&obs(true, store(Phase::Prepared, u64::MAX), intent(0, "v0", "z"), sel(0, "v0", "z", true)));
        assert!(matches!(v.state, State::Damaged { .. }), "{v:?}");
        assert_eq!(v.run, Run::None);
        assert!(matches!(state(&obs(true, store(Phase::Prepared, u64::MAX), Record::Absent, sel(0, "v0", "z", true))), State::Damaged { .. }));
    }
    #[test]
    fn history_max_minus_one_with_max_intent_and_selection_is_history_lagging() {
        let v = judge(&obs(true, store(Phase::Prepared, u64::MAX - 1), intent(u64::MAX, "vmax", "m"), sel(u64::MAX, "vmax", "m", true)));
        assert_eq!(v.state, State::Selected { generation: u64::MAX, version: "vmax".into(), history_lagging: true });
        assert!(v.interrupted.iter().any(|t| t.starts_with("history_lagging")));
    }

    // ── every input combination ends in exactly one state (exhaustive over a bounded domain) ──
    #[test]
    fn every_combination_has_exactly_one_verdict_and_bundled_only_when_never_selected() {
        const MAX: u64 = u64::MAX;
        let stores = [Record::Absent, Record::Invalid, store(Phase::Initializing, 0), store(Phase::Prepared, 0), store(Phase::Prepared, 1), store(Phase::Prepared, 2), store(Phase::Prepared, MAX - 1), store(Phase::Prepared, MAX)];
        let intents = [Record::Absent, Record::Invalid, intent(0, "v0", "z"), intent(1, "v1", "a"), intent(2, "v2", "b"), intent(3, "v3", "c"), intent(2, "v2", "zz"), intent(MAX - 1, "vm1", "m1"), intent(MAX, "vmax", "m")];
        let selections = [Record::Absent, Record::Invalid, sel(0, "v0", "z", true), sel(1, "v1", "a", true), sel(2, "v2", "b", true), sel(3, "v3", "c", true), sel(2, "v2", "b", false), sel(1, "v1", "a", false), sel(MAX - 1, "vm1", "m1", true), sel(MAX, "vmax", "m", true)];
        let mut count = 0;
        for root in [false, true] {
            for blocked in [vec![], vec!["x".to_string()]] {
                for st in &stores { for i in &intents { for s in &selections {
                    let o = Observation { blocked: blocked.clone(), root_exists: root, store: st.clone(), intent: i.clone(), selection: s.clone(), traces: vec![] };
                    let v = judge(&o); count += 1;
                    // consistency of flags with state
                    match &v.state {
                        State::Blocked { .. } => { assert_eq!(v.run, Run::None); assert!(v.diagnostic_only && !v.recovery_allowed); assert!(!blocked.is_empty()); }
                        State::FirstUse => { assert_eq!(v.run, Run::Bundled); assert!(!root); }
                        State::Initializing | State::Prepared => {
                            assert_eq!(v.run, Run::Bundled);
                            // bundled only when a valid record proves "never selected"
                            assert!(matches!(st, Record::Valid(Store { last_generation: 0, .. })));
                            assert_eq!(*i, Record::Absent); assert_eq!(*s, Record::Absent);
                        }
                        State::Selected { generation, .. } => {
                            assert_eq!(v.run, Run::Selected);
                            assert!(*generation >= 1, "Selected is always generation >= 1: {v:?}");
                            assert!(matches!(s, Record::Valid(Selection { version_complete: true, .. })));
                            if let Record::Valid(i) = i { assert!(i.generation >= 1); }
                        }
                        State::Damaged { .. } => { assert_eq!(v.run, Run::None); assert!(v.diagnostic_only && v.recovery_allowed); }
                    }
                }}}
            }
        }
        assert_eq!(count, 2 * 2 * stores.len() * intents.len() * selections.len());
    }
}
