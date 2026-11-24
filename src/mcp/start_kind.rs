//! Which new-start contract one `gil_start` call takes.
//!
//! A build may hold two contracts for a **new** project, and they do not mix in one project:
//!
//! ```text
//! start_kind      what the folder becomes                          how it starts
//! tracked_scope   a project whose tracked files follow an exact    start preparation → exact proposal →
//!                 .gilignore the user approved                     the user's Monitor approval → first archive
//! image_reports   a project whose reports can hold PNG visuals     created at once; whole-folder observation
//! ```
//!
//! This module decides the route **once per call**, from the explicit input and from what the
//! folder already holds, and creates nothing while deciding. It never picks a contract the user
//! did not choose, never continues one contract's leftovers under the other, and never converts
//! an existing record. Storage format numbers and build feature names stay out of every answer.
//!
//! | build holds            | `start_kind` omitted                       | explicit value                         |
//! |------------------------|--------------------------------------------|----------------------------------------|
//! | neither / images only  | the build's one start, as before           | not in the schema (wire refusal)       |
//! | tracked only           | start preparation, as before               | `tracked_scope` only                   |
//! | both                   | existing preparation → continue it;        | that contract, or a refusal that names |
//! |                        | other `.gil` → inspect only; empty → ask   | what is already in the folder          |
use rmcp::model::{CallToolResult, ContentBlock, Tool};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub(super) const INPUT: &str = "start_kind";
pub(super) const TRACKED: &str = "tracked_scope";
pub(super) const IMAGES: &str = "image_reports";

/// The start preparation path exists in this build (same cfg as `start_prep`).
pub(super) const HAS_TRACKED: bool = cfg!(all(feature = "tracking-policy", unix));
/// The image-report creation entry exists in this build. Whether this platform's storage is
/// supported is decided by that entry itself, which refuses instead of falling back.
pub(super) const HAS_IMAGES: bool = cfg!(feature = "visual-authoring");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Route {
    /// Tracked-scope start preparation. No Chain, Cycle, Step or Snapshot until the Monitor approval.
    Preparation,
    /// The build's direct start: image reports where built, otherwise the plain start.
    Direct,
    /// Nothing starts and nothing was created or changed. `code` is stable; `key` names the text.
    Refused { code: &'static str, key: &'static str },
}

/// The route could not be decided (the deciding task was lost). Nothing was created.
pub(super) const UNDECIDED: Route = Route::Refused { code: "start_kind_undecided", key: "start_kind.undecided" };
const REQUIRED: Route = Route::Refused { code: "start_kind_required", key: "start_kind.required" };
const UNKNOWN: Route = Route::Refused { code: "start_kind_unknown", key: "start_kind.unknown" };
const UNAVAILABLE: Route = Route::Refused { code: "start_kind_unavailable", key: "start_kind.unavailable" };
const TRACKED_HERE: Route = Route::Refused { code: "start_kind_conflict", key: "start_kind.tracked_here" };
const UNSETTLED: Route = Route::Refused { code: "start_record_unsettled", key: "start_kind.unsettled" };
const STARTED: Route = Route::Refused { code: "already_started", key: "start_prep.already_started" };

/// What the selected folder already holds, read without following the last component and
/// without opening, locking or creating anything. An entry that cannot be read counts as present.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) struct Held {
    /// `.gil` exists in any form (folder, file, link, unreadable).
    pub(super) record: bool,
    /// `.gil/start` exists: a tracked-scope start preparation, or the project it became.
    pub(super) preparation: bool,
    /// `.gil/state.yaml` exists: a started project.
    pub(super) state: bool,
    /// `.gil` is a real folder that holds nothing, or nothing but the project lock file: a start
    /// stopped before it recorded anything of either kind. There is nothing to adopt or convert.
    pub(super) bare: bool,
}

fn present(path: &Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(_) => true,
        Err(error) => error.kind() != std::io::ErrorKind::NotFound,
    }
}

#[cfg(all(feature = "tracking-policy", unix))]
const PREPARATION_DIR: &str = crate::start_prep::PREP_DIR;
#[cfg(not(all(feature = "tracking-policy", unix)))]
const PREPARATION_DIR: &str = "start";

pub(super) fn held(root: &Path) -> Held {
    let gil = root.join(".gil");
    if !present(&gil) { return Held::default(); }
    Held { record: true, preparation: present(&gil.join(PREPARATION_DIR)), state: present(&gil.join("state.yaml")), bare: bare(&gil) }
}

