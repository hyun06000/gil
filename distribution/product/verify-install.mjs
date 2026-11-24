// Maintainer verification of the product build THROUGH A GIVEN BINARY — the packaged or installed
// file itself, never a separately built one. Synthetic temporary projects and an isolated HOME
// only; nothing here touches a real Host setting or a user's project.
//
// What it is evidence of: the named bytes answer the protocol, start each kind of new project,
// keep the two kinds apart, and read the same records from a fresh process.
// What it is not: a person's approval (the App calls below are harness input), a rendered Host
// screen, power-loss durability, or the failure-injection paths, which need a debug build and are
// separate evidence.
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {createHash, randomUUID} from 'node:crypto';
import {lstat, mkdir, mkdtemp, readFile, readdir, readlink, realpath, rm, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {dirname, join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {deflateSync} from 'node:zlib';
import {checkCore, compatibility} from '../codex/compatibility.mjs';
import {checkVersionAgreement} from '../codex/build-version.mjs';
import {checkProductTools, withoutRecoveryTools} from '../codex/tool-contract.mjs';
import {LINUX, PROFILE, verifyLinuxTree} from './artifact.mjs';
import {Mcp, mustRefuse, mustSucceed, said, succeeded} from './mcp-client.mjs';

const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const CONTRACT = 'objective: 합성 작업 범위를 합의한다\nnext_action: 사용자에게 묻는다\ndone_when: 사용자의 답을 받았다\n';
const QUESTION = 'question: 어떤 합성 작업인가\nchoices: 보고서 / 비교\nresponse: 합성 자료로 항목별 보고서를 작성해줘\n';
const INTERPRETATION = 'interpretation: 항목별 파일 증거를 남기며 보고서를 만든다\nunresolved: 없음\n';
const PLAN_DOCUMENT = 'plan/approved-plan.md';

// A one-pixel 8-bit grayscale PNG, built here so the check carries no image file.
export function png(shade) {
  const table = Array.from({length: 256}, (_, n) => { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c >>> 0; });
  const crc = bytes => { let c = 0xffffffff; for (const b of bytes) c = table[(c ^ b) & 0xff] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0; };
  const chunk = (type, data) => {
    const body = Buffer.concat([Buffer.from(type, 'latin1'), data]);
    const out = Buffer.alloc(8 + data.length + 4);
    out.writeUInt32BE(data.length, 0); body.copy(out, 4); out.writeUInt32BE(crc(body), 8 + data.length);
    return out;
  };
  const header = Buffer.alloc(13);
  header.writeUInt32BE(1, 0); header.writeUInt32BE(1, 4); header[8] = 8; // 1x1, depth 8, grayscale, defaults
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk('IHDR', header),
    chunk('IDAT', deflateSync(Buffer.from([0, shade]))), chunk('IEND', Buffer.alloc(0))]);
}

// Every entry below `root`: folders, files by digest, links by target. For "nothing changed".
async function tree(root) {
  const out = {};
  async function walk(at, prefix) {
    for (const name of (await readdir(at)).sort()) {
      const path = join(at, name), rel = prefix ? `${prefix}/${name}` : name, info = await lstat(path);
      if (info.isSymbolicLink()) out[rel] = `link:${await readlink(path)}`;
      else if (info.isDirectory()) { out[rel] = 'dir'; await walk(path, rel); }
      else out[rel] = sha256(await readFile(path));
    }
  }
  await walk(root, '');
  return out;
}

function draft(ids, basis) {
  return {scope: {goal: '합성 자료 기반 항목별 보고서', in_scope: '제공된 합성 자료의 비교', out_of_scope: '실시간 데이터·실제 개인정보',
    constraints: '외부 네트워크 없음', success_criteria: '항목마다 파일 증거가 남는다'},
  items: ids.map(id => ({id, question: `항목 ${id}을 합성 자료로 확인한다`, done_when: `evidence 파일에 항목 ${id} 결과가 저장된다`})),
  synthesis: {statement: '합성 자료로 항목을 순서대로 확인하고 파일 증거를 남긴다', basis_refs: basis.join('\n'), exploration_checklist: ids.join('\n')}};
}
// The step an open receipt names, read without depending on the receipt's language.
function stepRef(result, kind) {
  const line = said(result).split('\n').find(text => text.includes(` · ${kind}`) && /step:(?:C\d+\/)+S\d+/.test(text));
  assert.ok(line, `no open receipt for ${kind}`);
  return line.match(/step:(?:C\d+\/)+S\d+/)[0];
}

