// Mock Host for fixture screenshots only. Not a Codex/Claude acceptance.
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
const repo = process.argv[2]; // repository root, passed explicitly
const port = Number(process.argv[3] || 48731);
const files = {
  '/': ['index.html', 'text/html', import.meta.dirname + '/index.html'],
  '/monitor.html': ['monitor.html', 'text/html', repo + '/plugins/gil-companion-prototype/assets/monitor.html'],
  '/view.json': ['view.json', 'application/json', repo + '/ui/fixtures/reading-one/view.json'],
  '/details.json': ['details.json', 'application/json', repo + '/ui/fixtures/reading-one/details.json'],
};
createServer(async (req, res) => {
  const path = new URL(req.url, 'http://x').pathname;
  const hit = files[path];
  if (!hit) { res.writeHead(404).end(); return; }
  res.setHeader('content-type', hit[1]); res.end(await readFile(hit[2]));
}).listen(port, '127.0.0.1', () => console.log('ready', port));
