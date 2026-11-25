//! First-Experiment controller for a disposable Linux benchmark container.
use crate::{ActionContract, CycleKind, NodeKind, Project, ProjectSession, Report, RuleSet};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    error::Error,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const MAX_TURNS: usize = 64;
const MAX_REPLY: u64 = 1024 * 1024;

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
enum Reply {
    Open { kind: NodeKind, contract: String },
    Close { report: String },
}

fn fields(report: &Report) -> BTreeMap<&str, &str> {
    report
        .field_names()
        .map(|name| (name, report.get(name).unwrap()))
        .collect()
}

fn history(project: &Project) -> Vec<Value> {
    project.cycles().nodes().iter().map(|cycle| {
        let steps: Vec<_> = cycle.steps().nodes().iter().filter(|n| !n.kind.is_boundary()).map(|n| {
            json!({"ref":cycle.step_ref(n.id).to_string(), "kind":n.kind.as_str(), "report":n.report.as_ref().map(fields)})
        }).collect();
        json!({"ref":cycle.id().to_ref().to_string(), "kind":cycle.kind().to_string(),
               "steps":steps, "report":cycle.report().map(fields)})
    }).collect()
}

struct Adapter {
    executable: PathBuf,
    records: PathBuf,
    workspace: PathBuf,
    task: String,
}

