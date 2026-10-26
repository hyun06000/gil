# Runtime update — U2 capability probe (synthetic; not a product feature)

> 이 디렉터리는 제품 updater가 아니다. 현재 제품 manifest, Companion 버튼, installer와 연결되어
> 있지 않다. 여기의 성공은 사용자가 업데이트할 수 있다는 뜻이 아니며 배포 문서에서 그렇게
> 인용하지 않는다.

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

Run the suite from PowerShell or cmd on Windows, not from an MSYS/Git-Bash shell: such a shell
enables the administrator token's backup/restore privileges, which bypass DACL denies and make the
permission fixtures non-enforcing (observed in CI run 39843768112). The fixtures check their own
precondition and record the token's enabled privileges in `*/fixture.json`.

Limits: an ordinary runner token and filesystem. The Codex Desktop restricted token, sandbox, real
MCP connection and persistent-store access are **not** reproduced. Rename failure is injected, not
observed. A green run is not a GIL update success and not an atomicity or power-loss durability
guarantee. The `GIL_U2_PROBE_SELFTEST_*` variables exist for the test file only.

제품 방향은 Companion이 업데이트를 알리고 사용자가 정확한 version을 확인한 뒤 버튼을 누르는
방식이다. 실제 구현은 서명된 installer, Host cache 밖의 고정 진입점, 불변 Core 버전 저장소,
원자적 선택 기록, 새 연결 identity 확인을 함께 갖춰야 한다. 이 probe를 그 기능의 일부로
재사용하려면 먼저 production call path와 실패 복구를 별도로 설계·검수해야 한다.
