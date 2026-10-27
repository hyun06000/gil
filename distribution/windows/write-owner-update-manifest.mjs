#!/usr/bin/env node

import {readFile, writeFile} from 'node:fs/promises';

function fail(message) {
  console.error(message);
  process.exit(1);
}

const [version, assetUrl, signaturePath, outputPath, ...extra] = process.argv.slice(2);
if (!version || !assetUrl || !signaturePath || !outputPath || extra.length) {
  fail('usage: write-owner-update-manifest.mjs VERSION HTTPS_ASSET_URL SIGNATURE_FILE OUTPUT_JSON');
}
if (!/^\d+\.\d+\.\d+-owner\.[1-9]\d*$/.test(version)) {
  fail('VERSION must look like 0.1.1-owner.2');
}
let url;
try {
  url = new URL(assetUrl);
} catch {
  fail('HTTPS_ASSET_URL is not a URL');
}
if (url.protocol !== 'https:' || url.username || url.password) {
  fail('HTTPS_ASSET_URL must be a credential-free HTTPS URL');
}
const signature = (await readFile(signaturePath, 'utf8')).trim();
if (!signature || /\s/.test(signature)) fail('signature file must contain one non-empty token');

const manifest = {
  version,
  notes: '사용자가 요청한 GIL Companion 및 Core 업데이트입니다. Codex는 닫지 않아도 됩니다.',
  platforms: {
    'windows-x86_64': {
      url: url.href,
      signature,
    },
  },
};
await writeFile(outputPath, `${JSON.stringify(manifest, null, 2)}\n`, {flag: 'wx'});
