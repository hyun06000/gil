#!/usr/bin/env node
// Deterministic synthetic recordings for tools/context-metrics. Nothing here comes from a real
// project, person or session; every token, path and sentence is invented test data. The marker
// strings SYNTHETIC_SECRET_* and /Users/synthetic-user/... exist only so the tests can prove that
// reports never copy raw responses, tokens or private paths.
//   node tools/context-metrics/fixtures/generate.mjs [--check]
import { writeFileSync, readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
export const SECRET_TOKEN = 'SYNTHETIC_SECRET_TOKEN_0123456789abcdef0123456789abcdef';
export const PRIVATE_PATH = '/Users/synthetic-user/secret-project';

const PREAMBLE = 'GIL은 한 번에 하나의 행동만 수행한다. 거절되면 오류가 제시한 다음 행동을 따른다. 그 규칙이 낯설면 오류가 가리킨 help topic을 읽는다. 같은 세션에서 이미 이해한 topic은 불확실할 때만 다시 읽는다.';
const APPROVAL_WARNING = '승인은 Monitor의 버튼으로만 기록된다. 응답이 없다는 이유로 승인으로 기록하지 않는다. 합성 입력은 사용자 동의가 아니다.';
const DIRTY_TOPIC = ['[언제 읽는가]\nVerify가 아닌 자리에서 Artifact가 바뀐 경우.', '[지금 할 일]\ngil restore로 기준 세계를 복원한 뒤 같은 행동을 이어 간다.', '[불변식]\n복원은 Journey를 되돌리지 않는다. Knowledge와 실패 가지는 삭제되지 않는다.', '[관련 주제]\nartifact/restore'].join('\n\n');
const OPEN_CONTRACT_TOPIC = ['[언제 읽는가]\n실행형 Step을 열며 낸 행동 계약의 칸이 없거나 비었다.', '[지금 할 일]\ncontract에 objective·method·done_when을 채워 다시 gil_open을 호출한다.', '[흔한 실패]\nreport 키로 보내면 거절된다. 열 때의 본문 키는 contract다.'].join('\n\n');

function ok(said, extra = {}, textParts = null) {
  const content = (textParts ?? [said]).map((t) => ({ type: 'text', text: t }));
  return { content, structuredContent: { ok: true, exit_code: 0, said, problem: '', ...extra }, isError: false };
}
function refused(problem, said = '') {
  return { content: [{ type: 'text', text: problem }], structuredContent: { ok: false, exit_code: 1, said, problem }, isError: true };
}

export function builder() {
  const lines = []; let seq = 0;
  const ev = (name, args, result, { error = null, actor = 'model', synthetic = false } = {}) => {
    seq += 1;
    lines.push(JSON.stringify({ seq, at: `2026-11-16T00:00:${String(seq).padStart(2, '0')}.000Z`, actor, synthetic, request: { name, arguments: args }, result, error, ms: 5 }));
  };
  return { ev, text: () => lines.join('\n') + (lines.length ? '\n' : '') };
}

const CONTEXT_SAID = ['작업 목표: 합성 도구 호출 기록의 전달량을 측정한다.', '현재 위치: cycle:C2 · experiment · step:C2/S3 · verify (open)', '유효한 제약: 실제 모델 호출 없음, 네트워크 없음.', '미해결: 비교 기준의 호출 수 차이를 어떻게 표시할지.', '근거 참조: step:C1/S3, snapshot:A2'].join('\n');

export function before() {
  const b = builder();
  b.ev('gil_start', { project_root: '<project_root>' }, ok('프로젝트를 시작했다.\nChain: chain:C1 · open\n현재: cycle:C1 · interview', { display_confirmed: false }, [PREAMBLE, '프로젝트를 시작했다.\nChain: chain:C1 · open\n현재: cycle:C1 · interview']));
  b.ev('gil_context', { project_root: '<project_root>' }, ok(CONTEXT_SAID, {}, [PREAMBLE, APPROVAL_WARNING, CONTEXT_SAID]));
  b.ev('gil_help', { project_root: '<project_root>', topic: 'artifact/dirty/non-verify' }, ok(DIRTY_TOPIC, {}, [PREAMBLE, DIRTY_TOPIC]));
  b.ev('gil_open', { project_root: '<project_root>', kind: 'verify', contract: 'c' }, refused('gil_open: 행동 계약의 칸이 비었다. Nothing was opened. 읽을 Topic: action/open-contract'));
  b.ev('gil_help', { project_root: '<project_root>', topic: 'action/open-contract' }, ok(OPEN_CONTRACT_TOPIC, {}, [PREAMBLE, OPEN_CONTRACT_TOPIC]));
  b.ev('gil_open', { project_root: '<project_root>', kind: 'verify', contract: 'objective: o\nmethod: m\ndone_when: d' }, ok('열었다: step:C2/S3 · verify\n', {}, [PREAMBLE, APPROVAL_WARNING, '열었다: step:C2/S3 · verify\n']));
  b.ev('gil_help', { project_root: '<project_root>', topic: 'artifact/dirty/non-verify' }, ok(DIRTY_TOPIC, {}, [PREAMBLE, DIRTY_TOPIC])); // identical repeat
  b.ev('gil_help', { project_root: '<project_root>', topic: 'close' }, ok('Verify close 계약\n  execution · result · evidence_files', {}, [PREAMBLE, 'Verify close 계약\n  execution · result · evidence_files']));
  b.ev('gil_plan_decide', { scope_id: 'project:' + 'a'.repeat(64), revision: 1, digest: 'd'.repeat(64), token: SECRET_TOKEN, decision: 'approve' }, ok('approved', {}, [`approved ${PRIVATE_PATH}`]), { actor: 'app' });
  b.ev('gil_close', { project_root: '<project_root>', report: 'execution: e\nresult: r\nevidence_files: evidence/a.md\n' }, ok('닫았다: step:C2/S3 · verify\n', {}, [PREAMBLE, APPROVAL_WARNING, '닫았다: step:C2/S3 · verify\n']));
  b.ev('gil_help', { project_root: '<project_root>', topic: 'close' }, ok('Experiment Cycle close 계약\n  verdict · outcome_ref · handoff_summary', {}, [PREAMBLE, 'Experiment Cycle close 계약\n  verdict · outcome_ref · handoff_summary'])); // state-dependent repeat, different text
  b.ev('gil_context', { project_root: '<project_root>' }, ok(CONTEXT_SAID, {}, [PREAMBLE, APPROVAL_WARNING, CONTEXT_SAID, `긴 설명 ${'x'.repeat(50)}…<truncated 120 chars>`]));
  b.ev('gil_status', { project_root: '<project_root>' }, null, { error: { code: -32000, message: 'timeout' } });
  b.ev('gil_status', { project_root: '<project_root>' }, null);
  return b.text();
}

export function after() {
  const b = builder();
  b.ev('gil_start', { project_root: '<project_root>' }, ok('프로젝트를 시작했다.\nChain: chain:C1 · open\n현재: cycle:C1 · interview', { display_confirmed: false }));
  b.ev('gil_context', { project_root: '<project_root>' }, ok(CONTEXT_SAID));
  b.ev('gil_help', { project_root: '<project_root>', topic: 'artifact/dirty/non-verify' }, ok(DIRTY_TOPIC));
  b.ev('gil_open', { project_root: '<project_root>', kind: 'verify', contract: 'objective: o\nmethod: m\ndone_when: d' }, ok('열었다: step:C2/S3 · verify\n', {}, [APPROVAL_WARNING, '열었다: step:C2/S3 · verify\n']));
  b.ev('gil_context', { project_root: '<project_root>' }, ok(CONTEXT_SAID)); // extra re-read: fewer help calls, more context calls
  b.ev('gil_close', { project_root: '<project_root>', report: 'execution: e\nresult: r\nevidence_files: evidence/a.md\n' }, ok('닫았다: step:C2/S3 · verify\n', {}, [APPROVAL_WARNING, '닫았다: step:C2/S3 · verify\n']));
  b.ev('gil_context', { project_root: '<project_root>' }, ok(CONTEXT_SAID));
  b.ev('gil_help', { project_root: '<project_root>', topic: 'close' }, ok('Experiment Cycle close 계약\n  verdict · outcome_ref · handoff_summary'));
  return b.text();
}

export function malformed() {
  const b = builder();
  b.ev('gil_help', { topic: 'current' }, ok('Bootstrap 안내'));
  return ['{ this is not json', JSON.stringify({ seq: 9, note: 'no request' }), JSON.stringify([1, 2, 3]), '', b.text().trim()].join('\n') + '\n';
}

export const FILES = { 'before.jsonl': before, 'after.jsonl': after, 'malformed.jsonl': malformed, 'empty.jsonl': () => '' };

if (process.argv[1] && dirname(process.argv[1]) && import.meta.url === new URL(`file://${process.argv[1]}`).href) {
  const check = process.argv.includes('--check');
  let stale = 0;
  for (const [name, make] of Object.entries(FILES)) {
    const path = join(here, name); const text = make();
    if (check) { let cur = null; try { cur = readFileSync(path, 'utf8'); } catch {} if (cur !== text) { stale += 1; process.stderr.write(`[fixtures] stale: ${name}\n`); } }
    else writeFileSync(path, text);
  }
  if (check && stale) process.exitCode = 1;
}
