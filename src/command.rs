//! `gil` — 걷기를 부를 수 있게 하는 가장 얇은 표면.
//!
//! 여기에는 규칙도 판정도 없다. 하는 일은 셋뿐이다: 저장된 것을 세우고, 라이브러리에
//! 한 수를 넘기고, 결과를 사람의 말로 적는다. 거절의 이유도 라이브러리의 말을 그대로 옮긴다 —
//! 여기서 다시 쓰면 같은 판정이 두 자리에서 서로 다르게 말하게 된다.

use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};

use crate::{
    ActionContract, CloseContract, CycleKind, NodeKind, ProjectSession, Refusal, Report, RuleSet,
    SessionError, StoreError, context, render_monitor_html,
    render_monitor_text, story, with_help,
};
#[cfg(not(feature = "product-safety"))]
use crate::Usage;
use crate::locale::text;
use crate::tr;

// Shared command execution. CLI owns ambient cwd/stdin; MCP supplies both explicitly.
// Never chdir or spawn this binary to execute a domain action.
enum Root<'a> { Current, Exact(&'a Path), Outside }
enum Input<'a> { Stdin, Body(&'a str) }
struct Invocation<'a> { root: Root<'a>, input: Input<'a> }
impl Invocation<'_> {
    fn said_path(&self, path: &Path) -> String {
        match self.root {
            Root::Current => crate::said_path(path),
            Root::Exact(root) => crate::store::said_path_from(root, path),
            Root::Outside => crate::STATE_PATH.to_owned(),
        }
    }
    fn directory(&self) -> Result<PathBuf, String> {
        match self.root {
            Root::Current => std::env::current_dir().map_err(|err| tr!("cmd.cwd_unknown", err = err)),
            Root::Exact(path) => Ok(path.to_path_buf()),
            Root::Outside => Ok(PathBuf::new()),
        }
    }
}
impl Input<'_> {
    fn is_terminal(&self) -> bool { matches!(self, Self::Stdin) && io::stdin().is_terminal() }
    fn text(&self) -> Result<String, String> {
        match self {
            Self::Body(body) => Ok((*body).to_string()),
            Self::Stdin => {
                let mut text = String::new();
                io::stdin().read_to_string(&mut text).map_err(|err| tr!("cmd.stdin_failed", err = err))?;
                Ok(text)
            }
        }
    }
}

/// CLI entry preserves ancestor discovery and lazy terminal input.
pub fn cli(args: &[String]) -> Result<String, String> {
    run(&Invocation { root: Root::Current, input: Input::Stdin }, args)
}

/// Only the typed MCP action adapter can call this; no monitor/transport escape hatch.
pub(crate) fn action(args: &[String], root: Option<&Path>, body: &str) -> Result<String, String> {
    if !matches!(args.first().map(String::as_str), Some("status" | "context" | "story" | "help" | "start" | "open" | "close" | "restore" | "revisit" | "cycle")) {
        return Err(text("action.disallowed").into());
    }
    if root.is_some_and(|p| !p.is_absolute()) || (root.is_none() && args[0] != "help") {
        return Err(crate::RootError::NotAbsolute.to_string());
    }
    run(&Invocation { root: root.map(Root::Exact).unwrap_or(Root::Outside), input: Input::Body(body) }, args)
}

fn run(ctx: &Invocation<'_>, args: &[String]) -> Result<String, String> {

    // **Agent Core handshake 가 먼저다.** Plugin 이 이 binary 를 믿어도 되는지 묻는 문이며,
    // Project 를 열지도 잠그지도 않는다. 평범한 명령은 이 문을 지나지 않는다.
    if let Some(said) = crate::agent::answer_cli(&args) {
        return said;
    }

    let command = args.first().map(String::as_str).unwrap_or("--help");

    match command {
        // 두 도움말은 **잠금 계약이 다르다.** 정확 조회는 프로젝트를 찾지 않고, 상태 기반
        // 조회는 상태를 읽어야 하므로 다른 상태 명령과 같은 안전 경계를 지난다.
        "help" if args.len() > 2 => Err(refusal(
            &tr!("cmd.help_many.what", n = args.len() - 1),
            text("cmd.help_many.why"),
            text("cmd.help_many.todo"),
            text("cmd.help_many.run"),
        )),
        "help" if args.get(1).is_some_and(|topic| topic == "close") => close_help(ctx),
        "help" if args.get(1).is_some() => help_topic(args.get(1).expect("방금 봤다")),
        "help" => help_here(ctx),
        "--help" | "-h" => Ok(help()),
        "--version" | "-V" | "version" => Ok(format!("gil {}\n", crate::CORE_VERSION)),
        "start" => start(ctx),
        // **`--help` 는 요청이 아니다.** stdin 없는 실제 Open·Close 로 읽으면, 계약을
        // 물어본 사람이 빈 Report 를 낸 것으로 거절당한다(실사용 M2C 보고).
        "open" if asks_for_help(args.get(1)) => open_help(ctx),
        "close" if asks_for_help(args.get(1)) => close_help(ctx),
        "open" => open(ctx, args.get(1)),
        "close" => close(ctx),
        "restore" => restore(ctx, args.get(1)),
        "revisit" => revisit(ctx, args.get(1)),
        "status" => status(ctx),
        // 이름 있는 변수로 받는다 — 임시 값으로 두면 잠금이 언제 떨어지는지가
        // 식(式)의 모양에 달리게 된다.
        "story" => {
            let session = open_session(ctx)?;
            let project = session.project();
            if project.chains().is_empty() {
                Ok(story(project.cycles()))
            } else {
                Ok(project.chains().iter().map(|chain|
                    format!("{} · {}\n{}", chain.id(), if chain.is_closed() { "closed" } else { "open" }, story(chain.cycles()))
                ).collect::<Vec<_>>().join("\n"))
            }
        }
        "context" => {
            let session = open_session(ctx)?;
            Ok(format!("{}{}", context(session.project()), crate::project_resume::status(&session.root().map_err(say)?)))
        }
        "monitor" => monitor(ctx, args.get(1)),
        "mcp" => mcp(args.get(1).map(String::as_str)),
        "cycle" => cycle(ctx, args.get(1).map(String::as_str), args.get(2)),
        other => Err(tr!("cmd.unknown_command", other = format!("{other:?}"), help = help())),
    }
}

/// `gil help <주소>` — **함께 실린 Topic 하나를 편다.**
///
/// 정확한 canonical 주소나 유효한 alias 하나만 받는다. 가까운 후보를 추측해 주지 않는다 —
/// 추측한 주소를 읽으면 지금 자리와 무관한 규칙을 배우게 된다.
fn help_topic(input: &str) -> Result<String, String> {
    crate::help_topic(input).map_err(|err| err.to_string())
}

/// `gil help` — **지금 상태에 관련된 주제만.**
///
/// 프로젝트를 찾으면 다른 상태 명령과 같은 길을 지난다: 잠금 → 복구 → 상태 읽기.
/// 못 찾으면 `.gil` 도 잠금 파일도 만들지 않고 Bootstrap 만 말한다.
fn help_here(ctx: &Invocation<'_>) -> Result<String, String> {
    match session_for_help(ctx)? {
        None => crate::help_outside().map_err(|err| err.to_string()),
        Some(session) => crate::help_here(&session).map_err(|err| err.to_string()),
    }
}

/// 도움말을 물은 것인가 — 여는·닫는 요청이 아니라.
fn asks_for_help(argument: Option<&String>) -> bool {
    matches!(
        argument.map(String::as_str),
        Some("--help" | "-h" | "help")
    )
}

/// 지금 이어 걷고 있는 것 — **그리고 그것을 쥔 잠금.**
///
/// [`ProjectSession`] 은 손에 넣는 순간 이 프로젝트를 잠근 것이고, 떨어지는 순간 푼 것이다.
/// 그래서 「잠그는 것을 잊었다」는 상태가 이 파일 어디에도 존재하지 않는다.
///
/// **명령 하나에 하나만 만든다.** 안쪽 함수가 다시 만들면 제가 쥔 잠금에 제가 걸린다.
type Session = ProjectSession;

/// 저장된 것이 어디에 있는가 — 그리고 그것이 이 gil 이 읽는 형식인가.
enum Found {
    State(PathBuf),
    /// 앞 형식의 파일. 읽지 않지만 **있다는 사실은 말한다.**
    Legacy(PathBuf),
}

/// 여기서 위로 거슬러 오르며 **가장 가까운** 저장을 찾는다.
///
/// Agent 는 소스·시험·하위 패키지 폴더를 계속 오간다. 서 있는 자리에서만 찾으면 명령마다
/// 뿌리로 돌아가야 하고, 잊으면 **같은 프로젝트에 두 번째 기록이 조용히 생긴다**
/// (실사용 보고 #125).
fn find_gil(ctx: &Invocation<'_>) -> Result<Option<Found>, String> {
    let here = ctx.directory()?;
    if matches!(ctx.root, Root::Outside) { return Ok(None); }
    for dir in here.ancestors().take(if matches!(ctx.root, Root::Exact(_)) { 1 } else { usize::MAX }) {
        let state = dir.join(crate::STATE_PATH);
        if state.exists() {
            return Ok(Some(Found::State(state)));
        }
        // 같은 `.gil` 안에 앞 형식만 있는 경우다. 위로 더 올라가기 전에 여기서 멈춘다 —
        // 조용히 지나치면 사람은 제 기록이 사라진 줄 안다.
        let legacy = dir.join(crate::LEGACY_WALK_PATH);
        if legacy.exists() {
            return Ok(Some(Found::Legacy(legacy)));
        }
    }
    Ok(None)
}

