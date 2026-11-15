// 합성 문서로 문서 연결 검사기를 시험한다. 실제 저장소 문서는 쓰지 않는다.
//
//   node --test tools/docs-check/*.test.mjs
//
// fixture 는 시험이 임시 폴더에 Git 저장소로 만든다(추적/비추적을 가르기 위해 git add 까지 한다).
// 저장소 안에 의도적으로 깨진 fixture 문서를 두지 않기 위해서다 — 두면 기본 검사가 그것을 문제로 센다.

import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync, readdirSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

import { extractLinks, maskNonContent } from './lib/markdown.mjs';
import { classify, checkDocument, openRepository, trackedMarkdown, PROBLEM_STATUSES } from './lib/check.mjs';
import { run } from './check-links.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const cli = path.join(here, 'check-links.mjs');

// ── 합성 저장소 ────────────────────────────────────────────────────────────

const TRACKED = {
  'README.md': [
    '# 안내',
    '',
    '정상 [설치](INSTALL.md) 와 없는 [삭제된 문서](REMOVED.md).',
    '중첩 [명세](spec/모델/기록%20모델.md) 와 인코딩 [명세2](spec/%EB%AA%A8%EB%8D%B8/%EC%8B%9C%EA%B0%84%20%EB%AA%A8%EB%8D%B8.md).',
    '맨 공백 [공백 대상](spec/모델/시간 모델.md) 은 렌더러가 연결로 보지 않는다.',
    '꺾쇠 [명세3](<spec/모델/시간 모델.md> "제목") 와 폴더 [명세 폴더](spec/) 그리고 [뿌리](./).',
    '절 [설치 절](INSTALL.md#요구-사항) 와 같은 문서 [위로](#안내) 와 [빈 절](#).',
    '외부 [GitHub](https://github.com/example/repo) [HTTP](http://example.com) [메일](mailto:someone@example.com) [기타](vscode://file/x).',
    '밖 [부모](../outside.md) 와 [더 밖](../../etc/passwd).',
    '이미지 ![그림](images/plot.png) 와 없는 그림 ![없음](images/missing.png).',
    '참조 [참조 연결][spec-ref] 와 [spec-ref][] 와 [spec-ref] 와 [정의 없음][nope].',
    '추적 안 됨 [개인 메모](notes/private.md).',
    '대소문자 [설치 소문자](install.md).',
    '',
    '[spec-ref]: <spec/모델/시간 모델.md>',
    '[bad-def]: spec/모델/시간 모델.md',
    '[broken-def]: nowhere/at-all.md',
    '',
    '```markdown',
    '[코드 안 가짜 연결](fake-in-fence.md)',
    '```',
    '',
    '인라인 코드 `[가짜](fake-inline.md)` 와 ``[가짜2](fake-inline2.md)`` 는 연결이 아니다.',
    '',
    '- 목록 안 울타리',
    '  ```text',
    '  [가짜3](fake-in-list-fence.md)',
    '  ```',
    '',
    '<!-- [주석 안](fake-in-comment.md) -->',
    '',
    '<a href="html-link.md">HTML</a> <img src="images/plot.png">',
    '[닫히지 않음](unclosed.md',
    '',
    '[빈 대상]() 과 [절대](/INSTALL.md) 와 [깨진 인코딩](bad%ZZ.md) 와 [질의](INSTALL.md?x=1).',
    '이스케이프 \\[연결 아님](not-a-link.md).',
  ].join('\n'),
  'INSTALL.md': '# 설치\n\n## 요구 사항\n\n[돌아가기](README.md)\n',
  'spec/모델/시간 모델.md': '# 시간 모델\n\n상위 [설치](../../INSTALL.md) 와 [형제](<./기록 모델.md>) 와 [없음](../없는%20폴더/x.md).\n',
  'spec/모델/기록 모델.md': '# 기록 모델\n\n인코딩된 형제 [시간](%EC%8B%9C%EA%B0%84%20%EB%AA%A8%EB%8D%B8.md#절) 과 비 Markdown [자료](../data.json).\n',
  'spec/data.json': '{}\n',
  'images/plot.png': 'not really a png\n',
  'notes/tracked-only.txt': 'x\n',
};
const UNTRACKED = {
  'notes/private.md': '# 개인 메모\n',
};

