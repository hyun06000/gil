#!/usr/bin/env node
// 문서 파일 연결 검사 — 읽기 전용. 사용법은 tools/docs-check/README.md 를 본다.
//
//   node tools/docs-check/check-links.mjs                  # Git 이 추적하는 Markdown 전체
//   node tools/docs-check/check-links.mjs README.md spec/  # 지정한 문서·폴더만
//   node tools/docs-check/check-links.mjs --json           # JSON 출력
//
// 종료 코드: 0 문제 없음 · 1 잘못된 연결 있음 · 2 도구 실행 오류(인자·git·입력 파일).

import { readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { ToolError, checkDocument, openRepository, trackedMarkdown } from './lib/check.mjs';
import { EXIT_TOOL_ERROR, toJson, toText } from './lib/report.mjs';

const USAGE = `사용법: node tools/docs-check/check-links.mjs [--root <폴더>] [--json] [--verbose] [문서|폴더 ...]
  --root <폴더>   이 폴더가 속한 Git 저장소를 검사한다(기본: 현재 폴더)
  --json          결과를 JSON 으로 stdout 에 쓴다
  --verbose       문제가 아닌 연결(외부 주소 등)도 모두 나열한다
  문서|폴더       지정하면 그 문서(폴더는 그 아래 추적 Markdown)만 검사한다`;

export function parseArgs(argv) {
  const opts = { root: '.', json: false, verbose: false, targets: [] };
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === '--json') opts.json = true;
    else if (a === '--verbose') opts.verbose = true;
    else if (a === '--root') {
      if (i + 1 >= argv.length) throw new ToolError('--root 뒤에 폴더가 필요하다');
      opts.root = argv[++i];
    } else if (a === '--help' || a === '-h') opts.help = true;
    else if (a.startsWith('-')) throw new ToolError(`알 수 없는 선택지: ${a}\n${USAGE}`);
    else opts.targets.push(a);
  }
  return opts;
}

/** 지정 대상(파일·폴더)을 저장소 상대 문서 목록으로 푼다. */
export function resolveTargets(repo, targets, cwd) {
  const all = trackedMarkdown(repo);
  const docs = [];
  for (const t of targets) {
    const absolute = path.resolve(cwd, t);
    const rel = path.relative(repo.root, absolute).split(path.sep).join('/').normalize('NFC');
    if (rel.startsWith('..') || path.isAbsolute(rel)) throw new ToolError(`저장소 밖의 경로는 검사하지 않는다: ${t}`);
    let st;
    try { st = statSync(absolute); } catch { throw new ToolError(`입력 문서가 없다: ${t}`); }
    if (st.isDirectory()) {
      const prefix = rel === '' ? '' : `${rel}/`;
      const under = all.filter((f) => f.startsWith(prefix));
      if (under.length === 0) throw new ToolError(`폴더 아래에 추적되는 Markdown 이 없다: ${t}`);
      docs.push(...under);
    } else {
      if (!/\.(md|markdown)$/i.test(rel)) throw new ToolError(`Markdown 문서가 아니다: ${t}`);
      docs.push(rel);
    }
  }
  return [...new Set(docs)].sort();
}

export function run(argv, { cwd = process.cwd() } = {}) {
  const opts = parseArgs(argv);
  if (opts.help) return { stdout: USAGE, exit: 0 };
  const repo = openRepository(path.resolve(cwd, opts.root));
  const mode = opts.targets.length ? 'requested' : 'tracked';
  const documents = mode === 'tracked' ? trackedMarkdown(repo) : resolveTargets(repo, opts.targets, cwd);
  const findings = [];
  const untrackedInputs = [];
  for (const doc of documents) {
    if (!repo.files.has(doc)) untrackedInputs.push(doc);
    const text = readFileSync(path.join(repo.root, ...doc.split('/')), 'utf8');
    findings.push(...checkDocument(repo, doc, text));
  }
  const scope = {
    mode,
    requested: opts.targets,
    documents,
    tracked_markdown_total: trackedMarkdown(repo).length,
    untracked_inputs: untrackedInputs,
    excluded: ['Git 이 추적하지 않는 파일', '.gil/ 기록', '설치된 Plugin 캐시', 'Markdown 이 아닌 파일'],
  };
  const result = { root: repo.root, scope, findings };
  const json = toJson(result);
  return { stdout: opts.json ? JSON.stringify(json, null, 2) : toText(result, { verbose: opts.verbose }), exit: json.exit_code };
}

const isMain = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) {
  process.stdout.on('error', (e) => { if (e.code === 'EPIPE') process.exit(process.exitCode ?? 0); throw e; });
  try {
    const { stdout, exit } = run(process.argv.slice(2));
    process.stdout.write(`${stdout}\n`);
    process.exitCode = exit;
  } catch (e) {
    if (e instanceof ToolError) {
      process.stderr.write(`오류: ${e.message}\n`);
      process.exitCode = EXIT_TOOL_ERROR;
    } else throw e;
  }
}
