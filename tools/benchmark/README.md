# Run one autonomous benchmark task

Normal Linux sessions use the interactive Monitor workflow. Setting `GIL_MCP_DISPLAY=headless` selects display-free MCP and keeps its existing consent rules. Only explicitly invoking `gil benchmark` selects autonomous execution.

Build the Linux binary with `cargo build --locked -p gil --release --bin gil`. In your Linux image, install that executable as `/usr/local/bin/gil`, include GNU `timeout` and put the task text and an executable benchmark agent adapter in the image. The adapter supplies the model or local inference implementation. The smoke adapter in this directory exercises a synthetic arithmetic task.

From a Docker host, run:

```sh
gil benchmark my-benchmark-image /case/task.txt /case/my-agent
```

The launcher creates a disposable, non-root, networkless container with a read-only image, temporary work and record directories, and no host bind mounts. Task assets needed by the agent can live in the image. The agent writes experiment files under its working directory. The controller emits the final Cycle Report and Step history as JSON. Exit 0 means the first Experiment reported success, 2 means it reported failure, and 1 means the run did not finish. The container is removed on exit. For an existing benchmark container the equivalent entrypoint is `gil benchmark --in-container TASK_FILE ADAPTER WORKSPACE NEW_RECORD_DIRECTORY`. That entrypoint relies on the harness for container isolation and checks the effective UID, capabilities and no-new-privileges flag in Linux procfs before creating records.

## Agent protocol

The controller starts the adapter once per turn. It reads one JSON request from stdin and writes exactly one JSON reply to stdout. Logs go to stderr. The request includes the task, previous Step and Cycle Reports with Core references, the current recommendation in `next`, and the previous Core refusal when retrying.

- For `next.action == "open"`, select the recommended Step or another kind from `next.allowed`. Reply with `{"action":"open","kind":"question","contract":"objective: …\nnext_action: …\ndone_when: …"}`.
- For `next.action == "close"`, perform the open contract, then return `{"action":"close","report":"…"}` using the listed Report fields and constraints. For a Question derive the response from the task and record ambiguities in Interpretation. For Synthesis write the specification, cite its actual basis references, and record the adapter's decision as `approved: benchmark_accept` or `approved: benchmark_revise`. Revision returns to Interview. The adapter has no Monitor approval capability.
- For Verify, actually execute the experiment and write new evidence files. List them in `evidence_files`. Analysis compares the observation to the success condition. The Core validates both Outcome and Cycle Reports. A refused close leaves the Step open. On retry inspect the existing evidence before repeating any execution.

A run uses a fresh record directory outside its workspace. It refuses workspaces inside ordinary GIL projects. Benchmark reports identify their source, and the standard project grammar continues to accept only its ordinary approval values. The runner covers the first complete Experiment, limits a run to 64 adapter calls, and bounds each call to 120 seconds and a 1 MiB JSON reply. Use a fresh container for a new run.

## Container acceptance

`tools/benchmark/test-container.sh target/debug/gil` runs the committed synthetic participant on Linux. It first checks ordinary Linux MCP Monitor handoff and display-only headless MCP waiting for a response. It then checks a passing and failing experiment, revision of a specification, rejection of human approval tokens, and refusal to close Verify without an evidence file. These checks exercise the protocol and Core transitions. Benchmark model scores require running the intended model adapter and task suite.

## Run one model trial with Harbor

Manually dispatch `product-candidate.yml` with **harbor_benchmark** selected. Pick
`luna` (default) or `qwen`. The dispatch runs only the Linux benchmark job. PR checks and
ordinary product builds do not invoke a model. For example:

```sh
gh workflow run product-candidate.yml -f harbor_benchmark=true -f harbor_model=luna
```

- **Luna** uses `gpt-5.6-luna` through the Responses API. Configure the repository Actions
  secret `OPENAI_API_KEY`. The Harbor host sends short model prompts and choices, then returns
  each answer through a file in the task container. The task has no API key or external model
  connection. The host requests standard service, no reasoning and at most 128 output tokens
  per call. It checks a 16-call limit and a conservative $0.005 estimated token budget before
  each request. The seven-day artifact includes token usage and estimated cost at $0.20/M input
  and $1.20/M output. Recheck these prices before a new evaluation campaign.
- **Qwen** runs Qwen2.5-1.5B-Instruct Q4_K_M on the container CPU. The runner downloads its
  hash-checked weights before the trial. This option requires no model key.

Harbor 0.27.0 runs the same CSV task in a Docker container with outbound network access denied.
The model answers the interview, proposes the operation and column for a structured specification,
selects a CSV aggregation tool for Verify, and interprets the result. The verifier checks the
question, pre-experiment claims, accepted specification, evidence file and reports against its
own calculation. See the job artifact for the submitted observation and full Step history.

On a Linux Docker host with Harbor 0.27.0 on PATH, pass a built product binary and a fresh output
directory to `tools/benchmark/harbor/run.sh target/debug/gil /tmp/gil-harbor-run`. Local runs
default to Qwen. Set `GIL_HARBOR_MODEL=luna` and supply `GIL_HARBOR_API_KEY` to select Luna.

In a [verified Luna run](https://github.com/hyun06000/gil/actions/runs/45570033791) the model
made 11 calls (1,612 input and 249 output tokens), measured 13 ms and received reward 1.
The estimated model cost for that trial was $0.0006212.

## Terminal-Bench 4.1 paired CPU pilot

Manually select **harbor_benchmark** and **terminal-bench-pair**, or dispatch
`gh workflow run product-candidate.yml -f harbor_benchmark=true -f harbor_model=terminal-bench-pair`.
Only the two Linux trial jobs and the report job run. The two arms independently solve the
official v4.1.0 sound-change-cascade task at commit 4070d6cef56a7d20cb5dc98d5e10ada88a6b9440.
Both see the same public training pairs and alignment summaries, can evaluate complete rule
lists and submit their best ordered rules. The official separate verifier grades each submission.
This task-specific pilot gives each arm 30 minutes of agent time, with no model call count
or estimated dollar cap. A single paired observation is not a suite-level effect estimate.

The GIL arm starts the product CLI and runs the shared solver in its open Verify step.
The isolated worker can set `GIL_BENCHMARK_VERIFY_SECONDS` for that step; the default is 120.
The plain arm directly invokes the same solver. Each call logs cumulative usage in its Linux job.
Download `terminal-bench-dashboard` and open `index.html` for paired scores, time, tokens,
estimated cost and the raw comparison measurements. An absent score is shown as missing.