function git(cwd, ...args) {
  const r = spawnSync('git', ['-C', cwd, ...args], { encoding: 'utf8' });
  if (r.status !== 0) throw new Error(`git ${args.join(' ')}: ${r.stderr}`);
  return r.stdout;
}

function makeRepo() {
  const dir = mkdtempSync(path.join(tmpdir(), 'gil-docs-check-'));
  const write = (files) => {
    for (const [rel, text] of Object.entries(files)) {
      const at = path.join(dir, ...rel.split('/'));
      mkdirSync(path.dirname(at), { recursive: true });
      writeFileSync(at, text);
    }
  };
  git(dir, 'init', '-q');
  git(dir, 'config', 'core.quotepath', 'false');
  write(TRACKED);
  git(dir, 'add', '-A');
  write(UNTRACKED);
  // 저장소 밖 파일 — 있어도 따라가지 않아야 한다.
  writeFileSync(path.join(dir, '..', `gil-docs-check-outside-${path.basename(dir)}.md`), '# 밖\n');
  return dir;
}

function snapshot(dir) {
  const out = new Map();
  const walk = (d) => {
    for (const name of readdirSync(d)) {
      if (name === '.git') continue;
      const p = path.join(d, name);
      if (statSync(p).isDirectory()) walk(p);
      else out.set(path.relative(dir, p), createHash('sha256').update(readFileSync(p)).digest('hex'));
    }
  };
  walk(dir);
  return out;
}

function runCli(cwd, ...args) {
  return spawnSync(process.execPath, [cli, ...args], { cwd, encoding: 'utf8' });
}

const byTarget = (findings, target) => findings.filter((f) => f.target === target);
const one = (findings, target) => {
  const hits = byTarget(findings, target);
  assert.equal(hits.length, 1, `${target}: ${hits.length}개 (${JSON.stringify(hits)})`);
  return hits[0];
};

// ── 추출 ──────────────────────────────────────────────────────────────────

test('코드 블록·인라인 코드·HTML 주석은 가리고 줄 수는 보존한다', () => {
  const text = TRACKED['README.md'];
  const masked = maskNonContent(text);
  assert.equal(masked.split('\n').length, text.split('\n').length);
  for (const fake of ['fake-in-fence', 'fake-inline.md', 'fake-inline2', 'fake-in-list-fence', 'fake-in-comment']) {
    assert.ok(!masked.includes(fake), `${fake} 가 가려지지 않았다`);
  }
  assert.ok(masked.includes('REMOVED.md'));
});

test('닫히지 않은 울타리는 문서 끝까지 코드다', () => {
  const { links } = extractLinks('[a](a.md)\n```\n[b](b.md)\n\n[c](c.md)\n');
  assert.deepEqual(links.map((l) => l.target), ['a.md']);
});

test('인라인·이미지·참조·정의·HTML 을 종류별로 뽑고 줄·칸을 적는다', () => {
  const { links } = extractLinks(TRACKED['README.md']);
  const kinds = (k) => links.filter((l) => l.kind === k);
  assert.equal(kinds('image').length, 2);
  assert.equal(kinds('definition').length, 2);
  assert.equal(kinds('reference').length, 3, '전체형·접힌형·바로가기형');
  assert.equal(kinds('undefined_reference').length, 1);
  assert.equal(kinds('html').length, 2);
  assert.equal(kinds('unsupported').length, 3, '닫히지 않은 괄호 · 맨 공백 목적지 · 깨진 정의');
  const removed = links.find((l) => l.target === 'REMOVED.md');
  assert.deepEqual([removed.line, removed.column, removed.kind], [3, 26, 'inline']);
  assert.ok(!links.some((l) => l.target === 'not-a-link.md'), '이스케이프된 대괄호는 연결이 아니다');
  assert.ok(!links.some((l) => l.text === '정의 없음' && l.kind === 'reference'));
});

test('이미지가 든 연결·괄호가 든 경로·제목이 있는 연결을 읽는다', () => {
  const { links } = extractLinks('[![그림](i.png)](l.md) [p](a(b).md) [t](x.md "제목") [q](<a b.md> \'t\')');
  assert.deepEqual(links.map((l) => [l.kind, l.target]), [['inline', 'l.md'], ['image', 'i.png'], ['inline', 'a(b).md'], ['inline', 'x.md'], ['inline', 'a b.md']]);
});

