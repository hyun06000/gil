import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync, mkdtempSync, writeFileSync, existsSync, rmSync, realpathSync, statSync, symlinkSync, mkdirSync, readdirSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';

const script = new URL('./prepare-gitless-macos.sh', import.meta.url).pathname;
const checker = new URL('./check-archive-members.sh', import.meta.url).pathname;
const source = readFileSync(script, 'utf8');
const checkerSource = readFileSync(checker, 'utf8');
const PIN = {
  version: '0.2.1-preview.6',
  sha256: '477ad55fb6757fc58c4264bd265c7dd8e196c4a5ecf648ac6e1a181a8338d2b5',
  bytes: 3189503,
  url: 'https://github.com/hyun06000/gil-marketplace/releases/download/v0.2.1-preview.6/gil-codex-macos-arm64-0.2.1-preview.6.tar.gz',
};
const darwinArm = process.platform === 'darwin' && process.arch === 'arm64';
const run = (args, env = {}) => spawnSync('/bin/sh', [script, ...args], {encoding: 'utf8', env: {...process.env, ...env}});

test('preparation pins the published preview.6 bytes and does not install or weaken trust', () => {
  assert.match(source, new RegExp(`expected=${PIN.sha256}`));
  assert.match(source, new RegExp(`expected_bytes=${PIN.bytes}`));
  assert.ok(source.includes(`url=${PIN.url}`));
  assert.ok(source.includes(`version=${PIN.version}`));
  assert.doesNotMatch(source, /preview\.[34]\b/, 'no stale preview.3/4 pin may remain');
  assert.ok(source.indexOf('SHA-256 mismatch') < source.indexOf('/usr/bin/tar -xzpf'));
  assert.ok(source.indexOf('check-archive-members.sh') < source.indexOf('/usr/bin/tar -xzpf'), 'members are inspected before extraction');
  assert.ok(source.indexOf('SHA-256 mismatch') < source.indexOf('check-archive-members.sh'), 'hash gate precedes member inspection');
  assert.doesNotMatch(source, /--skip|--no-verify|--force|--insecure|-k\b/, 'no option may bypass the hash gate');
  assert.match(checkerSource, /tar -tzf/); assert.match(checkerSource, /tar -tvzf[^\n]*cut -c1/);
  assert.doesNotMatch(checkerSource, /\$\{entry##\* \}|awk|tar -x/, 'checker parses no file names from verbose output and never extracts');
  for (const line of source.split('\n').filter(l => !l.trim().startsWith('#'))) {
    assert.doesNotMatch(line, /^\s*(?:git|xcode-select|xattr|spctl|sudo|npm|node|cargo|brew|codex)\s/);
  }
  assert.doesNotMatch(source, /config\.toml|plugins\/cache|curl[^\n]*\|/);
  assert.match(source, /--proto '=https' --proto-redir '=https'/);
});
test('shell syntax is valid', () => {
  assert.equal(spawnSync('/bin/sh', ['-n', script]).status, 0);
  assert.equal(spawnSync('/bin/sh', ['-n', checker]).status, 0);
});
test('member inspection refuses absolute, parent-traversing and link entries and accepts spaced names', {skip: !darwinArm}, () => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), 'gil-gitless-members-')));
  const tar = (args, cwd) => { const r = spawnSync('/usr/bin/tar', args, {cwd, encoding: 'utf8'}); assert.equal(r.status, 0, r.stderr); };
  const check = archive => spawnSync('/bin/sh', [checker, archive], {encoding: 'utf8'});
  try {
    // Benign archive whose names contain spaces and Korean: must be accepted exactly.
    const good = join(root, 'good'); mkdirSync(join(good, 'sub dir', '하위 폴더'), {recursive: true});
    writeFileSync(join(good, 'sub dir', '하위 폴더', 'file with spaces.txt'), 'x');
    writeFileSync(join(good, 'top level.json'), '{}');
    tar(['-czf', join(root, 'good.tar.gz'), '.'], good);
    let r = check(join(root, 'good.tar.gz'));
    assert.equal(r.status, 0, r.stderr); assert.match(r.stdout, /Archive members accepted/);
    // Absolute path member (bsdtar -P keeps the leading slash).
    const abs = join(root, 'abs'); mkdirSync(abs); writeFileSync(join(abs, 'f'), 'x');
    tar(['-P', '-czf', join(root, 'abs.tar.gz'), join(abs, 'f')]);
    r = check(join(root, 'abs.tar.gz')); assert.notEqual(r.status, 0); assert.match(r.stderr, /absolute path/);
    // Parent traversal member.
    const inner = join(root, 'trav', 'inner'); mkdirSync(inner, {recursive: true}); writeFileSync(join(root, 'trav', 'escape.txt'), 'x');
    tar(['-P', '-czf', join(root, 'trav.tar.gz'), '../escape.txt'], inner);
    r = check(join(root, 'trav.tar.gz')); assert.notEqual(r.status, 0); assert.match(r.stderr, /parent-traversing/);
    // Symbolic link member.
    const lnk = join(root, 'lnk'); mkdirSync(lnk); writeFileSync(join(lnk, 'target'), 'x'); symlinkSync('target', join(lnk, 'alias'));
    tar(['-czf', join(root, 'lnk.tar.gz'), '.'], lnk);
    r = check(join(root, 'lnk.tar.gz')); assert.notEqual(r.status, 0); assert.match(r.stderr, /links or special files/);
    // Hard link member.
    const hl = join(root, 'hl'); mkdirSync(hl); writeFileSync(join(hl, 'a'), 'x'); spawnSync('/bin/ln', [join(hl, 'a'), join(hl, 'b')]);
    tar(['-czf', join(root, 'hl.tar.gz'), '.'], hl);
    r = check(join(root, 'hl.tar.gz')); assert.notEqual(r.status, 0); assert.match(r.stderr, /links or special files/);
    // Checker never extracts anything and refuses a symlinked archive path.
    assert.deepEqual(readdirSync(root).filter(n => n.endsWith('.extracted')), []);
    symlinkSync(join(root, 'good.tar.gz'), join(root, 'alias.tar.gz'));
    r = check(join(root, 'alias.tar.gz')); assert.notEqual(r.status, 0); assert.match(r.stderr, /regular file/);
    // The published archive itself passes inspection when available.
    if (process.env.GIL_GITLESS_ARCHIVE) { r = check(process.env.GIL_GITLESS_ARCHIVE); assert.equal(r.status, 0, r.stderr); }
  } finally { rmSync(root, {recursive: true, force: true}); }
});
test('missing arguments fail without downloading', () => {
  assert.notEqual(spawnSync('/bin/sh', [script, '--destination']).status, 0);
});
test('bad, truncated and tampered archives and existing destinations are refused without mutation', {skip: !darwinArm}, () => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), 'gil-gitless-test-')));
  try {
    const next = join(root, '새 폴더');
    const invalid = join(root, 'invalid.tar.gz');
    writeFileSync(invalid, 'not the approved archive');
    let r = run(['--destination', next, '--archive', invalid]);
    assert.notEqual(r.status, 0); assert.match(r.stderr, /size mismatch/); assert.equal(existsSync(next), false);
    // Same byte length as the published archive but different content: size passes, hash must refuse.
    const sameSize = join(root, 'same-size.tar.gz');
    writeFileSync(sameSize, Buffer.alloc(PIN.bytes, 7));
    r = run(['--destination', next, '--archive', sameSize]);
    assert.notEqual(r.status, 0); assert.match(r.stderr, /SHA-256 mismatch/); assert.equal(existsSync(next), false);
    // Truncated download simulation: one byte short of the published size.
    const truncated = join(root, 'truncated.tar.gz');
    writeFileSync(truncated, Buffer.alloc(PIN.bytes - 1, 7));
    r = run(['--destination', next, '--archive', truncated]);
    assert.notEqual(r.status, 0); assert.match(r.stderr, /size mismatch/); assert.equal(existsSync(next), false);
    // Existing destination is never replaced.
    r = run(['--destination', root, '--archive', invalid]);
    assert.notEqual(r.status, 0); assert.match(r.stderr, /already exists/);
    assert.equal(readFileSync(invalid, 'utf8'), 'not the approved archive');
    // Symlinked archive is refused.
    const link = join(root, 'link.tar.gz'); symlinkSync(invalid, link);
    r = run(['--destination', next, '--archive', link]);
    assert.notEqual(r.status, 0); assert.match(r.stderr, /regular file/);
    // Relative destination and symlinked ancestor are refused.
    assert.notEqual(run(['--destination', 'relative/dir', '--archive', invalid]).status, 0);
    const realParent = join(root, 'real'); mkdirSync(realParent);
    const linkParent = join(root, 'linked'); symlinkSync(realParent, linkParent);
    r = run(['--destination', join(linkParent, 'dest'), '--archive', invalid]);
    assert.notEqual(r.status, 0); assert.match(r.stderr, /symbolic links/);
    assert.deepEqual(readdirSync(realParent), []);
  } finally { rmSync(root, {recursive: true, force: true}); }
});
test('interrupted download leaves no destination and no partial extraction', {skip: !darwinArm}, () => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), 'gil-gitless-net-')));
  try {
    const next = join(root, 'dest');
    // An unreachable proxy makes curl fail before any bytes are verified.
    const r = run(['--destination', next], {https_proxy: 'http://127.0.0.1:9', HTTPS_PROXY: 'http://127.0.0.1:9'});
    assert.notEqual(r.status, 0);
    assert.match(r.stderr, /Download failed or was interrupted/);
    assert.equal(existsSync(next), false);
  } finally { rmSync(root, {recursive: true, force: true}); }
});
test('the exact published archive prepares a complete local marketplace root', {
  skip: !darwinArm || !process.env.GIL_GITLESS_ARCHIVE ? 'set GIL_GITLESS_ARCHIVE to the verified published archive' : false,
}, () => {
  const archive = process.env.GIL_GITLESS_ARCHIVE;
  assert.equal(createHash('sha256').update(readFileSync(archive)).digest('hex'), PIN.sha256, 'env archive is not the published bytes');
  const root = realpathSync(mkdtempSync(join(tmpdir(), 'gil-gitless-ok-')));
  try {
    const dest = join(root, 'Application Support', 'GIL', 'preview.6-local');
    mkdirSync(join(root, 'Application Support', 'GIL'), {recursive: true});
    const r = run(['--destination', dest, '--archive', archive]);
    assert.equal(r.status, 0, r.stderr);
    assert.match(r.stdout, /Verified and prepared unsigned preview 0\.2\.1-preview\.6\. NOT installed yet\./);
    assert.match(r.stdout, new RegExp(`GIL_MARKETPLACE_ROOT=${dest.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}`));
    assert.equal(statSync(dest).mode & 0o777, 0o700);
    assert.equal(statSync(join(dest, 'plugins/gil-companion-prototype/core/darwin-arm64/gil')).mode & 0o777, 0o755);
    assert.equal(statSync(join(dest, 'release.json')).mode & 0o777, 0o644);
    assert.equal(statSync(join(dest, 'plugins/gil-companion-prototype/skills/gil-companion/SKILL.md')).mode & 0o777, 0o644);
    assert.ok(existsSync(join(dest, '.agents/plugins/marketplace.json')));
    assert.match(readFileSync(join(dest, 'plugins/gil-companion-prototype/.codex-plugin/plugin.json'), 'utf8'), /"version": "0\.2\.1-preview\.6"/);
    assert.match(readFileSync(join(dest, 'release.json'), 'utf8'), /"publishable": false/);
    // Running again against the same destination must refuse.
    assert.match(run(['--destination', dest, '--archive', archive]).stderr, /already exists/);
  } finally { rmSync(root, {recursive: true, force: true}); }
});
