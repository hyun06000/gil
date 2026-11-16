// Self-tests for tools/context-metrics. Node standard library only; nothing here runs a model,
// a Core, an App or the network. All inputs are synthetic.
import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, writeFileSync, existsSync, statSync, symlinkSync, linkSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync, spawnSync } from 'node:child_process';
import { measure, stableStringify, sha256 } from './lib/size.mjs';
import { parseRecording, classify, textOf, markersOf } from './lib/events.mjs';
import { analyzeRecording, splitBlocks, normalizeBlock, topicKeyOf } from './lib/analyze.mjs';
import { compareAnalyses } from './lib/compare.mjs';
import { formatAnalysis, formatComparison } from './lib/format.mjs';
import { parseArgs, run, analyzeFile } from './cli.mjs';
import { builder, FILES, SECRET_TOKEN, PRIVATE_PATH } from './fixtures/generate.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const fx = (name) => join(here, 'fixtures', name);
const ok = (said, parts = [said], extra = {}) => ({ content: parts.map((t) => ({ type: 'text', text: t })), structuredContent: { ok: true, exit_code: 0, said, problem: '', ...extra }, isError: false });

test('size: Korean, English and emoji are measured in three units and never mixed', () => {
  assert.deepEqual(measure('abc'), { utf8_bytes: 3, code_points: 3, utf16_units: 3 });
  assert.deepEqual(measure('한글'), { utf8_bytes: 6, code_points: 2, utf16_units: 2 });
  assert.deepEqual(measure('😀'), { utf8_bytes: 4, code_points: 1, utf16_units: 2 });
  // Family emoji: 7 code points (4 people + 3 ZWJ), one visible glyph. "문자 수" means code points here.
  assert.deepEqual(measure('👨‍👩‍👧‍👦'), { utf8_bytes: 25, code_points: 7, utf16_units: 11 });
  assert.deepEqual(measure('a한😀'), { utf8_bytes: 1 + 3 + 4, code_points: 3, utf16_units: 4 });
  assert.equal(measure(42), null);
  assert.equal(measure(null), null);
});

test('size: stable stringify sorts keys and drops undefined so the same value hashes the same', () => {
  assert.equal(stableStringify({ b: 1, a: [{ d: 2, c: null }], u: undefined }), '{"a":[{"c":null,"d":2}],"b":1}');
  assert.equal(sha256(stableStringify({ x: 1, y: 2 })), sha256(stableStringify({ y: 2, x: 1 })));
});

test('events: statuses are separated and malformed lines become unsupported, not errors of a tool', () => {
  const rec = parseRecording(readFileSync(fx('malformed.jsonl'), 'utf8'));
  assert.equal(rec.events.length, 1);
  assert.deepEqual(rec.unsupported.map((u) => u.reason), ['invalid_json', 'missing_request_name', 'not_an_object']);
  assert.equal(classify({ error: { code: 1 } }), 'rpc_error');
  assert.equal(classify({ result: null, error: null }), 'missing_result');
  assert.equal(classify({ result: { isError: true } }), 'error_response');
  assert.equal(classify({ result: { structuredContent: { ok: false } } }), 'error_response');
  assert.equal(classify({ result: { content: [] } }), 'ok');
  assert.deepEqual(textOf({ content: [{ type: 'text', text: 'a' }, { type: 'image', data: 'x' }, { type: 'text', text: 'b' }] }), { text: 'a\nb', parts: 2, list: ['a', 'b'] });
  assert.deepEqual(textOf({}), { text: null, parts: 0, list: [] });
  assert.deepEqual(markersOf({ a: 'x…<truncated 9 chars>', b: ['<redacted-token>', 'plain'] }), { truncated_strings: 1, redacted_strings: 1 });
});

test('events: empty recording analyzes to zeros without throwing', () => {
  const a = analyzeRecording(parseRecording(''));
  assert.equal(a.input.events, 0);
  assert.deepEqual(a.status_totals, { ok: 0, error_response: 0, rpc_error: 0, missing_result: 0 });
  assert.equal(a.gil_help.calls, 0);
  assert.equal(a.gil_context.calls, 0);
  assert.deepEqual(a.repetition_candidates.blocks, []);
  assert.ok(formatAnalysis(a).includes('기록 0건'));
});

