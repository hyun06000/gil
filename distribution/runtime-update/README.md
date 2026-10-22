# Runtime update — U2 capability probe (synthetic; not a product feature)

`gil-u2-probe.mjs` is the isolated capability probe from the Runtime Update contract (U2). It runs
inside one new folder under its working directory and exercises, independently: exclusive file
creation with conflict preservation, write/fsync/close/re-read, replacing a selection file with a
prepared file by **file** rename (never a directory rename, never an in-place overwrite), reading
the selection from a **new** child process, and a preservation check. The harness prints a plan
before creating anything, records every child's PID and exit code, and emits one JSON result on
stdout (also `result.json` inside the probe folder).

`gil-u2-probe.test.mjs` (`node --test`) runs the probe in fresh temp folders: the normal path, an
injected rename failure (originals preserved, new connection still reads the old selection, exit 1),
thrown operations (never reported as success), read errors (an unreadable file is an error, never
"absent" or "preserved"), blind identity lookups (incomplete on both sides is never sameness), a
server that exits too slowly (reported as unconfirmed, never killed) or replies garbage (timeout,
stdin still closed), a corrupt result block (raw stdout/stderr and exit code are saved before
parsing), and an unwritable working directory (exit 2, nothing created). The only permission
changes are made on fresh isolated temp folders/files owned by the tests, restored afterwards, with
the restore's exit code recorded in `unwritable/acl-restore.json` and asserted. Evidence of every
run is copied to `target/u2-probe-evidence/` for the CI artifact.

Limits: an ordinary runner token and filesystem. The Codex Desktop restricted token, sandbox, real
MCP connection and persistent-store access are **not** reproduced. Rename failure is injected, not
observed. A green run is not a GIL update success and not an atomicity or power-loss durability
guarantee. The `GIL_U2_PROBE_SELFTEST_*` variables exist for the test file only.
