// Korean Markdown summary of a bench result. Numbers, refs and statuses only — no response text.
import { table } from '../../scenarios/lib/markdown.mjs';

const n = (v) => (v === null || v === undefined ? '—' : String(v));
const yes = (v) => (v === null || v === undefined ? '—' : v ? '예' : '아니오');
const VERDICT = { ok: '맞음', missing: '누락', expanded: '펼쳐짐', missing_in_bundle: '읽기 호출 전체에서 누락', not_measured: '측정 안 됨' };

export function formatMarkdown(r) {
  const out = [];
  out.push(`# Chain context bench — ${r.status}`, '');
  out.push('개발용 측정 기록이다. 크기는 바이트·코드 포인트이며 **토큰 수가 아니다.** 모델을 호출하지 않았다.', '');
  out.push('## 실행 조건', '');
  out.push(table(['항목', '값'], [
    ['schema / tool', `${r.schema} / ${r.tool_version}`],
    ['source commit', `${n(r.source?.commit)} · dirty ${yes(r.source?.dirty)} (${n(r.source?.dirty_entry_count)} entries)`],
    ['Core binary', `${n(r.core?.binary)} · sha256 ${n(r.core?.sha256)}`],
    ['binary 출처', n(r.core?.provenance)],
    ['features (선언)', n(r.core?.features_declared)],
    ['관측', r.core?.observed ? `tools ${r.core.observed.tools} · plan 도구 ${yes(r.core.observed.approved_plan_tools)} · revisit 도구 ${yes(r.core.observed.revisit_tool)}` : '—'],
    ['locale', n(r.conditions?.locale)],
    ['읽기 호출', (r.conditions?.reads ?? []).join(', ')],
    ['측정 방식', n(r.conditions?.measurement)],
    ['base', Object.entries(r.plan?.base ?? {}).map(([k, v]) => `${k}=${v}`).join(' ')],
    ['반복 / 사례', `${n(r.plan?.repeat)} / ${n(r.plan?.cases?.length)}`],
    ['예상·상한 호출', `${n(r.plan?.estimated_calls)} / ${n(r.plan?.max_calls)}`],
    ['환경', `${n(r.environment?.platform)}-${n(r.environment?.arch)} · node ${n(r.environment?.node)}`],
  ]), '');
  if (!r.summary) { out.push('요약 없음(실행이 시작되기 전에 끝났다).', ''); return out.join('\n'); }

  const reads = r.conditions?.reads ?? [];
  for (const axis of r.summary.axes) {
    out.push(`## 축: ${axis.axis} — ${axis.label} (base ${axis.base})`, '');
    out.push(table(['level', 'case', '상태', '생성 호출', '.gil B', 'context text B', 'context 문자', 'context structured B', 'said⊂text', ...reads.filter((t) => t !== 'gil_context').map((t) => `${t} text B`), '읽기 text 합 B', '반복 동일', '읽기 전후 기록 동일'],
      axis.levels.map((l) => [l.level, l.case_id, l.status, n(l.generation_calls), n(l.gil_bytes), n(l.context_text?.utf8_bytes), n(l.context_text?.code_points), n(l.context_structured_utf8_bytes), yes(l.context_said_in_text), ...reads.filter((t) => t !== 'gil_context').map((t) => n(l.reads_text_utf8_bytes[t])), n(l.reads_text_total_utf8_bytes), yes(l.repeat_identical), yes(l.readonly_unchanged)])), '');
    const g = axis.growth.filter((x) => x.measured);
    if (g.length) {
      out.push(table(['구간', 'context text Δ B', 'Δ/단위', 'structured Δ B', '읽기 text 합 Δ B', '늘어난 context 절 (B)'],
        g.map((x) => [`${x.from}→${x.to}`, x.context_text_delta_utf8_bytes, x.context_text_delta_per_unit, x.context_structured_delta_utf8_bytes, x.reads_text_total_delta_utf8_bytes, Object.entries(x.context_sections_delta_utf8_bytes).sort((a, b) => b[1] - a[1]).map(([k, v]) => `${k} ${v > 0 ? '+' : ''}${v}`).join(', ') || '—'])), '');
    }
    const layers = [...new Set(axis.levels.flatMap((l) => Object.keys(l.resolution ?? {})))];
    if (layers.length) {
      out.push(table(['해상도 점검', ...axis.levels.map((l) => `level ${l.level}`)], layers.map((layer) => [layer, ...axis.levels.map((l) => (l.resolution?.[layer] ? VERDICT[l.resolution[layer]] ?? l.resolution[layer] : '—'))])), '');
    }
    const inv = axis.context_invariant_blocks;
    if (inv) {
      out.push(`gil_context 단락 중 이 축의 모든 level에 똑같이 있는 것: ${inv.shared_blocks}개 (${inv.shared_utf8_bytes_at_first_level} B). level별 고정/변동: ${inv.per_level.map((p, i) => `L${axis.levels[i]?.level}=${p.shared_utf8_bytes}/${p.varying_utf8_bytes}`).join(', ')} B.`, '');
    }
    const rep = axis.levels.map((l) => l.context_blocks ? `L${l.level}: 단락 ${l.context_blocks.blocks}, 같은 응답 안 동일 단락 반복 +${l.context_blocks.exact_repeats.extra_occurrences} (${l.context_blocks.exact_repeats.extra_utf8_bytes} B), 같은 틀 단락 ${l.context_blocks.template_repeats.occurrences}개 (${l.context_blocks.template_repeats.utf8_bytes} B)` : null).filter(Boolean);
    if (rep.length) out.push(`반복 근거(gil_context 한 응답 안): ${rep.join(' · ')}.`, '');
    out.push(`읽기 호출 사이에 똑같이 반복된 단락(context-metrics 반복 후보, 첫 등장 제외): ${axis.levels.map((l) => `L${l.level}=${n(l.cross_call_repeated_utf8_bytes)} B`).join(', ')}. 40자 이상 같은 줄의 반복분(첫 등장 제외, 그중 다른 호출 사이): ${axis.levels.map((l) => `L${l.level}=${n(l.line_repeat_extra_utf8_bytes)}(${n(l.line_repeat_across_tools_utf8_bytes)}) B`).join(', ')}.`, '');
  }
  const inv = r.summary.context_invariant_blocks_all_cases;
  if (inv) out.push('## 모든 사례에 공통인 gil_context 단락', '', `${inv.levels}개 사례 모두에 똑같이 있는 단락 ${inv.shared_blocks}개, ${inv.shared_utf8_bytes_at_first_level} B. 상태와 무관하게 매 context에 실리는 고정 안내 후보다(필요성 판단은 하지 않는다).`, '');
  if (r.summary.context_section_variability?.length) {
    out.push('## gil_context 절별 크기 변화 (전체 사례)', '');
    out.push(table(['절', '나타난 사례', '최소 B', '최대 B', '모든 사례에 같은 크기'], r.summary.context_section_variability.map((s) => [s.name, s.cases, s.min, s.max, yes(s.constant)])), '');
  }
  if (r.summary.layer_coverage?.length) {
    out.push('## 기록 계층별 도착 위치 (전체 사례, 태그 수)', '');
    const tools = [...new Set(r.summary.layer_coverage.flatMap((c) => Object.keys(c.per_tool_text)))];
    out.push(table(['계층', 'context 기대', '사례', '태그', ...tools.map((t) => `${t} text`), '판정'], r.summary.layer_coverage.map((c) => [c.layer, c.expectation, c.cases, c.tags, ...tools.map((t) => c.per_tool_text[t] ?? 0), Object.entries(c.verdicts).map(([k, v]) => `${VERDICT[k] ?? k} ${v}`).join(', ')])), '');
  }
  if (r.summary.steepest_context_growth.length) {
    out.push('## gil_context text 증가가 큰 구간 (단위당 바이트)', '');
    out.push(table(['축', '구간', 'Δ/단위 B', 'Δ B'], r.summary.steepest_context_growth.slice(0, 8).map((s) => [s.axis, `${s.from}→${s.to}`, s.per_unit, s.delta])), '');
  }
  out.push('## 재현성', '');
  out.push(table(['case', '비교 실행', '모두 동일', 'tool별 text B'], r.cases.map((c) => [c.id, n(c.repeat?.compared), yes(c.repeat?.identical), (c.repeat?.rows ?? []).map((x) => `${x.tool} ${x.text_utf8_bytes.join('/')}${x.text_content_identical ? '' : ' (내용 다름)'}`).join(' · ') || '—'])), '');
  out.push('## 중단·생성 실패·측정 누락', '');
  if (!r.incomplete.length) out.push('없음.', '');
  else out.push(table(['case', 'run', '상태', '자리', '내용'], r.incomplete.map((x) => [x.case_id, n(x.run), x.status, n(x.error?.step), n(x.error?.detail?.split('\n')[0]?.slice(0, 160))])), '');
  out.push('## 측정 한계', '', ...r.limits.map((l) => `- ${l}`), '');
  return out.join('\n');
}
