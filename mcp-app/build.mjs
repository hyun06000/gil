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
html = html.replace('<title>GIL Companion</title>', '<title>GIL Monitor</title>')
  .replace('<link rel="stylesheet" href="companion.css">', () => `<style>${css}\n${extra}</style>`)
  .replace('<body>', `<body><section class="host-bar" aria-label="Monitor 표시 모드">
    <b>GIL Monitor</b><button id="expand" disabled>모니터 펼치기</button>
    <button id="inline" disabled>대화 안으로</button><button id="layout" disabled>세로로 보기</button>
    <button id="native" disabled>별도 창 열기</button>
    <span id="host-say" role="status">MCP Apps 연결 중…</span>
    <details><summary>연결 정보</summary><pre id="host-evidence"></pre></details>
  </section>`)
  .replace('<b>GIL Companion</b>', '<b>GIL Monitor</b>')
  .replace('<script src="host.js"></script>\n<script type="module" src="companion.js"></script>',
    () => `<script>${result.outputFiles[0].text.replace(/<\/script/gi, '<\\/script')}</script>`);
const digest = createHash('sha256').update(html).digest('hex');
const output = resolve(here, '../plugins/gil-companion-prototype/assets');
await mkdir(output, { recursive: true });
await writeFile(resolve(output, 'monitor.html'), html);
await writeFile(resolve(output, 'monitor.json'), JSON.stringify({
  resource_uri: `ui://gil-monitor/${digest.slice(0, 20)}.html`, sha256: digest,
}, null, 2) + '\n');
console.log(`GIL Monitor bundle: ${Buffer.byteLength(html)} bytes · ${digest.slice(0, 20)}`);
