// **공용 Skill 의 시험** — `node --test`
//
// Skill 은 Agent 가 언제 무엇을 부를지 정하는 유일한 자리다. T1.1 은 도구·schema 를 바꾸지 않고
// 이 문서 하나로 「프로젝트 시작 → Monitor 표시 → 첫 Interview」를 한 요청에 잇는다. 그러니
// 여기서 지키는 것은 그 문서가 실제 도구 표·Core 의 거절 문구·Monitor 거절 code 에 어긋나지
// 않고, 도구 응답을 화면 성공으로 단정하는 문장이 다시 들어오지 않는다는 사실이다.
//
//   1. Skill 이 부르는 도구 이름은 전부 실제 19개 tool 표에 있다.
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
  assert.equal(TOOLS.length, 19);
  const named = new Set([...SKILL.matchAll(/`((?:gil|show_gil)_[a-z_]+)(?:\(|`)/g)].map((m) => m[1]));
  for (const name of named) {
    if (name.startsWith("gil_migration_")) {
      const catalog=JSON.parse(readFileSync(join(REPO,"src","mcp","migration-tools.json"),"utf8"));
      assert.ok(catalog.some(t=>t.name===name));
      assert.ok(section("## Existing projects — separately approved image migration").includes(name));
      continue;
    }
    if (name === "gil_report_visuals") {
      assert.ok(section("## Visual reports — capability-gated authoring").includes(name));
      continue; // Explicit opt-in authoring route, never a promise about the default catalog.
    }
    if (name === "gil_prepare_workspace") {
      // Windows development surface only (src/mcp/workspace.rs); mentioned solely in the
      // unsupported-location paragraph, which is entered only after that refusal code.
      // Allowed only in the unsupported-location paragraph and the protection-lost paragraph
      // that hands over to it; never in the normal start flow.
      const paragraph = SKILL.slice(SKILL.indexOf("**Unsupported location at start"), SKILL.indexOf("Report three things separately"));
      assert.ok(paragraph.includes(name));
      assert.ok(!SKILL.replace(paragraph, "").includes(name), "gil_prepare_workspace must not appear in the normal start flow");
      continue;
    }
    assert.ok(TOOLS.includes(name), `Skill 이 없는 도구를 부른다 — ${name}`);
  }
  for (const owed of ["gil_start", "gil_monitor_prepare", "show_gil_monitor", "gil_open", "gil_context"]) {
    assert.ok(named.has(owed), `시작 흐름에 필요한 도구가 문서에 없다 — ${owed}`);
  }
});

test("시각 작성은 실제 확장 schema와 조회 도구를 모두 확인하고 이관을 금지한다", () => {
  const flow = section("## Visual reports — capability-gated authoring");
  const schema = JSON.parse(readFileSync(join(REPO, "src/mcp/authoring-schema.json"), "utf8"));
  for (const key of ["visuals", "expected_owner"]) {
    assert.ok(schema.close.properties[key]);
    assert.ok(flow.includes(`\`${key}\``));
  }
  assert.match(flow, /Otherwise stop/);
  assert.match(flow, /or migrate an existing Project/);
  assert.match(flow, /visual_upgrade_required/);
  assert.match(flow, /closed|Closed/);
});