test('analyze: text and structured responses are measured separately and said duplication is flagged, not summed', () => {
  const b = builder();
  b.ev('gil_context', { project_root: '<project_root>' }, ok('목표: g', ['안내', '목표: g'], { next_call: { name: 'show_gil_monitor', arguments: { scope_id: 'project:' + 'a'.repeat(64) } } }));
  b.ev('gil_context', { project_root: '<project_root>' }, { content: [], structuredContent: { ok: true, exit_code: 0, said: '목표: g', problem: '' }, isError: false });
  const a = analyzeRecording(parseRecording(b.text()));
  const [c1, c2] = a.gil_context.per_call;
  assert.deepEqual(c1.text_response, measure('안내\n목표: g'));
  assert.equal(c1.said_in_text, true);
  assert.ok(c1.structured_response.utf8_bytes > c1.text_response.utf8_bytes, 'structured carries next_call and is measured on its own');
  assert.equal(c2.text_response, null, 'no text part → no text size, not zero');
  assert.equal(c2.said_in_text, null);
  assert.equal(a.gil_context.events_with_text, 1);
  assert.equal(a.gil_context.events_with_structured, 2);
  assert.equal(a.gil_context.said_in_text_count, 1);
  assert.ok(!('total_transferred' in a.totals) && !('model_input' in a.totals), 'no merged "amount sent to the model" field');
});

test('analyze: the same help topic repeated is counted; different topics and state-dependent topics are kept apart', () => {
  const b = builder();
  b.ev('gil_help', { topic: 'artifact/restore' }, ok('A 본문 '.repeat(10)));
  b.ev('gil_help', { topic: 'artifact/restore' }, ok('A 본문 '.repeat(10)));
  b.ev('gil_help', { topic: 'artifact/restore ' }, ok('A 본문 변경 '.repeat(10)));
  b.ev('gil_help', { topic: 'step/verify/close' }, ok('B 본문 '.repeat(10)));
  b.ev('gil_help', { project_root: '<project_root>', topic: 'close' }, ok('C1'));
  b.ev('gil_help', { project_root: '<project_root>', topic: 'close' }, ok('C2'));
  b.ev('gil_help', { project_root: '<project_root>' }, ok('현재 상태 Topic'));
  b.ev('gil_help', { topic: 'artifact/restore' }, { content: [{ type: 'text', text: 'refused' }], structuredContent: { ok: false, exit_code: 1, said: '', problem: 'p' }, isError: true });
  const a = analyzeRecording(parseRecording(b.text()));
  assert.equal(a.gil_help.calls, 8);
  assert.equal(a.gil_help.distinct_topics, 4);
  const byTopic = Object.fromEntries(a.gil_help.topics.map((t) => [t.topic, t]));
  assert.equal(byTopic['artifact/restore'].calls, 4, 'trimmed topic key merges "artifact/restore " with the same topic');
  assert.equal(byTopic['artifact/restore'].ok_calls, 3);
  assert.equal(byTopic['artifact/restore'].repeated_lookups, 3);
  assert.equal(byTopic['artifact/restore'].distinct_text_responses, 3, 'identical, changed and refusal texts');
  assert.equal(byTopic['artifact/restore'].identical_text_repeats, 1);
  assert.equal(byTopic['step/verify/close'].repeated_lookups, 0);
  assert.equal(byTopic.close.state_dependent, true);
  assert.equal(byTopic.close.repeated_lookups, 1);
  assert.equal(byTopic.close.identical_text_repeats, 0, 'different state answers are not identical repeats');
  assert.equal(byTopic['(current-state)'].state_dependent, true);
  assert.equal(topicKeyOf({}), '(current-state)');
  assert.equal(a.gil_help.repeated_lookups, 3 + 0 + 1 + 0);
  assert.equal(a.gil_help.identical_text_repeats, 1);
});

test('analyze: repetition candidates use declared normalization and a minimum block size; approvals are reported, not judged', () => {
  assert.deepEqual(splitBlocks(['첫 단락\n둘째 줄\n\n\n  둘째   단락 ', '셋째\t단락']), ['첫 단락 둘째 줄', '둘째 단락', '셋째 단락']);
  assert.equal(normalizeBlock('é'.normalize('NFD')), 'é'.normalize('NFC'));
  const warning = '승인은 Monitor의 버튼으로만 기록된다. 응답이 없다는 이유로 승인으로 기록하지 않는다.';
  const b = builder();
  b.ev('gil_open', {}, ok('열었다', [warning, '열었다']));
  b.ev('gil_close', {}, ok('닫았다', [`  ${warning.replace(' ', '   ')}\n`, '닫았다']));
  b.ev('gil_help', { topic: 't' }, ok(`${warning}\n\n${warning}`));
  b.ev('gil_status', {}, ok('짧다', ['짧다', '짧다']));
  const a = analyzeRecording(parseRecording(b.text()));
  assert.equal(a.repetition_candidates.blocks.length, 1, 'short blocks fall under the minimum and are not candidates');
  const [blk] = a.repetition_candidates.blocks;
  assert.equal(blk.occurrences, 4);
  assert.equal(blk.events, 3);
  assert.deepEqual(blk.tools, ['gil_close', 'gil_help', 'gil_open']);
  assert.deepEqual(blk.seqs, [1, 2, 3, 3]);
  assert.deepEqual(blk.size, measure(warning));
  assert.equal(blk.repeated_size.utf8_bytes, measure(warning).utf8_bytes * 3);
  assert.equal(blk.id.length, 16);
  assert.ok(!JSON.stringify(a).includes('승인은 Monitor'), 'the block text itself is never copied into the report');
  assert.ok(a.repetition_candidates.blocks.every((x) => !('verdict' in x) && !('waste' in x) && !('remove' in x)), 'a candidate row carries no waste verdict and no removal advice');
  const low = analyzeRecording(parseRecording(b.text()), { minBlockCodePoints: 2 });
  assert.ok(low.repetition_candidates.blocks.some((x) => x.tools.includes('gil_status')), 'threshold is a parameter and is recorded');
  assert.equal(low.repetition_candidates.min_block_code_points, 2);
});

