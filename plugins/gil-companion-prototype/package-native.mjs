// Development-only native artifact. Build tools may use Node; the installed plugin does not.
// Usage: node package-native.mjs <codex|claude> <new-output.zip>
import {mkdtemp, mkdir, copyFile, chmod, writeFile, readFile, rm, stat} from 'node:fs/promises';
import {spawnSync} from 'node:child_process';
import {join, resolve, dirname} from 'node:path';
import {tmpdir} from 'node:os';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';

const [host, destination] = process.argv.slice(2);
if (!['codex', 'claude'].includes(host) || !destination) throw new Error('Expected host and a new output zip path');
if (process.platform !== 'darwin' || process.arch !== 'arm64') throw new Error('Development artifact is macOS arm64 only');
const output = resolve(destination);
try { await stat(output); throw new Error('Refusing to overwrite an existing artifact'); }
catch (error) { if (error.code !== 'ENOENT') throw error; }
const plugin = import.meta.dirname;
const binary = join(plugin, 'core/darwin-arm64/gil');
const descriptor = spawnSync(binary, ['--gil-agent-descriptor'], {encoding: 'utf8', env: {PATH:'/usr/bin:/bin'}});
assert.equal(descriptor.status, 0, 'Core descriptor');
const identity = JSON.parse(descriptor.stdout);
assert.equal(identity.product, 'gil_core'); assert.equal(identity.os, 'macos'); assert.equal(identity.arch, 'aarch64');
const adapter = host === 'codex' ? '.codex-plugin' : '.claude-plugin';
const manifest = JSON.parse(await readFile(join(plugin, adapter, 'plugin.json'), 'utf8'));
const entry = manifest.mcpServers['gil-companion'];
assert.equal(entry.command, host === 'codex' ? './core/darwin-arm64/gil' : '${CLAUDE_PLUGIN_ROOT}/core/darwin-arm64/gil');
assert.deepEqual(entry.args, ['mcp', '--serve']);
const stage = await mkdtemp(join(tmpdir(), 'gil-native-package-'));
try {
  for (const dir of [adapter, 'core/darwin-arm64', 'skills/gil-companion']) await mkdir(join(stage, dir), {recursive: true});
  const files = [`${adapter}/plugin.json`, 'core/darwin-arm64/gil', 'skills/gil-companion/SKILL.md'];
  for (const file of files) await copyFile(join(plugin, file), join(stage, file));
  await chmod(join(stage, 'core/darwin-arm64/gil'), 0o755);
  await copyFile(join(plugin, '../../LICENSE'), join(stage, 'LICENSE')); files.push('LICENSE');
  await writeFile(join(stage, 'DEVELOPMENT.txt'),
    'GIL native MCP development acceptance artifact — macOS arm64 only.\nNot Developer ID signed or notarized. Not a public release.\nNo Node, npm, cargo, source checkout, or native Companion is needed to serve the MCP App.\nCompanion remains a separately installed optional fallback.\n');
  files.push('DEVELOPMENT.txt');
  await mkdir(dirname(output), {recursive: true});
  const zipped = spawnSync('/usr/bin/zip', ['-X', '-q', output, ...files], {cwd:stage, encoding:'utf8'});
  assert.equal(zipped.status, 0, zipped.stderr);
  const bytes = await readFile(output);
  console.log(JSON.stringify({host, output, files, bytes:bytes.length,
    sha256:createHash('sha256').update(bytes).digest('hex'),
    core_sha256:createHash('sha256').update(await readFile(binary)).digest('hex'),
    runtime:'gil mcp --serve', channel:'development_unsigned'}, null, 2));
} finally { await rm(stage, {recursive:true, force:true}); }
