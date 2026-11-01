// Development only. Installed plugins carry this self-contained HTML, not a build toolchain.
import { build } from 'esbuild';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { resolve } from 'node:path';

const here = import.meta.dirname;
const result = await build({ absWorkingDir: here, entryPoints: ['app.js'], bundle: true,
  write: false, format: 'iife', platform: 'browser', minify: true, legalComments: 'inline' });
let html = await readFile(resolve(here, '../ui/index.html'), 'utf8');
const css = await readFile(resolve(here, '../ui/companion.css'), 'utf8');
const extra = await readFile(resolve(here, 'monitor.css'), 'utf8');
// Every anchor must exist exactly once: a renamed line in index.html must fail the build,
// never silently drop the Monitor's buttons or its evidence block.
const must = (from, to) => {
  const parts = html.split(from);
  if (parts.length !== 2) throw new Error(`build anchor not found exactly once: ${from.slice(0, 60)}`);
  html = parts.join(typeof to === 'function' ? to(from) : to);
};
must('<title>GIL Companion</title>', '<title>GIL Monitor</title>');
must('<link rel="stylesheet" href="companion.css">', `<style>${css}\n${extra}</style>`);
must('<b>GIL Companion</b>', '<b>GIL Monitor</b>');
// The approval banner stands at the top, so a pending decision is never only at the end of a long graph.
must('<div class="notice"></div>', (line) => `${line}
  <section id="plan-banner" class="plan-banner" role="status" aria-live="polite" hidden></section>`);
// Secondary controls sit in the header beside the Project switcher, as ghost buttons.
must('<button class="refresh" type="button" title="완전한 View 를 다시 읽는다">새로고침</button>',
  (line) => `${line}
      <button id="layout" type="button" disabled>세로로 보기</button>
      <button id="native" type="button" disabled>별도 창 열기</button>`);
// The display-mode row sits at the bottom. The Host card and the header already carry the title.
must('<footer class="status"><i></i><span></span></footer>',
  (line) => `${line}
  <div class="plan-panels">
  <section id="plan-review" class="plan-review" role="region" aria-label="사용자 계획 승인" hidden></section>
  <section id="revisit-review" class="plan-review" role="region" aria-label="되돌아가기 변경 목록과 승인" hidden></section>
  </div>
  <section class="host-bar" aria-label="Monitor 표시 모드">
    <span id="host-say" role="status">MCP Apps 연결 중…</span>
    <button id="expand" type="button" disabled>모니터 펼치기</button>
    <button id="inline" type="button" disabled>대화 안으로</button>
    <details><summary>연결 정보</summary><pre id="host-evidence"></pre></details>
  </section>`);
must('<script src="host.js"></script>\n<script type="module" src="companion.js"></script>',
  `<script>${result.outputFiles[0].text.replace(/<\/script/gi, '<\\/script')}</script>`);
const digest = createHash('sha256').update(html).digest('hex');
const output = resolve(here, '../plugins/gil-companion-prototype/assets');
await mkdir(output, { recursive: true });
await writeFile(resolve(output, 'monitor.html'), html);
await writeFile(resolve(output, 'monitor.json'), JSON.stringify({
  resource_uri: `ui://gil-monitor/${digest.slice(0, 20)}.html`, sha256: digest,
}, null, 2) + '\n');
console.log(`GIL Monitor bundle: ${Buffer.byteLength(html)} bytes · ${digest.slice(0, 20)}`);