class Project {
  constructor(binary, state, name) { this.binary = binary; this.state = state; this.name = name; }
  async create(files) {
    this.root = join(this.state, this.name);
    await mkdir(this.root);
    for (const [path, bytes] of Object.entries(files)) await this.write(path, bytes);
    this.env = {PATH: '/usr/bin:/bin', HOME: join(this.state, 'home'), TMPDIR: tmpdir(), GIL_MONITOR_STATE_DIR: join(this.state, 'bindings', this.name)};
    await this.connect();
    return this;
  }
  async connect() { this.mcp = new Mcp(this.binary, {cwd: this.state, env: this.env}); this.initialized = await this.mcp.initialize(); }
  async restart() { await this.mcp.close(); await this.connect(); }
  async write(path, bytes) { await mkdir(dirname(join(this.root, path)), {recursive: true}); await writeFile(join(this.root, path), bytes); }
  model(name, args = {}) { return this.mcp.call(name, {project_root: this.root, ...args}); }
  app(name, args = {}) { return this.mcp.call(name, {scope_id: this.scope, ...args}); }
  record() { return readFile(join(this.root, '.gil/state.yaml')); }
  async plan() { return mustSucceed(await this.model('gil_plan_read'), 'plan read').structuredContent; }

  // Opening Interview up to an approved plan, the way a Host reaches it: proposal, the App-only
  // review and decision (harness input here), the exact close report, Outcome, Cycle Report.
  async interview(ids) {
    const basis = [];
    for (const [kind, report] of [['question', QUESTION], ['interpretation', INTERPRETATION]]) {
      basis.push(stepRef(mustSucceed(await this.model('gil_open', {kind, contract: CONTRACT}), kind), kind));
      mustSucceed(await this.model('gil_close', {report}), `close ${kind}`);
    }
    const synthesis = stepRef(mustSucceed(await this.model('gil_open', {kind: 'synthesis', contract: CONTRACT}), 'synthesis'), 'synthesis');
    mustSucceed(await this.model('gil_plan_propose', {draft: draft(ids, basis)}), 'plan proposal');
    const review = mustSucceed(await this.app('gil_plan_review'), 'plan review');
    const capability = review._meta?.['gil/planApprovalToken'];
    assert.ok(capability, 'the approval capability is given to the App only');
    assert.ok(!JSON.stringify([review.content, review.structuredContent]).includes(capability), 'the capability leaked into model-visible content');
    const proposal = review.structuredContent.proposal;
    // No model tool can approve: the decision without the capability is refused.
    const forged = await this.mcp.raw('gil_plan_decide', {scope_id: this.scope, revision: proposal.revision, digest: proposal.digest, token: 'f'.repeat(64), decision: 'approve'});
    assert.ok(forged.error || !succeeded(forged.result), 'a guessed capability approved a plan');
    mustSucceed(await this.app('gil_plan_decide', {revision: proposal.revision, digest: proposal.digest, token: capability, decision: 'approve'}), 'plan decision');
    const plan = await this.plan();
    mustSucceed(await this.model('gil_close', {report: plan.close_report}), 'close synthesis with the exact approved report');
    const outcome = stepRef(mustSucceed(await this.model('gil_open', {kind: 'outcome', contract: CONTRACT}), 'outcome'), 'outcome');
    mustSucceed(await this.model('gil_close', {report: `verdict: success\nlesson: 합의한 요구를 기록했다\nsynthesis_ref: ${synthesis}\nnext_direction.action: close_cycle\nnext_direction.reason: 승인된 요구를 탐색한다\n`}), 'close outcome');
    mustSucceed(await this.model('gil_close', {report: `verdict: success\noutcome_ref: ${outcome}\nhandoff_summary: 합성 요구를 Monitor 응답으로 승인받았다\nnext_direction.action: open_child\nnext_direction.reason: 다음 단계로 간다\n`}), 'close interview');
    const read = await this.plan();
    assert.equal(read.contract, 'exploration', 'a Chain this build opens runs under the exploration contract');
    return read;
  }
  // `gil_open(kind: experiment)` linked to an approved requirement of the active revision.
  async experimentArgs(requirement) {
    const active = (await this.plan()).active_plan;
    assert.ok(active?.digest, 'an approved plan is active');
    return {kind: 'experiment', contract: `plan_revision: ${active.revision}\nplan_digest: ${active.digest}\nrequirement_id: ${requirement}\nrelevance: 요구 ${requirement}의 답에 필요한 관측이다\nreadiness: 입력 파일이 프로젝트에 있다\nuncertainty: 아직 판정하지 못한 한 가지를 줄인다\n`};
  }
}

