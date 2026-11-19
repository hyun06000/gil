#!/usr/bin/env node
// Split one downloaded GitHub Actions job log into non-overlapping wall-clock categories.
//
//   gh api repos/<owner>/<repo>/actions/jobs/<job_id>/logs > job.log
//   node tools/ci/job-timing.mjs job.log [--steps]
//
// Every step runs from its own `##[group]Run …` (or `Post job cleanup.`) line to the next one, so
// adjacent steps never share a second. Inside a Cargo step the compile/link part ends at the last
// `Finished \`test|dev\` profile` line and the rest is test execution (process start-up included);
// a `cargo build` step is compile time only, and a step without Cargo output is a Node check.
// The Cargo- and libtest-reported durations are summed separately: they are what the tools printed,
// not wall time, and are never added to the categories. Runner queue time is not in the log; read
// it from the run's created_at and the job's started_at in the API.
import {readFileSync} from 'node:fs';
import {pathToFileURL} from 'node:url';

const seconds = text => {
  const m = text.match(/in (?:(\d+)m )?([\d.]+)s/);
  return m ? Number(m[1] || 0) * 60 + Number(m[2]) : 0;
};

export function timing(log) {
  const rows = [];
  for (const line of log.replace(/^\uFEFF/, '').split(/\r?\n/)) {
    const m = line.match(/^(\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d+)?)Z (.*)$/);
    if (m) rows.push({t: Date.parse(`${m[1].slice(0, 23)}Z`) / 1000, text: m[2]});
  }
  if (rows.length === 0) throw new Error('no timestamped lines: is this a job log?');
  const marks = rows.flatMap((r, i) => r.text.startsWith('##[group]Run ') || r.text === 'Post job cleanup.' ? [i] : []);
  const cat = {setup: 0, cache_restore: 0, cargo_compile: 0, test_run: 0, node_checks: 0, cache_prune: 0, cache_archive: 0, cache_upload: 0, post_other: 0};
  const reported = {cargo_build: 0, libtest: 0};
  const steps = [];
  const first = rows[0].t, last = rows[rows.length - 1].t;
  cat.setup += marks.length ? rows[marks[0]].t - first : last - first;
  for (let k = 0; k < marks.length; k++) {
    const from = marks[k], to = k + 1 < marks.length ? marks[k + 1] : rows.length - 1;
    const seg = rows.slice(from, to), start = rows[from].t, end = rows[to].t, wall = end - start;
    const name = rows[from].text.replace(/^##\[group\]Run /, '');
    const finished = seg.filter(r => /Finished `(test|dev)` profile/.test(r.text));
    for (const r of finished) reported.cargo_build += seconds(r.text);
    const libtest = seg.flatMap(r => { const m = r.text.match(/^test result: .*finished in ([\d.]+)s/); return m ? [Number(m[1])] : []; });
    for (const s of libtest) reported.libtest += s;
    let kind;
    if (/^actions\/checkout|^rustup /.test(name)) { kind = 'setup'; cat.setup += wall; }
    else if (/^actions\/cache/.test(name)) { kind = 'cache_restore'; cat.cache_restore += wall; }
    else if (/prune-workspace-cache/.test(name)) { kind = 'cache_prune'; cat.cache_prune += wall; }
    else if (name === 'Post job cleanup.') {
      const sent = seg.filter(r => /^Sent \d+ of \d+/.test(r.text));
      if (sent.length) {
        // The first progress line already counts bytes sent; back it off by its own rate.
        const m = sent[0].text.match(/^Sent (\d+) of \d+.*?([\d.]+) MBs\/sec/);
        const upload = sent[0].t - (m && Number(m[2]) > 0 ? Number(m[1]) / (Number(m[2]) * 1048576) : 0);
        kind = 'cache_save'; cat.cache_archive += upload - start; cat.cache_upload += end - upload;
      } else { kind = 'post_other'; cat.post_other += wall; }
    } else if (finished.length) {
      const compile = /^cargo build/.test(name) ? wall : finished[finished.length - 1].t - start;
      kind = 'cargo'; cat.cargo_compile += compile; cat.test_run += wall - compile;
    } else { kind = 'node_checks'; cat.node_checks += wall; }
    steps.push({kind, wall: +wall.toFixed(1), cargo_finished: finished.map(r => r.text.replace(/^.* in /, '')), libtest: +libtest.reduce((a, b) => a + b, 0).toFixed(2), name: name.slice(0, 160)});
  }
  const round = o => Object.fromEntries(Object.entries(o).map(([k, v]) => [k, +v.toFixed(1)]));
  return {log_wall: +(last - first).toFixed(1), categories: round(cat), reported: round(reported), steps};
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [file, flag] = process.argv.slice(2);
  if (!file) { console.error('usage: node tools/ci/job-timing.mjs <job.log> [--steps]'); process.exit(2); }
  const result = timing(readFileSync(file, 'utf8'));
  if (flag !== '--steps') delete result.steps;
  console.log(JSON.stringify(result, null, 1));
}
