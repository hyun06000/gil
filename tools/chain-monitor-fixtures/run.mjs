#!/usr/bin/env node
// Run both profiles sequentially. A local build queue may wrap this command, but is optional.
import { spawnSync } from 'node:child_process';
import { readFileSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';
const root = fileURLToPath(new URL('../../', import.meta.url));
const args = process.argv.slice(2);
if (args.length !== 1 || !['--check', '--write'].includes(args[0])) throw new Error('usage: node tools/chain-monitor-fixtures/run.mjs --check|--write');
const run = (exe, argv, env = process.env) => {
  const result = spawnSync(exe, argv, { cwd: root, env, stdio: 'inherit' });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
};
const profiles = [[], ['--features', 'approved-plan,visual-authoring,product-safety']];
const generate = (write) => {
  for (const profile of profiles) run('cargo', ['test', '--locked', '-p', 'gil', ...profile,
    '--test', 'chain_monitor_fixtures', '--', '--test-threads=1'], { ...process.env, GIL_CHAIN_MONITOR_WRITE: write ? '1' : '0' });
};
const snapshot = () => Object.fromEntries(readdirSync(resolve(root, 'ui/fixtures/chain-core')).filter(n => n.endsWith('.json')).sort()
  .map(n => [n, readFileSync(resolve(root, 'ui/fixtures/chain-core', n), 'utf8')]));
generate(args[0] === '--write');
if (args[0] === '--write') {
  const first = snapshot();
  generate(true);
  if (JSON.stringify(snapshot()) !== JSON.stringify(first)) throw new Error('Regeneration is not byte-identical');
  process.stdout.write('Two independent generations are byte-identical.\n');
}
run(process.execPath, ['--test', 'mcp-app/chain-core-fixtures.test.mjs', 'mcp-app/chain.test.mjs']);
