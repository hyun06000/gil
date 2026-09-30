// **공용 Skill 의 시험** — `node --test`
//
// Skill 은 Agent 가 언제 무엇을 부를지 정하는 유일한 자리다. T1.1 은 도구·schema 를 바꾸지 않고
// 이 문서 하나로 「프로젝트 시작 → Monitor 표시 → 첫 Interview」를 한 요청에 잇는다. 그러니
// 여기서 지키는 것은 그 문서가 실제 도구 표·Core 의 거절 문구·Monitor 거절 code 에 어긋나지
// 않고, 도구 응답을 화면 성공으로 단정하는 문장이 다시 들어오지 않는다는 사실이다.
//
//   1. Skill 이 부르는 도구 이름은 전부 실제 17개 tool 표에 있다.
//   2. 시작 요청 하나가 start → prepare → show → 첫 Interview 순서로 이어진다.
//   3. 시작 성공·Monitor 데이터 준비·실제 화면을 가르고, 화면은 사용자 확인 전까지 미확인이다.
//   4. 이미 시작된 Project 의 거절 문구가 Core 가 실제로 말하는 글자와 같다.
//   5. 시작 실패는 기존 기록이 없다는 증거가 아니며 원본을 보존한다.
//   6. 화면 재시도는 일시적 답에만 한 번, `gil_start` 는 다시 부르지 않는다.
//   7. fullscreen 거절·inline 선택·단일 열린 Cycle·읽기 전용 경계가 남아 있다.

import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";

const PLUGIN = import.meta.dirname;
const REPO = resolve(PLUGIN, "..", "..");
const SKILL = readFileSync(join(PLUGIN, "skills", "gil-companion", "SKILL.md"), "utf8");
const TOOLS = JSON.parse(readFileSync(join(REPO, "src", "mcp", "tools.json"), "utf8")).map((t) => t.name);
const START = "## Starting a project and watching it — one request";

const frontmatter = () => {
  const match = SKILL.match(/^---\n([\s\S]*?)\n---\n/);
  assert.ok(match, "frontmatter 가 없다");
  return Object.fromEntries(match[1].split("\n").map((line) => {
    const at = line.indexOf(":");
    return [line.slice(0, at).trim(), line.slice(at + 1).trim()];
  }));
};

const section = (heading) => {
  const start = SKILL.indexOf(`\n${heading}\n`);
  assert.notEqual(start, -1, `절이 없다 — ${heading}`);
  const rest = SKILL.slice(start + heading.length + 2);
  const end = rest.search(/\n#{1,2} /);
  return end === -1 ? rest : rest.slice(0, end);
};

/** 따옴표 안의 예시 문구를 지운 본문 — 「이렇게 말하지 말라」의 인용을 단정으로 세지 않는다. */
const unquoted = (body) => body.replace(/"[^"\n]*"/g, '""');

test("Skill 의 identity 는 그대로이고 description 이 시작 요청을 부른다", () => {
  const meta = frontmatter();
  assert.equal(meta.name, "gil-companion");
  assert.match(meta.description, /start a GIL project/i, "시작 요청이 이 Skill 을 부르지 못한다");
  assert.match(meta.description, /Monitor/, "Monitor 표시가 description 에서 빠졌다");
});

test("Skill 이 부르는 도구 이름은 전부 실제 tool 표에 있다", () => {
  assert.equal(TOOLS.length, 17);
  const named = new Set([...SKILL.matchAll(/`((?:gil|show_gil)_[a-z_]+)(?:\(|`)/g)].map((m) => m[1]));
  for (const name of named) {
    assert.ok(TOOLS.includes(name), `Skill 이 없는 도구를 부른다 — ${name}`);
  }
  for (const owed of ["gil_start", "gil_monitor_prepare", "show_gil_monitor", "gil_open", "gil_context"]) {
    assert.ok(named.has(owed), `시작 흐름에 필요한 도구가 문서에 없다 — ${owed}`);
  }
});