/// Listing only: the entry itself must be a folder (a link is not followed), and an entry that
/// cannot be read counts as something else being there.
fn bare(gil: &Path) -> bool {
    std::fs::symlink_metadata(gil).is_ok_and(|entry| entry.is_dir())
        && std::fs::read_dir(gil).is_ok_and(|mut entries| entries.all(|entry| entry.is_ok_and(|entry| entry.file_name() == crate::lock::LOCK_FILE)))
}

/// The already resolved absolute root of this call, if it carries one. A missing or relative
/// root is left to the tool's own root check, which refuses it without touching anything.
fn root_of(input: &Value) -> Option<PathBuf> {
    input.get("project_root").and_then(Value::as_str).map(PathBuf::from).filter(|root| root.is_absolute())
}

pub(super) fn route(input: &Value) -> Route {
    let chosen = match input.get(INPUT) {
        None => None,
        Some(Value::String(kind)) => Some(kind.as_str()),
        Some(_) => return UNKNOWN,
    };
    let held = root_of(input).map(|root| held(&root));
    decide(chosen, held, HAS_TRACKED, HAS_IMAGES)
}

/// Pure decision, so every row of the table above is checked without a filesystem or a build matrix.
/// `held` is `None` when the call carries no absolute root.
pub(super) fn decide(chosen: Option<&str>, held: Option<Held>, tracked: bool, images: bool) -> Route {
    let both = tracked && images;
    match chosen {
        Some(TRACKED) if !tracked => UNAVAILABLE,
        Some(IMAGES) if !images => UNAVAILABLE,
        Some(TRACKED) if both => match held {
            // An existing preparation (or the project it became) is continued or answered by its own record.
            Some(Held { preparation: true, .. }) | None => Route::Preparation,
            // A started project of the other contract: read it, never prepare over it.
            Some(Held { record: true, state: true, .. }) => STARTED,
            // A start that stopped before it recorded anything (an empty `.gil`, or only its lock
            // file): the ordinary retry. The preparation checks the same thing again under the lock.
            Some(Held { record: true, bare: true, .. }) => Route::Preparation,
            // Any other `.gil` without a state file and without a preparation: its origin cannot be
            // told (an interrupted creation of either contract). Preserved; neither contract adopts it.
            Some(Held { record: true, .. }) => UNSETTLED,
            Some(_) => Route::Preparation,
        },
        Some(TRACKED) => Route::Preparation,
        Some(IMAGES) if both => match held {
            // The image-report start never replaces or converts a tracked-scope record.
            Some(Held { preparation: true, .. }) => TRACKED_HERE,
            // Any other existing `.gil` is only inspected by the creation entry, which refuses.
            _ => Route::Direct,
        },
        Some(IMAGES) => Route::Direct,
        Some(_) => UNKNOWN,
        None if both => match held {
            Some(Held { preparation: true, .. }) => Route::Preparation,
            // Inspect-only: the creation entry refuses every existing `.gil` and creates nothing.
            Some(Held { record: true, .. }) | None => Route::Direct,
            // An empty folder: the user chooses. Nothing is written before that.
            Some(_) => REQUIRED,
        },
        None if tracked => Route::Preparation,
        None => Route::Direct,
    }
}

/// A preparation tool that takes the project lock (`gil_start_prepare`, `gil_start_cancel`) was
/// aimed at a folder whose `.gil` is not a tracked-scope record. With two kinds in one build that
/// is the other kind's project or leftovers of an unknown creation: answer before the lock, so the
/// request neither recovers nor reaps anything there. A build with one kind is unchanged.
pub(super) fn other_record(name: &str, input: &Value) -> Option<(&'static str, &'static str)> {
    if !(HAS_TRACKED && HAS_IMAGES) || !matches!(name, "gil_start_prepare" | "gil_start_cancel") { return None; }
    match held(&root_of(input)?) {
        Held { record: true, preparation: false, state: true, .. } => Some(("already_started", "start_prep.already_started")),
        // An empty `.gil` holds no preparation: the tool's own "nothing is being prepared" answers it.
        Held { record: true, preparation: false, state: false, bare: false } => Some(("start_record_unsettled", "start_kind.unsettled")),
        _ => None,
    }
}

