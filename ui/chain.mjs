// Chain exploration — a read-only projection of the Chain facts the Monitor View already carries.
//
// What the View says (Monitor Model, M6-C projection): `current.chain_ref` names the Chain the
// Project is working in, `chain` is that Chain, and `chains` lists every Chain in issue order,
// each with its owned `cycle_refs`, parent, Entry/Exit and the stored Chain Report of a closed one.
// Legacy worlds (format 4/5) carry none of these; then `chainModel` returns null and the screen
// stays exactly as before.
//
// This module never invents a title, a verdict, an approval or a Report. A Cycle verdict is not a
// Chain verdict; an available `close_chain` action is a possibility, not a closure. Selection of a
// previous Chain is presentation state only — it changes neither Current nor any record.
import { t } from './locale.js';

const text = (value) => (value === null || value === undefined ? '' : String(value));
const shortId = (ref) => text(ref).split(':').pop();
const said = (key, fallback) => { const found = t(key); return found === key ? fallback : found; };
// The verdict is spoken in the person's words with the stored value beside it. `failure` is
// "success not confirmed", never a refutation (Monitor Model §1.2). Unknown values stay as stored.
export const verdictText = (verdict) => {
  if (verdict === null || verdict === undefined || verdict === '') return '';
  const key = `ui.verdict.${verdict}`, found = t(key);
  return found === key ? String(verdict) : `${found} (${verdict})`;
};
const stateWord = (state) => state === 'closed' ? t('ui.closed') : state === 'open' ? t('ui.open') : text(state);
const cycleName = (cycle, ref) => `${said(`ui.cycle.${cycle?.kind}`, text(cycle?.kind))} ${shortId(ref)}`.trim();

const chainsOf = (view) => Array.isArray(view?.chains) ? view.chains.filter((one) => one && typeof one === 'object') : [];
const currentRefOf = (view) => view?.current?.chain_ref || view?.chain?.chain_ref || null;
const reportOf = (chain) => chain && chain.report && typeof chain.report === 'object' ? chain.report : null;

/** One Chain as the screen speaks of it. Nothing beyond the record: no title, no inferred verdict. */
function entry(chain, view, currentRef) {
  const report = reportOf(chain);
  const ref = text(chain.chain_ref);
  const cycleRefs = Array.isArray(chain.cycle_refs) ? chain.cycle_refs.map(text) : [];
  return {
    ref, id: shortId(ref), state: chain.state === 'closed' ? 'closed' : 'open',
    isCurrent: ref === currentRef,
    verdict: report ? text(report.verdict) : '',
    handoff: report ? text(report.handoff_summary) : '',
    hasReport: !!report,
    cycleRefs,
    ownsCurrentCycle: cycleRefs.includes(text(view?.current?.cycle_ref)),
    parent: chain.parent_chain_ref ? text(chain.parent_chain_ref) : null,
  };
}

/**
 * The Chain facts of a View, or null when the record has no Chain (legacy 4/5).
 *
 * `all` keeps the View's issue order; `current` is the Chain named by `current.chain_ref`
 * (falling back to `chain` when `chains` omits it, so a single-Chain View still reads); `previous`
 * is every other Chain in that same order.
 */
export function chainModel(view) {
  const currentRef = currentRefOf(view);
  const listed = chainsOf(view);
  if (!currentRef && listed.length === 0) return null;
  const all = listed.map((one) => entry(one, view, currentRef));
  if (currentRef && !all.some((one) => one.ref === currentRef) && view.chain && typeof view.chain === 'object') {
    all.push(entry(view.chain, view, currentRef));
  }
  const current = all.find((one) => one.isCurrent) || null;
  return { current, previous: all.filter((one) => !one.isCurrent), all };
}

/** The Chain that owns a Cycle, from `cycle_refs` only. Not inferred from Cycle ancestry. */
export function chainOfCycle(view, cycleRef) {
  const model = chainModel(view);
  if (!model) return null;
  return model.all.find((one) => one.cycleRefs.includes(text(cycleRef))) || null;
}

const verdictTail = (one) => one.state === 'closed'
  ? ` · ${one.hasReport && one.verdict ? verdictText(one.verdict) : t('chain.no_report_short')}` : '';

/** The one line under the position: which Chain the work is in, and how many came before. */
export function chainLine(view) {
  const model = chainModel(view);
  if (!model) return '';
  const { current, previous } = model;
  if (!current) return t('chain.line_unknown', { id: shortId(currentRefOf(view)) });
  const head = current.state === 'closed'
    ? t('chain.line_closed', { id: current.id, verdict: current.hasReport && current.verdict ? verdictText(current.verdict) : t('chain.no_report_short') })
    : t('chain.line_open', { id: current.id });
  const before = previous.length ? t('chain.line_previous', { n: previous.length }) : t('chain.line_first');
  return `${head}${before}`;
}