// ── 판정 ──────────────────────────────────────────────────────────────────

test('합성 저장소에서 각 연결이 기대한 상태로 분류된다', () => {
  const dir = makeRepo();
  try {
    const repo = openRepository(dir);
    assert.deepEqual(trackedMarkdown(repo), ['INSTALL.md', 'README.md', 'spec/모델/기록 모델.md', 'spec/모델/시간 모델.md']);
    const f = checkDocument(repo, 'README.md', readFileSync(path.join(dir, 'README.md'), 'utf8'));

    assert.equal(one(f, 'INSTALL.md').status, 'ok');
    assert.equal(one(f, 'REMOVED.md').status, 'missing');
    assert.equal(one(f, 'spec/모델/기록%20모델.md').status, 'ok', '한글·공백 경로');
    const bareSpace = f.find((x) => x.line === 5 && x.status === 'unsupported');
    assert.match(bareSpace.reason, /공백.*<\.\.\.>/, '맨 공백 목적지는 이유를 밝혀 보고한다');
    assert.equal(byTarget(f, 'spec/모델/시간 모델.md').find((x) => x.kind === 'inline').status, 'ok', '꺾쇠 안 공백 경로');
    assert.equal(one(f, 'spec/%EB%AA%A8%EB%8D%B8/%EC%8B%9C%EA%B0%84%20%EB%AA%A8%EB%8D%B8.md').status, 'ok', '퍼센트 인코딩');
    assert.equal(byTarget(f, 'spec/모델/시간 모델.md')[0].resolved, 'spec/모델/시간 모델.md');
    const badDef = f.find((x) => x.text === 'bad-def');
    assert.equal(badDef.status, 'unsupported');
    assert.match(badDef.reason, /참조 정의 형식/);
    assert.equal(one(f, 'spec/').status, 'ok', '폴더');
    assert.equal(one(f, './').status, 'ok', '뿌리');

    const anchored = one(f, 'INSTALL.md#요구-사항');
    assert.equal(anchored.status, 'ok_anchor_unchecked');
    assert.equal(anchored.anchor, '요구-사항');
    assert.match(anchored.reason, /검사하지 않았다/);
    assert.equal(one(f, '#안내').status, 'same_doc_anchor');
    assert.equal(one(f, '#').status, 'same_doc_anchor');

    assert.equal(one(f, 'https://github.com/example/repo').status, 'external');
    assert.equal(one(f, 'http://example.com').status, 'external');
    assert.equal(one(f, 'mailto:someone@example.com').status, 'email');
    assert.equal(one(f, 'vscode://file/x').status, 'other_scheme');

    assert.equal(one(f, '../outside.md').status, 'outside_repo');
    assert.equal(one(f, '../outside.md').resolved, null, '밖은 따라가지 않는다');
    assert.equal(one(f, '../../etc/passwd').status, 'outside_repo');

    assert.equal(byTarget(f, 'images/plot.png').find((x) => x.kind === 'image').status, 'ok');
    assert.equal(one(f, 'images/missing.png').status, 'missing');

    const refs = f.filter((x) => x.kind === 'reference');
    assert.equal(refs.filter((x) => x.status === 'ok').length, 3, '참조 연결 세 형태가 정의를 따라 판정된다');
    assert.equal(refs.find((x) => x.label === 'nope').status, 'unsupported');
    assert.equal(one(f, 'nowhere/at-all.md').status, 'missing');
    assert.equal(one(f, 'nowhere/at-all.md').kind, 'definition');

    assert.equal(one(f, 'notes/private.md').status, 'untracked');
    const lower = one(f, 'install.md');
    assert.equal(lower.status, 'missing');
    assert.match(lower.reason, /INSTALL\.md/, '대소문자가 다른 파일을 알려 준다');

    for (const fake of ['fake-in-fence.md', 'fake-inline.md', 'fake-inline2.md', 'fake-in-list-fence.md', 'fake-in-comment.md', 'not-a-link.md']) {
      assert.equal(byTarget(f, fake).length, 0, fake);
    }
    assert.equal(one(f, 'html-link.md').status, 'unsupported');
    assert.equal(byTarget(f, 'images/plot.png').find((x) => x.kind === 'html').status, 'unsupported');
    assert.equal(one(f, '').status, 'unsupported', '빈 대상');
    assert.equal(one(f, '/INSTALL.md').status, 'unsupported', '절대 경로');
    assert.equal(one(f, 'bad%ZZ.md').status, 'unsupported', '깨진 인코딩');
    assert.equal(one(f, 'INSTALL.md?x=1').status, 'unsupported');
    assert.ok(f.some((x) => x.status === 'unsupported' && x.line === 33 && /닫히지 않았/.test(x.reason)), '닫히지 않은 괄호');
    assert.ok(f.every((x) => x.status !== 'ok' || x.reason === ''));
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('중첩 폴더 문서의 상위·형제·인코딩 상대 경로는 그 문서 폴더 기준이다', () => {
  const dir = makeRepo();
  try {
    const repo = openRepository(dir);
    const read = (rel) => readFileSync(path.join(dir, ...rel.split('/')), 'utf8');
    const a = checkDocument(repo, 'spec/모델/시간 모델.md', read('spec/모델/시간 모델.md'));
    assert.equal(one(a, '../../INSTALL.md').status, 'ok');
    assert.equal(one(a, '../../INSTALL.md').resolved, 'INSTALL.md');
    assert.equal(one(a, './기록 모델.md').status, 'ok');
    assert.equal(one(a, '../없는%20폴더/x.md').status, 'missing');
    const b = checkDocument(repo, 'spec/모델/기록 모델.md', read('spec/모델/기록 모델.md'));
    assert.equal(one(b, '%EC%8B%9C%EA%B0%84%20%EB%AA%A8%EB%8D%B8.md#절').status, 'ok_anchor_unchecked');
    assert.equal(one(b, '../data.json').status, 'ok', 'Markdown 이 아닌 파일도 존재만 본다');
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('classify 는 저장소 뿌리 밖으로 올라가는 모든 경로를 outside_repo 로 두고 ../ 를 뿌리에서 자르지 않는다', () => {
  const repo = { root: '/repo', files: new Set(['a.md']), dirs: new Set(['']) };
  assert.equal(classify(repo, 'a.md', { kind: 'inline', target: '../a.md' }).status, 'outside_repo');
  assert.equal(classify(repo, 'd/a.md', { kind: 'inline', target: '../a.md' }).status, 'ok');
  assert.equal(classify(repo, 'a.md', { kind: 'inline', target: 'C:/x.md' }).status, 'unsupported');
});

// ── CLI ───────────────────────────────────────────────────────────────────

test('CLI: 기본은 추적 Markdown 전체, 문제가 있으면 1, 입력 문서는 바뀌지 않는다', () => {
  const dir = makeRepo();
  try {
    const before = snapshot(dir);
    const statusBefore = git(dir, 'status', '--porcelain');
    const r = runCli(dir);
    assert.equal(r.status, 1, r.stderr);
    assert.match(r.stdout, /검사 범위: 문서 4개 \(Git 이 추적하는 Markdown 전체\)/);
    assert.match(r.stdout, /README\.md:3:26  \[missing\]  REMOVED\.md  — 파일이 없다/);
    assert.match(r.stdout, /\[outside_repo\]  \.\.\/outside\.md/);
    assert.match(r.stdout, /\[untracked\]  notes\/private\.md/);
    assert.match(r.stdout, /\[unsupported\]  html-link\.md/);
    assert.match(r.stdout, /검사하지 않은 것:/);
    assert.match(r.stdout, /절\(heading\) 존재는 검사하지 않았다/);
    assert.doesNotMatch(r.stdout, /fake-/);
    assert.match(r.stdout, /종료 코드 1/);
    assert.deepEqual(snapshot(dir), before, '문서가 바뀌었다');
    assert.equal(git(dir, 'status', '--porcelain'), statusBefore, 'Git 상태가 바뀌었다');
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('CLI: 사람이 읽는 결과와 JSON 결과가 같은 문제·같은 개수를 말한다', () => {
  const dir = makeRepo();
  try {
    const text = runCli(dir);
    const json = runCli(dir, '--json');
    assert.equal(json.status, 1);
    const data = JSON.parse(json.stdout);
    assert.equal(data.tool, 'gil-docs-check');
    assert.equal(data.exit_code, 1);
    assert.deepEqual(data.problem_statuses, PROBLEM_STATUSES);
    assert.deepEqual(data.scope.documents, ['INSTALL.md', 'README.md', 'spec/모델/기록 모델.md', 'spec/모델/시간 모델.md']);
    assert.equal(data.problems.length, data.findings.filter((f) => PROBLEM_STATUSES.includes(f.status)).length);
    for (const p of data.problems) {
      assert.ok(text.stdout.includes(`${p.file}:${p.line}:${p.column}  [${p.status}]  ${p.target}  — ${p.reason}`), `텍스트에 없다: ${JSON.stringify(p)}`);
    }
    const listed = text.stdout.split('\n').filter((l) => /^  \S.*:\d+:\d+  \[/.test(l)).length;
    assert.equal(listed, data.problems.length, '텍스트가 JSON 보다 많이/적게 나열한다');
    assert.equal(text.stdout.match(/문제 (\d+)개/)[1], String(data.problems.length));
    for (const [status, n] of Object.entries(data.counts)) {
      if (n > 0) assert.match(text.stdout, new RegExp(`^  ${status}\\s+${n}$`, 'm'), status);
    }
    assert.equal(Object.values(data.counts).reduce((a, b) => a + b, 0), data.findings.length);
    assert.ok(Array.isArray(data.not_checked) && data.not_checked.length > 0);
    for (const f of data.findings) {
      for (const key of ['file', 'line', 'column', 'kind', 'target', 'status', 'reason']) assert.ok(key in f, key);
    }
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('CLI: 지정한 문서·폴더만 검사하고, 깨끗하면 0 이다', () => {
  const dir = makeRepo();
  try {
    const clean = runCli(dir, 'INSTALL.md');
    assert.equal(clean.status, 0, clean.stdout);
    assert.match(clean.stdout, /문서 1개 \(지정한 문서\)/);
    assert.match(clean.stdout, /문제 없음/);

    const folder = runCli(dir, '--json', 'spec');
    const data = JSON.parse(folder.stdout);
    assert.deepEqual(data.scope.documents, ['spec/모델/기록 모델.md', 'spec/모델/시간 모델.md']);
    assert.equal(data.scope.mode, 'requested');
    assert.equal(folder.status, 1, '없는 폴더를 가리키는 연결 하나');
    assert.deepEqual(data.problems.map((p) => p.target), ['../없는%20폴더/x.md']);

    const fromSub = spawnSync(process.execPath, [cli, '기록 모델.md'], { cwd: path.join(dir, 'spec', '모델'), encoding: 'utf8' });
    assert.equal(fromSub.status, 0, fromSub.stderr);
    assert.match(fromSub.stdout, /문서 1개/);

    const viaRoot = runCli(here, '--root', dir, 'INSTALL.md');
    assert.equal(viaRoot.status, 2, '--root 와 다른 cwd 의 상대 문서는 저장소 밖이다');
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('CLI: 도구 오류는 2 로 끝나고 stderr 에 이유를 적는다', () => {
  const dir = makeRepo();
  try {
    for (const args of [['없는문서.md'], ['spec/data.json'], ['--bogus'], ['--root'], ['../']]) {
      const r = runCli(dir, ...args);
      assert.equal(r.status, 2, args.join(' '));
      assert.match(r.stderr, /^오류: /);
      assert.equal(r.stdout, '');
    }
    const noGit = mkdtempSync(path.join(tmpdir(), 'gil-docs-check-nogit-'));
    try {
      const r = runCli(noGit);
      assert.equal(r.status, 2);
      assert.match(r.stderr, /git/);
    } finally { rmSync(noGit, { recursive: true, force: true }); }
    const help = runCli(dir, '--help');
    assert.equal(help.status, 0);
    assert.match(help.stdout, /사용법/);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});

test('run() 은 --verbose 에서 문제 아닌 연결도 나열한다', () => {
  const dir = makeRepo();
  try {
    const quiet = run([], { cwd: dir });
    const verbose = run(['--verbose'], { cwd: dir });
    assert.equal(quiet.exit, 1);
    assert.doesNotMatch(quiet.stdout, /\[external\]  https:/);
    assert.match(verbose.stdout, /문제 아님\(참고\):/);
    assert.match(verbose.stdout, /\[external\]  https:\/\/github\.com\/example\/repo/);
    assert.match(verbose.stdout, /\[same_doc_anchor\]  #안내/);
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