/// The refusal for a route that starts nothing. The text is in the caller's locale.
pub(super) fn refusal(code: &'static str, key: &'static str) -> CallToolResult {
    let mut out = super::actions::refusal(crate::locale::text(key));
    if let Some(fields) = out.structured_content.as_mut().and_then(Value::as_object_mut) {
        fields.insert("code".into(), json!(code));
        fields.insert("created".into(), json!(false));
        if code == "start_kind_required" {
            fields.insert("start_kinds".into(), json!([
                {"value": TRACKED, "summary": crate::locale::text("start_kind.summary.tracked_scope")},
                {"value": IMAGES, "summary": crate::locale::text("start_kind.summary.image_reports")},
            ]));
            out.content.push(ContentBlock::text(format!("{TRACKED}: {}\n{IMAGES}: {}",
                crate::locale::text("start_kind.summary.tracked_scope"), crate::locale::text("start_kind.summary.image_reports"))));
        }
    }
    out
}

const BOTH: &str = "Start a NEW GIL project in the user's explicitly selected absolute project_root. This GIL offers two kinds of new project; they are separate storage contracts, a project is one or the other, and it is never converted later. Before the first call in an empty folder, tell the user both options with their limits and pass the user's own choice as start_kind; never choose for the user and never edit .gil files to change the kind.\n\
- start_kind \"tracked_scope\": the user approves exactly which files GIL tracks. This call only begins or resumes a START PREPARATION: no Chain, Cycle, Step or Snapshot is created and no work-file content is archived. It returns recommendations (general/secret candidates preselected, project-specific ones unselected; .gitignore is reference only), a default .gilignore text and the Monitor scope. Next: discuss the scope with the user, call gil_start_prepare with the exact rules text, then show_gil_monitor(scope_id) and wait for the user's Monitor approval. Chat approval, editing .gilignore or any agent input is never approval, and no model tool turns the preparation into a start. Later scope changes go through the same Monitor approval. Limits: reports cannot hold image attachments, and restore/revisit of work files is refused in such a project.\n\
- start_kind \"image_reports\": reports can hold PNG visuals (declare, cite, read in the Monitor). The project is created at once with one Interview and zero Steps, observing the whole folder with GIL's fixed exclusions; there is no tracked-scope approval and no scope change. Requires verified private local storage and refuses elsewhere; it never falls back to the other kind. Success also prepares the Monitor: the receipt names the exact show_gil_monitor({scope_id}) call, which is the only next call before the first gil_open.\n\
Both kinds keep Chains, approved plans, requirement-linked Experiments, original report and evidence reads and the single Will end history.\n\
Without start_kind: an open preparation in the folder is continued; an existing project or other .gil record is only reported (read gil_context); in a folder with no .gil the call refuses with code start_kind_required, lists both options and creates nothing. A .gil that an interrupted start left empty holds no record: tracked_scope may be started there again, while image_reports never creates into an existing .gil (the user removes that empty folder themself if they want that kind). A choice the folder's existing record contradicts is refused and nothing is changed. Never repeat start to open a screen or after a display failure.";

const TRACKED_ONLY_NOTE: &str = " Optional start_kind \"tracked_scope\" names this same preparation path explicitly; omitting it keeps the same behavior.";