test('analyze: errors, missing results, truncation and redaction are counted and surfaced as limits', () => {
  const a = analyzeFile(fx('before.jsonl'));
  assert.deepEqual(a.status_totals, { ok: 11, error_response: 1, rpc_error: 1, missing_result: 1 });
  assert.equal(a.by_tool.find((t) => t.tool === 'gil_status').events_with_text, 0);
  assert.equal(a.by_tool.find((t) => t.tool === 'gil_open').by_status.error_response, 1);
  assert.equal(a.markers.truncated_strings, 1);
  assert.ok(a.markers.redacted_strings > 0);
  assert.ok(a.limits.some((l) => l.includes('잘림')) && a.limits.some((l) => l.includes('가림')));
  assert.ok(a.limits.some((l) => l.includes('토큰 수가 아니다')));
  const m = analyzeFile(fx('malformed.jsonl'));
  assert.equal(m.input.unsupported_lines.length, 3);
  assert.ok(m.limits.some((l) => l.includes('제외했다')));
});

test('compare: per-tool, per-topic and total deltas sit next to call counts; differing counts are flagged', () => {
  const before = analyzeFile(fx('before.jsonl'));
  const after = analyzeFile(fx('after.jsonl'));
  const c = compareAnalyses(before, after);
  assert.equal(c.totals.call_count_differs, true);
  assert.ok(c.caveats.some((x) => x.includes('같은 작업으로 보지 않')));
  assert.ok(c.caveats.some((x) => x.includes('모델 비용이 줄었다')));
  const help = c.by_tool.find((r) => r.tool === 'gil_help');
  assert.deepEqual(help.calls, { before: 5, after: 2, delta: -3 });
  const ctx = c.by_tool.find((r) => r.tool === 'gil_context');
  assert.deepEqual(ctx.calls, { before: 2, after: 3, delta: 1 }, 'fewer help calls but more context re-reads are both visible');
  const status = c.by_tool.find((r) => r.tool === 'gil_status');
  assert.equal(status.calls.after, 0);
  assert.equal(status.text_response.utf8_bytes.ratio, null, 'no ratio against a zero baseline');
  const dirty = c.gil_help_topics.find((r) => r.topic === 'artifact/dirty/non-verify');
  assert.deepEqual(dirty.repeated_lookups, { before: 1, after: 0 });
  assert.equal(c.gil_help_topics.find((r) => r.topic === 'action/open-contract').calls.after, 0);
  assert.ok(c.totals.text_response.utf8_bytes.delta < 0);
  assert.ok(!('combined' in c.totals), 'text and structured deltas are never merged');
  const same = compareAnalyses(before, before);
  assert.equal(same.totals.call_count_differs, false);
  assert.equal(same.totals.text_response.utf8_bytes.ratio, 1);
  assert.ok(formatComparison(c).includes('호출 수가 다르므로'));
});

test('determinism: two runs give identical JSON and the input file is unchanged', () => {
  const path = fx('before.jsonl');
  const stat1 = statSync(path); const hash1 = sha256(readFileSync(path, 'utf8'));
  const a1 = JSON.stringify(analyzeFile(path));
  const a2 = JSON.stringify(analyzeFile(path));
  assert.equal(a1, a2);
  assert.ok(!a1.includes('"generated_at"'), 'no timestamp in the result');
  assert.equal(sha256(readFileSync(path, 'utf8')), hash1);
  assert.equal(statSync(path).mtimeMs, stat1.mtimeMs);
  // Line order inside the recording changes seqs but not the measured sizes of the same lines.
  const shuffled = readFileSync(path, 'utf8').trim().split('\n').reverse().join('\n') + '\n';
  const r = analyzeRecording(parseRecording(shuffled));
  assert.deepEqual(r.totals, analyzeFile(path).totals);
});