/// 여기에 새로 눕힐 자리. **언제나 온전한 경로다** — 반쪽 경로는 서 있는 자리에 따라
/// 다른 것을 가리킨다.
fn state_path_here(ctx: &Invocation<'_>) -> Result<PathBuf, String> {
    let here = ctx.directory()?;
    Ok(here.join(crate::STATE_PATH))
}

/// 이 기록이 서 있는 자리가 아닌 곳에 누워 있는가.
fn found_above(ctx: &Invocation<'_>, path: &Path) -> bool {
    match state_path_here(ctx) {
        Ok(here) => path != here,
        Err(_) => false,
    }
}

fn rules() -> Result<RuleSet, String> {
    RuleSet::builtin().map_err(|err| tr!("cmd.spec_unreadable", err = err))
}

/// 프로젝트를 찾아 **잠그고** 세운다 — 상태를 읽는 모든 명령의 유일한 입구.
///
/// 잠금은 읽기보다 **먼저**다. 읽고 나서 잠그면 그 사이에 다른 명령이 끝나 버려, 이미
/// 낡은 상태를 쥔 채 잠금을 얻는다 — 그것은 잠그지 않은 것과 같다.
fn open_session(ctx: &Invocation<'_>) -> Result<Session, String> {
    if let Root::Exact(root) = ctx.root {
        return crate::open_project_root(rules()?, root).map_err(|err| err.to_string());
    }
    let path = match find_gil(ctx)? {
        Some(Found::State(path)) => path,
        Some(Found::Legacy(path)) => {
            return Err(StoreError::LegacyFormat {
                    path: ctx.said_path(&path),
            }
            .to_string());
        }
        None => {
            return Err(tr!("cmd.no_walk_found", path = crate::STATE_PATH));
        }
    };

    ProjectSession::open(rules()?, &path).map_err(|err| match err {
        // 방금 있는 것을 보고 왔다. 그새 사라졌다면 그건 다른 이야기다.
        SessionError::Store(StoreError::NotFound { .. }) => {
            tr!("cmd.state_vanished", path = ctx.said_path(&path))
        }
        other => say(other),
    })
}

/// 지금 상태를 눕힌다 — **잠금을 쥔 채로.**
fn write(session: &Session) -> Result<(), String> {
    session.commit().map_err(say)
}

/// 세계 쪽 거절을 사람의 글로 — **복구할 Topic 이 있으면 그 주소 한 줄과 함께.**
///
/// 오류가 문자열이 되는 자리는 여기 하나로 모은다. 그러지 않으면 어떤 길로 나온 오류는
/// Topic 을 달고 어떤 길로 나온 것은 안 다는 일이 생기고, 그것은 사람이 예측할 수 없다.
fn say(err: SessionError) -> String {
    let said = err.to_string();
    with_help(&Refusal::Session(&err), said)
}

// ── 명령 ───────────────────────────────────────────────────────────────────

fn start(ctx: &Invocation<'_>) -> Result<String, String> {
    // 위에 이미 기록이 있으면 여기서 새로 시작하지 않는다 — 그러면 한 프로젝트에
    // 기록이 둘이 되고, 어느 쪽에 적히는지는 서 있는 자리가 정하게 된다.
    match find_gil(ctx)? {
        Some(Found::State(existing)) => {
            return Err(refusal(
                text("cmd.start_exists.what"),
                &tr!("cmd.start_exists.why", path = ctx.said_path(&existing)),
                text("cmd.start_exists.todo"),
                "gil status",
            ));
        }
        Some(Found::Legacy(path)) => {
            return Err(StoreError::LegacyFormat {
                path: ctx.said_path(&path),
            }
            .to_string());
        }
        None => {}
    }

    // **여기서부터 잠금 안이다.** `.gil/` 을 만들며 잠그고, 잠근 뒤에 다시 한 번
    // 무엇이 이미 있는지 본다 — 두 `gil start` 가 나란히 「비어 있다」를 읽지 못하게.
    let session = ProjectSession::start_guided(rules()?, &state_path_here(ctx)?).map_err(|err| match err {
        SessionError::AlreadyStarted { path } => refusal(
            text("cmd.start_exists.what"),
            &tr!("cmd.start_exists.why", path = path),
            text("cmd.start_exists.todo"),
            "gil status",
        ),
        other => other.to_string(),
    })?;
    write(&session)?;

    Ok(start_receipt(ctx, &session))
}

/// Development Agent entry: exact root, no migration, no recovery of creation remnants.
#[cfg(feature = "visual-authoring")]
pub(crate) fn start_visual_action(root: &Path) -> Result<String, String> {
    let ctx = Invocation { root: Root::Exact(root), input: Input::Body("") };
    #[cfg(windows)]
    let externalized = crate::project_state::externalize_inline_if_needed(root)?;
    #[cfg(windows)]
    {
        // An unfinished inline record is evidence, not an empty Project. Do not silently create a
        // second record in LOCALAPPDATA merely because state.yaml was never published.
        let external = crate::project_state::external_storage_root(root)?;
        let inline = root.join(".gil");
        if !external.exists() && inline.exists() && !root.join(crate::STATE_PATH).is_file() {
            return Err(text("cmd.visual.trace").into());
        }
    }
    #[cfg(windows)]
    let state = crate::project_state::state_for_root(root)?;
    #[cfg(not(windows))]
    let state = root.join(crate::STATE_PATH);
    let gil = state.parent().expect("state path has a parent").to_path_buf();
    match std::fs::symlink_metadata(&gil) {
        Ok(dir) => {
            // Do not create a lock just to inspect partial/unknown storage.
            let regular = |name| std::fs::symlink_metadata(gil.join(name))
                .is_ok_and(|m| m.is_file());
            if dir.is_dir() && regular("state.yaml") && regular("project.lock") {
                #[cfg(windows)]
                ProjectSession::open_read_only_at(rules()?, root, &state)
                    .map_err(|e| tr!("cmd.visual.existing_unverified", e = e))?;
                #[cfg(not(windows))]
                ProjectSession::open_read_only(rules()?, &state)
                    .map_err(|e| tr!("cmd.visual.existing_unverified", e = e))?;
                #[cfg(windows)]
                if externalized {
                    return Ok(text("cmd.visual.externalized").into());
                }
                return Err(text("cmd.visual.already_walking").into());
            }
            return Err(text("cmd.visual.trace").into());
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
        Err(e) => return Err(tr!("cmd.visual.record_unverified", e = e)),
    }
    #[cfg(windows)]
    let session = {
        let storage_root = crate::project_state::external_storage_root(root)?;
        std::fs::create_dir_all(&storage_root)
            .map_err(|e| tr!("cmd.visual.windows_store", e = e))?;
        ProjectSession::start_visual_guided(rules()?, root, &storage_root)
    };
    #[cfg(not(windows))]
    let session = ProjectSession::start_visual_guided(rules()?, root, root);
    let session = session
        .map_err(|e| tr!("cmd.visual.start_unknown", e = e))?;
    Ok(start_receipt(&ctx, &session))
}

/// Safe post-start inspection: no recovery, temporary-object reaping, or bootstrap lock.
#[cfg(feature = "visual-authoring")]
pub(crate) fn visual_read_action(root: &Path, command: &str) -> Result<String, String> {
    #[cfg(windows)]
    let state = crate::project_state::state_for_root(root)?;
    #[cfg(not(windows))]
    let state = root.join(crate::STATE_PATH);
    let gil = state.parent().expect("state path has a parent").to_path_buf();
    match std::fs::symlink_metadata(&gil) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(crate::RootError::NoProjectHere.to_string()),
        Ok(dir) if dir.is_dir() => {},
        _ => return Err(text("cmd.visual.read_unverified").into()),
    }
    for name in ["state.yaml", "project.lock"] {
        if !std::fs::symlink_metadata(gil.join(name)).is_ok_and(|m| m.is_file()) {
            return Err(text("cmd.visual.read_unverified").into());
        }
    }
    #[cfg(windows)]
    let session = ProjectSession::open_read_only_at(rules()?, root, &state)
        .map_err(|e| e.to_string())?;
    #[cfg(not(windows))]
    let session = ProjectSession::open_read_only(rules()?, &state)
        .map_err(|e| e.to_string())?;
    match command {
        "status" => Ok(session.status_said()),
        "context" => Ok(format!("{}{}", context(session.project()), crate::project_resume::status(root))),
        _ => Err(text("cmd.visual.read_disallowed").into()),
    }
}

