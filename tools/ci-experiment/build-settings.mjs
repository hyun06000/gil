#!/usr/bin/env node
// Temporary CI build-setting experiment (never merged). For each debuginfo value it builds the
// locked dependencies once (CI cache state), then times one feature group's Cargo commands with
// incremental on and off, removing only the workspace outputs between runs, and measures the
// dependency-only output size and its tar+zstd archive.
import {spawnSync} from 'node:child_process';
import {readdirSync, lstatSync, rmSync, statSync, mkdirSync, writeFileSync} from 'node:fs';
import {join} from 'node:path';
const F = 'will-end-history,visual-authoring,product-safety';
const group = [
  ['test', '--lib', '--no-run'],
  ['test', '--test', 'will_end_visual'],
  ['test', '--test', 'will_end_session'],
  ['test', '--test', 'visual_start', '--test', 'chain_resume_benchmark', '--test', 'mcp', '--test', 'chain_handoff', '--test', 'visual_attachment'],
  ['build', '--all-targets'],
];
const debugs = (process.env.EXP_DEBUGS || '2,line-tables-only,0').split(',');
const incs = (process.env.EXP_INCS || '1,0').split(',');
const out = process.env.EXP_OUT || 'exp-out'; mkdirSync(out, {recursive: true});
const results = [];
const size = p => { let n = 0, files = 0; const walk = q => { let s; try { s = lstatSync(q); } catch { return; } if (s.isDirectory()) for (const e of readdirSync(q)) walk(join(q, e)); else { n += s.size; files++; } }; walk(p); return {bytes: n, files}; };
const run = (args, env) => { const t = process.hrtime.bigint(); const r = spawnSync('cargo', args, {env, encoding: 'utf8', maxBuffer: 1 << 28}); const wall = Number(process.hrtime.bigint() - t) / 1e9; return {r, wall}; };
const finished = text => [...text.matchAll(/Finished `(\w+)` profile \[([^\]]+)\] target\(s\) in (?:(\d+)m )?([\d.]+)s/g)].map(m => ({profile: m[1], label: m[2], s: Number(m[3] || 0) * 60 + Number(m[4])}));
const libtest = text => [...text.matchAll(/^test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored; \d+ measured; (\d+) filtered out; finished in ([\d.]+)s/gm)].map(m => m.slice(1).map(Number));
for (const debug of debugs) {
  const target = join(process.cwd(), `target-exp-${debug}`);
  const env = {...process.env, CARGO_TARGET_DIR: target, CARGO_PROFILE_DEV_DEBUG: debug};
  delete env.CARGO_INCREMENTAL;
  const deps = run(['build', '--locked', '-p', 'gil', '--all-targets', '--features', F], env);
  if (deps.r.status !== 0) { console.error(deps.r.stderr); process.exit(1); }
  const clean = () => { const c = spawnSync('cargo', ['clean', '--locked', '-p', 'gil', '-p', 'gil-companion', '--target-dir', target], {env, encoding: 'utf8'}); if (c.status) { console.error(c.stderr); process.exit(1); } return c.stderr.trim().split('\n').pop(); };
  clean();
  const depSize = size(join(target, 'debug'));
  const t = process.hrtime.bigint();
  const tarRes = spawnSync('tar', ['--posix', '-cf', join(out, `deps-${debug}.tzst`), '-P', '-C', process.cwd(), '--use-compress-program', 'zstd -T0', `target-exp-${debug}/debug`], {encoding: 'utf8', shell: false});
  const archive = {wall: Number(process.hrtime.bigint() - t) / 1e9, status: tarRes.status, err: (tarRes.stderr || '').slice(0, 300)};
  try { archive.bytes = statSync(join(out, `deps-${debug}.tzst`)).size; rmSync(join(out, `deps-${debug}.tzst`)); } catch {}
  console.log(JSON.stringify({debug, deps_build_wall: deps.wall, deps_finished: finished(deps.r.stderr), deps_only: depSize, archive}));
  for (const inc of incs) {
    const e = {...env, CARGO_INCREMENTAL: inc};
    const steps = [];
    for (const g of group) {
      const args = [g[0], '--locked', '-p', 'gil', '--features', F, ...g.slice(1)];
      const {r, wall} = run(args, e);
      steps.push({cmd: args.join(' '), status: r.status, wall, finished: finished(r.stderr), libtest: libtest(r.stdout)});
      if (r.status !== 0) { console.error(r.stdout.slice(-3000), r.stderr.slice(-3000)); process.exit(1); }
    }
    const workspace = size(join(target, 'debug'));
    const removed = clean();
    const row = {debug, incremental: inc, wall: steps.reduce((a, s) => a + s.wall, 0), cargo_build: steps.reduce((a, s) => a + s.finished.reduce((b, f) => b + f.s, 0), 0), tests: steps.reduce((a, s) => a + s.libtest.reduce((b, l) => b + l[4], 0), 0), passed: steps.reduce((a, s) => a + s.libtest.reduce((b, l) => b + l[0], 0), 0), target_after_group: workspace, removed, steps};
    results.push(row);
    console.log(JSON.stringify({debug: row.debug, incremental: inc, wall: row.wall.toFixed(1), cargo_build: row.cargo_build.toFixed(1), tests: row.tests.toFixed(1), passed: row.passed, target_after_group_gib: (workspace.bytes / 2 ** 30).toFixed(2), removed, labels: [...new Set(steps.flatMap(s => s.finished.map(f => f.label)))]}));
  }
  rmSync(target, {recursive: true, force: true});
}
writeFileSync(join(out, 'results.json'), JSON.stringify(results, null, 1));