/// Adds the selection input where the preparation path exists. Builds without that path keep
/// the base `gil_start` schema byte for byte, so their published tool contract is unchanged and
/// the input is refused at the wire as an unknown field.
pub(super) fn catalog(mut tools: Vec<Tool>) -> Vec<Tool> {
    if !HAS_TRACKED { return tools; }
    let Some(start) = tools.iter_mut().find(|tool| tool.name == "gil_start") else { return tools; };
    let mut raw = serde_json::to_value(&*start).expect("tool");
    let offered: Vec<&str> = if HAS_IMAGES { vec![TRACKED, IMAGES] } else { vec![TRACKED] };
    raw["inputSchema"]["properties"][INPUT] = json!({
        "type": "string",
        "enum": offered,
        "description": "Which kind of NEW project to start, exactly as the user chose it. Never guessed by the agent. It does not change or convert an existing project or preparation."
    });
    // A preparation answer (phase, recommendations, proposal state) is not the direct start
    // receipt, so the base receipt schema would be untrue for this tool here.
    if let Some(fields) = raw.as_object_mut() { fields.remove("outputSchema"); }
    if HAS_IMAGES {
        raw["description"] = json!(BOTH);
    } else {
        let description = raw["description"].as_str().unwrap_or_default().to_owned();
        raw["description"] = json!(format!("{description}{TRACKED_ONLY_NOTE}"));
    }
    *start = serde_json::from_value(raw).expect("tool");
    tools
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY: Held = Held { record: false, preparation: false, state: false, bare: false };
    const PREPARING: Held = Held { record: true, preparation: true, state: false, bare: false };
    const TRACKED_PROJECT: Held = Held { record: true, preparation: true, state: true, bare: false };
    const OTHER_PROJECT: Held = Held { record: true, preparation: false, state: true, bare: false };
    const REMNANT: Held = Held { record: true, preparation: false, state: false, bare: false };
    /// A start that stopped before it recorded anything: `.gil` is empty or holds only the lock file.
    const INTERRUPTED: Held = Held { record: true, preparation: false, state: false, bare: true };

    fn code(route: Route) -> &'static str {
        match route { Route::Refused { code, .. } => code, Route::Preparation => "preparation", Route::Direct => "direct" }
    }

    #[test]
    fn builds_with_one_start_keep_their_route_when_the_input_is_omitted() {
        for held in [None, Some(EMPTY), Some(PREPARING), Some(TRACKED_PROJECT), Some(OTHER_PROJECT), Some(REMNANT), Some(INTERRUPTED)] {
            assert_eq!(decide(None, held, false, false), Route::Direct);
            assert_eq!(decide(None, held, false, true), Route::Direct);
            assert_eq!(decide(None, held, true, false), Route::Preparation);
            assert_eq!(decide(Some(TRACKED), held, true, false), Route::Preparation);
        }
    }

    #[test]
    fn a_kind_the_build_does_not_hold_is_refused_and_never_replaced_by_the_other() {
        for held in [None, Some(EMPTY), Some(OTHER_PROJECT)] {
            assert_eq!(code(decide(Some(IMAGES), held, true, false)), "start_kind_unavailable");
            assert_eq!(code(decide(Some(TRACKED), held, false, true)), "start_kind_unavailable");
            assert_eq!(code(decide(Some(TRACKED), held, false, false)), "start_kind_unavailable");
            assert_eq!(code(decide(Some(IMAGES), held, false, false)), "start_kind_unavailable");
            for tracked in [false, true] { for images in [false, true] {
                assert_eq!(code(decide(Some("format_10"), held, tracked, images)), "start_kind_unknown");
                assert_eq!(code(decide(Some(""), held, tracked, images)), "start_kind_unknown");
            } }
        }
    }

    #[test]
    fn with_both_kinds_an_empty_folder_needs_the_users_choice_and_each_choice_keeps_its_route() {
        assert_eq!(code(decide(None, Some(EMPTY), true, true)), "start_kind_required");
        assert_eq!(decide(Some(TRACKED), Some(EMPTY), true, true), Route::Preparation);
        assert_eq!(decide(Some(IMAGES), Some(EMPTY), true, true), Route::Direct);
    }

    #[test]
    fn with_both_kinds_an_existing_record_is_continued_or_reported_and_never_switched() {
        // A preparation, or the tracked project it became: only the preparation path answers it.
        for held in [PREPARING, TRACKED_PROJECT] {
            assert_eq!(decide(None, Some(held), true, true), Route::Preparation);
            assert_eq!(decide(Some(TRACKED), Some(held), true, true), Route::Preparation);
            assert_eq!(code(decide(Some(IMAGES), Some(held), true, true)), "start_kind_conflict");
        }
        // A started project without a preparation: the creation entry only inspects it; the
        // preparation path never takes its lock, recovers or reaps in it.
        assert_eq!(decide(None, Some(OTHER_PROJECT), true, true), Route::Direct);
        assert_eq!(decide(Some(IMAGES), Some(OTHER_PROJECT), true, true), Route::Direct);
        assert_eq!(code(decide(Some(TRACKED), Some(OTHER_PROJECT), true, true)), "already_started");
        // Leftovers of an unknown interrupted creation are preserved by both routes.
        assert_eq!(decide(None, Some(REMNANT), true, true), Route::Direct);
        assert_eq!(decide(Some(IMAGES), Some(REMNANT), true, true), Route::Direct);
        assert_eq!(code(decide(Some(TRACKED), Some(REMNANT), true, true)), "start_record_unsettled");
    }

    #[test]
    fn with_both_kinds_a_start_that_recorded_nothing_is_retried_by_the_tracked_scope_start_only() {
        // The interrupted bootstrap has an ordinary retry; the preparation rechecks under the lock.
        assert_eq!(decide(Some(TRACKED), Some(INTERRUPTED), true, true), Route::Preparation);
        // The image-report creation never creates into an existing `.gil`: its entry inspects and refuses.
        assert_eq!(decide(Some(IMAGES), Some(INTERRUPTED), true, true), Route::Direct);
        assert_eq!(decide(None, Some(INTERRUPTED), true, true), Route::Direct);
        // The preparation tools: an empty `.gil` is answered by their own "nothing is being prepared".
        let root = std::env::temp_dir().join(format!("gil-start-kind-bare-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(root.join(".gil")).unwrap();
        let input = json!({"project_root": root});
        if HAS_TRACKED && HAS_IMAGES {
            assert_eq!(other_record("gil_start_prepare", &input), None);
            std::fs::create_dir(root.join(".gil/artifacts")).unwrap();
            assert_eq!(other_record("gil_start_prepare", &input), Some(("start_record_unsettled", "start_kind.unsettled")));
            assert_eq!(other_record("gil_start_cancel", &input), Some(("start_record_unsettled", "start_kind.unsettled")));
        } else {
            assert_eq!(other_record("gil_start_prepare", &input), None, "a build with one kind is unchanged");
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_call_without_an_absolute_root_is_left_to_the_tools_own_root_refusal() {
        assert_eq!(decide(None, None, true, true), Route::Direct);
        assert_eq!(decide(Some(TRACKED), None, true, true), Route::Preparation);
        assert_eq!(decide(Some(IMAGES), None, true, true), Route::Direct);
        assert_eq!(route(&json!({"project_root": "relative/place", INPUT: 7})), UNKNOWN);
    }

    #[test]
    fn the_folder_reading_follows_no_link_and_creates_nothing() {
        let root = std::env::temp_dir().join(format!("gil-start-kind-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&root).unwrap();
        assert_eq!(held(&root), EMPTY);
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0, "reading created an entry");
        std::fs::create_dir(root.join(".gil")).unwrap();
        assert_eq!(held(&root), INTERRUPTED, "an empty `.gil` holds no record");
        std::fs::write(root.join(".gil").join(crate::lock::LOCK_FILE), b"").unwrap();
        assert_eq!(held(&root), INTERRUPTED, "the lock file alone is not a record");
        std::fs::create_dir(root.join(".gil/artifacts")).unwrap();
        assert_eq!(held(&root), REMNANT, "anything else is a leftover to preserve");
        std::fs::write(root.join(".gil/state.yaml"), b"x").unwrap();
        assert_eq!(held(&root), OTHER_PROJECT);
        std::fs::create_dir(root.join(".gil").join(PREPARATION_DIR)).unwrap();
        assert_eq!(held(&root), TRACKED_PROJECT);
        std::fs::remove_dir_all(&root).unwrap();
        // A `.gil` that is a file (or a dangling link) is still a record to preserve.
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join(".gil"), b"not a folder").unwrap();
        assert!(held(&root).record && !held(&root).bare);
        // A link named `.gil` is never followed into the folder it points at.
        std::fs::remove_file(root.join(".gil")).unwrap();
        std::fs::create_dir(root.join("elsewhere")).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("elsewhere"), root.join(".gil")).unwrap();
            assert!(held(&root).record && !held(&root).bare, "a linked `.gil` is not an empty start");
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_schema_offers_the_input_only_where_the_preparation_path_exists() {
        let start = super::super::surface::tools().iter().find(|tool| tool.name == "gil_start").expect("start");
        let raw = serde_json::to_value(start).unwrap();
        let property = &raw["inputSchema"]["properties"][INPUT];
        if HAS_TRACKED && HAS_IMAGES {
            assert_eq!(property["enum"], json!([TRACKED, IMAGES]));
            assert!(raw.get("outputSchema").is_none());
            let description = raw["description"].as_str().unwrap();
            for word in [TRACKED, IMAGES, "start_kind_required", "never converted"] { assert!(description.contains(word), "{word}"); }
            // The user-facing text names features, not storage numbers or build flags.
            for word in ["format 1", "format 9", "tracking-policy", "visual-authoring"] { assert!(!description.contains(word), "{word}"); }
        } else if HAS_TRACKED {
            assert_eq!(property["enum"], json!([TRACKED]));
            assert!(raw.get("outputSchema").is_none());
        } else {
            assert!(property.is_null(), "a build with one direct start keeps the base schema");
            assert!(raw.get("outputSchema").is_some());
        }
        assert_eq!(raw["inputSchema"]["required"], json!(["project_root"]));
    }
}