async function imageReports(binary, state) {
  const p = await new Project(binary, state, 'image-reports').create({'data/dates.csv': 'id,date\n1,2026-01-02\n2,02/01/2026\n'});
  try {
    const started = mustSucceed(await p.model('gil_start', {start_kind: 'image_reports'}), 'image-report start');
    assert.equal(started.structuredContent.storage_format, 9, 'an image-report project of this build');
    assert.equal(started.structuredContent.next_call?.name, 'show_gil_monitor', 'the direct start hands the Monitor over');
    p.scope = started.structuredContent.next_call.arguments.scope_id;
    assert.ok((await p.record()).toString('utf8').startsWith('format: 9\n'));
    await assert.rejects(lstat(join(p.root, '.gil/start')), 'no tracked-scope preparation was begun');
    await assert.rejects(lstat(join(p.root, '.gilignore')), 'no tracking policy file was written');
    const shown = mustSucceed(await p.mcp.call('show_gil_monitor', {scope_id: p.scope}), 'show monitor');
    assert.ok(shown.structuredContent.view, 'a started project shows its graph');

    // The other kind never takes this project over; refusals change nothing.
    const before = await tree(p.root);
    assert.equal(mustRefuse(await p.model('gil_start', {start_kind: 'tracked_scope'}), 'tracked scope over image reports').structuredContent.code, 'already_started');
    mustRefuse(await p.model('gil_start'), 'a repeated start');
    assert.equal(mustRefuse(await p.model('gil_start_prepare', {rules: 'data/\n'}), 'a proposal over image reports').structuredContent.code, 'already_started');
    assert.deepEqual(await tree(p.root), before);

    await p.interview(['R1', 'R2']);
    mustSucceed(await p.model('gil_open', await p.experimentArgs('R1')), 'open the linked Experiment');
    const plan = await p.plan();
    assert.equal(plan.current_experiment?.requirement_id, 'R1');
    const cycle = plan.current_experiment.cycle_ref;
    stepRef(mustSucceed(await p.model('gil_open', {kind: 'define', contract: CONTRACT}), 'define'), 'define');
    mustSucceed(await p.model('gil_close', {report: 'problem: 지정한 자료에 허용 날짜 형식을 벗어나는 값이 존재하는가?\nsuccess_condition: 지정한 파일의 관측 증거가 위 명제의 긍정 답을 뒷받침한다. 근거 있는 부정 답이나 판정 미도달은 성공이 아니다\n'}), 'close define');
    mustSucceed(await p.model('gil_open', {kind: 'hypothesis', contract: CONTRACT}), 'hypothesis');
    mustSucceed(await p.model('gil_close', {report: 'hypothesis: 합성 자료만으로 판정할 수 있다\nrationale: 자료가 제공됐다\nguardrail: 외부 데이터를 쓰지 않는다\n'}), 'close hypothesis');
    const verify = {kind: 'step', ref: stepRef(mustSucceed(await p.model('gil_open', {kind: 'verify', contract: CONTRACT}), 'verify'), 'verify')};
    const picture = png(91);
    await p.write(PLAN_DOCUMENT, plan.document);
    await p.write('evidence/C2.md', '2행이 DD/MM/YYYY 형식이다: 존재한다\n');
    await p.write('report/irregular.png', picture);
    const report = `execution: 합성 자료를 관측했다\nresult: 관측한 값을 파일에 남겼다\nplan_document: ${PLAN_DOCUMENT}\nevidence_files: |\n  evidence/C2.md\n  report/irregular.png\n`;
    // A Verify of an image-report project states its visuals: the plain close is the wrong shape.
    const plain = await p.mcp.raw('gil_close', {project_root: p.root, report});
    assert.ok(plain.error || !succeeded(plain.result), 'a Verify closed without its visual declaration');
    const closed = mustSucceed(await p.model('gil_close', {report, expected_owner: verify, visuals: {
      attachments: [{key: 'chart', path: 'report/irregular.png', source_kind: 'generated', source_description: '합성 1픽셀 PNG, 실제 화면 캡처가 아니다'}],
      uses: [{source: {kind: 'new', key: 'chart'}, caption: '관측한 값', alt: '회색 픽셀 하나', purpose: '관측 결과 제시'}],
      declaration: {state: 'attached'}}}), 'close Verify with an attached PNG');
    const asset = closed.structuredContent.visuals.uses[0];
    assert.equal(asset.sha256, sha256(picture), 'the stored asset is the PNG that was written');
    const citing = caption => ({attachments: [], uses: [{source: {kind: 'stored', owner: verify, asset_id: asset.asset_id, sha256: asset.sha256}, caption, alt: '회색 픽셀 하나', purpose: '근거 인용'}]});
    const analysis = {kind: 'step', ref: stepRef(mustSucceed(await p.model('gil_open', {kind: 'analysis', contract: CONTRACT}), 'analysis'), 'analysis')};
    const analysisReport = 'hypothesis_fit: yes\nproblem_solved: yes\nsuccess_condition_met: yes\nguardrail_triggered: no\ninterpretation: 관측한 값과 판정 기준을 대조했다\n';
    mustRefuse(await p.model('gil_close', {report: analysisReport}), 'an Analysis that cites nothing');
    const cited = mustSucceed(await p.model('gil_close', {report: analysisReport, expected_owner: analysis, visuals: citing('해석의 근거')}), 'Analysis citing the image');
    assert.equal(cited.structuredContent.visuals.uses[0].origin_step_ref, verify.ref);
    const outcome = stepRef(mustSucceed(await p.model('gil_open', {kind: 'outcome', contract: CONTRACT}), 'outcome'), 'outcome');
    mustSucceed(await p.model('gil_close', {report: 'verdict: success\nlesson: 원인 위치와 복원 기준점을 구분해 다음 작은 질문의 근거로 남긴다\nnext_direction.action: close_cycle\nnext_direction.reason: 이 질문의 판정을 마쳤다\n'}), 'close outcome');
    const cycleOwner = {kind: 'cycle', ref: cycle};
    mustSucceed(await p.model('gil_close', {report: `verdict: success\noutcome_ref: ${outcome}\nhandoff_summary: ${cycle}의 관측과 한계를 남겼다\nnext_direction.action: open_child\nnext_direction.reason: 관측에 따라 다음을 정한다\n`,
      expected_owner: cycleOwner, visuals: citing('인계의 근거')}), 'Cycle Report citing the image');

    // Read back: the model-side read, the original Report read, and the stored bytes through the Monitor scope.
    const reads = async () => {
      const visuals = mustSucceed(await p.model('gil_report_visuals', {owner: verify}), 'report visuals').structuredContent;
      const original = mustSucceed(await p.model('gil_report_read', {owner: cycleOwner}), 'original Cycle Report').structuredContent;
      const image = mustSucceed(await p.app('gil_monitor_report_image', {owner: verify, asset_id: asset.asset_id, offset: 0}), 'monitor image').structuredContent;
      assert.equal(image.sha256, asset.sha256);
      assert.ok(Buffer.from(image.data_base64, 'base64').equals(picture), 'the Monitor reads back the exact stored PNG bytes');
      const requirement = (await p.plan()).requirements.find(r => r.id === 'R1');
      assert.deepEqual(requirement.experiments.map(e => [e.cycle_ref, e.verdict]), [[cycle, 'success']]);
      return {visuals, original};
    };
    const record = await p.record();
    const first = await reads();
    assert.equal(first.visuals.visuals.uses[0].sha256, asset.sha256);
    await p.restart();
    mustSucceed(await p.model('gil_context'), 'context after a restart');
    const prepared = mustSucceed(await p.model('gil_monitor_prepare'), 'monitor scope after a restart');
    p.scope = prepared.structuredContent.scope_id; // the scope this process prepared for the same project
    assert.deepEqual(await reads(), first, 'a fresh process reads the same Reports and visuals');
    assert.ok((await p.record()).equals(record), 'reading after a restart changed the record');
    return {started_format: 9, asset_sha256: asset.sha256, experiment: cycle, restart: 'same records and bytes'};
  } finally { await p.mcp.close(); }
}

