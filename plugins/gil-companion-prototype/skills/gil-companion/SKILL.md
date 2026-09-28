---
name: gil-companion
description: Use GIL from this plugin — open and close steps, read context and status, restore or revisit — and open the persistent GIL Monitor when the user asks to show, pin, or keep the journey visible.
---

# GIL

This plugin carries two doors. **GIL itself**, which the agent calls, and the **GIL Monitor**,
which a human watches.

## Calling GIL

The GIL core ships inside this plugin. Do not look for a global `gil`, a repository build, or
cargo — they are not what these tools use, and their absence is not a problem.

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

# GIL Monitor

The GIL Monitor is the **persistent human surface** on the user's journey. It is not an optional
extra: an installation is complete only when the agent can call GIL *and* a human can open a
persistent Monitor.

Two surfaces can carry it, in this order:

1. **MCP App fullscreen** — Codex, or Claude Desktop **Cowork**, shows the shared Monitor UI alongside the Host's chat composer
2. **native companion** — the installed desktop app shows the same UI in its own window

For the in-host Monitor, use `gil_monitor_prepare(project_root)` with the user's explicit project
or a host-verified root, then `show_gil_monitor(scope_id)` with its returned scope. The first tool
does not render a UI; the second receives no filesystem path. Ask for the project if it is unknown.
The App defaults to horizontal layout and automatically requests fullscreen **once**, after its
Monitor is ready and the Host advertises support. If it stays inline, the user can press
**모니터 펼치기**. A user returning inline or choosing vertical must not be overridden. Do not call
rendering tools repeatedly to refresh it: the App reads complete Views and exact Step details
through its own read-only tools. Native Companion is never launched automatically by this App.

The preview is **not** proof of persistent display. The App declares inline/fullscreen and checks
the Host's advertised modes, request reply and context events. PiP is not required or claimed.
On the macOS build tested on 2026-09-23, Claude Desktop Code advertised only inline; guide users
to Cowork for fullscreen. Do not generalize that result to all builds, or claim Windows is tested.

Keep Companion installed. If fullscreen is unavailable, the user wants an independent window,
or the Host cannot keep the Monitor visible, use `show_gil_companion`. Its launcher starts a
closed Companion and confirms its handshake. The App's **별도 창 열기** button uses the same tool.
`gil_companion_status` currently reports the native coordinator's availability, not the mode of
an individual MCP App instance. Never treat it as proof that fullscreen succeeded.

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
