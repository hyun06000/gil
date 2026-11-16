// Markdown 에서 연결 후보를 뽑는다 — **큰 해석기가 아니다.** 지원 범위는 README.md 에 적는다.
//
// 돌려주는 것은 "이 문서 어디에 어떤 글자가 연결 대상으로 적혀 있는가" 뿐이다.
// 대상이 파일인지, 있는지, 저장소 밖인지는 check.mjs 가 판단한다.
//
// 지원: 인라인 연결 `[t](dest)` `[t](dest "title")` `[t](<dest with space>)`, 이미지 `![a](dest)`,
//       참조 정의 `[label]: dest`, 참조 연결 `[t][label]` `[label][]` `[label]`(정의가 있을 때만).
// 가림: 울타리 코드(``` ~~~), 인라인 코드(`...`), HTML 주석(<!-- -->).
// 미지원(발견하면 `unsupported` 로 보고): HTML `href=`/`src=`.
// 인식하지 않음(검사 대상에서 조용히 빠짐): 4칸 들여쓰기 코드 블록 — 목록 안 문단과 구분하지 않는다.

const SCHEME = /^[A-Za-z][A-Za-z0-9+.-]{1,31}:/;

/** 코드·주석 자리를 공백으로 바꾼다. 줄 수와 글자 자리는 그대로 둔다. */
export function maskNonContent(text) {
  const chars = Array.from(text); // code point 단위 — 한글·이모지가 자리를 흔들지 않게
  const blank = (from, to) => {
    for (let i = from; i < to; i += 1) if (chars[i] !== '\n') chars[i] = ' ';
  };
  const n = chars.length;

  // 1) 울타리 코드 블록 — 줄 단위. 열린 울타리와 같은 글자·같거나 긴 길이의 울타리 줄에서 닫힌다.
  //    닫히지 않으면 문서 끝까지 코드다(CommonMark). 들여쓰기는 묻지 않는다(목록 안 울타리를 위해).
  let i = 0;
  let fence = null; // { ch, len, start }
  while (i < n) {
    let end = i;
    while (end < n && chars[end] !== '\n') end += 1;
    const line = chars.slice(i, end).join('');
    const m = line.match(/^\s*(`{3,}|~{3,})(.*)$/);
    if (fence) {
      if (m && m[1][0] === fence.ch && m[1].length >= fence.len && m[2].trim() === '') {
        blank(fence.start, end);
        fence = null;
      }
    } else if (m && !(m[1][0] === '`' && m[2].includes('`'))) {
      fence = { ch: m[1][0], len: m[1].length, start: i };
    }
    i = end + 1;
  }
  if (fence) blank(fence.start, n);

  // 2) HTML 주석 — 여러 줄에 걸칠 수 있다. 닫히지 않으면 그대로 둔다(글로 본다).
  const joined = () => chars.join('');
  let s = joined();
  let at = 0;
  for (;;) {
    const open = s.indexOf('<!--', at);
    if (open < 0) break;
    const close = s.indexOf('-->', open + 4);
    if (close < 0) break;
    // s 의 index 는 UTF-16 단위라 code point 배열과 어긋날 수 있다 — 변환한다.
    blank(cpIndex(s, open), cpIndex(s, close + 3));
    at = close + 3;
  }

  // 3) 인라인 코드 — 같은 길이의 백틱 묶음으로 닫힌다. 빈 줄을 넘지 않는다.
  s = joined();
  const cps = Array.from(s);
  let k = 0;
  while (k < cps.length) {
    if (cps[k] === '\\') { k += 2; continue; }
    if (cps[k] !== '`') { k += 1; continue; }
    let run = k;
    while (run < cps.length && cps[run] === '`') run += 1;
    const len = run - k;
    let j = run;
    let found = -1;
    while (j < cps.length) {
      if (cps[j] === '\n' && cps[j + 1] === '\n') break; // 문단 끝
      if (cps[j] === '`') {
        let r = j;
        while (r < cps.length && cps[r] === '`') r += 1;
        if (r - j === len) { found = r; break; }
        j = r;
        continue;
      }
      j += 1;
    }
    if (found < 0) { k = run; continue; }
    blank(k, found);
    k = found;
  }
  return chars.join('');
}

function cpIndex(str, utf16Index) {
  return Array.from(str.slice(0, utf16Index)).length;
}

/** 1-based 줄·칸. `cp` 는 code point 배열의 index. */
function makeLocator(chars) {
  const starts = [0];
  chars.forEach((c, idx) => { if (c === '\n') starts.push(idx + 1); });
  return (cp) => {
    let lo = 0;
    let hi = starts.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (starts[mid] <= cp) lo = mid; else hi = mid - 1;
    }
    return { line: lo + 1, column: cp - starts[lo] + 1 };
  };
}

function normalizeLabel(label) {
  return label.trim().replace(/\s+/g, ' ').toLowerCase();
}

/**
 * 괄호 목적지 `(dest "title")` 를 읽는다. `open` 은 `(` 의 자리.
 * 돌려주는 값: { dest, end } 또는 null (닫히지 않음·형식 아님).
 */
function readParenDestination(cps, open) {
  let i = open + 1;
  while (i < cps.length && /\s/.test(cps[i])) i += 1;
  let dest = '';
  if (cps[i] === '<') {
    i += 1;
    for (;;) {
      if (i >= cps.length || cps[i] === '\n' || cps[i] === '<') return null;
      if (cps[i] === '\\' && i + 1 < cps.length) { dest += cps[i + 1]; i += 2; continue; }
      if (cps[i] === '>') { i += 1; break; }
      dest += cps[i]; i += 1;
    }
  } else {
    let depth = 0;
    for (;;) {
      if (i >= cps.length) return null;
      const c = cps[i];
      if (c === '\\' && i + 1 < cps.length) { dest += c + cps[i + 1]; i += 2; continue; }
      if (c === '(') { depth += 1; dest += c; i += 1; continue; }
      if (c === ')') {
        if (depth === 0) break;
        depth -= 1; dest += c; i += 1; continue;
      }
      if (/\s/.test(c)) break;
      dest += c; i += 1;
    }
  }
  const afterDest = i;
  // 선택 제목: 공백 뒤 "..." '...' (...)
  while (i < cps.length && /\s/.test(cps[i])) i += 1;
  if (i < cps.length && cps[i] !== ')') {
    const q = cps[i];
    const closeQ = q === '"' ? '"' : q === "'" ? "'" : q === '(' ? ')' : null;
    if (!closeQ) {
      // 같은 줄에 `)` 가 더 있으면 공백이 든 목적지, 없으면 닫히지 않은 괄호다.
      let eol = afterDest;
      while (eol < cps.length && cps[eol] !== '\n') eol += 1;
      const sameLine = cps.slice(afterDest, eol).join('');
      return { error: sameLine.includes(')') ? 'space' : 'unclosed', shown: cps.slice(open, Math.min(cps.length, afterDest + 40)).join('').split('\n')[0] };
    }
    i += 1;
    while (i < cps.length && cps[i] !== closeQ) { if (cps[i] === '\\') i += 1; i += 1; }
    if (i >= cps.length) return null;
    i += 1;
    while (i < cps.length && /\s/.test(cps[i])) i += 1;
  }
  if (cps[i] !== ')') return null;
  return { dest: unescape(dest), end: i + 1 };
}

function unescape(s) {
  return s.replace(/\\([!"#$%&'()*+,\-./:;<=>?@[\\\]^_`{|}~])/g, '$1');
}

/** `[` 에서 짝이 되는 `]` 를 찾는다. 안의 `[...]` 도 센다. 못 찾으면 -1. */
function matchBracket(cps, open) {
  let depth = 0;
  for (let i = open; i < cps.length; i += 1) {
    const c = cps[i];
    if (c === '\\') { i += 1; continue; }
    if (c === '[') depth += 1;
    else if (c === ']') { depth -= 1; if (depth === 0) return i; }
  }
  return -1;
}

/**
 * 문서 하나에서 연결 후보를 뽑는다.
 * @returns {{links: Array<{kind, text, target, line, column, label?}>, definitions: Map}}
 *   kind: 'inline' | 'image' | 'reference' | 'definition' | 'html' | 'undefined_reference'
 */
export function extractLinks(text) {
  const masked = maskNonContent(text);
  const cps = Array.from(masked);
  const locate = makeLocator(cps);
  const links = [];

  // 참조 정의 — 줄 시작(3칸 이하 들여쓰기) `[label]: dest` . 정의 줄은 다른 연결 해석에서 빼기 위해 자리를 기억한다.
  const definitions = new Map();
  const definitionSpans = [];
  {
    let lineStart = 0;
    for (let i = 0; i <= cps.length; i += 1) {
      if (i === cps.length || cps[i] === '\n') {
        const line = cps.slice(lineStart, i).join('');
        const m = line.match(/^ {0,3}\[((?:[^\]\\]|\\.)+)\]:\s*(<[^>\n]*>|\S+)(?:\s+(?:"[^"]*"|'[^']*'|\([^)]*\)))?\s*$/);
        if (m) {
          const label = normalizeLabel(m[1]);
          let dest = m[2];
          if (dest.startsWith('<') && dest.endsWith('>')) dest = dest.slice(1, -1);
          dest = unescape(dest);
          const loc = locate(lineStart);
          if (!definitions.has(label)) definitions.set(label, { target: dest, line: loc.line });
          links.push({ kind: 'definition', text: m[1], target: dest, line: loc.line, column: loc.column, label });
          definitionSpans.push([lineStart, i]);
        } else {
          // 정의처럼 시작하지만 형식이 맞지 않는 줄 — 렌더러는 글로 보여 주고 참조는 모두 끊어진다.
          const almost = line.match(/^ {0,3}\[((?:[^\]\\]|\\.)+)\]:\s*(\S.*)$/);
          if (almost) {
            const loc = locate(lineStart);
            links.push({ kind: 'unsupported', text: almost[1], target: almost[2], line: loc.line, column: loc.column, reason: '참조 정의 형식을 읽을 수 없다 — 공백이 든 대상은 <...> 로 감싼다' });
            definitionSpans.push([lineStart, i]);
          }
        }
        lineStart = i + 1;
      }
    }
  }
  const inDefinition = (idx) => definitionSpans.some(([a, b]) => idx >= a && idx < b);

  const pendingRefs = [];
  for (let i = 0; i < cps.length; i += 1) {
    const c = cps[i];
    if (c === '\\') { i += 1; continue; }
    if (c !== '[') continue;
    if (inDefinition(i)) continue;
    const isImage = i > 0 && cps[i - 1] === '!' && (i < 2 || cps[i - 2] !== '\\');
    const close = matchBracket(cps, i);
    if (close < 0) continue;
    const label = cps.slice(i + 1, close).join('');
    const loc = locate(isImage ? i - 1 : i);
    const next = cps[close + 1];
    if (next === '(') {
      const read = readParenDestination(cps, close + 1);
      if (read && read.dest !== undefined) {
        links.push({ kind: isImage ? 'image' : 'inline', text: label, target: read.dest, line: loc.line, column: loc.column });
        continue;
      }
      // `](` 로 열렸는데 연결이 되지 않음 — 렌더러도 연결로 보지 않는다. 조용히 넘기지 않는다.
      const shown = read?.shown ?? cps.slice(close + 1, Math.min(cps.length, close + 60)).join('').split('\n')[0];
      const reason = read?.error === 'space'
        ? '목적지에 공백이 있다 — 렌더러가 연결로 보지 않는다. <...> 로 감싸거나 %20 으로 적는다'
        : '괄호 목적지가 닫히지 않았거나 형식을 읽을 수 없다';
      links.push({ kind: 'unsupported', text: label, target: shown, line: loc.line, column: loc.column, reason });
      continue;
    }
    if (next === '[') {
      const close2 = matchBracket(cps, close + 1);
      if (close2 > 0) {
        const inner = cps.slice(close + 2, close2).join('');
        const ref = normalizeLabel(inner === '' ? label : inner);
        pendingRefs.push({ kind: isImage ? 'image' : 'reference', text: label, ref, line: loc.line, column: loc.column, shortcut: false });
        i = close2; // `[label]` 부분을 바로가기 참조로 다시 읽지 않는다
        continue;
      }
    }
    // 바로가기 참조 `[label]` — 정의가 있을 때만 연결로 본다. 없으면 보통 글이다.
    pendingRefs.push({ kind: isImage ? 'image' : 'reference', text: label, ref: normalizeLabel(label), line: loc.line, column: loc.column, shortcut: true });
  }
  for (const p of pendingRefs) {
    const def = definitions.get(p.ref);
    if (def) {
      links.push({ kind: p.kind === 'image' ? 'image' : 'reference', text: p.text, target: def.target, line: p.line, column: p.column, label: p.ref, via_definition_line: def.line });
    } else if (!p.shortcut) {
      links.push({ kind: 'undefined_reference', text: p.text, target: `[${p.ref}]`, line: p.line, column: p.column, label: p.ref, reason: '참조 정의가 없다' });
    }
  }

  // HTML 속성 — 검사하지 않고 미지원으로 보고한다.
  const html = /<(?:a|img|source|link)\b[^>]*?\b(?:href|src)\s*=\s*("[^"]*"|'[^']*'|[^\s>]+)/gi;
  const maskedStr = cps.join('');
  let hm;
  while ((hm = html.exec(maskedStr)) !== null) {
    let target = hm[1];
    if ((target.startsWith('"') && target.endsWith('"')) || (target.startsWith("'") && target.endsWith("'"))) target = target.slice(1, -1);
    const loc = locate(cpIndex(maskedStr, hm.index));
    links.push({ kind: 'html', text: hm[0].slice(0, 40), target, line: loc.line, column: loc.column, reason: 'HTML 속성 연결은 검사하지 않는다' });
  }

  links.sort((a, b) => a.line - b.line || a.column - b.column);
  return { links, definitions };
}

export function hasScheme(target) {
  return SCHEME.test(target);
}