test('privacy: reports carry ids, counts and sizes but never raw responses, tokens or private paths', () => {
  const raw = readFileSync(fx('before.jsonl'), 'utf8');
  assert.ok(raw.includes(SECRET_TOKEN) && raw.includes(PRIVATE_PATH), 'the synthetic recording deliberately contains both markers');
  const a = analyzeFile(fx('before.jsonl'));
  const c = compareAnalyses(a, analyzeFile(fx('after.jsonl')));
  for (const out of [JSON.stringify(a), formatAnalysis(a), JSON.stringify(c), formatComparison(c)]) {
    assert.ok(!out.includes(SECRET_TOKEN), 'token leaked');
    assert.ok(!out.includes(PRIVATE_PATH), 'private path leaked');
    assert.ok(!out.includes('/Users/'), 'absolute home path leaked');
    assert.ok(!out.includes('GIL은 한 번에 하나의 행동만'), 'response body leaked');
    assert.ok(!out.includes('[언제 읽는가]'), 'help topic body leaked');
  }
  assert.equal(a.input.file_name, 'before.jsonl', 'only the basename of the input is recorded');
});

test('cli: analyze/compare write JSON only to --json, reject bad usage, and run without network or Core', () => {
  const work = mkdtempSync(join(tmpdir(), 'gil-ctx-metrics-'));
  const cli = join(here, 'cli.mjs');
  const out1 = join(work, 'nested', 'a.json');
  const text = execFileSync(process.execPath, [cli, 'analyze', '--events', fx('before.jsonl'), '--json', out1], { encoding: 'utf8' });
  assert.ok(text.includes('# 문맥 전달량 측정'));
  assert.ok(existsSync(out1));
  assert.equal(JSON.parse(readFileSync(out1, 'utf8')).input.events, 14);
  const out2 = join(work, 'c.json');
  const quiet = execFileSync(process.execPath, [cli, 'compare', '--before', fx('before.jsonl'), '--after', fx('after.jsonl'), '--json', out2, '--quiet'], { encoding: 'utf8' });
  assert.equal(quiet, '');
  assert.equal(JSON.parse(readFileSync(out2, 'utf8')).totals.call_count_differs, true);
  const noOut = execFileSync(process.execPath, [cli, 'analyze', '--events', fx('empty.jsonl')], { encoding: 'utf8', cwd: work });
  assert.ok(noOut.includes('기록 0건'));
  assert.deepEqual(readFileSync(out1, 'utf8'), readFileSync(out1, 'utf8'));
  for (const args of [[], ['analyze'], ['compare', '--before', fx('before.jsonl')], ['analyze', '--events', fx('before.jsonl'), '--bogus'], ['analyze', '--events', join(work, 'missing.jsonl')], ['analyze', '--events', fx('before.jsonl'), '--min-block-chars', '0']]) {
    const r = spawnSync(process.execPath, [cli, ...args], { encoding: 'utf8' });
    assert.equal(r.status, 2, `args ${args.join(' ')} should exit 2`);
    assert.ok(r.stderr.startsWith('[context-metrics]'));
  }
  assert.throws(() => parseArgs(['analyze', '--min-block-chars', 'x', '--events', 'f']), /positive integer/);
  const { result } = run(parseArgs(['analyze', '--events', fx('after.jsonl'), '--min-block-chars', '5']));
  assert.equal(result.repetition_candidates.min_block_code_points, 5);
  writeFileSync(join(work, 'stray.txt'), '');
});

test('JSON output never overwrites inputs, aliases or existing reports', () => {
  const work = mkdtempSync(join(tmpdir(), 'gil-metrics-output-'));
  try {
    const before = join(work, 'before.jsonl'), after = join(work, 'after.jsonl');
    const original = readFileSync(fx('before.jsonl'), 'utf8');
    writeFileSync(before, original); writeFileSync(after, original);
    const hard = join(work, 'hard.json'); linkSync(before, hard);
    const report = join(work, 'report.json'); writeFileSync(report, 'existing report');
    const outputs = [before, after, hard, report];
    if (process.platform !== 'win32') {
      const sym = join(work, 'symbolic.json'); symlinkSync(before, sym); outputs.push(sym);
    }
    for (const output of outputs) {
      assert.throws(() => run(parseArgs(['compare', '--before', before, '--after', after, '--json', output])), { code: 'EEXIST' });
      assert.equal(readFileSync(before, 'utf8'), original);
      assert.equal(readFileSync(after, 'utf8'), original);
      assert.equal(readFileSync(report, 'utf8'), 'existing report');
    }
  } finally { rmSync(work, { recursive: true, force: true }); }
});

test('fixtures: committed synthetic recordings match their generator', () => {
  for (const [name, make] of Object.entries(FILES)) assert.equal(readFileSync(fx(name), 'utf8'), make(), name);
});