fn start_receipt(ctx: &Invocation<'_>, session: &ProjectSession) -> String {
    let cycle = session.project().cycles().current();
    #[cfg(windows)]
    let record = match session.root() {
        Ok(root) if session.state_path() == root.join(crate::STATE_PATH) => ctx.said_path(session.state_path()),
        _ => text("cmd.windows_record").to_owned(),
    };
    #[cfg(not(windows))]
    let record = ctx.said_path(session.state_path());
    tr!("cmd.start_receipt",
        chain = session.project().current_chain().map(|chain| tr!("cmd.start_chain_line", id = chain.id(), entry = chain.entry_snapshot())).unwrap_or_default(),
        cycle = cycle.id().to_ref(),
        kind = cycle.kind(),
        record = record
    ) + &crate::project_resume::status(&session.root().expect("session root"))
}

/// 지금 이 자리가 무엇을 여는 자리인가.
///
/// **AI 가 Step 과 Cycle 경계를 따로 외우지 않게 한다**(Agent UX Model §2). 어느 계층을
/// 여는지는 세계의 상태가 이미 알고 있으므로, 그것을 사람에게 묻지 않는다.
enum Place {
    /// 지금 열려 있는 자리를 먼저 닫아야 한다.
    CloseThisStep,
    /// 이 Cycle 안에서 다음 Step 을 연다.
    Step(Vec<NodeKind>),
    /// Step Graph 가 끝 경계에 닿았다 — 이제 Cycle 을 닫는다.
    CloseThisCycle,
    CloseThisChain,
    /// 닫힌 Cycle 뒤에서 다음 Cycle 을 연다.
    Cycle(Vec<CycleKind>),
    /// 되돌아온 자리다 — 다음 Cycle 은 **갈래**로 열린다.
    Branch(Vec<CycleKind>),
    /// 여기서 열 수 있는 것이 없다.
    Nothing(String),
}

fn place(session: &Session) -> Place {
    if let Some(chain) = session.project().current_chain() {
        if chain.is_closed() { return Place::Nothing(text("cmd.chain_closed_open_chain").into()); }
        if chain.awaiting_report() { return Place::CloseThisChain; }
    }
    let cycles = session.project().cycles();
    let cycle = cycles.current();

    // **되돌아온 자리가 먼저다.** 그 자리도 닫힌 Cycle 이지만, 거기서 여는 것은 평범한
    // 자식이 아니라 갈래다 — 두 자리를 한 갈래로 묶으면 `revisit_from` 이 사라진다.
    if cycles.pending_revisit().is_some() {
        return Place::Branch(CycleKind::ALL.to_vec());
    }

    if cycle.is_closed() {
        return match cycles.why_not_open_child() {
            None => Place::Cycle(CycleKind::ALL.to_vec()),
            Some(why) => Place::Nothing(why.to_string()),
        };
    }

    let walk = cycle.steps();
    if walk
        .current()
        .and_then(|id| walk.node(id))
        .is_some_and(|node| !node.is_closed())
    {
        return Place::CloseThisStep;
    }

    let openable = cycle.openable_here();
    if !openable.is_empty() {
        return Place::Step(openable);
    }
    match cycle.can_close() {
        true => Place::CloseThisCycle,
        false => Place::Nothing(text("cmd.nowhere_in_cycle").to_string()),
    }
}

/// `gil open [종류]` — **어느 계층을 여는지는 GIL 이 판정한다.**
fn open(ctx: &Invocation<'_>, name: Option<&String>) -> Result<String, String> {
    open_with(ctx, open_session(ctx)?, name)
}

/// 이미 잠근 프로젝트 위에서 연다.
///
/// **다시 잠그지 않는다.** 같은 프로세스가 잠금을 두 번 잡으면 제 발에 걸려 「다른 명령이
/// 쓰고 있다」로 스스로를 거절한다. 그래서 잠금은 명령 경계에서 한 번 잡고, 안쪽 함수에는
/// 잡은 것을 **넘긴다**.
fn open_with(ctx: &Invocation<'_>, mut session: Session, name: Option<&String>) -> Result<String, String> {
    if name.is_some_and(|name| name == "chain") {
        let id = session.open_child_chain().map_err(|err| err.to_string())?;
        session.commit().map_err(|err| err.to_string())?;
        return Ok(tr!("cmd.opened_chain", lead = text("receipt.opened_lead"), id = id, cycle = session.project().current_cycle().id().to_ref()));
    }
    match place(&session) {
        Place::CloseThisStep => {
            let cycle = session.project().cycles().current();
            let here = here_ref(cycle).expect("열려 있는 자리가 있다");
            Err(refusal(
                text("cmd.not_open_place"),
                &tr!("cmd.still_open", here = here),
                text("cmd.close_it_first"),
                "gil close",
            ))
        }
        Place::CloseThisChain => Err(session.project().current_chain().unwrap().close_hint()),
        Place::CloseThisCycle => Err(refusal(
            text("cmd.not_open_place"),
            text("cmd.cycle_judged.why"),
            text("cmd.cycle_judged.todo"),
            "gil close",
        )),
        Place::Nothing(why) => Err(refusal(
            text("cmd.nothing_to_open"),
            &why,
            text("cmd.check_where"),
            "gil status",
        )),
        Place::Step(openable) => open_step(ctx, &mut session, name, &openable),
        Place::Cycle(openable) => open_cycle(&mut session, name, &openable, false),
        Place::Branch(openable) => open_cycle(&mut session, name, &openable, true),
    }
}

fn open_step(ctx: &Invocation<'_>,
    session: &mut Session,
    name: Option<&String>,
    openable: &[NodeKind],
) -> Result<String, String> {
    let kind = match name {
        Some(name) => NodeKind::parse(name).ok_or_else(|| {
            refusal(
                &tr!("cmd.unknown_kind", name = format!("{name:?}")),
                text("cmd.misspelled"),
                &tr!("cmd.now_openable", names = names(openable)),
                "gil open",
            )
        })?,
        // 갈 곳이 하나면 묻지 않는다 — 아는 것을 다시 묻는 것은 마찰이다.
        None => match openable {
            [only] => *only,
            many => {
                return Err(refusal(
                    text("cmd.choose_kind"),
                    &tr!("cmd.openable_count", n = many.len()),
                    &tr!("cmd.pick_kind", choices = choices(session.project().cycles().current(), many)),
                    text("cmd.run_open_kind"),
                ));
            }
        },
    };

    // 실행형 자리는 **무엇을 하려는지 적지 않으면 열지 않는다.** 그 계약은 Report 와 같은
    // field 문법으로 stdin 에 오지만 Close Report 가 아니다 — 행동 계약이다.
    let hint = contract_skeleton(kind);
    let contract = read_contract(ctx, &hint)?;

    let opened = session
        .open_action_step(kind, contract)
        // 거절의 이유는 라이브러리의 말을 **그대로** 옮긴다. 여기서 다시 쓰면 같은 판정이
        // 두 자리에서 서로 다르게 말하게 된다. 더하는 것은 복구로 가는 길뿐이다.
        .map_err(|err| {
            let said = refusal(
                &tr!("cmd.cannot_open", kind = kind),
                &err.to_string(),
                &tr!("cmd.now_openable", names = names(openable)),
                "gil open",
            );
            with_help(&Refusal::Session(&err), said)
        })?;
    write(&session)?;
    // The open transition is durable before any Artifact change, under the same Project lock.
    let setup = if kind == crate::NodeKind::Verify {
        crate::project_resume::ensure(&session.root().map_err(say)?)
    } else { String::new() };
    Ok(opened_step(session, opened) + &setup)
}

/// Cycle 하나를 연다 — **평범한 자식인지 갈래인지는 부르는 쪽이 이미 판정했다.**
///
/// 두 자리는 고르는 방식도 거절하는 말도 같다. 다른 것은 **무엇이 기록되는가** 하나뿐이라,
/// 그 차이를 문 하나 안에 두고 나머지를 나누지 않는다.
fn open_cycle(
    session: &mut Session,
    name: Option<&String>,
    openable: &[CycleKind],
    branch: bool,
) -> Result<String, String> {
    let Some(name) = name else {
        // 고를 것이 하나뿐이면 묻지 않는다. 설명은 **고를 때만** 필요하다.
        if let [only] = openable {
            return open_this_cycle(session, *only, branch);
        }
        return Err(refusal(
            text("cmd.choose_kind"),
            &tr!("cmd.openable_count", n = openable.len()),
            &tr!("cmd.pick_kind", choices = cycle_choices(session, openable)),
            text("cmd.run_open_kind"),
        ));
    };
    let kind = CycleKind::parse(name).ok_or_else(|| {
        // **Step 종류를 적은 것인지 먼저 본다.** 이름을 잘못 적은 것과 계층을 잘못 본 것은
        // 사람이 할 일이 다르다 — 후자는 지금 여는 것이 무엇인지부터 알아야 한다.
        let (why, todo) = match NodeKind::parse(name) {
            Some(step) => (
                tr!("cmd.step_not_cycle", step = step),
                tr!("cmd.open_cycle_first", names = cycle_names(openable)),
            ),
            None => (
                text("cmd.misspelled").to_string(),
                tr!("cmd.now_openable", names = cycle_names(openable)),
            ),
        };
        refusal(
            &tr!("cmd.cannot_open_with", name = format!("{name:?}")),
            &why,
            &todo,
            text("cmd.run_open_kind"),
        )
    })?;
    open_this_cycle(session, kind, branch)
}