async function trackedScope(binary, state) {
  const p = await new Project(binary, state, 'tracked-scope').create({'README.md': 'hello\n', 'src/a.txt': 'alpha\n', 'data/raw.csv': '1,2,3\n'});
  const P1 = 'scratch/\n', P2 = 'scratch/\ndata/\n';
  const absent = path => assert.rejects(lstat(join(p.root, path)), `${path} exists before the approval`);
  try {
    // An empty folder starts nothing until a kind is chosen.
    const empty = await tree(p.root);
    const asked = mustRefuse(await p.model('gil_start'), 'a start without a kind');
    assert.equal(asked.structuredContent.code, 'start_kind_required');
    assert.deepEqual(asked.structuredContent.start_kinds.map(kind => kind.value), ['tracked_scope', 'image_reports']);
    assert.equal(mustRefuse(await p.model('gil_start', {start_kind: 'format_11'}), 'an unknown kind').structuredContent.code, 'start_kind_unknown');
    assert.ok((await p.mcp.raw('gil_start', {project_root: p.root, start_kind: 7})).error, 'a non-string kind is a wire error');
    assert.deepEqual(await tree(p.root), empty, 'asking wrote nothing');

    const begun = mustSucceed(await p.model('gil_start', {start_kind: 'tracked_scope'}), 'tracked-scope preparation');
    assert.equal(begun.structuredContent.phase, 'preparing');
    p.scope = begun.structuredContent.scope_id;
    await absent('.gil/state.yaml'); await absent('.gil/artifacts');
    const preparing = await tree(p.root);
    assert.equal(mustRefuse(await p.model('gil_start', {start_kind: 'image_reports'}), 'image reports over a preparation').structuredContent.code, 'start_kind_conflict');
    assert.deepEqual(await tree(p.root), preparing);
    const shown = mustSucceed(await p.mcp.call('show_gil_monitor', {scope_id: p.scope}), 'preparation screen');
    assert.equal(shown.structuredContent.start_review_supported, true);
    assert.equal(shown.structuredContent.view, undefined, 'a preparation shows no invented Chain');

    const prepared = mustSucceed(await p.model('gil_start_prepare', {rules: P1}), 'exact proposal');
    const {revision, digest} = prepared.structuredContent.proposal;
    await absent('.gil/state.yaml'); await absent('.gil/artifacts');
    // Approval is App-only: a model-shaped input and a guessed capability are both refused.
    const model = await p.mcp.raw('gil_start_apply', {project_root: p.root, proposal_digest: digest});
    assert.ok(model.error || !succeeded(model.result));
    const forged = await p.mcp.raw('gil_start_decide', {scope_id: p.scope, revision, digest, token: 'f'.repeat(64), decision: 'approve'});
    assert.ok(forged.error || !succeeded(forged.result));
    await absent('.gil/state.yaml');
    // A fresh process continues the same preparation, with or without the kind.
    await p.restart();
    assert.equal(mustSucceed(await p.model('gil_start'), 'resume without a kind').structuredContent.phase, 'awaiting_approval');
    await absent('.gil/state.yaml');
    const review = mustSucceed(await p.app('gil_start_review'), 'start review');
    const capability = review._meta?.['gil/startApprovalToken'];
    assert.ok(capability && !JSON.stringify([review.content, review.structuredContent]).includes(capability));
    const decided = mustSucceed(await p.app('gil_start_decide', {revision, digest, token: capability, decision: 'approve'}), 'Monitor approval');
    assert.equal(decided.structuredContent.phase, 'started');
    const state = await p.record();
    assert.ok(state.toString('utf8').startsWith('format: 10\n') && !state.includes(capability));
    assert.equal(await readFile(join(p.root, '.gilignore'), 'utf8'), P1);
    assert.ok(!succeeded((await p.mcp.raw('gil_start_decide', {scope_id: p.scope, revision, digest, token: capability, decision: 'approve'})).result ?? {isError: true}), 'a used capability approved again');
    assert.equal(mustRefuse(await p.model('gil_start', {start_kind: 'image_reports'}), 'image reports over a tracked project').structuredContent.code, 'start_kind_conflict');
    assert.ok((await p.record()).equals(state));

    // Review Interview with an approved plan, then a same-Chain scope change approved in the Monitor.
    await p.interview(['A', 'B', 'C']);
    const proposed = mustSucceed(await p.model('gil_scope_propose', {rules: P2}), 'scope proposal');
    assert.equal(proposed.structuredContent.phase, 'awaiting_approval');
    assert.deepEqual(proposed.structuredContent.proposal.newly_excluded, ['data/raw.csv']);
    const change = proposed.structuredContent.proposal;
    const scopeReview = mustSucceed(await p.app('gil_scope_review'), 'scope review');
    const scopeCapability = scopeReview._meta?.['gil/scopeApprovalToken'];
    assert.ok(scopeCapability, 'the scope approval capability is given to the App only');
    const approved = mustSucceed(await p.app('gil_scope_decide', {revision: change.revision, digest: change.digest, token: scopeCapability, decision: 'approve'}), 'scope approval');
    assert.equal(approved.structuredContent.facts.baseline.result, 'archived', 'the new baseline was archived with the approved policy');
    assert.equal(await readFile(join(p.root, '.gilignore'), 'utf8'), P2);
    mustSucceed(await p.model('gil_open', await p.experimentArgs('A')), 'the next Experiment from the new baseline');

    await p.restart();
    const status = mustSucceed(await p.model('gil_scope_status'), 'scope status after a restart').structuredContent;
    assert.equal(status.tracking.standing.state, 'open_experiment');
    assert.equal(status.tracking.applied_policy, 'policy:P2');
    assert.equal(status.tracking.confirmed_policy, 'policy:P2');
    assert.equal(status.tracking.waiting ?? null, null);
    assert.equal(mustSucceed(await p.model('gil_start_status'), 'start status after a restart').structuredContent.phase, 'started');
    mustSucceed(await p.model('gil_context'), 'context after a restart');
    // The limits this kind states are real refusals, not silent fallbacks.
    mustRefuse(await p.model('gil_restore'), 'restore in a tracked-scope project');
    const visuals = await p.mcp.raw('gil_report_visuals', {project_root: p.root, owner: {kind: 'cycle', ref: 'cycle:C1'}});
    assert.ok(visuals.error || !succeeded(visuals.result), 'a tracked-scope project accepted an image-report read');
    assert.ok((await p.record()).toString('utf8').startsWith('format: 10\n'), 'nothing converted the project');
    return {started_format: 10, policies: ['policy:P1', 'policy:P2'], scope_change: 'approved and archived', restart: 'same records'};
  } finally { await p.mcp.close(); }
}

