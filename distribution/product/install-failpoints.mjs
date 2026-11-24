// Maintainer check of the Linux user install's failure paths through the real `gil install` and
// `gil uninstall --remove-binary` entries. It needs a DEBUG build: the steps between two renames
// or unlinks are made to fail with `GIL_INSTALL_FAIL=<stage>`, which only a debug build reads.
// What this is evidence of: the debug executable's handling of those injected failures in an
// isolated HOME. It is not evidence about release bytes (a release build has no such variable;
// verify-install.mjs checks that) and not a power-loss test.
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {mkdir, mkdtemp, readFile, readdir, realpath, rm, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const CONFIG = '# sentinel: an existing Codex setting\n[mcp_servers.other]\ncommand = "/usr/bin/true"\nargs = []\n';

export async function checkInstallFailpoints(binary) {
  assert.equal(process.platform, 'linux', 'the Linux user install is checked on Linux');
  const state = await realpath(await mkdtemp(join(tmpdir(), 'gil-install-failpoints-')));
  try {
    const core = await readFile(resolve(binary));
    const a = join(state, 'gil-a'), b = join(state, 'gil-b');
    await writeFile(a, core, {mode: 0o755});
    await writeFile(b, Buffer.concat([core, Buffer.from('\n# second build for the update\n')]), {mode: 0o755});
    const [digestA, digestB] = [sha256(await readFile(a)), sha256(await readFile(b))];
    const home = async name => {
      const at = join(state, name);
      await mkdir(join(at, '.codex'), {recursive: true});
      await writeFile(join(at, '.codex/config.toml'), CONFIG);
      return {at, bin: join(at, '.local/share/gil/bin'), fixed: join(at, '.local/share/gil/bin/gil'), env: {PATH: '/usr/bin:/bin', HOME: at, CODEX_HOME: join(at, '.codex')}};
    };
    const run = (exe, args, env, stage) => spawnSync(exe, args, {cwd: state, env: stage ? {...env, GIL_INSTALL_FAIL: stage} : env, encoding: 'utf8', timeout: 60000});
    const receipt = async h => JSON.parse(await readFile(join(h.bin, 'install.json'), 'utf8'));
    const names = async dir => (await readdir(dir)).sort();
    const results = {};

    // The injection is real in this build: without that, every case below would pass vacuously.
    const probe = await home('probe');
    assert.notEqual(run(a, ['install', '--no-wait'], probe.env, 'first-binary').status, 0, 'this executable ignores GIL_INSTALL_FAIL: not a debug build');

    // (4) An update that stops at each step: the installed file stays recorded, nobody is told to
    // move a file, and the same command run again finishes the update.
    for (const stage of ['update-receipt', 'update-binary', 'settle-receipt']) {
      const h = await home(`update-${stage}`);
      assert.equal(run(a, ['install', '--no-wait'], h.env).status, 0);
      const registered = await readFile(join(h.at, '.codex/config.toml'));
      const failed = run(b, ['install', '--no-wait'], h.env, stage);
      assert.notEqual(failed.status, 0, `${stage}: the injected failure was not reported`);
      assert.doesNotMatch(failed.stderr, /yourself|move it away/, `${stage}: the user was told to move a file`);
      assert.match(failed.stderr, /run gil install again|was not changed/, stage);
      assert.deepEqual(await names(h.bin), ['gil', 'install.json'], `${stage}: a staged name was left`);
      const installed = sha256(await readFile(h.fixed)), record = await receipt(h);
      assert.equal(installed, stage === 'settle-receipt' ? digestB : digestA, stage);
      assert.ok(record.sha256 === installed || record.previous_sha256 === installed, `${stage}: the installed file is no longer recorded`);
      assert.ok((await readFile(join(h.at, '.codex/config.toml'))).equals(registered), `${stage}: a Host setting changed during the failed update`);
      const retry = run(b, ['install', '--no-wait'], h.env);
      assert.equal(retry.status, 0, `${stage}: the retry failed: ${retry.stderr}`);
      assert.equal(sha256(await readFile(h.fixed)), digestB);
      const settled = await receipt(h);
      assert.deepEqual([settled.sha256, settled.previous_sha256], [digestB, undefined], `${stage}: the record is not settled`);
      assert.equal(run(h.fixed, ['uninstall', '--remove-binary'], h.env).status, 0);
      results[stage] = `failed as injected; installed ${stage === 'settle-receipt' ? 'new' : 'previous'} executable stayed recorded; rerun completed`;
    }

    // A first install whose receipt cannot be placed takes its executable back; no Host change.
    const first = await home('first-receipt');
    const refused = run(a, ['install', '--no-wait'], first.env, 'first-receipt');
    assert.notEqual(refused.status, 0);
    assert.match(refused.stderr, /Nothing was installed/);
    assert.deepEqual(await names(first.bin), [], 'something of the failed first install stayed');
    assert.equal(await readFile(join(first.at, '.codex/config.toml'), 'utf8'), CONFIG, 'a Host setting changed although the install failed');
    assert.equal(run(a, ['install', '--no-wait'], first.env).status, 0);
    results['first-receipt'] = 'receipt publication refused before binary placement; Host settings untouched; rerun installed';

    // (3) A removal that only half happens is reported as partial, with both lists, and fails.
    const partial = run(first.fixed, ['uninstall', '--remove-binary'], first.env, 'remove-receipt');
    assert.notEqual(partial.status, 0, 'a partial removal exited as a success');
    assert.match(partial.stdout, /^binary: PARTIAL removed \[[^\]]*\/gil\]; remaining \[[^\]]*install\.json/m);
    assert.doesNotMatch(partial.stdout, /^binary: REMOVED/m);
    assert.deepEqual(await names(first.bin), ['install.json'], 'the report and the folder disagree');
    results['remove-receipt'] = 'PARTIAL with the removed executable and the remaining receipt; non-zero exit';
    assert.equal(run(a, ['uninstall', '--remove-binary'], first.env).status, 0);
    assert.deepEqual(await names(join(first.at, '.local/share')), []);
    for (const stage of ['first-intent', 'first-published', 'remove-receipt']) {
      for (const retry of ['install', 'uninstall']) {
        const h = await home(`exit-${stage}-${retry}`);
        const install = ['install', '--no-wait'], remove = ['uninstall', '--remove-binary'];
        if (stage === 'remove-receipt') assert.equal(run(a, install, h.env).status, 0);
        const exited = run(a, stage === 'remove-receipt' ? remove : install, {...h.env, GIL_INSTALL_EXIT: stage});
        assert.equal(exited.status, 86, `the process did not exit at ${stage}: ${exited.stderr}`);
        if (retry === 'install') {
          assert.equal(run(b, install, h.env).status, 0);
          assert.equal(sha256(await readFile(h.fixed)), digestB);
          assert.deepEqual(await names(h.bin), ['gil', 'install.json']);
        }
        assert.equal(run(a, remove, h.env).status, 0);
        assert.deepEqual(await names(join(h.at, '.local/share')), []);
        assert.equal(await readFile(join(h.at, '.codex/config.toml'), 'utf8'), CONFIG);
      }
      results[`exit-${stage}`] = 'abrupt process exit; both explicit retry commands verified';
    }
    const held = await home('concurrent');
    for (const args of [['install', '--no-wait'], ['uninstall', '--remove-binary']]) {
      const blocked = spawnSync('flock', ['--exclusive', held.at, a, ...args], {env: held.env, encoding: 'utf8', timeout: 10000});
      assert.equal(blocked.status, 1);
      assert.match(blocked.stderr, /cannot lock HOME/);
      assert.equal(await readFile(join(held.at, '.codex/config.toml'), 'utf8'), CONFIG);
    }
    assert.equal(run(a, ['install', '--no-wait'], held.env).status, 0);
    assert.equal(run(a, ['uninstall', '--remove-binary'], held.env).status, 0);
    results.concurrent = 'both mutating entries refuse while locked; retry after release succeeds';
    const untouched = await home('remove-binary');
    assert.equal(run(a, ['install', '--no-wait'], untouched.env).status, 0);
    const nothing = run(untouched.fixed, ['uninstall', '--remove-binary'], untouched.env, 'remove-binary');
    assert.notEqual(nothing.status, 0);
    assert.match(nothing.stdout, /^binary: PARTIAL removed \[\]; remaining \[/m);
    assert.deepEqual(await names(untouched.bin), ['gil', 'install.json']);
    const whole = run(untouched.fixed, ['uninstall', '--remove-binary'], untouched.env);
    assert.equal(whole.status, 0);
    assert.match(whole.stdout, /^binary: REMOVED /m);
    assert.deepEqual(await names(join(untouched.at, '.local/share')), [], 'the folders this installation made were left');
    assert.equal(await readFile(join(untouched.at, '.codex/config.toml'), 'utf8'), CONFIG);
    results['remove-binary'] = 'nothing removed, reported as partial; a later removal completed and took the folders it had made';
    return {evidence: 'debug build with GIL_INSTALL_FAIL; isolated HOME; not release bytes, process exits are separate from error returns; no power-loss test', results};
  } finally { await rm(state, {recursive: true, force: true}); }
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [binary, ...extra] = process.argv.slice(2);
  assert.ok(binary && extra.length === 0, 'Usage: install-failpoints.mjs <debug gil executable>');
  console.log(JSON.stringify(await checkInstallFailpoints(binary), null, 2));
}
