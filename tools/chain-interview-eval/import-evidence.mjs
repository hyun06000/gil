#!/usr/bin/env node
// Explicit local inputs only; never execute/follow anything found inside them.
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { TextDecoder } from 'node:util';
import { sha256 } from './lib/io.mjs';
import { loadCaseSet } from './lib/cases.mjs';
import { checkResult } from './lib/results.mjs';
import { prepareEvidence } from './lib/evidence.mjs';
import { DEFAULT_CASES } from './check-fixtures.mjs';

export function parseArgs(argv) {
  const o = { cases: DEFAULT_CASES };
  const allowed = ['result', 'events', 'conversation', 'manifest', 'out', 'cases'];
  for (let i = 0; i < argv.length; i += 2) {
    const key = argv[i].slice(2);
    if (!argv[i].startsWith('--') || !allowed.includes(key) || !argv[i + 1]
      || argv[i + 1].startsWith('--') || (Object.hasOwn(o, key) && key !== 'cases')) throw new Error('invalid_arguments');
    o[key] = resolve(argv[i + 1]);
  }
  if (!['result', 'events', 'manifest', 'out'].every((k) => o[k])) throw new Error('missing_arguments');
  return o;
}
function read(path) {
  // Fatal decoding makes the embedded text hash equal to the original bytes, including BOM.
  return new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(readFileSync(path));
}
function source(pin, text) { return { id: pin.id, ref: pin.ref, sha256: sha256(text), text }; }

export function run(o) {
  const set = loadCaseSet(o.cases);
  if (set.problems.length) throw new Error('invalid_case_set');
  const result = JSON.parse(read(o.result));
  const before = checkResult(result, set);
  if (before.problems.length || before.case_set_match !== true) throw new Error('invalid_base_result');
  const manifestText = read(o.manifest); const manifest = JSON.parse(manifestText);
  const rc = result.cases.find((c) => c.case_id === manifest.case_id);
  if (!rc || manifest.run_id !== result.run.id) throw new Error('scope_mismatch');
  if (Object.hasOwn(rc, 'prepared_evidence')) throw new Error('evidence_already_present');
  if (Boolean(o.conversation) !== (manifest.conversation !== null)) throw new Error('conversation_input_mismatch');
  const sources = {
    manifest: source(manifest, manifestText),
    events: source(manifest.events, read(o.events)),
    conversation: o.conversation ? source(manifest.conversation, read(o.conversation)) : null,
  };
  const priorPlan = set.cases.find((c) => c.value.id === rc.case_id)?.value.input?.prior_state?.approved_plan;
  rc.prepared_evidence = prepareEvidence(sources, result.run.id, rc.case_id, priorPlan);
  if (checkResult(result, set).problems.length) throw new Error('invalid_import_result');
  // No mkdir, overwrite, symlink following on an existing destination, or writes to inputs.
  writeFileSync(o.out, JSON.stringify(result, null, 2) + '\n', { flag: 'wx', mode: 0o600 });
  return result;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    run(parseArgs(process.argv.slice(2)));
    process.stdout.write('evidence: prepared; execution, approval and evaluation states unchanged\n');
  } catch {
    // JSON parser/fs/checker errors may contain supplied text, paths or capability tokens.
    process.stderr.write('[chain-interview-eval] import rejected; check input format, scope, hashes, references and new output destination\n');
    process.exitCode = 2;
  }
}
