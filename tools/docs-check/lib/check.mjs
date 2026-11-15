// 연결 대상을 분류하고 저장소 안의 파일인지 판정한다. **읽기만 한다.**
//
// 존재 판정의 기준은 Git index(`git ls-files`)다. 파일 시스템을 돌아다니지 않으므로
// `.gil/`, 설치된 Plugin 캐시, 비추적 개인 파일은 보지 않는다. 그런 파일만 가리키는 연결은
// 다른 사람의 checkout 과 GitHub 에서 깨진다 — `untracked` 로 따로 보고한다.

import { spawnSync } from 'node:child_process';
import { lstatSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { extractLinks, hasScheme } from './markdown.mjs';

export const PROBLEM_STATUSES = ['missing', 'untracked', 'outside_repo', 'unsupported'];
export const INFO_STATUSES = ['ok', 'ok_anchor_unchecked', 'same_doc_anchor', 'external', 'email', 'other_scheme'];
export const ALL_STATUSES = [...PROBLEM_STATUSES, ...INFO_STATUSES];

export class ToolError extends Error {}

function git(root, args) {
  const r = spawnSync('git', ['-C', root, '-c', 'core.quotepath=false', ...args], { encoding: 'utf8' });
  if (r.error) throw new ToolError(`git 을 실행할 수 없다: ${r.error.message}`);
  if (r.status !== 0) throw new ToolError(`git ${args.join(' ')} 실패: ${(r.stderr || '').trim()}`);
  return r.stdout;
}

const nfc = (s) => s.normalize('NFC');

/** 저장소 뿌리와 추적 파일 집합. */
export function openRepository(startDir) {
  const root = nfc(git(startDir, ['rev-parse', '--show-toplevel']).trim());
  const files = new Set(git(root, ['ls-files', '-z']).split('\0').filter(Boolean).map(nfc));
  const dirs = new Set(['']);
  for (const f of files) {
    const parts = f.split('/');
    for (let i = 1; i < parts.length; i += 1) dirs.add(parts.slice(0, i).join('/'));
  }
  return { root, files, dirs };
}

export function trackedMarkdown(repo) {
  return [...repo.files].filter((f) => /\.(md|markdown)$/i.test(f)).sort();
}

/** 저장소 뿌리 기준 상대 경로(항상 `/`). 밖이면 null. */
function repoRelative(root, absolute) {
  const rel = path.relative(root, absolute);
  if (rel === '') return '';
  if (rel.startsWith('..') || path.isAbsolute(rel)) return null;
  return rel.split(path.sep).join('/');
}

/**
 * 연결 하나를 판정한다.
 * @returns {{status, reason, resolved?, anchor?}}
 */
export function classify(repo, docRel, link) {
  if (link.kind === 'html' || link.kind === 'unsupported' || link.kind === 'undefined_reference') {
    return { status: 'unsupported', reason: link.reason };
  }
  const raw = link.target;
  if (raw.trim() === '') return { status: 'unsupported', reason: '연결 대상이 비어 있다' };
  if (raw.startsWith('#')) return { status: 'same_doc_anchor', reason: '같은 문서의 절 주소 — 절 존재는 검사하지 않는다' };
  if (hasScheme(raw)) {
    const scheme = raw.slice(0, raw.indexOf(':')).toLowerCase();
    if (scheme === 'http' || scheme === 'https') return { status: 'external', reason: '외부 웹 주소 — 네트워크로 확인하지 않는다' };
    if (scheme === 'mailto') return { status: 'email', reason: '이메일 주소 — 검사하지 않는다' };
    return { status: 'other_scheme', reason: `${scheme}: 주소 — 검사하지 않는다` };
  }
  if (raw.startsWith('//')) return { status: 'external', reason: '프로토콜 상대 주소 — 네트워크로 확인하지 않는다' };

  const hashAt = raw.indexOf('#');
  const pathPart = hashAt >= 0 ? raw.slice(0, hashAt) : raw;
  const anchor = hashAt >= 0 ? raw.slice(hashAt + 1) : undefined;
  if (pathPart === '') return { status: 'same_doc_anchor', reason: '같은 문서의 절 주소 — 절 존재는 검사하지 않는다' };

  let decoded;
  try {
    decoded = decodeURIComponent(pathPart);
  } catch {
    return { status: 'unsupported', reason: '퍼센트 인코딩을 해석할 수 없다', anchor };
  }
  if (decoded.includes('\0') || decoded.includes('\n')) return { status: 'unsupported', reason: '경로에 쓸 수 없는 글자가 있다', anchor };
  if (decoded.startsWith('/') || /^[A-Za-z]:[\\/]/.test(decoded) || decoded.startsWith('\\')) {
    return { status: 'unsupported', reason: '절대 경로 — 렌더러마다 뿌리가 달라 검사하지 않는다', anchor };
  }
  if (decoded.includes('?')) {
    return { status: 'unsupported', reason: '질의 문자열(?)이 붙은 파일 경로는 검사하지 않는다', anchor };
  }

  const docDir = path.posix.dirname(docRel);
  const absolute = path.resolve(repo.root, docDir === '.' ? '' : docDir, decoded.split('/').join(path.sep));
  const rel = repoRelative(repo.root, absolute);
  if (rel === null) return { status: 'outside_repo', reason: '저장소 밖을 가리킨다 — 따라가지 않는다', anchor, resolved: null };

  const normalized = nfc(rel).replace(/\/+$/, '');
  const ok = anchor !== undefined
    ? { status: 'ok_anchor_unchecked', reason: '파일은 있다 — #절 주소는 검사하지 않았다' }
    : { status: 'ok', reason: '' };
  if (normalized === '' || repo.files.has(normalized) || repo.dirs.has(normalized)) {
    return { ...ok, anchor, resolved: normalized === '' ? '.' : normalized };
  }
  const hint = caseHint(repo, normalized);
  if (hint) return { status: 'missing', reason: `파일이 없다 — 대소문자가 다른 ${hint} 는 있다`, anchor, resolved: normalized };
  // 추적되지 않지만 디스크에는 있는가 — 그 경로 하나만 본다. 폴더를 재귀 탐색하지 않는다.
  if (existsExactly(repo.root, normalized)) {
    return { status: 'untracked', reason: 'Git 이 추적하지 않는 파일 — 다른 checkout 과 GitHub 에서는 없다', anchor, resolved: normalized };
  }
  return { status: 'missing', reason: '파일이 없다', anchor, resolved: normalized };
}

/** 대소문자를 가리지 않는 파일 시스템(macOS 기본)에서도 **적힌 이름 그대로** 있는지 본다. */
function existsExactly(root, rel) {
  const abs = path.join(root, ...rel.split('/'));
  try { lstatSync(abs); } catch { return false; }
  const parts = rel.split('/');
  let dir = root;
  for (const part of parts) {
    let names;
    try { names = readdirSync(dir); } catch { return false; }
    if (!names.some((n) => nfc(n) === part)) return false;
    dir = path.join(dir, part);
  }
  return true;
}

function caseHint(repo, rel) {
  const lower = rel.toLowerCase();
  for (const f of repo.files) if (f.toLowerCase() === lower) return f;
  for (const d of repo.dirs) if (d && d.toLowerCase() === lower) return `${d}/`;
  return null;
}

/** 문서 하나를 검사한다. 본문은 호출자가 읽어 넘긴다(읽기 전용을 한눈에 보이게). */
export function checkDocument(repo, docRel, text) {
  const { links } = extractLinks(text);
  return links.map((link) => {
    const verdict = classify(repo, docRel, link);
    const finding = {
      file: docRel,
      line: link.line,
      column: link.column,
      kind: link.kind === 'undefined_reference' ? 'reference' : link.kind,
      text: link.text,
      target: link.target,
      status: verdict.status,
      reason: verdict.reason,
    };
    if (verdict.resolved !== undefined) finding.resolved = verdict.resolved;
    if (verdict.anchor !== undefined) finding.anchor = verdict.anchor;
    if (link.label !== undefined) finding.label = link.label;
    return finding;
  });
}
