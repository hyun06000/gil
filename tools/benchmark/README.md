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