impl Adapter {
    fn ask(&self, project: &Project, next: Value, previous_error: Option<&str>) -> Result<Reply> {
        let request = json!({"protocol":1, "task":self.task, "authority":"benchmark_adapter",
            "history":history(project), "next":next, "previous_error":previous_error});
        let input = self.records.join("request.json");
        serde_json::to_writer(File::create(&input)?, &request)?;
        // A regular input file prevents a writer/reader deadlock with a child that writes first.
        let mut child = Command::new("/usr/bin/timeout")
            .args(["--kill-after=5s", "120s"])
            .arg(&self.executable)
            .current_dir(&self.workspace)
            .stdin(File::open(input)?)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let mut bytes = Vec::new();
        child
            .stdout
            .take()
            .unwrap()
            .take(MAX_REPLY + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_REPLY {
            let _ = child.kill();
            let _ = child.wait();
            return Err("adapter reply exceeds 1 MiB".into());
        }
        let status = child.wait()?;
        if !status.success() {
            return Err(format!("adapter exited with {status}").into());
        }
        Ok(serde_json::from_slice(&bytes)?)
    }
}

fn next_action(project: &Project) -> Value {
    let cycle = project.current_cycle();
    let (contract, target, kind) = if let Some(id) = cycle.step_now_open() {
        let kind = cycle.steps().node(id).unwrap().kind;
        (
            cycle.step_contract(id, kind).unwrap(),
            cycle.step_ref(id).to_string(),
            Some(kind),
        )
    } else if cycle.can_close() {
        (
            cycle.close_contract().unwrap(),
            cycle.id().to_ref().to_string(),
            None,
        )
    } else {
        let allowed = project.openable_here();
        let previous = cycle
            .steps()
            .current()
            .and_then(|id| cycle.steps().node(id))
            .map(|node| node.kind);
        let preferred = match previous {
            Some(NodeKind::Interpretation) => NodeKind::Synthesis,
            Some(NodeKind::Analysis) => NodeKind::Outcome,
            _ => allowed.first().copied().unwrap_or(NodeKind::CycleExit),
        };
        let recommended = allowed
            .iter()
            .find(|kind| **kind == preferred)
            .or_else(|| allowed.first());
        return json!({"action":"open", "cycle":cycle.kind().to_string(),
            "recommended":recommended.map(|k| k.as_str()), "allowed":allowed.iter().map(|k| k.as_str()).collect::<Vec<_>>(),
            "contract_fields":["objective","next_action","done_when"]});
    };
    let mut required = contract.field_names();
    if kind == Some(NodeKind::Verify) && !required.contains(&"evidence_files") {
        required.push("evidence_files");
    }
    let will = project.active_will().map(|w| json!({"objective":w.objective(),"next_action":w.next_action(),"done_when":w.done_when()}));
    json!({"action":"close", "target":target, "kind":kind.map(|k| k.as_str()).unwrap_or("cycle"),
        "cycle":cycle.kind().to_string(), "contract":will, "fields":required, "constraints":contract.constraints(""),
        "workspace_writes":kind == Some(NodeKind::Verify)})
}

fn apply(session: &mut ProjectSession, reply: Reply) -> Result<()> {
    let cycle = session.project().current_cycle();
    match reply {
        Reply::Open { kind, contract } if cycle.step_now_open().is_none() && !cycle.can_close() => {
            let contract = ActionContract::from_report(&Report::parse(&contract)?)?;
            session.open_action_step(kind, contract)?;
        }
        Reply::Close { report } if cycle.step_now_open().is_some() => {
            let mut report = Report::parse(&report)?;
            report.insert("benchmark_source", "task_adapter");
            session.close_step(report)?;
        }
        Reply::Close { report } if cycle.can_close() => {
            session.close_cycle(Report::parse(&report)?)?;
        }
        _ => return Err("action does not match the current Core position".into()),
    }
    Ok(())
}

/// Runs one task with a host-supplied adapter inside the benchmark container. The Core owns
/// transitions and evidence. No Monitor or desktop project is opened.
fn run(task: &Path, executable: &Path, workspace: &Path, records: &Path) -> Result<Value> {
    if !cfg!(target_os = "linux") {
        return Err("the benchmark worker runs inside a Linux container".into());
    }
    let workspace = workspace.canonicalize()?;
    let records = records
        .parent()
        .ok_or("missing records parent")?
        .canonicalize()?
        .join(
            records
                .file_name()
                .ok_or("missing records directory name")?,
        );
    if records.starts_with(&workspace)
        || workspace
            .ancestors()
            .any(|p| p.join(".gil").symlink_metadata().is_ok())
    {
        return Err(
            "use a separate record directory and a workspace outside any GIL project".into(),
        );
    }
    let adapter = Adapter {
        executable: executable.canonicalize()?,
        records: records.clone(),
        workspace: workspace.clone(),
        task: fs::read_to_string(task)?,
    };
    if adapter.task.trim().is_empty() {
        return Err("empty benchmark task".into());
    }
    fs::create_dir(&records)?;
    let mut session = ProjectSession::start_at(
        RuleSet::benchmark(),
        &workspace,
        records.join(".gil").join("benchmark.yaml"),
    )?;
    session.commit()?;
    let mut previous_error = None;
    let mut turns = 0;
    loop {
        let cycle = session.project().current_cycle();
        if cycle.is_closed() {
            if cycle.kind() == CycleKind::Experiment {
                let report = cycle.report().ok_or("closed Experiment has no Report")?;
                return Ok(
                    json!({"authority":"benchmark_adapter", "verdict":report.get("verdict"),
                    "cycle":cycle.id().to_ref().to_string(), "report":fields(report), "history":history(session.project())}),
                );
            }
            session.open_child_cycle(CycleKind::Experiment)?;
            session.commit()?;
            continue;
        }
        if turns == MAX_TURNS {
            break;
        }
        turns += 1;
        let next = next_action(session.project());
        if next["action"] == "open" && next["recommended"].is_null() {
            return Err("the Core has no forward step for this benchmark run".into());
        }
        let verify_open = next["workspace_writes"] == true;
        let reply = adapter.ask(session.project(), next, previous_error.as_deref())?;
        if !verify_open && !matches!(session.world_state()?, crate::WorldState::Clean { .. }) {
            return Err("workspace changed outside an open Verify".into());
        }
        match apply(&mut session, reply) {
            Ok(()) => {
                session.commit()?;
                previous_error = None;
            }
            Err(err) => {
                previous_error = Some(err.to_string());
            }
        }
    }
    Err(format!(
        "turn limit reached; last refusal: {}",
        previous_error.as_deref().unwrap_or("none")
    )
    .into())
}

/// Only the explicit CLI subcommand enters this path. Platform detection never selects a mode.
pub fn cli(args: &[String]) -> std::process::ExitCode {
    use std::process::ExitCode;
    let usage = "Usage: gil benchmark IMAGE TASK_PATH_IN_IMAGE ADAPTER_PATH_IN_IMAGE";
    if matches!(args, [arg] if arg == "--help" || arg == "-h") {
        println!("{usage}");
        return ExitCode::SUCCESS;
    }
    if let [flag, task, adapter, workspace, records] = args
        && flag == "--in-container"
    {
        match run(
            task.as_ref(),
            adapter.as_ref(),
            workspace.as_ref(),
            records.as_ref(),
        ) {
            Ok(summary) => {
                println!("{summary}");
                return if summary["verdict"] == "success" {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(2)
                };
            }
            Err(err) => {
                eprintln!("Benchmark stopped: {err}");
                return ExitCode::FAILURE;
            }
        }
    }
    let [image, task, adapter] = args else {
        eprintln!("{usage}");
        return ExitCode::FAILURE;
    };
    match Command::new("docker")
        .args([
            "run",
            "--rm",
            "--network",
            "none",
            "--read-only",
            "--cap-drop",
            "ALL",
            "--security-opt",
            "no-new-privileges",
            "--user",
            "65534:65534",
            "--pids-limit",
            "128",
            "--memory",
            "1g",
            "--cpus",
            "2",
            "--ulimit",
            "fsize=268435456:268435456",
            "--tmpfs",
            "/work:rw,nosuid,nodev,size=512m,mode=1777",
            "--tmpfs",
            "/tmp:rw,nosuid,nodev,size=128m,mode=1777",
            "--entrypoint",
            "/usr/local/bin/gil",
            "--",
            image,
            "benchmark",
            "--in-container",
            task,
            adapter,
            "/work",
            "/tmp/gil-run",
        ])
        .status()
    {
        Ok(status) => ExitCode::from(
            status
                .code()
                .and_then(|n| u8::try_from(n).ok())
                .unwrap_or(1),
        ),
        Err(err) => {
            eprintln!("Could not start benchmark container: {err}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_explicit_benchmark_rules_accept_adapter_decisions() {
        let interactive = RuleSet::from_yaml_str(crate::BUILTIN_SPEC).unwrap();
        let benchmark = RuleSet::benchmark();
        for (decision, human) in [
            ("yes", true),
            ("no", true),
            ("benchmark_accept", false),
            ("benchmark_revise", false),
        ] {
            let report = Report::new()
                .with("statement", "Scope")
                .with("basis_refs", "step:C1/S1")
                .with("approved", decision);
            let check = |rules: &RuleSet| {
                rules
                    .validate_close(CycleKind::Interview, NodeKind::Synthesis, &report)
                    .is_ok()
            };
            assert_eq!(check(&interactive), human, "{decision}");
            assert_eq!(check(&benchmark), !human, "{decision}");
        }
    }
}
