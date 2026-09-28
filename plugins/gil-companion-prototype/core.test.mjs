// **실린 GIL Core 의 시험** — `node --test`
//
// 두 가지를 잰다. 하나는 **판정** — 무엇을 ready 라 부르고 무엇을 거절하는가. 가짜 core 를
// 세워 각 경계를 따로 민다. 다른 하나는 **동치** — 실린 Core 가 저장소에서 지은 것과 같은
// 일을 하는가. 같은 fixture 에 같은 명령을 주고 종료 코드·산문·`.gil` 바이트를 견준다.

import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { chmod, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { AGENT_STATE, callCore, compatible, probeCore } from "./core.mjs";
import { ACTIONS, argvFor, safePositional } from "./actions.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
const BUNDLED = join(HERE, "core", "darwin-arm64", "gil");
const CONTROL = join(HERE, "..", "..", "target", "release", "gil");

const WHOLE = {
  schema_version: 1,
  product: "gil_core",
  core_version: "0.1.0",
  protocol: { min: 1, max: 1 },
  storage_format: { min: 4, max: 4 },
  action_surface: { min: 1, max: 1 },
  os: "macos",
  arch: "aarch64",
};

/** 시키는 대로 답하는 가짜 core 를 세운다 — 경계를 하나씩 밀기 위해서다. */
async function fakeCore(body, { mode = 0o755, classify = false } = {}) {
  const root = await mkdtemp(join(tmpdir(), "gil-core-"));
  const slot = join(root, "core", "darwin-arm64");
  await mkdir(slot, { recursive: true });
  const exe = join(slot, "gil");
  await writeFile(exe, body);
  await chmod(exe, mode);
  if (classify) {
    // These cases measure identity/version/challenge classification, not cold
    // shell startup. Fresh macOS fixtures sometimes exceed 5s; repeating the same
    // file answers in milliseconds. Prepare it before the real, still-5s probe.
    // No warm-up for the real bundled Core, non-executable/no-answer or call-count cases.
    try { await promisify(execFile)(exe, ["--gil-test-fixture-ready"], {timeout: 60000}); }
    catch (error) { if (error.code !== 1) throw error; } // echoing fixture's non-probe exit
  }
  return { root, exe };
}

/** challenge 를 그대로 돌려주며 주어진 descriptor 를 말하는 core. */
const echoing = (descriptor) => `#!/bin/sh
if [ "$1" = "--gil-agent-probe" ]; then
  printf '{"schema_version":1,"challenge":"%s","core":%s}\\n' "$2" '${JSON.stringify(descriptor)}'
  exit 0
fi
exit 1
`;

// ── ① 무엇을 ready 라 부르는가 ──────────────────────────────────────

test("실린 Core 가 fresh challenge 에 답하면 ready 다", async (t) => {
  if (!existsSync(BUNDLED)) return t.skip("core 가 아직 실리지 않았다 — make-core.sh");
  const seen = await probeCore();
  assert.equal(seen.state, AGENT_STATE.ready, seen.why ?? "");
  assert.equal(seen.descriptor.product, "gil_core");
  assert.ok(seen.exe, "부를 자리를 돌려주지 않았다");
});

test("Core 가 없으면 unavailable 이고 아무것도 실행하지 않는다", async () => {
  const root = await mkdtemp(join(tmpdir(), "gil-core-none-"));
  const seen = await probeCore({ root });
  assert.equal(seen.state, AGENT_STATE.unavailable);
  assert.equal(seen.why, "core_not_executable");
  assert.equal(seen.exe, null, "부를 자리를 주면 안 된다");
  await rm(root, { recursive: true, force: true });
});

test("실행 권한이 없으면 unavailable 이다", async () => {
  const { root } = await fakeCore(echoing(WHOLE), { mode: 0o644 });
  const seen = await probeCore({ root });
  assert.equal(seen.state, AGENT_STATE.unavailable);
  assert.equal(seen.why, "core_not_executable");
  await rm(root, { recursive: true, force: true });
});

test("답하지 못하는 파일은 unavailable 이다", async () => {
  const { root } = await fakeCore("#!/bin/sh\nexit 3\n");
  const seen = await probeCore({ root });
  assert.equal(seen.state, AGENT_STATE.unavailable);
  assert.equal(seen.why, "core_did_not_answer");
  await rm(root, { recursive: true, force: true });
});

test("이름이 다른 물건은 낡은 판이 아니라 거절이다", async () => {
  const { root } = await fakeCore(echoing({ ...WHOLE, product: "something_else" }), {classify: true});
  const seen = await probeCore({ root });
  assert.equal(seen.state, AGENT_STATE.unavailable);
  assert.equal(seen.why, "identity_mismatch");
  await rm(root, { recursive: true, force: true });
});

test("protocol·action surface 범위가 어긋나면 outdated_agent 다", async () => {
  for (const off of [{ protocol: { min: 2, max: 2 } }, { action_surface: { min: 9, max: 9 } }]) {
    const { root } = await fakeCore(echoing({ ...WHOLE, ...off }), {classify: true});
    const seen = await probeCore({ root });
    assert.equal(seen.state, AGENT_STATE.outdatedAgent, JSON.stringify(off));
    assert.equal(seen.exe, null, "부를 수 없는데 자리를 주었다");
    await rm(root, { recursive: true, force: true });
  }
});

test("다른 기계를 말하는 Core 는 거절한다", async () => {
  for (const off of [{ os: "windows" }, { arch: "x86_64" }]) {
    const { root } = await fakeCore(echoing({ ...WHOLE, ...off }), {classify: true});
    const seen = await probeCore({ root });
    assert.notEqual(seen.state, AGENT_STATE.ready, JSON.stringify(off));
    await rm(root, { recursive: true, force: true });
  }
});

test("예전 답을 다시 보여 주는 Core 는 거절한다", async () => {
  // challenge 를 무시하고 **고정된** 답을 돌려준다.
  const { root } = await fakeCore(`#!/bin/sh
printf '{"schema_version":1,"challenge":"stale_challenge_xxxx","core":%s}\\n' '${JSON.stringify(WHOLE)}'
exit 0
`, {classify: true});
  const seen = await probeCore({ root });
  assert.equal(seen.state, AGENT_STATE.unavailable);
  assert.equal(seen.why, "challenge_not_returned");
  await rm(root, { recursive: true, force: true });
});

test("storage format 은 문지기로 쓰지 않는다 — Core 가 스스로 거절할 일이다", () => {
  const machine = { os: "macos", arch: "aarch64" };
  assert.equal(compatible({ ...WHOLE, storage_format: { min: 9, max: 9 } }, machine), true);
  assert.equal(compatible({ ...WHOLE, protocol: { min: 9, max: 9 } }, machine), false);
});

// ── ② 사용자 입력이 문법이 되지 않는다 ──────────────────────────────

test("flag 모양의 값은 위치 인자가 되지 못한다", () => {
  for (const bad of ["--help", "-h", "--gil-agent-probe"]) {
    assert.ok(safePositional(bad, "값"), `${bad} 를 받아 주었다`);
  }
  for (const bad of ["줄\n바꿈", "\u0000", "x".repeat(201), ""]) {
    assert.ok(safePositional(bad, "값"), "받아 주면 안 되는 값을 받았다");
  }
  assert.equal(safePositional("experiment", "값"), null);
});

test("각 tool 이 subcommand 하나에만 대응하고 argv 를 조립하지 않는다", () => {
  const names = ACTIONS.map((one) => one.subcommand);
  assert.equal(new Set(names).size, names.length, "한 subcommand 를 두 tool 이 쓴다");
  assert.ok(!names.includes("monitor"), "Agent 표면이 monitor 를 노출한다");

  const cycle = ACTIONS.find((one) => one.name === "gil_cycle");
  assert.deepEqual(argvFor(cycle, { action: "open", kind: "experiment" }).argv,
    ["cycle", "open", "experiment"]);
  assert.ok(argvFor(cycle, { action: "restart" }).error, "허용하지 않은 동작을 통과시켰다");
  assert.ok(argvFor(cycle, {}).error, "필수 인자 없이 통과시켰다");

  const status = ACTIONS.find((one) => one.name === "gil_status");
  // 인자를 받지 않는 명령은 무엇을 넣어도 argv 가 늘지 않는다.
  assert.deepEqual(argvFor(status, { kind: "x", topic: "y" }).argv, ["status"]);
});

test("bridge 는 `.gil` 을 직접 읽거나 쓰지 않는다", async () => {
  for (const one of ["core.mjs", "actions.mjs", "server.mjs", "capability.mjs"]) {
    const body = await readFile(join(HERE, one), "utf8");
    const writes = body.match(/writeFile|appendFile|\.gil\/|state\.yaml/g) || [];
    assert.deepEqual(writes, [], `${one} 가 저장소 파일에 손을 댄다: ${writes}`);
  }
});

// ── ③ 실린 Core 와 저장소 Core 가 같은 일을 하는가 ──────────────────

async function fingerprint(root) {
  const seen = [];
  async function walk(dir, shown) {
    let entries;
    try { entries = await readdir(dir, { withFileTypes: true }); } catch { return; }
    for (const entry of entries.sort((a, b) => a.name.localeCompare(b.name))) {
      // 잠금 파일은 **실행 중에만** 산다. 세계의 일부가 아니다.
      if (entry.name === "project.lock") continue;
      const here = join(dir, entry.name);
      const name = `${shown}/${entry.name}`;
      if (entry.isDirectory()) await walk(here, name);
      else seen.push(`${name}:${createHash("sha256").update(await readFile(here)).digest("hex")}`);
    }
  }
  await walk(root, "");
  return seen.join("\n");
}

/** GIL 의 canonical 한 바퀴 — Interview 를 성공으로 닫고, 그 자식 Experiment 도 닫는다.
 *
 *  Grammar 는 Core 가 소유한다. 이 표는 `gil open --help` 와 `gil close --help` 가 각 자리에서
 *  요구한 칸을 그대로 옮긴 것이다 — JavaScript 가 Report 문법을 따로 정하지 않는다. */
const CONTRACT = "objective: 목적\nnext_action: 다음 행동\ndone_when: 끝나는 조건\n";
const CANONICAL = [
  ["start", [], null],
  // ── Cycle 1 · interview ──
  ["open", [], CONTRACT],
  ["close", [], "question: 무엇을 원하는가\nchoices: A 또는 B\nresponse: A\n"],
  ["open", [], CONTRACT],
  ["close", [], "interpretation: A 를 원한다\nunresolved: 없음\n"],
  ["open", ["synthesis"], CONTRACT],
  ["close", [], "statement: A 로 간다\nbasis_refs: |\n  step:C1/S1\n  step:C1/S2\napproved: yes\n"],
  ["open", [], CONTRACT],
  ["close", [], "verdict: success\nlesson: 배운 것\nsynthesis_ref: step:C1/S3\n"
    + "next_direction.action: close_cycle\nnext_direction.reason: Interview 를 닫는다\n"],
  ["close", [], "verdict: success\noutcome_ref: step:C1/S4\nhandoff_summary: A 로 가기로 했다\n"
    + "next_direction.action: open_child\nnext_direction.reason: 실험으로 간다\n"],
  // ── Cycle 2 · experiment (Cycle 은 계약 없이 연다) ──
  ["open", ["experiment"], null],
  ["open", [], CONTRACT],
  ["close", [], "problem: 무엇을 풀까\nsuccess_condition: 이러면 성공\n"],
  ["open", [], CONTRACT],
  ["close", [], "hypothesis: 이것이 원인이다\nrationale: 근거\nguardrail: 넘지 않을 선\n"],
  ["open", [], CONTRACT],
  ["close", [], "execution: 이렇게 했다\nresult: 이렇게 나왔다\n"],
  ["open", [], CONTRACT],
  ["close", [], "hypothesis_fit: yes\nproblem_solved: yes\nsuccess_condition_met: yes\n"
    + "guardrail_triggered: no\ninterpretation: 가설대로였다\n"],
  ["open", ["outcome"], CONTRACT],
  ["close", [], "verdict: success\nlesson: 실험에서 배운 것\n"
    + "next_direction.action: close_cycle\nnext_direction.reason: 성공으로 닫는다\n"],
  ["close", [], "verdict: success\noutcome_ref: step:C2/S5\nhandoff_summary: 실험이 성공했다\n"
    + "next_direction.action: open_child\nnext_direction.reason: 다음으로 간다\n"],
  // ── 읽기 ──
  ["status", [], null],
  ["context", [], null],
  ["story", [], null],
  // ── 거절 경로도 함께 — 성공만 견주면 아무것도 증명하지 못한다 ──
  ["close", [], null],
  ["restore", ["extra"], null],
  ["revisit", ["extra"], null],
  ["cycle", ["nonsense"], null],
  ["help", ["no-such-topic"], null],
];

async function walkThrough(exe) {
  const root = await mkdtemp(join(tmpdir(), "gil-equiv-"));
  const seen = [];
  for (const [sub, args, input] of CANONICAL) {
    const got = await callCore(exe, [sub, ...args], { input, cwd: root });
    seen.push({ sub, args, ok: got.ok, exit_code: got.exit_code, said: got.said, problem: got.problem });
  }
  const bytes = await fingerprint(join(root, ".gil"));
  await rm(root, { recursive: true, force: true });
  // **정규화하지 않는다.** 두 조건은 서로 다른 임시 폴더에서 도는데도 말이 같아야 한다 —
  // Core 는 자리를 상대 경로로만 말하기 때문이다. 여기서 임시 경로를 지워 주면 그 계약이
  // 깨져도 시험이 통과한다.
  return { seen, bytes };
}

test("실린 Core 가 저장소에서 지은 Core 와 같은 일을 한다", async (t) => {
  if (!existsSync(BUNDLED)) return t.skip("core 가 아직 실리지 않았다 — make-core.sh");
  if (!existsSync(CONTROL)) return t.skip("control 이 없다 — cargo build --release -p gil");

  const control = await walkThrough(CONTROL);
  const bundled = await walkThrough(BUNDLED);

  assert.equal(bundled.seen.length, control.seen.length);
  for (let i = 0; i < control.seen.length; i += 1) {
    const a = control.seen[i], b = bundled.seen[i];
    const where = `${a.sub} ${a.args.join(" ")}`.trim();
    assert.equal(b.exit_code, a.exit_code, `${where}: 종료 코드가 다르다`);
    assert.equal(b.ok, a.ok, `${where}: 성패가 다르다`);
    assert.equal(b.said, a.said, `${where}: stdout 이 다르다`);
    assert.equal(b.problem, a.problem, `${where}: stderr 가 다르다`);
  }
  // 성공과 거절이 **둘 다** 실제로 일어났는지 — 한쪽만이면 아무것도 증명하지 못한다.
  assert.ok(control.seen.some((one) => one.ok === false), "거절 경로를 밟지 않았다");
  assert.ok(control.seen.filter((one) => one.ok === true).length >= 20, "성공 경로가 너무 짧다");

  // 두 Cycle 이 정말 **성공으로 닫혔는지** — 여기까지 가지 못하면 동치는 빈 주장이다.
  const closes = control.seen.filter((one) => one.sub === "close" && one.ok);
  assert.ok(closes.length >= 11, `close 성공이 ${closes.length}번뿐이다`);
  const status = control.seen.find((one) => one.sub === "status" && one.ok);
  assert.match(status.said, /Cycle 2 experiment \(닫힘 · success\)/, `실험이 성공으로 닫히지 않았다:\n${status.said}`);
  assert.match(status.said, /Cycle 2개/, "두 Cycle 을 걷지 않았다");
  // Journey 가 걸음마다 자랐는지. **Step** close 가 한 칸씩 민다 — Cycle close 는 밀지 않는다.
  // 이 한 바퀴의 Step close 는 C1 의 넷과 C2 의 다섯, 모두 아홉이다.
  const journeys = new Set(
    control.seen.flatMap((one) => [...one.said.matchAll(/journey:\S+/g)].map((m) => m[0])),
  );
  assert.equal(journeys.size, 9, `Journey revision 이 ${journeys.size}개다 — Step close 는 아홉이었다`);
  // 그리고 bundled 도 **같은 이름들**을 말해야 한다.
  const theirs = new Set(
    bundled.seen.flatMap((one) => [...one.said.matchAll(/journey:\S+/g)].map((m) => m[0])),
  );
  assert.deepEqual([...theirs].sort(), [...journeys].sort(), "Journey 가 다르다");

  // 그리고 **세계가 같다** — state 도 Artifact 객체도 바이트까지.
  assert.equal(bundled.bytes, control.bytes, "`.gil` 바이트가 다르다");
  assert.ok(control.bytes.includes("/state.yaml:"), "state.yaml 이 없다");
  assert.ok(control.bytes.includes("/artifacts/"), "Artifact 객체가 없다");
});

test("실패한 action 은 `.gil` 을 한 바이트도 바꾸지 않는다", async (t) => {
  if (!existsSync(BUNDLED)) return t.skip("core 가 아직 실리지 않았다");
  const root = await mkdtemp(join(tmpdir(), "gil-fail-"));
  await callCore(BUNDLED, ["start"], { cwd: root });
  const before = await fingerprint(join(root, ".gil"));

  for (const [sub, args, input] of [
    ["close", [], null],
    ["restore", ["extra"], null],
    ["revisit", ["extra"], null],
    ["cycle", ["nonsense"], null],
  ]) {
    const got = await callCore(BUNDLED, [sub, ...args], { input, cwd: root });
    assert.equal(got.ok, false, `${sub} 가 성공했다 — 시나리오가 틀렸다`);
  }
  assert.equal(await fingerprint(join(root, ".gil")), before, "실패가 세계를 바꿨다");
  await rm(root, { recursive: true, force: true });
});

test("한 번의 호출이 Core 를 한 번만 띄운다", async (t) => {
  if (!existsSync(BUNDLED)) return t.skip("core 가 아직 실리지 않았다");
  // 부를 때마다 자기 호출을 한 줄씩 적는 가짜 core 로 센다.
  const { root, exe } = await fakeCore(`#!/bin/sh
echo "$@" >> "$(dirname "$0")/calls.txt"
exit 0
`);
  const log = join(dirname(exe), "calls.txt");
  await callCore(exe, ["status"], {});
  const lines = (await readFile(log, "utf8")).trim().split("\n");
  assert.deepEqual(lines, ["status"], "한 번의 호출이 여러 번 실행됐다");
  await rm(root, { recursive: true, force: true });
});