fn open_this_cycle(
    session: &mut Session,
    kind: CycleKind,
    branch: bool,
) -> Result<String, String> {
    match branch {
        // **되돌아온 자리에서 여는 것은 갈래다.** 여기서 평범한 자식을 열면 어느 실패가
        // 이 갈래를 낳았는지가 사라지고, pending 도 소비되지 않은 채 남는다.
        true => session.open_branch_cycle(kind).map(|_| ()),
        false => session.open_child_cycle(kind).map(|_| ()),
    }
    .map_err(|err| {
        let said = refusal(
            &tr!("cmd.cannot_open_cycle", kind = kind),
            &err.to_string(),
            text("cmd.check_where"),
            "gil status",
        );
        with_help(&Refusal::Session(&err), said)
    })?;
    write(&session)?;
    Ok(opened_cycle(session))
}

/// 고를 Cycle Kind 들 — **설명은 문법이 갖는다.**
///
/// 이름과 그 한 줄을 여기 옮겨 적지 않는다. Kind 가 늘면 `gil-spec.yaml` 한 자리만 고치면
/// 화면도 함께 는다(Agent UX Model §4.2).
fn cycle_choices(session: &Session, kinds: &[CycleKind]) -> String {
    let rules = session.project().cycles().current().rules();
    let width = kinds
        .iter()
        .map(|kind| kind.as_str().len())
        .max()
        .unwrap_or(0);
    kinds
        .iter()
        .map(|kind| {
            let description = rules
                .cycle_rules(*kind)
                .map(|rules| rules.description.as_str())
                .unwrap_or_default();
            format!("{:<width$}  {description}", kind.as_str(), width = width)
        })
        .collect::<Vec<_>>()
        .join("\n  ")
}

/// `gil close` — **무엇을 닫는지는 GIL 이 판정한다.**
fn close(ctx: &Invocation<'_>) -> Result<String, String> {
    let mut session = open_session(ctx)?;
    match place(&session) {
        Place::CloseThisStep => close_step(ctx, &mut session),
        Place::CloseThisCycle => close_cycle(ctx, &mut session),
        Place::CloseThisChain => {
            let hint = session.project().current_chain().unwrap().close_hint();
            let report = read_report(ctx, &hint)?;
            let id = session.close_chain(report).map_err(|err| format!("{err}\n{hint}"))?;
            write(&session)?;
            Ok(tr!("cmd.closed_chain", lead = text("receipt.closed_lead"), id = id, verdict = session.project().current_chain().unwrap().report().unwrap().get("verdict").unwrap()))
        },
        Place::Step(openable) => Err(refusal(
            text("cmd.nothing_to_close"),
            text("cmd.no_open_place"),
            &tr!("cmd.open_next_first", names = names(&openable)),
            "gil open",
        )),
        Place::Cycle(_) => Err(refusal(
            text("cmd.nothing_to_close"),
            text("cmd.cycle_already_closed"),
            text("cmd.open_next_cycle"),
            text("cmd.run_open_kind"),
        )),
        Place::Branch(openable) => Err(refusal(
            text("cmd.nothing_to_close"),
            text("cmd.revisited_no_cycle"),
            text("cmd.open_cycle_below"),
            &format!("gil open {}", cycle_names(&openable)),
        )),
        Place::Nothing(why) => Err(refusal(
            text("cmd.nothing_to_close"),
            &why,
            text("cmd.check_where"),
            "gil status",
        )),
    }
}

fn typed_verify_hint(here: crate::StepRef) -> String {
    tr!("cmd.typed_verify_hint", here = here)
}

fn close_step(ctx: &Invocation<'_>, session: &mut Session) -> Result<String, String> {
    let cycle = session.project().cycles().current();
    let node = cycle
        .steps()
        .current()
        .and_then(|id| cycle.steps().node(id))
        .expect("열려 있는 자리가 있다");
    let kind = node.kind;
    let here = cycle.step_ref(node.id);
    let contract = cycle
        .step_contract(node.id, kind)
        .expect("열린 자리의 종류는 문법이 선언한 것이다");
    let mut hint = skeleton_for(&contract, cycle, Some(ctx));
    if kind == crate::NodeKind::Verify && session.project().visual_state.is_some() {
        hint.push_str(&typed_verify_hint(here));
    }

    let report = read_report(ctx, &hint)?;
    // **한 번에 확정된다** — Report·Closed·Will Done·Journey 판이 함께 눕거나, 아무것도
    // 눕지 않는다. 거절되면 열린 자리도 걸린 행동도 그대로다.
    let closed = session.close_step(report).map_err(|err| {
        // 세계 쪽 거절은 **이미 완결된 receipt** 다 — 무엇이·왜·다음에 무엇을 까지 스스로
        // 말한다. 그것을 다시 감싸면 이유 안에 또 이유가 들어간다.
        let said = match err.is_receipt() {
            true => err.to_string(),
            false => refusal(
                &tr!("cmd.cannot_close", here = here, kind = kind),
                &err.to_string(),
                &format!("{}\n\n{hint}", fix_it(ctx)),
                "gil close",
            ),
        };
        // **오류가 Manual 의 router 다.** 복구할 Topic 이 있으면 그 주소 한 줄만 더한다.
        with_help(&Refusal::Session(&err), said)
    })?;
    write(&session)?;
    Ok(closed_step(session, &closed))
}

fn close_cycle(ctx: &Invocation<'_>, session: &mut Session) -> Result<String, String> {
    let cycle = session.project().cycles().current();
    let here = cycle.id().to_ref();
    let kind = cycle.kind();
    let contract = cycle.close_contract().expect("문법이 이 Cycle Kind 를 선언했다");
    let hint = skeleton_for(&contract, cycle, Some(ctx));

    let report = read_report(ctx, &hint)?;
    let closed = session.close_cycle(report).map_err(|err| {
        let said = match err.is_receipt() {
            true => err.to_string(),
            false => refusal(
                &tr!("cmd.cannot_close", here = here, kind = kind),
                &err.to_string(),
                &format!("{}\n\n{hint}", fix_it(ctx)),
                "gil close",
            ),
        };
        with_help(&Refusal::Session(&err), said)
    })?;
    write(&session)?;
    Ok(closed_cycle(session, &closed))
}

/// `gil restore` — **명령 실행 자체가 복원 의사다.**
///
/// 인수를 받지 않는다. 목표는 지금 위치가 이미 정했고(Artifact Model §8.1), 파일을 골라
/// 되돌리는 것은 **부분 복원**이라 어느 Snapshot 에도 속하지 않는 세계를 만든다.
/// 확인 절차도 두지 않는다 — 안전성은 물어보는 데서 오지 않고 원자성에서 온다(§8.8).
#[cfg(feature = "product-safety")]
fn restore(_ctx: &Invocation<'_>, _extra: Option<&String>) -> Result<String, String> {
    Err(product_artifact_rollback_refusal("restore"))
}

#[cfg(not(feature = "product-safety"))]
fn restore(ctx: &Invocation<'_>, extra: Option<&String>) -> Result<String, String> {
    if let Some(extra) = extra {
        // Domain 명령이 서기 전의 거절도 복구할 Topic 이 있다 — 사용법 하나면 풀린다.
        return Err(with_help(
            &Refusal::Usage(Usage::RestoreTakesNoArgument),
            refusal(
                &tr!("cmd.restore_no_arg", extra = format!("{extra:?}")),
                text("cmd.restore_why"),
                text("cmd.run_without_args"),
                "gil restore",
            ),
        ));
    }

    let session = open_session(ctx)?;
    let done = session.restore().map_err(|err| {
        let said = match err.is_receipt() {
            true => err.to_string(),
            false => refusal(
                text("cmd.restore_failed"),
                &err.to_string(),
                text("cmd.check_where"),
                "gil status",
            ),
        };
        with_help(&Refusal::Session(&err), said)
    })?;
    // **저장하지 않는다.** 복원은 논리 상태를 바꾸지 않는다.
    Ok(restored(&session, &done))
}

/// 복원의 receipt — **개수와 목표, 그리고 그대로 남은 자리.**
///
/// 개별 파일 목록을 기본 출력에 늘어놓지 않는다. 되돌린 파일이 수백 개일 때 그 목록은
/// 다음 수를 가린다. raw manifest·blob 주소도 내보이지 않는다 — 공개 표면은 `snapshot:A*` 다.
#[cfg(not(feature = "product-safety"))]
fn restored(session: &Session, done: &crate::Restored) -> String {
    let mut out = match done.no_op {
        true => tr!("cmd.restore_noop", world = done.world),
        false => {
            let mut out = tr!("cmd.restored_to", world = done.world);
            for (label, count) in [
                (text("cmd.count.replaced"), done.replaced),
                (text("cmd.count.created"), done.created),
                (text("cmd.count.deleted"), done.deleted),
            ] {
                if count > 0 {
                    out.push_str(&tr!("cmd.count_line", label = label, count = count));
                }
            }
            out
        }
    };

    // **없는 것을 지어내지 않는다.** 열린 자리가 없는 합법적 경계면 Cycle 만 말한다.
    out.push_str(text("cmd.current_position"));
    let cycle = session.project().cycles().current();
    match here_ref(cycle) {
        Some(here) => {
            let kind = cycle
                .step_now_open()
                .and_then(|at| cycle.steps().node(at))
                .map(|node| node.kind.to_string())
                .unwrap_or_default();
            out.push_str(&format!("  {here} · {kind} · open\n"));
        }
        None => out.push_str(&format!("  {} · {}\n", cycle.id().to_ref(), cycle.kind())),
    }
    match session.project().active_will() {
        Some(will) => out.push_str(&tr!("cmd.will_kept", will = will.id())),
        None => {}
    }
    out
}

