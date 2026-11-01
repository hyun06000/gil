// GIL Report text: `key: value` lines and `key: |` blocks with two-space indented lines.
// Mirrors the wire form used by gil_close; only what the harness needs to read and write.
export function parseReport(text) {
  const fields = {};
  if (typeof text !== 'string') return fields;
  const lines = text.split(/\r?\n/);
  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];
    const m = line.match(/^([A-Za-z0-9_.\-]+):(?:\s(.*))?$/);
    if (!m) continue;
    const key = m[1];
    const rest = (m[2] ?? '').trimEnd();
    if (rest === '|') {
      const block = [];
      while (i + 1 < lines.length && (lines[i + 1].startsWith('  ') || lines[i + 1] === '')) {
        i += 1;
        if (lines[i] === '' && !(i + 1 < lines.length && lines[i + 1].startsWith('  '))) break;
        block.push(lines[i].slice(2));
      }
      fields[key] = block.join('\n');
    } else {
      fields[key] = rest;
    }
  }
  return fields;
}

export function formatReport(fields) {
  let out = '';
  for (const [key, raw] of Object.entries(fields)) {
    const value = String(raw);
    if (value.includes('\n')) out += `${key}: |\n${value.split('\n').map((l) => `  ${l}`).join('\n')}\n`;
    else out += `${key}: ${value}\n`;
  }
  return out;
}

/** evidence_files as a list of project-relative paths (one per line or a single value). */
export function evidenceFiles(fields) {
  const raw = fields.evidence_files;
  if (!raw) return [];
  return raw.split('\n').map((s) => s.trim()).filter(Boolean);
}