test("Skill 이 드는 Monitor 거절 code 는 전부 Rust adapter 가 실제로 내는 것이다", () => {
  const rust = readFileSync(join(REPO, "src", "mcp", "monitor.rs"), "utf8")
    + readFileSync(join(REPO, "src", "mcp", "bindings.rs"), "utf8");
  const codes = new Set([...rust.matchAll(/Refusal::new\("([a-z_]+)"/g)].map((m) => m[1]));
  const flow = section(START);
  const named = [...flow.matchAll(/`([a-z_]+)`/g)].map((m) => m[1])
    .filter((word) => /^(busy|not_a_project|unsupported_format|project_moved)$/.test(word));
  assert.ok(named.length >= 4, "재시도 규칙이 구체적인 거절 code 를 들지 않는다");
  for (const code of named) assert.ok(codes.has(code), `adapter 에 없는 code 를 든다 — ${code}`);
  assert.ok([...codes].some((c) => c.startsWith("settings_")), "settings_* 거절이 adapter 에서 사라졌다");
  assert.match(flow, /`settings_\*`/);
});

test("시작 요청 하나가 start → prepare → show → 첫 Interview 순서로 이어진다", () => {
  const flow = section(START);
  const order = ["`gil_start(project_root)`", "`gil_monitor_prepare(project_root)`", "`show_gil_monitor(scope_id)`", "`gil_open`"]
    .map((call) => { const at = flow.indexOf(call); assert.notEqual(at, -1, `흐름에 없다 — ${call}`); return at; });
  for (let i = 1; i < order.length; i++) assert.ok(order[i - 1] < order[i], "시작 흐름의 순서가 뒤집혔다");
  assert.match(flow, /Do not wait for a separate "Monitor 열어 줘"/, "별도 Monitor 요청을 기다리지 않는다는 문장이 없다");
  assert.match(flow, /never guess one/, "폴더를 추측하지 않는다는 문장이 없다");
  assert.match(flow, /do not ask them to install anything/, "Companion 설치를 요구하지 않는다는 문장이 없다");
  assert.match(flow, /requests\s+fullscreen once by itself/, "기존 자동 fullscreen 요청을 재사용한다는 문장이 없다");
  assert.match(flow, /from this first step on/, "첫 Step 부터 갱신된다는 문장이 없다");
});

test("시작 성공·Monitor 데이터 준비·실제 화면을 가르고, 화면은 사용자 확인 전까지 미확인이다", () => {
  const flow = section(START);
  for (const owed of ["**Start**", "**Monitor data**", "**Display**"]) assert.ok(flow.includes(owed), `구분이 없다 — ${owed}`);
  assert.match(flow, /the display was requested and its data is\s+ready/, "「요청했고 데이터가 준비됐다」가 없다");
  assert.match(flow, /not proof anything is visible/);
  assert.match(flow, /unconfirmed until the user says so/, "화면이 미확인이라는 문장이 없다");
  assert.match(flow, /still unconfirmed/);
  assert.match(flow, /Only after the user confirms[^.]*may you report that the Monitor is showing/, "사용자 확인 뒤에만 표시 성공을 보고한다는 문장이 없다");
});

test("회귀 방지: 도구 응답을 화면 성공으로 단정하는 문장이 없다", () => {
  const body = unquoted(SKILL.slice(SKILL.indexOf("\n---\n", 4) + 5)).replace(/\s+/g, " ");
  // 문장 단위로 본다. 조건(사용자 확인 뒤에만)이나 금지(never/do not)를 담은 문장은 단정이 아니다.
  const guarded = /\b(only after the user|only when the user|never|do not|must not|not proof|unconfirmed|cannot|is not)\b/i;
  const claims = [
    /\b(say|tell|report|announce|confirm|state)\b[^.]*\bthe Monitor (was|is|has been) (opened|open|showing|visible|displayed)\b/i,
    /\bSay the Monitor was opened\b/i,
    /\bfullscreen (succeeded|is on|is active|has opened)\b/i,
    /\b(you|agent) (opened|have opened|showed) the Monitor\b/i,
  ];
  for (const sentence of body.split(/(?<=[.!?])\s+/)) {
    if (guarded.test(sentence)) continue;
    for (const claim of claims) {
      const hit = sentence.match(claim);
      assert.equal(hit, null, `화면 성공을 단정하는 문장이 들어왔다 — ${sentence.trim()}`);
    }
  }
  // 그 검사가 실제로 무는지 — 옛 문장을 넣으면 걸려야 한다.
  const relapse = "Say the Monitor was opened and ask them to press 모니터 펼치기.";
  assert.ok(!guarded.test(relapse) && claims.some((claim) => claim.test(relapse)), "회귀 검사가 옛 문장을 놓친다");
  // 「보이지 않는다」를 말하는 자리는 그대로 있어야 한다.
  assert.match(SKILL, /is \*\*not\*\* proof of persistent display/);
});

test("이미 시작된 Project 의 거절 문구는 Core 가 실제로 말하는 글자다", () => {
  const core = readFileSync(join(REPO, "src", "command.rs"), "utf8");
  assert.ok(core.includes('"이미 걷고 있다 ({})."'), "Core 의 거절 문구가 바뀌었다 — Skill 도 함께 고친다");
  const flow = section(START);
  assert.match(flow, /「이미 걷고 있다」/, "Skill 이 Core 의 거절 문구를 모른다");
  assert.match(flow, /never reinitialised/);
  assert.match(flow, /Do not delete or move `\.gil`, do not\s+retry `gil_start`, and do not open another Cycle/);
});

test("시작 실패는 기존 기록이 없다는 증거가 아니며 원본을 보존한다", () => {
  const flow = section(START);
  assert.match(flow, /A start failure is not a display failure, and not proof that no record exists/);
  assert.match(flow, /this start request did not create anything/);
  assert.match(flow, /Do not conclude that the folder has no GIL record/);
  assert.match(flow, /do not clear, move or convert it/);
  assert.match(flow, /whether an earlier record exists is unknown from this\s+answer/);
  assert.doesNotMatch(unquoted(flow), /nothing was started, so there is nothing to show/, "「없다」로 읽히는 옛 문장이 남아 있다");
});

test("화면 재시도는 일시적 답에만 한 번이고 gil_start 를 반복하지 않는다", () => {
  const flow = section(START);
  assert.match(flow, /Display retries are limited/);
  assert.match(flow, /at most once, and only for a transient answer such as `busy` or a broken\s+connection/);
  assert.match(flow, /Do not repeat them for a lasting refusal/);
  assert.match(flow, /or when the Host does not advertise the display/);
  assert.match(flow, /ask the user instead of calling `show_gil_monitor` again/);
  assert.match(flow, /every call can add another card/);
  assert.match(flow, /returns the same scope; it does not\s+create a second project, Cycle or binding/);
  assert.match(flow, /Never repeat `gil_start`/);
  assert.doesNotMatch(unquoted(flow), /retry (only )?those two tools/, "무제한 재시도의 옛 문장이 남아 있다");
});

test("fullscreen 거절·inline 선택·단일 열린 Cycle·읽기 전용 경계가 남아 있다", () => {
  const flow = section(START);
  assert.match(flow, /Fullscreen can be refused/);
  assert.match(flow, /do\s+not claim fullscreen/);
  assert.match(flow, /\*\*모니터 펼치기\*\*/);
  assert.match(flow, /open the Companion only if they choose it/);
  assert.match(flow, /The user's inline choice wins/);
  assert.match(flow, /do not\s+request fullscreen again/);
  assert.match(flow, /One open Cycle at a time/);
  assert.match(flow, /do not leave it suspended to open a new one/);
  assert.match(flow, /The Monitor stays read-only/);
  // 기존 Monitor 절의 규칙도 그대로다 — 시작 흐름이 그것을 대체하지 않는다.
  assert.match(SKILL, /Native Companion is never launched automatically/);
  assert.match(SKILL, /A user returning inline or choosing vertical must not be overridden/);
});