/// `gil revisit` — **어느 계층을 되돌아가는지는 GIL 이 판정한다.**
///
/// ```text
/// 열린 Cycle 안, 닫힌 Outcome 에 되돌아감이 적혀 있다  → Step 계층
/// 닫힌 Cycle 의 Cycle Report 에 되돌아감이 적혀 있다   → Cycle 계층
/// ```
///
/// **인수를 받지 않는다.** 어디로 갈지는 닫는 순간 Report 에 이미 확정됐고, 여기서 다시
/// 고르게 하면 근거와 이동이 갈린다 — Snapshot 을 고를 수 없는 것과 같은 까닭이다.
#[cfg(feature = "product-safety")]
fn revisit(ctx: &Invocation<'_>, extra: Option<&String>) -> Result<String, String> {
    if extra.is_some() { return Err(text("cmd.revisit_no_arg_short").into()); }
    let mut session = open_session(ctx)?;
    let result = session.resume_revisit()?;
    Ok(tr!("cmd.revisited_preserved", result = result))
}

#[cfg(not(feature = "product-safety"))]
fn revisit(ctx: &Invocation<'_>, extra: Option<&String>) -> Result<String, String> {
    if let Some(extra) = extra {
        return Err(with_help(
            &Refusal::Usage(Usage::RevisitTakesNoArgument),
            refusal(
                &tr!("cmd.revisit_no_arg", extra = format!("{extra:?}")),
                text("cmd.revisit_why"),
                text("cmd.run_without_args"),
                "gil revisit",
            ),
        ));
    }

    let mut session = open_session(ctx)?;
    // **닫힌 Cycle 이면 Cycle 계층이다.** 그 안에는 되돌아갈 Step 자리가 없다.
    if session.project().cycles().current().is_closed() {
        return revisit_cycle(&mut session);
    }
    session.revisit_step().map_err(say)?;
    write(&session)?;
    Ok(where_now(ctx, &session))
}

#[cfg(feature = "product-safety")]
fn product_artifact_rollback_refusal(command: &str) -> String {
    refusal(
        &tr!("cmd.product_rollback.what", command = command),
        text("cmd.product_rollback.why"),
        text("cmd.product_rollback.todo"),
        "gil status",
    )
}

/// Cycle 계층의 되돌아감 — **논리 이동과 세계 복원이 한 명령이다.**
///
/// 저장은 이 안에서 이미 끝난다(②→③ 순서). 그래서 여기서 `write` 를 다시 부르지 않는다 —
/// 부르면 복원이 끝난 뒤 같은 상태를 한 번 더 쓰는 셈이고, 그 사이의 의미가 흐려진다.
#[cfg(not(feature = "product-safety"))]
fn revisit_cycle(session: &mut Session) -> Result<String, String> {
    let done = session.revisit_cycle().map_err(say)?;
    Ok(revisited(session, &done))
}

/// 되돌아감의 receipt — **어디서 갈라졌고, 어디에 섰고, 세계는 어떻게 됐는가.**
///
/// 파일 목록도 내부 주소도 전체 Context 도 내지 않는다. Existence 와 판은 **지금 값을 읽어**
/// 보일 뿐이며, 되돌아감이 새 판을 만든 것처럼 말하지 않는다 — 만들지 않았다.
#[cfg(not(feature = "product-safety"))]
fn revisited(session: &Session, done: &crate::CycleRevisited) -> String {
    let cycles = session.project().cycles();
    let mut out = String::from(text("cmd.revisited_head"));

    let from = cycles
        .node(done.moved.from.into())
        .expect("방금 떠나온 자리는 실재한다");
    let verdict = from
        .report()
        .and_then(|report| report.get("verdict"))
        .unwrap_or(text("cmd.no_verdict"));
    out.push_str(&tr!("cmd.revisit_source", from = done.moved.from, kind = from.kind(), verdict = verdict));
    out.push_str(&tr!("cmd.revisit_target", target = done.moved.target, status = cycles.current().status()));

    out.push_str(text("cmd.artifact_world_head"));
    match done.world.no_op {
        true => out.push_str(&tr!("cmd.world_noop", world = done.world.world)),
        false => {
            out.push_str(&tr!("cmd.world_restored", world = done.world.world));
            for (label, count) in [
                (text("cmd.count.replaced"), done.world.replaced),
                (text("cmd.count.created"), done.world.created),
                (text("cmd.count.deleted"), done.world.deleted),
            ] {
                if count > 0 {
                    out.push_str(&tr!("cmd.count_line_indented", label = label, count = count));
                }
            }
        }
    }

    let existence = session.project().current_existence();
    out.push_str(&tr!("cmd.existence_line", existence = existence.id(), journey = existence.current_journey()));
    out.push_str(&branch_block());
    out
}

/// 되돌아온 자리에서 **지금 실제로 밟을 수 있는 수** — 갈래를 여는 것 하나뿐이다.
#[cfg(not(feature = "product-safety"))]
fn branch_block() -> String {
    let openable = CycleKind::ALL.to_vec();
    tr!("cmd.branch_block", commands = openable
            .iter()
            .map(|kind| format!("gil open {}", kind.as_str()))
            .collect::<Vec<_>>()
            .join("\n  "))
}

/// `gil monitor` — **사람이 지금 상태와 그 근거를 읽는 화면.**
///
/// 순서가 계약이다.
///
/// ```text
/// ProjectSession::open   잠그고 · 복구하고 · 검증된 판을 읽는다
/// session.monitor()      세계를 한 번 관측해 불변 Snapshot 을 만든다
/// drop(session)          ← **여기서 잠금이 풀린다**
/// render_monitor_text    Snapshot 하나만 보고 글을 짓는다
/// ```
///
/// 그리는 동안 프로젝트를 쥐고 있지 않는다. 화면 하나를 만드는 데 걸리는 시간만큼 다른
/// GIL 명령이 막히면, 사람이 보는 창이 곧 도구를 멈추는 자물쇠가 된다.
///
/// **아무것도 바꾸지 않는다.** 저장하지 않고, 세계를 확정하지 않고, 작업 파일을 만지지
/// 않는다 — 프로젝트를 여는 순간의 중단 복구는 Storage Model 의 원자성 복구이지 이 명령의
/// 쓰기가 아니다.
fn monitor(ctx: &Invocation<'_>, extra: Option<&String>) -> Result<String, String> {
    // 세 가지뿐이다 — 한 번 읽기, 저장해 열 문서 하나, 그리고 계속 지켜보기.
    let html = match extra.map(String::as_str) {
        None => false,
        Some("--html") => true,
        Some("--serve") => return watch_here(ctx),
        Some(other) => {
            return Err(refusal(
                &tr!("cmd.monitor_no_arg", other = format!("{other:?}")),
                text("cmd.monitor_why"),
                text("cmd.monitor_todo"),
                "gil monitor · gil monitor --html · gil monitor --serve",
            ));
        }
    };

    // **Snapshot 을 손에 쥐고 세션을 놓는다.** 값은 소유한 것이라 세션보다 오래 산다.
    //
    // 그리는 것은 잠금 밖에서 일어난다 — 두 표현 모두 같은 순서를 지킨다.
    let seen = {
        let session = open_session(ctx)?;
        session.monitor().map_err(say)?
    };
    Ok(match html {
        true => render_monitor_html(&seen),
        false => render_monitor_text(&seen),
    })
}

/// `gil monitor --serve` — **사람이 browser 로 계속 지켜본다.**
///
/// 앞에서 도는 명령이다. 주소를 한 번 적고, 사람이 끝낼 때까지 돈다.
///
/// **browser 를 열어 주지 않는다.** 어느 browser 를 어떤 profile 로 열지는 이 도구가 정할
/// 일이 아니고, 자동으로 열면 그 주소가 그 browser 의 기록에 남는다.
fn watch_here(ctx: &Invocation<'_>) -> Result<String, String> {
    let path = match find_gil(ctx)? {
        Some(Found::State(path)) => path,
        Some(Found::Legacy(path)) => {
            return Err(StoreError::LegacyFormat {
                path: ctx.said_path(&path),
            }
            .to_string());
        }
        None => {
            return Err(tr!("cmd.no_walk_found", path = crate::STATE_PATH));
        }
    };
    let mut server = crate::serve_monitor(rules()?, &path)?;

    // **주소 한 줄뿐이다.** token 을 따로 되풀이하지 않고, 프로젝트 경로도 `.gil` 자리도
    // 창고 주소도 적지 않는다 — 사람이 이 화면을 그대로 붙여 넣을 자리가 있기 때문이다.
    println!("{}", text("cmd.serve.intro"));
    println!("{}", server.url());
    println!();
    if server.blind().is_some() {
        println!("{}", text("cmd.serve.blind"));
        println!();
    }
    println!("{}", text("cmd.serve.valid"));
    println!("{}", text("cmd.serve.quit"));

    server.wait().map_err(|said| {
        refusal(
            &tr!("cmd.serve_busy", said = said),
            text("cmd.serve_busy_why"),
            text("cmd.serve_busy_todo"),
            "gil monitor --serve",
        )
    })?;
    Ok(String::new())
}