/**
 * How the current Cycle's state differs from the Chain's, when the difference matters.
 * Empty when nothing needs saying — a closed Cycle inside an open Chain, a Chain that may be
 * closed or followed by a next Chain. Possibilities are named as possibilities.
 */
export function chainContrast(view) {
  const model = chainModel(view);
  if (!model || !model.current) return '';
  const current = model.current;
  const cycle = Array.isArray(view.timeline) ? view.timeline.find((one) => one.cycle_ref === view.current?.cycle_ref) : null;
  const actions = Array.isArray(view.next_actions) ? view.next_actions.map((one) => one && one.kind) : [];
  const lines = [];
  if (current.state === 'open' && cycle && cycle.state === 'closed') {
    const verdict = cycle.report ? verdictText(cycle.report.verdict) : t('chain.no_report_short');
    lines.push(t('chain.contrast_cycle_closed', { cycle: cycleName(cycle, cycle.cycle_ref), verdict, id: current.id }));
  }
  if (current.state === 'open' && actions.includes('close_chain')) lines.push(t('chain.contrast_close_possible', { id: current.id }));
  if (current.state === 'closed' && actions.includes('open_chain')) lines.push(t('chain.contrast_open_next', { id: current.id }));
  return lines.join(' ');
}

/** One row of the Chain list: label text and the data the button needs. */
export function chainListEntries(view) {
  const model = chainModel(view);
  if (!model) return [];
  return model.all.map((one) => ({
    ref: one.ref, isCurrent: one.isCurrent,
    label: `${t('chain.entry', { id: one.id, state: stateWord(one.state) })}${verdictTail(one)}${one.isCurrent ? t('chain.entry_current') : ''}`,
    aria: t('chain.select_aria', { id: one.id }),
    cycles: one.cycleRefs.length,
  }));
}

/** Report keys with a sentence of their own; any other stored key is shown under its raw name. */
const REPORT_NAMES = { verdict: 'report.verdict', handoff_summary: 'report.handoff', closing_synthesis_ref: 'chain.report_basis' };
const REPORT_ORDER = ['verdict', 'handoff_summary', 'closing_synthesis_ref'];

/**
 * The detail-panel projection of one Chain: its Report as literal fields, its owned Cycles joined
 * to the timeline for kind/state/verdict, and the record identifiers. Shaped like the Cycle
 * projection so the same panel can draw it. Returns null when the View has no such Chain.
 */
export function chainDetail(view, chainRef) {
  const model = chainModel(view);
  if (!model) return null;
  const one = model.all.find((item) => item.ref === text(chainRef));
  if (!one) return null;
  const raw = chainsOf(view).find((item) => text(item.chain_ref) === one.ref) || (view.chain && text(view.chain.chain_ref) === one.ref ? view.chain : null);
  const report = reportOf(raw);
  let fields = null;
  if (report) {
    const keys = [...REPORT_ORDER.filter((key) => Object.prototype.hasOwnProperty.call(report, key)),
      ...Object.keys(report).filter((key) => !REPORT_ORDER.includes(key)).sort()];
    fields = keys.map((key) => ({ name: REPORT_NAMES[key] ? t(REPORT_NAMES[key]) : key,
      value: key === 'verdict' ? verdictText(report[key]) : text(report[key]) }));
  }
  const timeline = Array.isArray(view.timeline) ? view.timeline : [];
  const cycles = one.cycleRefs.map((ref) => {
    const found = timeline.find((cycle) => cycle.cycle_ref === ref) || null;
    const isHere = ref === text(view.current?.cycle_ref);
    if (!found) return { ref, known: false, isHere, label: t('chain.cycle_missing', { ref }) };
    const verdict = found.state === 'closed' && found.report ? ` · ${verdictText(found.report.verdict)}` : '';
    return { ref, known: true, isHere, kind: found.kind, state: found.state,
      label: `${t('chain.cycle_entry', { cycle: cycleName(found, ref), state: stateWord(found.state) })}${verdict}${isHere ? t('chain.cycle_here') : ''}` };
  });
  const none = t('chain.ref_none');
  return {
    owner: { kind: 'chain', ref: one.ref }, chain_ref: one.ref, state: one.state, isCurrent: one.isCurrent,
    head: t('chain.detail_head', { id: one.id, state: stateWord(one.state) }),
    note: one.isCurrent ? t('chain.detail_note_current') : t('chain.detail_note_previous'),
    report: fields ? { fields } : null,
    noReport: fields ? '' : one.state === 'closed' ? t('chain.no_report_closed') : t('chain.no_report_open'),
    cyclesHead: t('chain.cycles_head', { n: cycles.length }),
    cycles,
    refs: t('chain.refs', { existence: text(raw?.existence_ref) || none, parent: one.parent || none,
      entry: text(raw?.entry_snapshot_ref) || none, exit: text(raw?.exit_snapshot_ref) || none,
      journey: text(raw?.journey_ref) || none }),
  };
}
