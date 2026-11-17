// Test-only Host. Reuse actual markup, CSS and companion.js without modifying their input facts.
import { setLocale } from '../../ui/locale.js';
import { chainLine, chainContrast } from '../../ui/chain.mjs';
setLocale('ko');
const read = async url => {
  const r = await fetch(url, { cache: 'no-store' });
  if (!r.ok) throw new Error(`${url}: ${r.status}`);
  return r.json();
};
const results = [];
window.CHAIN_CORE_FIXTURE_RESULTS = results;
const output = document.querySelector('#fixture-results');
const check = async (name, run) => {
  try { await run(); results.push({ name, ok: true }); }
  catch (e) { results.push({ name, ok: false, message: e.message }); }
  output.textContent = `${results.filter(r => r.ok).length}/${results.length} 통과 · 로컬 fixture Host\n` + results.map(r => `${r.ok ? 'PASS' : 'FAIL'} ${r.name}${r.message ? ': ' + r.message : ''}`).join('\n');
};
const need = (value, message) => { if (!value) throw new Error(message); };
const phases = ['first-interview','experiment-closed','closing-ready','chain-closed','next-chain'];
const names = ['legacy', ...['success','failure'].flatMap(v => phases.map(p => `${v}-${p}`))];
try {
  const packets = Object.fromEntries(await Promise.all(names.map(async name => [name,
    await read(new URL(`../../ui/fixtures/chain-core/${name}.json`, import.meta.url))])));
  const page = new DOMParser().parseFromString(await (await fetch(new URL('../../ui/index.html', import.meta.url))).text(), 'text/html');
  document.body.prepend(page.getElementById('gil-companion'));
  const id = name => `fixture:core-${name}`;
  const packet = scope => packets[scope.slice('fixture:core-'.length)];
  window.GIL_HOST = {
    listProjects: async () => names.map(n => ({ scope_id: id(n), label: n })),
    loadView: async scope => structuredClone(packet(scope).view),
    loadDetail: async (scope, ref) => structuredClone(packet(scope).details[ref] ?? null),
  };
  await import('../../ui/companion.js');
  const ui = window.GIL_COMPANION;
  await ui.ready;
  const at = selector => document.querySelector(`#gil-companion ${selector}`);
  for (const name of names) await check(name, async () => {
    const source = packets[name], before = JSON.stringify(source);
    await ui.showScope(id(name));
    need(at('.chain-now').textContent === chainLine(source.view), 'Current Chain text differs');
    need(at('.chain-contrast').textContent === chainContrast(source.view), 'Chain/Cycle contrast differs');
    if (name === 'legacy') {
      need(at('.chain-now').hidden && at('.chain-list').hidden, 'Legacy invented a Chain');
    } else {
      need(!at('.chain-now').hidden && !at('.chain-list').hidden, 'Chain facts are hidden');
      need(at('.chain-entries').children.length === source.view.chains.length, 'Wrong Chain count');
      if (name.endsWith('next-chain')) {
        const here = JSON.stringify(ui.plan().nodes.filter(n => n.here).map(n => n.ref));
        at('.chain-list').open = true;
        at('.chain-entries button').click();
        need(at('.detail').textContent.includes(source.view.chains[0].report.handoff_summary), 'Original handoff truncated');
        need(!at('.detail script') && !window.chainFixtureInjected, 'Report interpreted as executable HTML');
        need(JSON.stringify(ui.plan().nodes.filter(n => n.here).map(n => n.ref)) === here, 'Selection moved Current');
        at('.detail .chain-cycles button').click();
        need(at('.detail small').textContent.includes('사이클 보고서'), 'Old Cycle report unavailable');
        need(JSON.stringify(ui.plan().nodes.filter(n => n.here).map(n => n.ref)) === here, 'Old Cycle selection moved Current');
      }
    }
    need(JSON.stringify(source) === before, 'Host mutated original response');
  });
  // Leave a useful inspectable final view, including the original literal handoff.
  await ui.showScope(id('failure-next-chain'));
  at('.chain-list').open = true;
  ui.selectChain('chain:C1');
} catch (e) { await check('harness setup', () => { throw e; }); }
window.CHAIN_CORE_FIXTURE_DONE = true;