/// `gil mcp --serve` — **Agent 가 부를 MCP stdio 표면.**
///
/// 여기서 돌아오면 프로세스가 끝난다. stdout 은 이미 JSON-RPC 가 다 썼으므로 **빈 글자**를
/// 준다 — 한 글자라도 더 적으면 마지막 frame 뒤에 쓰레기가 붙는다.
fn mcp(sub: Option<&str>) -> Result<String, String> {
    match sub {
        Some("--serve") => crate::mcp::serve_stdio().map(|()| String::new()),
        Some(other) => Err(refusal(
            &tr!("cmd.mcp_unknown", other = format!("{other:?}")),
            text("cmd.mcp_only_serve"),
            text("cmd.mcp_agent_door"),
            "gil mcp --serve",
        )),
        None => Err(refusal(
            text("cmd.mcp_missing"),
            text("cmd.mcp_missing_why"),
            text("cmd.mcp_missing_todo"),
            "gil mcp --serve",
        )),
    }
}

fn status(ctx: &Invocation<'_>) -> Result<String, String> {
    // 이름 있는 변수로 받는다 — 임시 값으로 두면 잠금이 언제 떨어지는지가 식(式)의 모양에
    // 달리게 된다. 읽기만 하는 명령도 같은 exclusive 잠금을 **짧게** 쓴다.
    let session = open_session(ctx)?;
    Ok(where_now(ctx, &session))
}

/// `gil cycle …` — 옛 자리. `gil open`·`gil close` 가 두 계층을 함께 판정하므로 더는
/// 기본 경로가 아니지만, 이미 이 이름을 쓰던 손을 끊지 않으려고 남겨 둔다.
fn cycle(ctx: &Invocation<'_>, sub: Option<&str>, argument: Option<&String>) -> Result<String, String> {
    match sub {
        Some("close") => close(ctx),
        Some("open") => {
            let mut session = open_session(ctx)?;
            match place(&session) {
                Place::Cycle(openable) => open_cycle(&mut session, argument, &openable, false),
                Place::Branch(openable) => open_cycle(&mut session, argument, &openable, true),
                // 이미 잠근 것을 넘긴다 — `open()` 을 부르면 같은 프로세스가 잠금을
                // 두 번 잡으려다 스스로를 거절한다.
                _ => open_with(ctx, session, argument),
            }
        }
        Some(other) => Err(refusal(
            &tr!("cmd.cycle_unknown", other = format!("{other:?}")),
            text("cmd.cycle_only"),
            text("cmd.cycle_short_names"),
            "gil open · gil close",
        )),
        None => Err(refusal(
            text("cmd.cycle_missing"),
            text("cmd.cycle_missing_why"),
            text("cmd.cycle_short_names"),
            "gil open · gil close",
        )),
    }
}

// ── 상태에 민감한 도움말 — 읽기만 한다 ────────────────────────────────────

/// `gil close --help` — **지금 자리의 Close 계약만.**
///
/// 이 명령은 상태를 한 글자도 바꾸지 않는다. 저장하지 않고, Report 를 읽지 않고,
/// stdin 을 기다리지 않는다. 그래서 `Session::write` 로 가는 길이 여기엔 없다.
///
/// 실사용 M2C 에서 `gil close --help` 가 **stdin 없는 실제 Close** 로 처리됐다 —
/// 계약을 물어본 사람이 빈 Report 를 낸 것으로 거절당했다. 그 자리가 여기다.
fn close_help(ctx: &Invocation<'_>) -> Result<String, String> {
    let Some(session) = session_for_help(ctx)? else {
        return Ok(no_project_yet("gil close"));
    };
    let cycle = session.project().cycles().current();

    let contract = match place(&session) {
        Place::CloseThisStep => step_contract(cycle),
        Place::CloseThisCycle => cycle_contract(cycle),
        Place::CloseThisChain => {
            let hint = session.project().current_chain().unwrap().close_hint();
            return Ok(if matches!(ctx.input, Input::Body(_)) {
                hint.replacen(&format!("{}.", text("receipt.marker.chain_report")), text("cmd.chain_report_mcp"), 1)
            } else { hint });
        },
        // 닫을 자리가 아니면 **무엇을 먼저 해야 하는지**를 말한다.
        Place::Step(openable) => {
            return Ok(tr!("cmd.help_not_close_step", names = names(&openable)));
        }
        Place::Cycle(_) => {
            return Ok(text("cmd.help_not_close_cycle").to_string());
        }
        Place::Branch(_) => {
            return Ok(text("cmd.help_not_close_branch").to_string());
        }
        Place::Nothing(why) => {
            return Ok(tr!("cmd.help_not_close_nothing", why = why));
        }
    };

    let Some(contract) = contract else {
        return Ok(text("cmd.help_no_contract").to_string());
    };
    let mut skeleton = skeleton_for(&contract, cycle, Some(ctx));
    if session.project().visual_state.is_some() {
        if let Some(id) = cycle.step_now_open().filter(|id| cycle.steps().node(*id).is_some_and(|n| n.kind == NodeKind::Verify)) {
            skeleton.push_str(&typed_verify_hint(cycle.step_ref(id)));
        }
    }
    Ok(tr!("cmd.help_close",
        subject = contract.subject(),
        constraints = contract.constraints("  "),
        skeleton = skeleton,
        extra = if session.can_confirm_artifact() { text("cmd.help_close_evidence") } else { "" }
    ))
}

/// `gil open --help` — **지금 자리에서 열 수 있는 것과 그 계약만.**
///
/// 마찬가지로 아무것도 바꾸지 않고 stdin 을 기다리지 않는다.
fn open_help(ctx: &Invocation<'_>) -> Result<String, String> {
    let Some(session) = session_for_help(ctx)? else {
        return Ok(no_project_yet("gil open"));
    };
    let cycle = session.project().cycles().current();

    match place(&session) {
        // 열린 자리가 있으면 여는 법이 아니라 **먼저 할 일**을 말한다.
        Place::CloseThisStep => {
            let here = here_ref(cycle).expect("열려 있는 자리가 있다");
            let will = session.project().active_will();
            let mut out = tr!("cmd.help_not_open_step", here = here);
            if let Some(will) = will {
                out.push_str(&tr!("cmd.help_todo", text = indent(will.next_action())));
                out.push_str(&tr!("cmd.help_done_when", text = indent(will.done_when())));
            }
            out.push_str(text("cmd.help_close_first"));
            Ok(out)
        }
        Place::CloseThisChain => Ok(session.project().current_chain().unwrap().close_hint()),
        Place::CloseThisCycle => Ok(text("cmd.help_not_open_cycle").to_string()),
        Place::Nothing(why) => Ok(tr!("cmd.help_not_open_nothing", why = why)),
        // 실행형 Step — **행동 계약이 필요하다.**
        Place::Step(openable) => {
            let mut out = String::from(text("cmd.help_openable_head"));
            match openable.as_slice() {
                [only] => {
                    out.push_str(&format!("  {only}  {}\n", step_description(cycle, *only)));
                    out.push_str(&tr!("cmd.help_contract", skeleton = action_skeleton(None)));
                }
                many => {
                    out.push_str(&format!("  {}\n", choices(cycle, many)));
                    out.push_str(&tr!("cmd.help_contract", skeleton = action_skeleton(Some(text("cmd.kind_placeholder")))));
                }
            }
            out.push_str(text("cmd.help_you_write"));
            Ok(out)
        }
        // 컨테이너 Cycle — **행동 계약이 필요 없다.**
        Place::Cycle(openable) => Ok(tr!("cmd.help_open_cycle", choices = cycle_choices(&session, &openable))),
        // 되돌아온 자리 — 같은 명령이 **갈래**를 연다.
        Place::Branch(openable) => Ok(tr!("cmd.help_open_branch", choices = cycle_choices(&session, &openable))),
    }
}

/// 저장이 있으면 세우고, 아직 없으면 없다고 답한다 — help 는 없다고 실패하지 않는다.
fn session_for_help(ctx: &Invocation<'_>) -> Result<Option<Session>, String> {
    // MCP's explicit root uses the product record resolver, including Windows private
    // app-data storage. An inline-only existence probe would hide a real Project.
    if let Root::Exact(root) = ctx.root {
        return match crate::open_project_root(rules()?, root) {
            Ok(session) => Ok(Some(session)),
            Err(crate::RootError::NoProjectHere) => Ok(None),
            Err(error) => Err(error.to_string()),
        };
    }
    match find_gil(ctx)? {
        None => Ok(None),
        Some(_) => open_session(ctx).map(Some),
    }
}

