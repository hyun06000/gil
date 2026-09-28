// Synthetic unit-test input only. Never used by the release/preview builder.
import {mkdir, writeFile} from 'node:fs/promises';
import {join, dirname} from 'node:path';
import {DIRECTORY, INPUTS, TARGET, sha256} from './notices.mjs';

export async function noticeFixture(root) {
  const original = 'Fixture copyright\r\nFixture permission text (no final newline)';
  const stdlib = '<!doctype html><title>Synthetic standard library notice</title>';
  const textHash = sha256(original), stdHash = sha256(stdlib);
  const inputs = {};
  for (const path of INPUTS) {
    const data = `synthetic ${path}\n`; inputs[path] = sha256(data);
    await mkdir(dirname(join(root, path)), {recursive: true});
    await writeFile(join(root, path), data);
  }
  const policy = {schema: 1, target: TARGET, inputs, toolchain: {release: 'test-only', commit: '1'.repeat(40)},
    packages: [{ecosystem: 'cargo', name: 'fixture', version: '0.0.0', role: 'normal-candidate',
      declared_license: 'MIT', source: 'https://example.invalid/fixture', integrity: '2'.repeat(64),
      files: [{path: 'LICENSE', sha256: textHash}]}],
    stdlib: {path: 'COPYRIGHT-library.html', sha256: stdHash, source: 'https://static.rust-lang.org/test-fixture'}};
  await mkdir(join(root, DIRECTORY, 'texts'), {recursive: true});
  await writeFile(join(root, DIRECTORY, 'notice-policy.json'), JSON.stringify(policy, null, 2) + '\n');
  await writeFile(join(root, DIRECTORY, 'texts', `${textHash}.json`), JSON.stringify({text: original}) + '\n');
  await writeFile(join(root, DIRECTORY, 'texts', `${stdHash}.json`), JSON.stringify({text: stdlib}) + '\n');
  return {policy, original, stdlib, textHash, stdHash};
}
