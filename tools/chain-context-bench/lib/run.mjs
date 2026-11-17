// One bench run = one fresh isolated project, one Core process to generate the record, then a
// second fresh Core process (a new session) for the read calls that are measured.
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { readFileSync, existsSync, rmSync, statSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { join, dirname, relative, isAbsolute } from 'node:path';
import { execFileSync } from 'node:child_process';
import { CoreClient, isolatedEnv, killGroup, runCommand } from '../../scenarios/lib/rpc.mjs';
import { projectStatePath } from '../../scenarios/lib/state-path.mjs';
import { makeRedactor, leaks } from '../../scenarios/lib/redact.mjs';
import { parseRecording, textOf } from '../../context-metrics/lib/events.mjs';
import { analyzeRecording } from '../../context-metrics/lib/analyze.mjs';
import { stableStringify } from '../../context-metrics/lib/size.mjs';
import { generate, UnsupportedError, RECOVERY, recoveryPathFor } from './scenario.mjs';
import { treeDigest, snapshotReadRoots, compareReadRoots, readOnlyUnchanged, segmentContext, evaluateResolution, evaluateLineage, inspectPreserved, blockProfile, lineRepeats, normalizedSha, measure, sha256 } from './measure.mjs';

/**
 * Live resources, so an interrupt can clean up synchronously. `secrets` holds every private
 * capability the Core handed to the harness in this process (plan and recovery approvals); it is
 * only ever used to refuse writing a result that still contains one.
 */
export const live = { clients: new Set(), workDirs: new Set(), keep: false, secrets: new Set() };

/** How a generation error is classified in a result (see scenario.mjs GenerationError). */
export function errorKind(err) {
  if (err?.kind) return err.kind;
  return /timeout after|Core exited|stdin is closed/.test(String(err?.message ?? '')) ? 'transport' : 'harness';
}

export function cleanupSync() {
  for (const c of live.clients) { try { killGroup(c.child, 'SIGKILL'); } catch {} }
  live.clients.clear();
  if (!live.keep) for (const d of live.workDirs) { try { rmSync(d, { recursive: true, force: true }); } catch {} }
  live.workDirs.clear();
}

export async function buildCore({ repo, features }) {
  const args = ['build', '--locked', '-p', 'gil', '--features', features, '--message-format=json-render-diagnostics'];
  process.stderr.write(`[bench] cargo ${args.join(' ')}\n`);
  const r = await runCommand('cargo', args, { cwd: repo, timeoutMs: 1800000, onStderr: (s) => process.stderr.write(s) });
  if (r.timedOut || r.code !== 0) throw new Error(`cargo build failed (exit ${r.code}${r.timedOut ? ', timeout' : ''})`);
  let executable = null;
  for (const line of r.stdout.split('\n')) {
    if (!line.startsWith('{')) continue;
    try { const m = JSON.parse(line); if (m.reason === 'compiler-artifact' && m.target?.name === 'gil' && (m.target.kind || []).includes('bin') && m.executable) executable = m.executable; } catch {}
  }
  if (!executable) throw new Error('cargo did not report the gil executable');
  return executable;
}

const git = (repo, args) => { try { return execFileSync('git', args, { cwd: repo, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim(); } catch { return null; } };

/** Commit and working-tree state of the source checkout; paths are repository-relative. */
export function sourceInfo(repo) {
  const porcelain = git(repo, ['status', '--porcelain', '--untracked-files=all']);
  const entries = porcelain === null ? null : porcelain.split('\n').filter(Boolean);
  return { commit: git(repo, ['rev-parse', 'HEAD']), dirty: entries === null ? null : entries.length > 0, dirty_entries: entries === null ? null : entries.slice(0, 50), dirty_entry_count: entries?.length ?? null };
}

/** A binary path without the user's home: relative to the checkout or the main checkout. */
export function binaryLabel(repo, binary) {
  const common = git(repo, ['rev-parse', '--path-format=absolute', '--git-common-dir']);
  const bases = [['<worktree>', repo], ['<main-checkout>', common ? dirname(common) : null]];
  for (const [label, base] of bases) {
    if (!base) continue;
    const rel = relative(base, binary);
    if (rel && !rel.startsWith('..') && !isAbsolute(rel)) return `${label}/${rel.split('\\').join('/')}`;
  }
  return `<external>/${binary.split(/[\\/]/).pop()}`;
}

export function fileSha256(path) { return createHash('sha256').update(readFileSync(path)).digest('hex'); }

/**
 * Generate one record and measure its reads. Never throws for Core behavior: failures become
 * the run's status. `opts`: binary, features, scale, reads, locale, timeoutMs, workRoot, keep,
 * localeTable markers, recordingPath (or null), modelRevisitBlockedByProfile (the declared profile
 * refuses a model's gil_revisit by design).
 */
export async function runOnce(opts) {
  const { binary, scale, reads, locale, timeoutMs, workRoot, markers, typedVerify, modelRevisitBlockedByProfile = false } = opts;
  const started = Date.now();
  const work = await mkdtemp(join(workRoot, 'ccb-'));
  live.workDirs.add(work);
  const root = join(work, 'project'); const home = join(work, 'home'); const settings = join(work, 'settings'); const localAppData = join(work, 'localappdata');
  for (const d of [root, home, settings, localAppData]) await mkdir(d);
  const childEnv = isolatedEnv({ home, localAppData });
  const redactor = makeRedactor({ roots: { project_root: root, settings_dir: settings, home_dir: home, localappdata_dir: localAppData, work_dir: work, binary_dir: dirname(binary) }, maxString: Infinity });
  const out = { status: 'complete', error: null, generation: null, storage: null, reads: [], context_sections: null, resolution: null, lineage: null, readonly: null, metrics: null, observed: {}, recording_lines: null, wall_ms: null };
  const secrets = [];
  let client = null;
  const connect = async (label) => {
    client = new CoreClient({ binary, settingsDir: join(settings, 'bindings'), isolated: childEnv, requestTimeoutMs: timeoutMs, label }).start();
    live.clients.add(client);
    await client.initialize({ name: 'gil-chain-context-bench', version: '0' });
    return client;
  };
  const disconnect = async () => { if (!client) return; const c = client; client = null; try { await c.close(3000); } finally { live.clients.delete(c); } };
  try {
    // ── generation ─────────────────────────────────────────────────────────────────────────
    let calls = 0; let refusedCalls = 0;
    const genStarted = Date.now();
    await connect('generate');
    const listed = await client.request('tools/list', {});
    const names = (listed.result?.tools ?? []).map((t) => t.name).sort();
    const recovery = recoveryPathFor(names);
    out.observed = { tools: names.length, tools_sha256: sha256(JSON.stringify(names)), approved_plan_tools: names.includes('gil_plan_propose'), revisit_tool: names.includes('gil_revisit'), revisit_approval_tools: recovery === RECOVERY.app_approval, recovery_path: recovery };
    const wrap = async (name, args) => { calls += 1; const r = await client.call(name, args); if (!r.ok) refusedCalls += 1; return r; };
    const g = {
      root, locale, typedVerify, recovery, modelRevisitBlockedByProfile,
      model: wrap, app: wrap,
      onSecret: (s) => { redactor.addSecret(s); if (typeof s === 'string' && s.length >= 8) { secrets.push(s); live.secrets.add(s); } },
      writeProjectFile: async (rel, text) => { const p = join(root, rel); await mkdir(dirname(p), { recursive: true }); await writeFile(p, text); },
    };
    let generated = null;
    try {
      generated = await generate(g, scale);
    } catch (err) {
      out.status = err instanceof UnsupportedError ? 'unsupported' : 'generation_failed';
      out.error = { step: err.step ?? 'harness', kind: errorKind(err), detail: redactor.text(String(err.detail ?? err.message)).slice(0, 600) };
    }
    out.generation = {
      calls, refused_calls: refusedCalls, wall_ms: Date.now() - genStarted, counts: generated?.counts ?? null, current: generated?.current ?? null,
      // How failed Cycles were left. The App path's approvals are harness input, never a person's click.
      recovery: generated ? {
        path: generated.recoveries.length ? generated.recoveries[0].path : null, available_path: recovery, recoveries: generated.recoveries.length,
        synthetic_approvals: generated.counts.recovery_approvals, approval_input: generated.counts.recovery_approvals ? 'synthetic harness input through the App-only tools; not a person\'s Monitor click' : null,
        branch_point: `cycle:C${generated.branch_point}`,
        steps: generated.recoveries.map((r) => ({ path: r.path, from: r.from, target: r.target, changes: r.changes, policy: r.policy, core_approval_source: r.core_approval_source })),
      } : null,
    };
    await disconnect();
    if (!generated) return out;

    const gilDir = join(root, '.gil');
    const record = projectStatePath(root, { env: childEnv });
    const gilTree = existsSync(gilDir) ? treeDigest(gilDir) : null;
    out.storage = { record_layout: record.layout, state_utf8_bytes: existsSync(record.path) ? statSync(record.path).size : null, gil_files: gilTree?.files ?? null, gil_bytes: gilTree?.bytes ?? null };
    // Originals an approved recovery preserved, read before the measured reads start.
    out.storage.preserved = generated.recoveries.some((r) => r.path === RECOVERY.app_approval) ? inspectPreserved(dirname(record.path), generated.recoveries, secrets) : null;

    // ── measured reads in a fresh Core process (a new session) ─────────────────────────────
    // Windows stores the record outside the project. Observe the resolved record tree too.
    const readRoots = { project: root, settings, record: dirname(record.path) };
    const before = snapshotReadRoots(readRoots);
    await connect('read');
    const events = [];
    const rawReads = [];
    let seq = 0;
    for (const tool of reads) {
      const args = { project_root: root };
      const t0 = Date.now();
      let r = null; let note = null;
      try { r = await client.call(tool, args); } catch (err) { note = redactor.text(err.message).slice(0, 200); }
      seq += 1;
      // A timeout or a lost process is recorded as a missing result, never dropped.
      events.push({ seq, at: null, actor: 'harness', synthetic: true, request: { name: tool, arguments: redactor.redact(args) }, result: r?.result ? redactor.redact(r.result) : null, error: r?.error ? redactor.redact(r.error) : null });
      rawReads.push({ tool, raw_text: textOf(r?.result).text, ms: Date.now() - t0, note });
      if (note && client.exit) { await disconnect(); await connect('read-retry'); }
    }
    await disconnect();
    out.readonly = compareReadRoots(before, snapshotReadRoots(readRoots));

    // ── measurement through tools/context-metrics ─────────────────────────────────────────
    const measured = measureReadPhase({ events, rawReads, tags: generated.tags, markers, expectedReads: reads, recordingName: opts.recordingName ?? null, walk: { failed: generated.failed, current: generated.current } });
    // A capability must never reach a recording: refuse the whole read phase rather than write it.
    if (leaks(measured.recording, { secrets }).length) throw new Error('a private approval capability reached a read response; the recording was not kept');
    Object.assign(out, measured.fields);
    if (measured.missing) out.status = 'measurement_missing';
    if (readOnlyUnchanged(out.readonly) !== true) out.status = out.status === 'complete' ? 'record_changed_by_read' : out.status;
    return out;
  } catch (err) {
    out.status = out.status === 'complete' ? 'generation_failed' : out.status;
    out.error = out.error ?? { step: 'harness', kind: errorKind(err), detail: redactor.text(String(err.message)).slice(0, 600) };
    return out;
  } finally {
    if (client) { try { await client.kill(); } catch {} live.clients.delete(client); }
    out.wall_ms = Date.now() - started;
    if (!opts.keep) { await rm(work, { recursive: true, force: true }); live.workDirs.delete(work); }
  }
}

/**
 * Measure one read phase. `events` are redacted recording events (context-metrics format);
 * `rawReads[i]` holds the unredacted text and timing of event i. Pure: no I/O.
 * A read without a usable result, or fewer recorded reads than requested, sets `missing`.
 */
export function measureReadPhase({ events, rawReads, tags, markers, expectedReads, recordingName = null, walk = null }) {
  const recording = events.map((e) => JSON.stringify(e)).join('\n') + '\n';
  const analysis = analyzeRecording(parseRecording(recording), { source: { file_name: recordingName, sha256: sha256(recording) } });
  const metrics = { status_totals: analysis.status_totals, markers: analysis.markers, totals: { text_response: analysis.totals.text_response, structured_response: analysis.totals.structured_response, record_line: analysis.totals.record_line }, by_tool: analysis.by_tool.map((t) => ({ tool: t.tool, calls: t.calls, by_status: t.by_status, text_response: t.text_response, structured_response: t.structured_response })), cross_call_repeated_blocks: analysis.repetition_candidates.blocks.length, cross_call_repeated_total: analysis.repetition_candidates.repeated_total, cross_call_repeated_tools: analysis.repetition_candidates.blocks.map((b) => ({ id: b.id, tools: b.tools, utf8_bytes: b.size.utf8_bytes, occurrences: b.occurrences })), identical_text_responses: analysis.repetition_candidates.identical_text_responses.length, limits: analysis.limits, _analysis: analysis };
  const parsed = parseRecording(recording).events;
  const reads = [];
  const readsForTags = [];
  for (let i = 0; i < parsed.length; i += 1) {
    const e = parsed[i];
    const raw = rawReads[i] ?? { raw_text: null, ms: null, note: null };
    const { text, parts, list } = textOf(e.result);
    const sc = e.result?.structuredContent;
    const structured = sc && typeof sc === 'object' ? stableStringify(sc) : null;
    const said = typeof sc?.said === 'string' ? sc.said : null;
    const profile = text === null ? null : blockProfile(list);
    reads.push({
      tool: e.name, status: e.status, note: raw.note ?? null,
      text_response: measure(text), text_parts: parts,
      structured_response: measure(structured),
      said: measure(said), said_in_text: said !== null && text !== null ? text.includes(said) : null,
      raw_text_utf8_bytes: typeof raw.raw_text === 'string' ? Buffer.byteLength(raw.raw_text, 'utf8') : null,
      text_sha: normalizedSha(text), structured_sha: normalizedSha(structured),
      wall_ms: raw.ms,
      blocks: profile ? { blocks: profile.blocks, exact_repeats: profile.exact_repeats, template_repeats: profile.template_repeats } : null,
      _profile: profile,
    });
    readsForTags.push({ tool: e.name, status: e.status, text, structured, parts: list });
  }
  const ctx = readsForTags.find((r) => r.tool === 'gil_context' && r.status === 'ok');
  const missingTools = expectedReads.filter((t) => !readsForTags.some((r) => r.tool === t && r.status === 'ok'));
  const sections = ctx?.text && markers?.length ? segmentContext(ctx.parts, markers) : null;
  return {
    recording,
    missing: missingTools.length > 0 || parsed.length !== expectedReads.length,
    fields: {
      recording_lines: recording,
      metrics,
      reads,
      missing_reads: missingTools,
      context_sections: sections,
      resolution: evaluateResolution(tags, readsForTags),
      lineage: walk ? evaluateLineage(walk, tags, readsForTags, sections) : null,
      line_repeats: lineRepeats(readsForTags.filter((r) => r.status === 'ok')),
    },
  };
}
