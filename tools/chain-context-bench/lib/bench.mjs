// Runs a plan case by case and keeps a result object that is valid at every moment, so an
// interrupt can write what was finished. Summaries are derived from finished runs only.
import { writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { compareAnalyses } from '../../context-metrics/lib/compare.mjs';
import { runOnce } from './run.mjs';
import { compareRepeats, invariantAcross, sectionVariability, readOnlyUnchanged, lineageHolds } from './measure.mjs';

export const SCHEMA = 'gil-chain-context-bench/0.1';
const STATUS_ORDER = ['complete', 'record_changed_by_read', 'measurement_missing', 'unsupported', 'generation_failed', 'interrupted', 'not_run'];
const worst = (xs) => xs.reduce((w, s) => (STATUS_ORDER.indexOf(s) > STATUS_ORDER.indexOf(w) ? s : w), 'complete');

export const LIMITS = [
  '크기는 UTF-8 바이트·유니코드 코드 포인트("문자")·UTF-16 단위다(tools/context-metrics). 토큰 수가 아니며 토큰 계산기를 쓰지 않았다.',
  'text 응답과 structuredContent는 서로 다른 표현이며 더하지 않는다. Host가 어느 표현을 모델 입력에 넣는지는 이 도구가 관측하지 않는다.',
  '"읽기 호출 text 합"은 이 벤치가 고른 읽기 호출의 text 크기 합일 뿐 모델 입력량·전체 실행 비용이 아니다. 모델 호출이 없으므로 재조회·재시도·작업 정확성은 측정하지 않았다.',
  '경로는 <project_root> 등 표식으로 가린 기록을 측정한다. 가리기 전 text 크기는 raw_text_utf8_bytes에 따로 적었다(임시 경로 길이에 따라 실행마다 다를 수 있다).',
  '합성 기록이다. 문장 길이·항목 수가 실제 사용자 작업과 다르므로 절대값보다 축별 증가 모양을 본다. 한 번의 측정으로 임계값이나 CI 실패 기준을 만들지 않는다.',
  '승인 계획 결정은 시험 harness가 App 경로로 보낸 합성 입력이다. 사람의 승인·화면 표시의 증거가 아니다.',
  '실패 뒤 복구는 Core가 도구 목록에 내놓은 경로로 밟는다. 제품 구성(product-safety)에서는 App 전용 gil_revisit_review → gil_revisit_apply에 Core가 돌려준 digest와 비공개 승인 정보를 넣는 harness의 합성 승인이며, 사람이 Monitor 버튼을 누른 것이 아니다. Core가 그 입력을 monitor_button으로 적은 기록은 사람 클릭의 증거가 아니다. 승인 정보는 결과에 남기지 않는다.',
  '보존 점검은 새 세션의 읽기 응답에 실패 Cycle 참조·표식·현재 항목·실행 연결이 문자열·구조로 남아 있는지, 승인 복구가 남긴 보존본의 bytes가 기록과 같은지만 본다. 복원된 파일 내용의 의미나 실제 Monitor 화면은 판정하지 않는다.',
  '이 결과는 environment에 적힌 OS·CPU에서 한 번 실행한 기록이다. 다른 OS·CPU에서의 실행 성공을 뜻하지 않는다.',
];

export function newResult(meta) {
  return { schema: SCHEMA, status: 'running', ...meta, cases: [], summary: null, incomplete: [], limits: LIMITS };
}

/** Internal per-run data that must not be written (block ids, full analyses). */
const side = new Map();

export async function runBench(result, plan, opts) {
  for (const c of plan.cases) {
    const row = { id: c.id, scale: c.scale, axes: c.axes, estimated_calls: c.estimated_calls, status: 'running', runs: [] };
    result.cases.push(row);
    for (let r = 1; r <= plan.repeat; r += 1) {
      opts.onProgress?.(`${c.id} (${c.key}) run ${r}/${plan.repeat}`);
      const recordingName = `${c.id}-r${r}.jsonl`;
      const run = await runOnce({ ...opts, scale: c.scale, reads: plan.reads, recordingName });
      run.run = r;
      if (opts.recordingsDir && run.recording_lines) {
        writeFileSync(join(opts.recordingsDir, recordingName), run.recording_lines, { flag: 'wx' });
        run.recording = `recordings/${recordingName}`;
      } else run.recording = null;
      delete run.recording_lines;
      side.set(`${c.id}/${r}`, { analysis: run.metrics?._analysis ?? null, profiles: Object.fromEntries(run.reads.map((x) => [x.tool, x._profile])) });
      if (run.metrics) delete run.metrics._analysis;
      for (const x of run.reads) delete x._profile;
      row.runs.push(run);
      if (run.status === 'unsupported' || run.status === 'generation_failed') break;
    }
    finishCase(row);
  }
  return finalize(result, plan);
}

export function finishCase(row) {
  row.status = row.runs.length ? worst(row.runs.map((r) => r.status)) : 'not_run';
  row.repeat = compareRepeats(row.runs);
  const done = row.runs.filter((r) => r.status === 'complete');
  const a = side.get(`${row.id}/${done[0]?.run}`)?.analysis; const b = side.get(`${row.id}/${done[1]?.run}`)?.analysis;
  // Repeat comparison through the context-metrics public compare (same call counts by construction).
  row.repeat_context_metrics = a && b ? compareAnalyses(a, b).totals : null;
}

/**
 * What survived failures and recoveries, over every finished run (repeats included): one row per
 * case with the failed attempts, the recovery path, the lineage checks and the preserved originals.
 */
export function preservation(result) {
  const rows = [];
  for (const c of result.cases) {
    const runs = c.runs.filter((r) => r.status === 'complete' && r.lineage);
    if (!runs.length) continue;
    const first = runs[0];
    const checks = Object.fromEntries(first.lineage.checks.map((k) => [k.name, { expected: k.expected, observed: k.observed, verdict: k.verdict }]));
    const kept = first.storage?.preserved ?? null;
    rows.push({
      case_id: c.id, failed_attempts: first.lineage.failed_attempts, runs: runs.length,
      recovery_path: first.generation?.recovery?.path ?? null,
      recoveries: first.generation?.recovery?.recoveries ?? 0,
      synthetic_approvals: first.generation?.recovery?.synthetic_approvals ?? 0,
      checks,
      holds_in_every_run: runs.every((r) => lineageHolds(r.lineage) === true) ? true : runs.some((r) => lineageHolds(r.lineage) === false) ? false : null,
      violations: runs.reduce((n, r) => n + r.lineage.violations.length, 0),
      status_names_revisit_source: first.lineage.status_names_revisit_source,
      preserved: kept ? { archives: `${kept.archives}/${kept.expected}`, originals: `${kept.originals_verified}/${kept.original_files}`, capability_in_storage: kept.capability_in_storage, problems: kept.problems.length } : null,
      preserved_ok_in_every_run: kept ? runs.every((r) => { const p = r.storage?.preserved; return p && p.archives === p.expected && p.complete === p.expected && p.originals_verified === p.original_files && p.approvals_stored_as_approved === p.expected && p.capability_in_storage === false && !p.problems.length; }) : null,
      readonly_unchanged_in_every_run: runs.every((r) => readOnlyUnchanged(r.readonly) === true),
    });
  }
  return {
    cases: rows.length,
    cases_with_failures: rows.filter((r) => r.failed_attempts > 0).length,
    violations: rows.reduce((n, r) => n + r.violations, 0),
    rows,
  };
}

const pick = (run, tool) => run?.reads?.find((x) => x.tool === tool) ?? null;
const bytes = (m) => m?.utf8_bytes ?? null;

export function finalize(result, plan, status = null) {
  const byId = new Map(result.cases.map((c) => [c.id, c]));
  const firstComplete = (c) => c?.runs.find((r) => r.status === 'complete') ?? null;
  const axes = plan.axes.map((axis) => {
    const levels = axis.levels.map(({ level, case_id }) => {
      const c = byId.get(case_id);
      const run = firstComplete(c);
      const ctx = pick(run, 'gil_context');
      const sections = Object.fromEntries((run?.context_sections ?? []).map((s) => [s.name, s.size.utf8_bytes]));
      return {
        level, case_id, status: c?.status ?? 'not_run',
        generation_calls: run?.generation?.calls ?? null,
        gil_bytes: run?.storage?.gil_bytes ?? null,
        context_text: ctx?.text_response ?? null,
        context_structured_utf8_bytes: bytes(ctx?.structured_response),
        context_said_in_text: ctx?.said_in_text ?? null,
        reads_text_utf8_bytes: Object.fromEntries(plan.reads.map((t) => [t, bytes(pick(run, t)?.text_response)])),
        reads_structured_utf8_bytes: Object.fromEntries(plan.reads.map((t) => [t, bytes(pick(run, t)?.structured_response)])),
        reads_text_total_utf8_bytes: run ? run.reads.reduce((n, x) => n + (x.text_response?.utf8_bytes ?? 0), 0) : null,
        read_calls: run?.reads.length ?? null,
        context_sections_utf8_bytes: sections,
        context_blocks: ctx?.blocks ?? null,
        cross_call_repeated_utf8_bytes: run?.metrics?.cross_call_repeated_total?.utf8_bytes ?? null,
        line_repeat_extra_utf8_bytes: run?.line_repeats?.extra_utf8_bytes ?? null,
        line_repeat_across_tools_utf8_bytes: run?.line_repeats?.across_tools_extra_utf8_bytes ?? null,
        resolution: run?.resolution ? Object.fromEntries(run.resolution.rows.map((r) => [r.layer, r.verdict])) : null,
        violations: run?.resolution?.violations ?? null,
        readonly_unchanged: readOnlyUnchanged(run?.readonly),
        repeat_identical: c?.repeat?.identical ?? null,
        recovery_path: run?.generation?.recovery?.path ?? null,
        recoveries: run?.generation?.recovery?.recoveries ?? null,
        synthetic_recovery_approvals: run?.generation?.recovery?.synthetic_approvals ?? null,
        lineage_holds: lineageHolds(run?.lineage),
        lineage_violations: run?.lineage?.violations ?? null,
      };
    });
    const growth = [];
    for (let i = 1; i < levels.length; i += 1) {
      const p = levels[i - 1]; const q = levels[i];
      const pb = bytes(p.context_text); const qb = bytes(q.context_text);
      if (pb === null || qb === null) { growth.push({ from: p.level, to: q.level, measured: false }); continue; }
      const units = q.level - p.level;
      const sectionDelta = {};
      for (const name of new Set([...Object.keys(p.context_sections_utf8_bytes), ...Object.keys(q.context_sections_utf8_bytes)])) {
        const d = (q.context_sections_utf8_bytes[name] ?? 0) - (p.context_sections_utf8_bytes[name] ?? 0);
        if (d) sectionDelta[name] = d;
      }
      growth.push({
        from: p.level, to: q.level, measured: true,
        context_text_delta_utf8_bytes: qb - pb,
        context_text_delta_per_unit: Number(((qb - pb) / units).toFixed(1)),
        context_structured_delta_utf8_bytes: (q.context_structured_utf8_bytes ?? 0) - (p.context_structured_utf8_bytes ?? 0),
        reads_text_total_delta_utf8_bytes: (q.reads_text_total_utf8_bytes ?? 0) - (p.reads_text_total_utf8_bytes ?? 0),
        context_sections_delta_utf8_bytes: sectionDelta,
      });
    }
    const profiles = axis.levels.map(({ case_id }) => { const run = firstComplete(byId.get(case_id)); return run ? side.get(`${case_id}/${run.run}`)?.profiles?.gil_context ?? null : null; });
    return { axis: axis.axis, label: axis.label, base: axis.base, levels, growth, context_invariant_blocks: invariantAcross(profiles) };
  });
  const allProfiles = result.cases.map((c) => { const run = firstComplete(c); return run ? side.get(`${c.id}/${run.run}`)?.profiles?.gil_context ?? null : null; });
  const steepest = axes.flatMap((a) => a.growth.filter((g) => g.measured).map((g) => ({ axis: a.axis, from: g.from, to: g.to, per_unit: g.context_text_delta_per_unit, delta: g.context_text_delta_utf8_bytes })))
    .sort((x, y) => y.per_unit - x.per_unit);
  const completeRuns = result.cases.map(firstComplete).filter(Boolean);
  const coverage = {};
  for (const run of completeRuns) for (const row of run.resolution?.rows ?? []) {
    const c = coverage[row.layer] ?? (coverage[row.layer] = { layer: row.layer, expectation: row.expectation, cases: 0, tags: 0, in_context_text: 0, per_tool_text: {}, verdicts: {} });
    c.cases += 1; c.tags += row.tags; c.in_context_text += row.in_context_text ?? 0;
    for (const [tool, v] of Object.entries(row.per_tool)) c.per_tool_text[tool] = (c.per_tool_text[tool] ?? 0) + v.text;
    c.verdicts[row.verdict] = (c.verdicts[row.verdict] ?? 0) + 1;
  }
  result.summary = { axes, context_invariant_blocks_all_cases: invariantAcross(allProfiles), context_section_variability: sectionVariability(completeRuns.map((r) => r.context_sections)), layer_coverage: Object.values(coverage), steepest_context_growth: steepest, preservation: preservation(result) };
  result.incomplete = result.cases.flatMap((c) => c.runs.filter((r) => r.status !== 'complete').map((r) => ({ case_id: c.id, run: r.run, status: r.status, error: r.error, violations: r.resolution?.violations?.length ?? null })))
    .concat(result.cases.filter((c) => !c.runs.length || c.status === 'interrupted').map((c) => ({ case_id: c.id, run: null, status: c.status })));
  const notRun = plan.cases.filter((c) => !byId.has(c.id)).map((c) => ({ case_id: c.id, run: null, status: 'not_run' }));
  result.incomplete.push(...notRun);
  result.status = status ?? (result.incomplete.length ? 'incomplete' : 'complete');
  return result;
}
