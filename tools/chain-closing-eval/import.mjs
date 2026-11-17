#!/usr/bin/env node
import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { TextDecoder } from 'node:util';
import { loadCases } from './lib/package.mjs';
import { importEvidence, ImportError } from './lib/import.mjs';
import { sha256 } from '../chain-interview-eval/lib/io.mjs';

export function main(args) {
  const opts = {};
  for (let i = 0; i < args.length; i += 2) {
    const key = args[i];
    if (!['--input', '--out'].includes(key) || Object.hasOwn(opts, key)
      || !args[i + 1] || args[i + 1].startsWith('--')) throw new Error('usage');
    opts[key] = args[i + 1];
  }
  if (!opts['--input'] || !opts['--out']) throw new Error('usage');
  // These two CLI paths are the only user-supplied paths opened by this command.
  const bytes = readFileSync(opts['--input']);
  // Reject damaged source text instead of silently replacing bytes in evidence.
  const text = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(bytes);
  const result = importEvidence(JSON.parse(text), loadCases());
  result.import_evidence.input_sha256 = sha256(bytes);
  writeFileSync(opts['--out'], JSON.stringify(result, null, 2) + '\n', { flag: 'wx', mode: 0o600 });
  const imported = result.import_evidence.cases.filter(c => c.state === 'imported').length;
  const prepared = result.import_evidence.cases.length - imported;
  const failures = result.cases.flatMap(c => c.automatic).filter(x => x.status === 'fail').length;
  console.log(`Imported: ${imported}; preparation only: ${prepared}; automatic failures: ${failures}; semantic evaluation: not performed.`);
  return 0;
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { process.exitCode = main(process.argv.slice(2)); }
  catch (error) {
    // No user strings, paths, exception stacks, messages or tokens on either stream.
    console.error(error instanceof ImportError ? `Import rejected: ${error.code}.`
      : 'Import input/output error. Use --input JSON --out NEW_JSON; existing files are never overwritten.');
    process.exitCode = error instanceof ImportError ? 1 : 2;
  }
}