/// 아직 프로젝트가 없다 — 일반적인 사용법과 시작하는 법.
fn no_project_yet(command: &str) -> String {
    tr!("cmd.no_project_yet", path = crate::STATE_PATH, command = command)
}

// ── 명령이 돌려주는 영수증 ─────────────────────────────────────────────────

/// 방금 연 자리 — **무엇을 열었고, 닫으려면 무엇이 필요하고, 다음에 무엇을 하는가.**
///
/// 전체 Journey 도 과거 Report 도 여기 다시 싣지 않는다(Agent UX Model §3). 같은 대화에는
/// 이미 있는 것이고, 없다면 그것은 `gil context` 의 몫이다.
fn opened_step(session: &Session, opened: crate::Opened) -> String {
    let cycle = session.project().cycles().current();
    let contract = cycle.contract_of_kind(opened.kind);
    let will = session
        .project()
        .active_will()
        .expect("실행형 자리는 Will 과 함께 열린다");

    // `지금 할 일` 과 `완료 조건` 은 방금 저장한 Will 에서 **투영한다** — 여기서 다시 쓰지
    // 않는다. `objective` 는 저장하되 무조건 되풀이하지 않는다(Agent UX Model §4.2).
    let mut out = format!("{}{} · {}\n", text("receipt.opened_lead"), opened.step, opened.kind);
    out.push_str(&tr!("cmd.help_todo", text = indent(will.next_action())));
    out.push_str(&tr!("cmd.help_done_when", text = indent(will.done_when())));
    out.push_str(text("cmd.opened_boundary"));
    if opened.kind == crate::NodeKind::Verify {
        if session.project().visual_state.is_some() {
            out.push_str(&typed_verify_hint(opened.step));
        }
        out.push_str(text("cmd.opened_verify_evidence"));
        out.push_str(text("cmd.opened_verify_do"));
    } else {
        out.push_str(text("cmd.opened_no_artifacts"));
    }
    if cycle.kind() == CycleKind::Interview {
        out.push_str(text("cmd.opened_interview_note"));
        match opened.kind {
            NodeKind::Synthesis => out.push_str(text("cmd.opened_synthesis_note")),
            NodeKind::Outcome => out.push_str(text("cmd.opened_interview_outcome_note")),
            _ => {},
        }
    } else if opened.kind == NodeKind::Define {
        out.push_str(text("cmd.opened_define_note"));
    }
    if opened.kind == NodeKind::Outcome {
        out.push_str(text("cmd.opened_outcome_note"));
    }
    // **닫기 전에 계약을 미리 알려 준다.** 허용값을 오류로 알아내게 두지 않는다
    // (실사용 M2C 보고). 말은 여기서 짓지 않고 문법에서 읽는다.
    if let Some(contract) = &contract
        && !contract.fields().is_empty()
    {
        out.push_str(text("cmd.opened_close_needs"));
        out.push_str(&contract.constraints("  "));
    }
    // **`실행` 블록을 두지 않는다.** 여기서 `gil close` 를 강조하면 실제 작업 전에 Node 부터
    // 닫는 오류가 난다(Agent UX Model §4.2) — 다음 수는 CLI 가 아니라 실제 세계에 있다.
    out
}

fn opened_cycle(session: &Session) -> String {
    let cycles = session.project().cycles();
    let cycle = cycles.current();
    let mut out = format!("{}{} · {}", text("receipt.opened_lead"), cycle.id().to_ref(), cycle.kind());
    match cycle.parent() {
        Some(parent) => out.push_str(&tr!("cmd.parent_suffix", parent = parent.to_ref())),
        None => out.push('\n'),
    }
    // **갈래로 열렸다면 그 사실을 말한다.** 부모만 보면 평범하게 이어 난 것과 구별되지
    // 않는데, 이 Cycle 은 어느 실패에서 갈라진 것이다.
    if let Some(from) = cycle.revisit_from() {
        out.push_str(&tr!("cmd.branch_source", from = from.to_ref()));
    }
    if cycle.kind() == CycleKind::Experiment {
        out.push_str(text("cmd.opened_experiment_scope"));
    }
    #[cfg(feature = "approved-plan")]
    if let Some(item) = session.project().current_plan_item() {
        out.push_str(&tr!("cmd.opened_plan_item", item = item.id));
    }
    out.push_str(&tr!("cmd.entry_world", snapshot = cycle.entry_snapshot()));
    out.push_str(&next_block(cycle));
    out
}

fn closed_step(session: &Session, closed: &crate::Closed) -> String {
    let cycle = session.project().cycles().current();
    // 방금 낸 판을 한 줄로 적는다 — 방금 제출한 Report 도 Will 전체도 되풀이하지 않는다.
    tr!("cmd.closed_step", lead = text("receipt.closed_lead"), step = closed.step, kind = closed.kind, journey = closed.journey, next = next_block(cycle))
}

fn closed_cycle(session: &Session, closed: &crate::ClosedCycle) -> String {
    let (here, kind) = (closed.cycle, closed.kind);
    let cycles = session.project().cycles();
    let cycle = cycles.current();
    let verdict = cycle
        .report()
        .and_then(|report| report.get("verdict"))
        .unwrap_or("");

    // 컨테이너는 판을 만들지 않는다 — 그때 서 있던 판을 그대로 provenance 로 적었다.
    let mut out = tr!("cmd.closed_cycle_head", lead = text("receipt.closed_lead"), here = here, kind = kind, verdict = verdict, journey = closed.journey);
    if let Some(chain) = session.project().current_chain().filter(|chain| chain.awaiting_report()) {
        out.push_str(&chain.close_hint());
        return out;
    }
    // **적어 둔 방향이 곧 다음 수다.** 되돌아감도 자식을 여는 것과 같은 자격으로 밟는다.
    if cycles.can_revisit() {
        if cfg!(feature = "product-safety") {
            out.push_str(&tr!("cmd.closed_cycle_revisit_product", marker = text("receipt.marker.revisit")));
            return out;
        }
        out.push_str(&tr!("cmd.closed_cycle_revisit_dev", marker = text("receipt.marker.revisit")));
        return out;
    }
    match cycles.why_not_open_child() {
        None => {
            if kind == CycleKind::Interview {
                out.push_str(text("cmd.closed_interview_note"));
            }
            out.push_str(&tr!("cmd.closed_openable", heading = text("receipt.openable_heading"), choices = cycle_choices(session, &CycleKind::ALL)));
        }
        Some(why) => {
            out.push_str(&tr!("cmd.closed_blocked", why = why));
        }
    }
    out
}

/// 다음에 열 수 있는 것 — 하나면 `gil open`, 여럿이면 고르라고 한다.
fn next_block(cycle: &crate::Cycle) -> String {
    let openable = cycle.openable_here();
    let mut out = String::new();
    if cycle.kind() == CycleKind::Interview {
        if cycle.can_close() {
            out.push_str(text("cmd.next_interview_wrap_cycle"));
        } else if openable.as_slice() == [NodeKind::Outcome] {
            out.push_str(text("cmd.next_interview_wrap_outcome"));
        }
    }
    out.push_str(&match openable.as_slice() {
        [] if cycle.can_close() => {
            let report = cycle.close_contract().map(|contract| skeleton_of(&contract, cycle)).unwrap_or_default();
            tr!("cmd.next_close_cycle", marker = text("receipt.marker.close_cycle"), report = report)
        }
        [] => tr!("cmd.next_blocked", marker = text("receipt.marker.blocked")),
        [_only] => tr!("cmd.next_open_one", prefix = text("receipt.openable_prefix"), names = names(&openable)),
        many => tr!("cmd.next_open_many", heading = text("receipt.openable_heading"), choices = choices(cycle, many)),
    });
    out
}

/// 여러 종류가 가능할 때 — 이름과 **그 자리가 무엇을 하는 자리인지.**
///
/// 설명을 여기 옮겨 적지 않는다. Kind 가 늘면 `gil-spec.yaml` 한 자리만 고치면 화면도
/// 함께 는다(Agent UX Model §4.2) — Cycle Kind 를 고르는 자리와 같은 규칙이다.
fn choices(cycle: &crate::Cycle, kinds: &[NodeKind]) -> String {
    let width = kinds
        .iter()
        .map(|kind| kind.as_str().len())
        .max()
        .unwrap_or(0);
    kinds
        .iter()
        .map(|kind| {
            format!(
                "{:<width$}  {}",
                kind.as_str(),
                step_description(cycle, *kind),
                width = width
            )
        })
        .collect::<Vec<_>>()
        .join("\n  ")
}

/// 그 Step Kind 가 무엇을 하는 자리인지 — **문법이 갖는 한 줄.**
fn step_description(cycle: &crate::Cycle, kind: NodeKind) -> &str {
    cycle
        .rules()
        .rules(cycle.kind(), kind)
        .map(|rules| rules.description.as_str())
        .unwrap_or_default()
}

/// 거절 — **무엇이 · 왜 · 지금 무엇을 · 다음 명령.** 넷을 늘 갖춘다.
///
/// 짧게 만들려고 까닭이나 복구를 빼지 않는다(Agent UX Model §9). 전체 문맥을 붙이지도
/// 않는다 — 복구에 필요한 국소 정보만.
fn refusal(what: &str, why: &str, todo: &str, run: &str) -> String {
    tr!("cmd.refusal", what = what, why = indent(why), todo = indent(todo), run = run)
}

