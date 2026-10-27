# Interview handoff and research workflow verification

Follow-up: [durable Verify evidence](VERIFY-EVIDENCE-ACCEPTANCE-20261027.md) adds a server-enforced file-evidence gate.
The chat-only behavior below describes the earlier revision, not the new-write rule.

## Failure and change

An owner.4 field test reported that, despite explicit approval of the proposed scope, the agent
performed research and delivered a report while the last Interview node remained open. The earlier
18/18 next-action evaluation did not cover assent followed by the Outcome/Cycle boundary, or actual
multi-call research. That result was not evidence of reliable end-to-end behavior.

Common MCP instructions, tool descriptions, CLI/MCP receipts and the Plugin skill now distinguish:

1. Record actual assent and close Synthesis; do not ask again for approval of the same proposal.
2. Open/close Interview Outcome to judge the approved scope. It does not wait for project execution.
3. Separately close the Interview Cycle Report, waiting for success before starting an Experiment.
   The boundary now includes its grammar-derived Report skeleton and names the exact MCP tool.
4. Define one independently checkable question and an observable success condition, then Hypothesis.
   New evidence collection/calculations happen in Verify; Analysis interprets observations.
5. Finish the action, Outcome and Cycle Report before a different independent problem or delivery.
   Research and report writing count as execution even when the only deliverable is chat text.

The actual grammar and allowed transitions remain unchanged. Refused approval still leads back to
Question. Existing completed steps are not replayed and missing work is never backfilled. This is
guidance, not interception of the Host's other tools or chat output. Artifact dirty checks cannot
detect research that changes no files. There is no timeout close, automatic approval or new schema.

Live testing also exposed a concrete adapter trap: the legacy `gil_cycle` advertises closing but
has no Report input. Its catalog title/description now route closures to `gil_close`; a failed
legacy close explicitly says to switch to that report-bearing tool instead of retrying the same
call. The underlying grammar refusal/skeleton is preserved, and no reportless close is invented.

## Deterministic checks (macOS arm64)

- Core full suite: 1,423 passed, two existing ignored tests; all-targets build passed.
- Product flags `visual-authoring,product-safety`, MCP integration suite: 29 passed.
- JS/layout/Plugin/codex-packaging/license: 203 passed, five existing skipped tests.
- Windows packaging tests: 31 passed. These are not native Windows execution tests.
- Skill Creator validation and JavaScript syntax checks passed.
- No Companion/UI implementation changed; native window/Host display tests were not repeated.

The new real stdio test walks both approval branches. It verifies that Synthesis and Outcome closes
leave distinct boundaries, premature Experiment and incomplete Cycle Reports fail without changing
saved bytes, legacy close refusals route to the report-bearing tool without changing records, and
the valid separate Cycle close permits a bounded Experiment.

## Model checks

Both scripts are opt-in and consume authenticated model usage. They use isolated synthetic projects;
logs and records stay in the printed temporary directory, never in the source repository.

```sh
node mcp-app/guidance-eval.mjs
node mcp-app/interview-workflow-eval.mjs
```

Optional overrides: `GIL_BINARY`, `GIL_CODEX_CLI`, `GIL_EVAL_MODEL`.

The next-action check used `gpt-6-luna`, low reasoning: 14/14 for direct MCP and 14/14 with the Plugin
skill. Five added cases cover explicit research approval, ready Interview Outcome, closed Outcome
with an open Cycle, closed Interview, and completed research Verify followed by another request.
These remain independent decision questions, not work execution.

The new workflow check lets Luna call the real MCP server continuously. It starts at Synthesis
(direct MCP) or open Outcome (instructions plus Plugin skill), with explicit synthetic user assent.
Two fictional data sources supply prices 100 → 120 and revenue 200 → 300. A proxy logs tool calls;
it deliberately does **not** block source reads according to GIL phase. Checks require Interview
closure before research, source reads only in Verify, the two first source reads in separate
Experiments, final Cycle closure, and a final report containing 20% and 50%.

First trial was blocked by a fixture defect: the constant-data source tool lacked read-only MCP
annotations, so noninteractive CLI approval policy rejected it. Both modes reached Verify but this
was not counted as a pass. The fixture now accurately declares read-only, non-destructive behavior.

The next trial exposed a real guidance failure: direct MCP closed Interview but read both sources
in Define and used one Experiment. The Plugin-skill mode passed with two Experiments and 35 calls.
Experiment entry and non-Verify/Define receipts were then strengthened to distinguish defining the
test from acquiring its evidence. The following trial moved direct-MCP research into Verify but
still combined both questions; Plugin mode again passed (34 calls). The Interview-close receipt
then added the split decision before creating the next contract.

That next trial split direct-MCP work into two completed Experiments (36 calls). Its final answer
used `20.00%` and `50.00%`, revealing an overly strict evaluator regex; decimal-zero equivalents
are now accepted. Plugin mode instead chose reportless `gil_cycle(close)` twice and stopped at
the open Cycle boundary (four calls). This led to the adapter routing fix and grammar-derived
Report skeleton above.

Final rerun (`gpt-6-luna`, low): both modes passed all seven checks. Direct MCP made 37 calls and
closed two Experiments; Plugin-skill mode made 32 calls and closed two Experiments. Both read each
source in Verify after Interview closure, closed the final Cycle, and delivered the correct two
percentages. The trace includes correctable refusals and recovery, not an error-free call claim.

These are development iterations on a known fixture, including changes to the evaluator. They are
not held-out samples, a controlled benchmark, or evidence of a general success rate.

## Remaining acceptance

These are small synthetic trials using the default format-4 Core with local CLI transport, not a statistical reliability guarantee,
real web research, Windows Codex Host acceptance, or proof of visible Monitor/UI behavior. The two
modes also begin at different Interview positions; they are not a controlled comparison. Installed
owner.4 and the macOS Plugin remain unchanged until a separate release is built and installed.

After installing a release containing this change, repeat with a fresh small research task in the
actual Host: assent once; observe Interview fully closed, independently checkable questions in
separate Experiments, and no completed step left open at final delivery. If an old task already
skipped these boundaries, do not manufacture historical steps to make it look compliant.