// Both kinds of new project through `binary`. Returns a small summary for a receipt.
export async function verifyStartKinds(binary) {
  const state = await realpath(await mkdtemp(join(tmpdir(), 'gil-product-kinds-')));
  await mkdir(join(state, 'home'));
  try {
    return {tracked_scope: await trackedScope(resolve(binary), state), image_reports: await imageReports(resolve(binary), state),
      scope: 'synthetic projects and harness App input; not a person\'s approval, a Host screen or failure injection'};
  } finally { await rm(state, {recursive: true, force: true}); } // Exactly the directory this check created.
}

// Protocol identity of `binary`: version agreement, probe, catalog contract, the embedded Monitor.
export async function verifyProtocol(binary, {platform, expectedVersion, expectedHtml}) {
  const state = await realpath(await mkdtemp(join(tmpdir(), 'gil-product-protocol-')));
  const env = {PATH: '/usr/bin:/bin', HOME: state, GIL_MONITOR_STATE_DIR: join(state, 'bindings')};
  let mcp;
  try {
    const version = spawnSync(binary, ['--version'], {cwd: state, env, encoding: 'utf8', timeout: 10000});
    assert.equal(version.status, 0, 'Core version query failed');
    const challenge = randomUUID();
    const probed = spawnSync(binary, ['--gil-agent-probe', challenge], {cwd: state, env, encoding: 'utf8', timeout: 30000, maxBuffer: 1024 * 1024});
    assert.equal(probed.error, undefined, 'first process failed/timed out; do not retry to hide it');
    assert.equal(probed.status, 0, 'native challenge failed');
    const {core, challenge: echoed} = JSON.parse(probed.stdout);
    assert.equal(echoed, challenge);
    checkCore(core, PROFILE, platform);
    mcp = new Mcp(binary, {cwd: state, env});
    const initialized = await mcp.initialize();
    checkVersionAgreement(expectedVersion, version.stdout, core.core_version, initialized.serverInfo.version);
    const {tools} = await mcp.request('tools/list', {});
    // The checks count the surface of one platform; a Linux binary has the macOS-sized catalog.
    checkProductTools(withoutRecoveryTools(tools), 'linux');
    const show = tools.find(t => t.name === 'show_gil_monitor');
    const uri = show?._meta?.ui?.resourceUri;
    assert.match(uri, /^ui:\/\/gil-monitor\/[a-f0-9]{20}\.html$/);
    const resource = await mcp.request('resources/read', {uri});
    assert.equal(resource.contents.length, 1);
    const html = resource.contents[0];
    assert.equal(html.mimeType, 'text/html;profile=mcp-app');
    assert.equal(html.uri, uri);
    const htmlBytes = Buffer.from(html.text, 'utf8'), uiHash = sha256(htmlBytes);
    assert.equal(uri, `ui://gil-monitor/${uiHash.slice(0, 20)}.html`);
    if (expectedHtml) assert.ok(htmlBytes.equals(await readFile(expectedHtml)), 'the embedded Monitor differs from the packaged asset');
    return {version: core.core_version, core, tools: tools.length, ui: {uri, mime: html.mimeType, bytes: htmlBytes.length, sha256: uiHash},
      compatibility: compatibility(PROFILE)};
  } finally {
    if (mcp) await mcp.close();
    await rm(state, {recursive: true, force: true});
  }
}

