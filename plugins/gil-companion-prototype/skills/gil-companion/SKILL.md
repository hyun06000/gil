---
name: gil-companion
description: Use GIL from this plugin — start a GIL project in the folder the user selected and open its Monitor in that same flow, open and close steps, read context and status, restore or revisit — and open the persistent GIL Monitor when the user asks to show, pin, or keep the journey visible.
---

# GIL

This plugin carries two doors. **GIL itself**, which the agent calls, and the **GIL Monitor**,
which a human watches.

## Calling GIL

The GIL core ships inside this plugin. Do not look for a global `gil`, a repository build, or
cargo — they are not what these tools use, and their absence is not a problem. The installed
native Plugin does not require Node, npm or Homebrew either.

`gil_start` · `gil_open` · `gil_close` · `gil_restore` · `gil_revisit` ·
`gil_status` · `gil_story` · `gil_context` · `gil_cycle` · `gil_help`

Every one of them needs `project_root`: the absolute path of the project to act on, taken from
the host-verified workspace or from what the user named. **Never guess it** — not from the
working directory, not from a recent folder, and not from whatever the Monitor happens to be
showing. The human may be watching one project while the agent works in another.

`gil_open` and `gil_close` carry their body on stdin (`contract`, `report`). Write the fields the
current grammar asks for; GIL validates them and refuses with its own words when they are wrong.
Do not invent the field list from memory — `gil_help` and GIL's own refusals tell you.

Each tool answers with `ok`, `exit_code`, `said` (what GIL printed) and `problem` (what GIL
refused with). **`ok` is the only success signal.** Read `said`/`problem` as GIL's own words and
pass them on; do not reword them into a verdict of your own.

**Run GIL actions on one project one at a time.** GIL takes a short exclusive lock on the
project while it works, so two calls fired in parallel — even two reads like `gil_status` and
`gil_context` — will have one of them refused. Wait for each answer before sending the next.
A refusal that says the project is busy means exactly this; retry it after the one in flight
returns, and do not treat it as a broken project.

If `gil_companion_status` reports `agent_surface: unavailable`, GIL actions cannot run here.
Say so plainly rather than falling back to a shell.

## Starting a project and watching it — one request