/// 여러 줄짜리 토막을 한 단 들여쓴다 — 뒷줄이 제목처럼 보이지 않게.
fn indent(text: &str) -> String {
    text.lines()
        .map(|line| match line.is_empty() {
            true => String::new(),
            false => format!("  {line}"),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 지금 열려 있는 Step 을 닫는 계약. 열린 자리가 없으면 없다.
fn step_contract(cycle: &crate::Cycle) -> Option<CloseContract> {
    let walk = cycle.steps();
    let node = walk.node(walk.current()?)?;
    match node.is_closed() {
        true => None,
        false => cycle.step_contract(node.id, node.kind),
    }
}

/// Cycle 자신을 닫는 계약 — `outcome_ref` 는 **가리켜야 하는 그 주소**가 이미 정해져 있다.
fn cycle_contract(cycle: &crate::Cycle) -> Option<CloseContract> {
    cycle.close_contract()
}

/// 그 계약의 골격. Cycle Report 의 `outcome_ref` 만 값을 미리 채워 준다.
fn skeleton_of(contract: &CloseContract, cycle: &crate::Cycle) -> String {
    skeleton_for(contract, cycle, None)
}
fn skeleton_for(contract: &CloseContract, cycle: &crate::Cycle, ctx: Option<&Invocation<'_>>) -> String {
    let last = cycle
        .steps()
        .current()
        .map(|id| cycle.step_ref(id).to_string());
    let fill = |field: &str| match field {
        "outcome_ref" => last.clone(),
        "next_direction.action" if cycle.can_close() && cycle.rules().supports_chain_closing()
            && cycle.closing_synthesis(&Report::new().with("verdict", "success").with("next_direction.action", "close_chain")).is_ok() => Some("close_chain".into()),
        _ => None,
    };
    let mut hint = match ctx {
        Some(Invocation { root: Root::Exact(root), input: Input::Body(_) }) => tr!("cmd.skeleton_mcp",
            json = serde_json::to_string_pretty(&serde_json::json!({"project_root":root,"report":contract.report_body(fill)})).unwrap()),
        _ => contract.skeleton(fill),
    };
    if contract.subject().contains("outcome") {
        hint.push_str(text("cmd.skeleton_outcome"));
    } else if contract.subject().starts_with("cycle:") {
        hint.push_str(text("cmd.skeleton_cycle"));
        if cycle.rules().supports_chain_closing() && cycle.kind() == CycleKind::Interview {
            hint.push_str(text("cmd.skeleton_close_chain"));
        }
    }
    if cycle.rules().supports_chain_closing() && cycle.step_now_open()
        .and_then(|id| cycle.steps().node(id)).is_some_and(|node| node.kind == NodeKind::Synthesis) {
        hint.push_str(text("cmd.skeleton_chain_verdict"));
    }
    hint
}

fn names(kinds: &[NodeKind]) -> String {
    kinds
        .iter()
        .map(|kind| kind.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn cycle_names(kinds: &[CycleKind]) -> String {
    kinds
        .iter()
        .map(|kind| kind.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// 지금 열려 있는 자리의 주소.
fn here_ref(cycle: &crate::Cycle) -> Option<crate::StepRef> {
    let walk = cycle.steps();
    let node = walk.node(walk.current()?)?;
    match node.is_closed() {
        true => None,
        false => Some(cycle.step_ref(node.id)),
    }
}

// ── 지금 어디인가 ──────────────────────────────────────────────────────────

/// 지금 어디인가 — 어느 Cycle · 무엇을 이어받았나 · 어느 Step · 다음에 무엇을 할 수 있나.
///
/// 기록이 **서 있는 자리에 없으면** 어느 파일인지를 먼저 말한다. 있으면 말하지 않는다 —
/// 예사로운 일에 줄을 쓰면 정작 알려야 할 때 그 줄이 안 읽힌다.
/// 지금 자리를 사람의 말로 — **문장을 짓는 자리는 lib 하나**(`crate::where_now`).
///
/// CLI 는 서 있는 자리에서 조상을 거슬러 Project 를 찾으므로, 기록이 여기가 아닌 경우를
/// 여기서 판정해 넘긴다. MCP 는 project_root 가 곧 그 자리라 이 물음이 없다.
fn where_now(ctx: &Invocation<'_>, session: &Session) -> String {
    crate::where_now(session, found_above(ctx, session.state_path()))
}

// ── stdin 에서 Report 를 받는다 ────────────────────────────────────────────

/// 거절이 가리키는 복구 — **계약 전체는 여기서 되풀이하지 않는다.**
///
/// 거절은 빠진 값과 고치는 법에 집중하고, 어떤 값을 적을 수 있는지는 `gil close --help` 와
/// Open Receipt 가 같은 문법에서 읽어 말한다. 셋이 각자 설명을 지으면 반드시 갈린다.
fn fix_it(ctx: &Invocation<'_>) -> &'static str {
    if matches!(ctx.input, Input::Body(_)) { text("cmd.fix_it_mcp") } else { text("cmd.fix_it_cli") }
}


/// Report 는 stdin 으로만 받는다.
///
/// 통로를 하나로 둔다. 같은 것을 플래그로도 받으면 두 통로가 생기고, 그러면 둘이 어긋날 때
/// 무엇이 실제로 실렸는지 아무도 모른다 — 옛 도구가 가장 깊게 물린 상처가 그것이다.
///
/// 보여 주는 꼴은 **지금 이 Node 의 것**이다. 상관없는 예시를 보여 주면 읽는 쪽은 그것을
/// 베끼고, 베낀 것은 또 거절당한다.
fn read_report(ctx: &Invocation<'_>, skeleton: &str) -> Result<Report, String> {
    if ctx.input.is_terminal() {
        return Err(refusal(
            text("cmd.no_report"),
            text("cmd.report_stdin_terminal"),
            &format!("{}\n\n{skeleton}", fix_it(ctx)),
            "gil close",
        ));
    }

    let text = ctx.input.text()?;

    // 빈 글도 그대로 넘긴다 — 무엇이 빠졌는지는 문법이 말한다(여기서 짐작하지 않는다).
    Report::parse(&text).map_err(|err| {
        refusal(
            crate::locale::text("cmd.report_unparsed"),
            &err.to_string(),
            &format!("{}\n\n{skeleton}", fix_it(ctx)),
            "gil close",
        )
    })
}

/// 실행형 자리를 여는 **행동 계약**의 골격.
///
/// GIL 은 Node Kind 만 보고 그 내용을 지어내지 않는다(Will Model §7) — `question` 이라는
/// 이름은 *무엇을* 물을지 모른다. 그래서 값은 비워 두고 칸만 보여 준다.
fn contract_skeleton(kind: NodeKind) -> String {
    action_skeleton(Some(kind.as_str()))
}

/// 행동 계약의 골격 — 종류를 적어야 하는 자리면 그것까지.
fn action_skeleton(kind: Option<&str>) -> String {
    let named = match kind {
        Some(kind) => format!(" {kind}"),
        None => String::new(),
    };
    let mut out = format!("  gil open{named} <<'EOF'\n");
    for field in [crate::OBJECTIVE, crate::NEXT_ACTION, crate::DONE_WHEN] {
        out.push_str(&format!("  {field}: …\n"));
    }
    out.push_str("  EOF");
    out
}

/// 행동 계약은 stdin 으로만 받는다 — Report 와 같은 통로, 같은 문법.
///
/// **여기서 기다리지 않는다.** 터미널이 붙어 있으면 대화형 입력을 열지 않고 골격을 보여
/// 준다(Will Model §11: *"별도 대화형 입력을 암묵적으로 기다리지 않는다"*). 상태는 한 글자도
/// 바뀌지 않는다 — 아직 아무것도 열지 않았다.
fn read_contract(ctx: &Invocation<'_>, skeleton: &str) -> Result<ActionContract, String> {
    let refuse = |why: String| {
        refusal(
            text("cmd.no_contract"),
            &why,
            &tr!("cmd.contract_todo", skeleton = skeleton),
            "gil open",
        )
    };

    if ctx.input.is_terminal() {
        return Err(refuse(text("cmd.contract_stdin_terminal").to_string()));
    }
    let text = ctx.input.text()?;

    // YAML 자체가 깨진 것은 **어느 자리인지 말하지 못하는** parser 오류다. 거기엔 읽을
    // Topic 을 붙이지 않는다.
    let report = Report::parse(&text).map_err(|err| refuse(err.to_string()))?;
    ActionContract::from_report(&report)
        .map_err(|err| with_help(&Refusal::Contract(&err), refuse(err.to_string())))
}

// ── 안내 ───────────────────────────────────────────────────────────────────

/// `gil --help` — **평상시에 외워야 하는 것만.**
///
/// 전체 문법도, 모든 Step Kind 도 여기 늘어놓지 않는다(Agent UX Model §2·§9). 지금 무엇을
/// 열 수 있는지는 **매 명령의 출력이** 말하고, 그것이 이 도구가 가르치는 방식이다.
fn help() -> String {
    text("cmd.help_text").to_string()
}