test("시각 작성 지침은 관측·세 선언·불확실 close·재사용·표시를 구분한다", () => {
  const flow = section("## Visual reports — capability-gated authoring");
  const schema = JSON.parse(readFileSync(join(REPO, "src/mcp/authoring-schema.json"), "utf8"));
  for (const variant of schema.close.properties.visuals.properties.declaration.oneOf) {
    assert.ok(flow.includes(`\`${variant.properties.state.const}\``));
  }
  assert.match(flow, /Code that could generate a picture is not an attached picture/);
  assert.match(flow, /not automatically repeat\s+close/);
  assert.match(flow, /not merely\s+`confirmed: true`/);
  assert.match(flow, /This is a citation, not a new/);
  assert.match(flow, /Storage confirmation is not display confirmation/);
  assert.match(flow, /Never keep one Cycle open/);
  assert.match(flow, /ask before sensitive capture or transmission/);
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

test("시작 응답의 준비된 scope를 표시하고 옛 서버만 별도 prepare를 사용한다", () => {
  const flow = section(START);
  const order = ["`gil_start(project_root)`", "`show_gil_monitor(scope_id)`", "`gil_open`"]
    .map((call) => { const at = flow.indexOf(call); assert.notEqual(at, -1, `흐름에 없다 — ${call}`); return at; });
  for (let i = 1; i < order.length; i++) assert.ok(order[i - 1] < order[i], "시작 흐름의 순서가 뒤집혔다");
  assert.match(flow, /Older servers without that handoff need\s+`gil_monitor_prepare\(project_root\)`/);
  assert.match(flow, /preparation failed[^;]*report that failure and do not start again/);
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
  assert.match(flow, /creation is not confirmed by this answer/);
  assert.doesNotMatch(flow, /this start request did not create anything/);
  assert.match(flow, /Never automatically retry `gil_start`/);
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

test("ok:false 뒤 처리 — 성공 추정 금지, 사유 읽기, 보정 가능 입력만 같은 동작 1회, 권한·호환 거절은 반복하지 않는다", () => {
  const body = SKILL;
  assert.match(body, /\*\*After `ok: false`, handle the refusal before anything else\.\*\*/);
  assert.match(body, /never treat it as success, and never go on to the next step, the next Cycle or\s+implementation/);
  assert.match(body, /\*\*Do not assume the transition succeeded on a refused call\*\*/);
  assert.doesNotMatch(body, /Only `ok: true` moves the position/, "위치 이동을 ok:true 로만 설명하는 문장은 쓰지 않는다");
  assert.match(body, /another worker may have changed it, so when the\s+position is in doubt read `gil_status` \/ `gil_context` first/);
  assert.match(body, /\*\*Correctable input\*\*[\s\S]*?send \*\*one\s+corrected call of the same action\*\*, and only if that action is still valid at the current\s+position/);
  assert.match(body, /Do not send a different action instead/);
  assert.match(body, /\*\*Permission, compatibility or approval refusals\*\*[\s\S]*?do not repeat the call/);
  assert.match(body, /\*\*Busy, lost connection or an uncertain close\*\* keep their own rules/);
  // 기존 별도 규칙은 그대로다.
  assert.match(body, /Never automatically retry `gil_start`/);
  assert.match(section("## Visual reports — capability-gated authoring"), /do not automatically repeat\s+close\*\*, even for a busy\/connection error/);
});

test("Synthesis 는 사람의 승인을 기다린다 — approved:yes 를 이전 요청·침묵·Agent 판단으로 적지 않는다 (Cycle Model §16)", () => {
  const flow = section("## Starting a project and watching it — one request");
  assert.match(flow, /\*\*Synthesis waits for the human\.\*\*/);
  assert.match(flow, /present the concrete `statement`[\s\S]*?ask the user\s+whether it is right; then \*\*end your turn and wait for their answer\*\*/);
  assert.match(flow, /never derive it from an\s+earlier request to build something, from silence, from the user's absence, or from your own\s+judgement/);
  assert.match(flow, /Until the user answers, the Synthesis and its Interview stay\s+open; do not close them, do not open an Experiment or any other Cycle, and do not start\s+implementing/);
  assert.match(flow, /If the user says no or changes the proposal, record their words and revise within\s+the same Interview/);
});

test("close 입력 형태와 -32602 뒤 절차 — 열린 Verify 의 정상 구현은 막지 않는다", () => {
  const closing = section("## Closing a step — two input shapes, never half of one");
  // 두 형태가 구분되고, 이미지 없는 format-5 Verify 도 typed close 를 쓴다.
  assert.match(closing, /\*\*Ordinary close\*\*/); assert.match(closing, /\*\*Typed close\*\*/);
  assert.match(closing, /open Verify of a format-5 project, always/);
  assert.match(closing, /format-4 project \*\*including its Verify\*\*/, "format-4 Verify 는 ordinary close 다");
  assert.match(closing, /any format-5 non-Verify step or Cycle report that cites stored images/, "기존 이미지 인용 예외가 없다");
  assert.match(closing, /empty `attachments`\/`uses` and a\s+`declaration` of `not_needed`[^.]*or `missing`/);
  assert.match(closing, /refuses an ordinary text close of a format-5 Verify/);
  // -32602 는 「그 호출이 실행되지 않았다」이지 Step 상태의 단정이 아니다. 상태는 조회로 확인한다.
  assert.match(closing, /that call was not executed/);
  assert.match(closing, /does\s+not tell you what state the project is in/);
  assert.match(closing, /gil_status` \/ `gil_context`/);
  assert.doesNotMatch(closing, /the project is untouched, the\s+step is still open/, "모든 -32602 를 열린 Step 으로 일반화하면 안 된다");
  // 열린 Step 전체의 파일 작성 금지는 없다. 인터뷰·거절 무시 구현만 금지하고, 열린 Verify 의 구현·검증·이미지 생성은 허용한다.
  assert.doesNotMatch(closing, /While a step is open[\s\S]*?do not create or edit the user's deliverable/);
  assert.match(closing, /open Interview step is for asking, interpreting and proposing/);
  assert.match(closing, /do not create or edit the user's deliverable[\s\S]*?while the Interview is\s+still open/);
  assert.match(closing, /open\s+\*\*Verify\*\* is where ordinary implementation, test runs, screenshots and chart generation happen/);
  assert.match(closing, /do not treat a rejected or refused close as if the step had been closed/);
  assert.match(closing, /never record steps or reports after the fact/);
  // 불확실 응답 뒤 close 무조건 재시도 금지는 그대로다.
  assert.match(closing, /no-automatic-retry rule below still applies to \*\*uncertain\*\*\s+responses/);
  assert.match(section("## Visual reports — capability-gated authoring"), /do not automatically repeat\s+close\*\*, even for a busy\/connection error/);
});

test('unsupported location at start: notice, approval-gated workspace preparation, no auto-connect', () => {
  const skill = readFileSync(new URL('./skills/gil-companion/SKILL.md', import.meta.url), 'utf8');
  const section = skill.slice(skill.indexOf('**Unsupported location at start'), skill.indexOf('Report three things separately'));
  assert.ok(section.length > 200);
  assert.match(section, /code: unsupported_location/);
  assert.match(section, /not a statement that OneDrive[\s\S]*caused an error/);
  assert.match(section, /gil_prepare_workspace\(name, confirm: false\)/);
  assert.match(section, /gil_prepare_workspace\(name, confirm: true,\s+approved_path:/);
  assert.match(section, /\*\*end your turn\*\* and wait/);
  assert.match(section, /other sync\s+products are not detected/);
  assert.match(section, /outside the roots OneDrive declares, no cloud-sync attributes/);
  assert.match(section, /Creating the folder is not connecting it/);
  assert.match(section, /never claim it is connected because the path exists/);
  assert.match(section, /never\s+elevate, never adjust permissions/);
});