When the user asks to **start a GIL project** ("GIL 프로젝트를 시작하자", "지금 폴더에서 GIL
프로젝트를 시작해 줘") in a folder they explicitly selected or named, do the whole flow from that
one request. Do not wait for a separate "Monitor 열어 줘", and do not ask them to install anything.

1. `gil_start(project_root)` with the selected folder. If no folder was explicitly selected or
   named, ask which folder first; never guess one.
2. On `ok: true`, immediately `gil_monitor_prepare(project_root)` on that same root, then
   `show_gil_monitor(scope_id)` with the scope it returned. The App starts horizontal and requests
   fullscreen once by itself; you do not request or re-render it.
3. Follow the start receipt: `gil_open` the first Interview step and ask the user what they want.
   The Monitor reads the project on its own, so the graph updates from this first step on without
   any further rendering call.

Report three things separately and never fold them into one "done":

- **Start** — `gil_start` answered `ok`. Its `said` is GIL's receipt; pass it on.
- **Monitor data** — `show_gil_monitor` returned a View: the display was requested and its data is
  ready. That is not proof anything is visible.
- **Display** — unconfirmed until the user says so. Say that the Monitor display was requested and
  its data is ready, and that whether it is on screen is still unconfirmed; ask them to press
  **모니터 펼치기** or tell you if it stays inline or hidden. Only after the user confirms the graph
  and the chat are on screen together may you report that the Monitor is showing. Never phrase a
  tool result as "the Monitor is open", "opened the Monitor" or "fullscreen succeeded".

Keep these boundaries:

- **An already started project is never reinitialised.** If `gil_start` refuses with
  「이미 걷고 있다」, that folder already has a GIL journey. Do not delete or move `.gil`, do not
  retry `gil_start`, and do not open another Cycle. Say the project already exists, open its
  Monitor (step 2 only), then read `gil_context` and continue from where it stands.
- **A start failure is not a display failure, and not proof that no record exists.** If
  `gil_start` is refused for any other reason (an older record format, an unreadable folder), pass
  on GIL's own words and stop before the Monitor steps: this start request did not create anything.
  Do not conclude that the folder has no GIL record. An older format or a permission problem may be
  sitting on an existing journey; leave every file as it is and do not clear, move or convert it.
  Say plainly that this request failed and that whether an earlier record exists is unknown from this
  answer. A busy refusal is the one exception described above: wait for the call in flight, then
  retry the same `gil_start` once.
- **Display retries are limited and never touch the project.** Retry `gil_monitor_prepare` /
  `show_gil_monitor` at most once, and only for a transient answer such as `busy` or a broken
  connection. Do not repeat them for a lasting refusal (`not_a_project`, `unsupported_format`,
  `project_moved`, `settings_*`) or when the Host does not advertise the display. If it is
  unclear whether the screen appeared, ask the user instead of calling `show_gil_monitor` again:
  every call can add another card. Preparing the same root again returns the same scope; it does not
  create a second project, Cycle or binding. Never repeat `gil_start` to "refresh" the screen.
- **Fullscreen can be refused.** If the Host keeps the App inline or answers with another mode, do
  not claim fullscreen. Tell the user the actual state, that **모니터 펼치기** requests it by hand,
  and that the optional native Companion exists — and open the Companion only if they choose it.
- **The user's inline choice wins.** If they return the Monitor to inline or pick vertical, do not
  request fullscreen again and do not call `show_gil_monitor` again to force it.
- **One open Cycle at a time.** `gil_start` opens the first Interview, and that is the only open
  Cycle. Do not open another Interview or Experiment while it is open, do not close it merely to
  satisfy a request, and do not leave it suspended to open a new one; close it through its own
  Report first, then open the next one where GIL allows.
- **The Monitor stays read-only.** It never writes the project; every change goes through the GIL
  actions the user asked for.

# GIL Monitor

The GIL Monitor is the **persistent human surface** on the user's journey. It is not an optional
extra: an installation is complete only when the agent can call GIL *and* a human can open a
persistent Monitor.

Two surfaces can carry it, in this order:

1. **MCP App fullscreen** — the supported Host shows the shared Monitor UI alongside its chat composer
2. **optional native Companion** — if the user chooses it, a compatible desktop app shows the same UI in its own window

The first unsigned preview's accepted target is **Codex on macOS Apple Silicon**. Its default is
Plugin-only fullscreen; a separate Companion installation is **not required**. Windows, Intel Macs
and Claude work-Plugin rendering are not accepted targets of this preview. Historical Claude Desktop
Cowork/direct-MCP fullscreen demonstrations do not prove that a folder-connected Plugin works there.
Do not present moving to Cowork as a verified fix. Fresh-Mac installation is deferred, not passed;
if macOS blocks execution, stop without bypassing its security checks.

For the in-host Monitor, use `gil_monitor_prepare(project_root)` with the user's explicit project
or a host-verified root, then `show_gil_monitor(scope_id)` with its returned scope. The first tool
does not render a UI; the second receives no filesystem path. Ask for the project if it is unknown.
The App defaults to horizontal layout and automatically requests fullscreen **once**, after its
Monitor is ready and the Host advertises support. If it stays inline, the user can press
**모니터 펼치기**. A user returning inline or choosing vertical must not be overridden. Do not call
rendering tools repeatedly to refresh it: the App reads complete Views and exact Step details
through its own read-only tools. Native Companion is never launched automatically by this App.

An inline preview or successful tool response is **not** proof of persistent display. The App declares inline/fullscreen and checks
the Host's advertised modes, request reply and context events. PiP is not required or claimed.

If fullscreen is unavailable or the Host cannot keep the Monitor visible, explain the limitation
and offer the accepted Host route or optional Companion. Do not install or launch Companion merely
because fullscreen failed. Only after the user chooses an independent native window, use
`show_gil_companion`; installation or update still requires explicit approval. Its launcher starts
a closed compatible Companion and confirms its handshake. The App's **별도 창 열기** button is that
explicit native-window choice and uses the same tool.
`gil_companion_status` currently reports the native coordinator's availability, not the mode of
an individual MCP App instance. A missing Companion is not a reason to block the in-host Monitor;
do not require `companion_state: ready` before `gil_monitor_prepare` / `show_gil_monitor`.
Never treat the native coordinator's status as proof that fullscreen succeeded.

## What the tools report

`gil_companion_status` returns the availability read model:

- `agent_surface` — `ready` or `unavailable`
- `monitor_surface` — `persistent_host`, `native_companion`, or `unavailable`
- `companion_state` — `missing`, `stopped`, `outdated`, or `ready`

`show_gil_companion` reports what actually happened:

- `opened_persistent_host` — the host's own persistent surface was used
- `focused_existing_companion` — the window was already open and is now in front
- `started_and_opened_companion` — the app was closed; it was started, its handshake was
  confirmed, and then the window was brought forward
- `needs_companion_install` — nothing is installed; ask before installing anything
- `needs_companion_update` — an incompatible version is installed; it is not opened
- `monitor_unavailable` — no persistent surface could be opened right now

Never restate `needs_companion_install`, `needs_companion_update`, or `monitor_unavailable` as
success. `ready` requires a fresh runtime challenge; process names and paths are not evidence
of compatibility.

## When the Monitor cannot open

Say so plainly and keep working. GIL's text loop and its record of the journey do not depend on
the Monitor:

> Monitor를 열 수 없지만 GIL 기록 작업은 계속할 수 있다.

`gil context` still shows where the journey stands.

## Rules

- Do not substitute `gil monitor`, a browser page, a local server, or filesystem inspection.
- Do not search for or run an executable path yourself; the tools use a fixed bundle identity.
- Do not install, download, or update anything without the user's explicit approval.
- Do not choose or guess a GIL Project. An MCP App binds to the explicitly prepared scope;
  open another scope only at the user's request. Paths never go to App tools or the resource.
- The native window shows whichever project the user last selected. If they want a different one,
  tell them to use **폴더 열기** in the window — not a shell command.
- Do not show the user a path, port, PID, or socket. They are not part of the Monitor's
  vocabulary.
- If the tools are unavailable, say the plugin MCP server is not connected. Do not reinterpret
  the GIL Monitor as something you can open another way.

## A note on persistent host surfaces

`monitor_surface` is `persistent_host` only when a real probe has confirmed it — the app's
declared display modes, the host's advertised capability, the raw return of a user-initiated
mode request, and the observed mode and surface lifetime. Until that probe exists, the host
surface is *unverified*, which is **not** the same as unsupported. Native installation, Windows,
task switching and restart recovery are not waived by a successful fullscreen counter demo.