// Linux: the exact archive → integrity → member inspection → extraction into a new folder →
// install into an isolated HOME → the INSTALLED binary's protocol and both kinds of project →
// uninstall → only the files the install recorded are gone.
export async function verifyLinuxArchive({archive, sums, expectedHtml, expectedVersion}) {
  assert.equal(process.platform, 'linux'); assert.equal(process.arch, 'x64');
  const bytes = await readFile(archive), digest = sha256(bytes);
  assert.equal((await readFile(sums, 'utf8')), `${digest}  ${LINUX.archive}\n`, 'archive differs from its published checksum');
  const state = await realpath(await mkdtemp(join(tmpdir(), 'gil-product-install-')));
  try {
    const members = spawnSync('/bin/sh', [fileURLToPath(new URL('../codex/check-archive-members.sh', import.meta.url)), resolve(archive)], {encoding: 'utf8', timeout: 60000});
    assert.equal(members.status, 0, `archive member inspection refused: ${members.stderr}`);
    const extracted = join(state, 'extracted'), home = join(state, 'home');
    await mkdir(extracted); await mkdir(home);
    const untar = spawnSync('/usr/bin/tar', ['-xzf', resolve(archive), '--no-same-owner', '-C', extracted], {encoding: 'utf8', timeout: 60000});
    assert.equal(untar.status, 0, 'extraction failed');
    const receipt = await verifyLinuxTree(extracted); // Integrity before executing any artifact byte.
    if (expectedVersion !== undefined) assert.equal(receipt.version, expectedVersion);

    // Synthetic pre-existing settings and files that the install must leave as they are.
    await mkdir(join(home, '.codex'));
    const config = '# sentinel: an existing Codex setting\n[mcp_servers.other]\ncommand = "/usr/bin/true"\nargs = []\n';
    await writeFile(join(home, '.codex/config.toml'), config);
    await mkdir(join(home, '.local/share/other-app'), {recursive: true});
    await writeFile(join(home, '.local/share/other-app/data'), 'sentinel\n');
    const env = {PATH: '/usr/bin:/bin', HOME: home, CODEX_HOME: join(home, '.codex')};
    const run = (binary, args) => spawnSync(binary, args, {cwd: state, env, encoding: 'utf8', timeout: 60000});
    const packaged = join(extracted, LINUX.core), installed = join(home, '.local/share/gil/bin/gil');
    const install = run(packaged, ['install', '--no-wait']);
    assert.equal(install.status, 0, `gil install failed: ${install.stderr}`);
    assert.match(install.stdout, /^binary: UPDATED /m);
    assert.match(install.stdout, /^claude-desktop: NOT_INSTALLED$/m, 'a Host that does not exist on this machine was reported as registered');
    assert.ok((await readFile(installed)).equals(await readFile(packaged)), 'the installed file is not the packaged bytes');
    assert.equal((await lstat(installed)).mode & 0o111, 0o111, 'the installed Core is not executable');
    const registered = await readFile(join(home, '.codex/config.toml'), 'utf8');
    assert.ok(registered.includes('[mcp_servers.other]') && registered.includes('# sentinel'), 'existing Codex settings were rewritten');
    assert.ok(registered.includes(installed), 'Codex was not pointed at the installed path');
    assert.equal(run(installed, ['install', '--no-wait']).status, 0, 'a repeated install is not idempotent');
    assert.equal(run(installed, ['doctor']).status, 0, 'gil doctor reports an unhealthy installation');

    const protocol = await verifyProtocol(installed, {platform: LINUX.platform, expectedVersion: receipt.version, expectedHtml});
    const kinds = await verifyStartKinds(installed);

    const removed = run(packaged, ['uninstall', '--remove-binary']);
    assert.equal(removed.status, 0, `gil uninstall failed: ${removed.stderr}`);
    await assert.rejects(lstat(installed), 'the installed Core was not removed');
    const after = await readFile(join(home, '.codex/config.toml'), 'utf8');
    assert.ok(after.includes('[mcp_servers.other]') && after.includes('# sentinel') && !after.includes(installed), 'uninstall did not restore exactly the foreign settings');
    assert.equal(await readFile(join(home, '.local/share/other-app/data'), 'utf8'), 'sentinel\n');
    const left = Object.keys(await tree(join(home, '.local/share'))).filter(path => path.startsWith('gil/') && !path.endsWith('/'));
    assert.deepEqual(left.filter(path => path !== 'gil/bin'), [], 'files of the installation were left behind');
    return {archive_sha256: digest, receipt: {version: receipt.version, elf: receipt.elf, source: receipt.source}, protocol, start_kinds: kinds,
      install: 'isolated HOME; existing Codex settings and unrelated files unchanged; uninstall removed the recorded files only'};
  } finally { await rm(state, {recursive: true, force: true}); }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [command, ...args] = process.argv.slice(2);
  if (command === 'kinds' && args.length === 1) console.log(JSON.stringify(await verifyStartKinds(resolve(args[0])), null, 2));
  else if (command === 'linux-archive' && (args.length === 2 || args.length === 3)) {
    console.log(JSON.stringify(await verifyLinuxArchive({archive: resolve(args[0]), sums: resolve(args[1]), expectedHtml: args[2] && resolve(args[2])}), null, 2));
  } else throw new Error('Usage: verify-install.mjs kinds <binary> | linux-archive <archive.tar.gz> <SHA256SUMS> [expected-monitor.html]');
}
