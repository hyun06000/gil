import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';

const workflow = await readFile(new URL('../../.github/workflows/windows-msix.yml', import.meta.url), 'utf8');

test('manual MSIX build cannot publish a release or use a development Store identity fallback', () => {
  assert.match(workflow, /on:\n  workflow_dispatch:/);
  assert.doesNotMatch(workflow, /^  (push|pull_request|schedule):/m);
  assert.match(workflow, /permissions:\n  contents: read\n/);
  assert.doesNotMatch(workflow, /contents: write|gh release|git push|secrets\.|GH_TOKEN|GITHUB_TOKEN/);
  assert.match(workflow, /persist-credentials: false/);
  assert.match(workflow, /GIT_CONFIG_KEY_0: core\.autocrlf\n\s+GIT_CONFIG_VALUE_0: 'false'/,
    'Windows checkout must preserve LF for the embedded UI build anchors');
  assert.match(workflow, /throw 'Real Partner Center identity repository variables are required/);
  assert.doesNotMatch(workflow, /else \{ 'GIL.Dev' \}/);
});

test('tested product build precedes unsigned packaging, and artifact allowlist excludes certificates', () => {
  assert.match(workflow, /GIL_BUILD_VERSION: \$\{\{ inputs.version \}\}/);
  assert.match(workflow, /--features approved-plan,visual-authoring,product-safety/);
  const check = workflow.indexOf('./distribution/windows/msix/test-msix-ci.ps1');
  const pack = workflow.indexOf('./distribution/windows/msix/build-msix.ps1');
  assert.ok(check >= 0 && check < pack);
  assert.doesNotMatch(workflow.slice(pack), /-SignWithPfx|\.pfx|\.cer/);
  const artifacts = workflow.slice(workflow.indexOf('          path: |')).split('          if-no-files-found:')[0];
  assert.deepEqual(artifacts.trim().split('\n').slice(1).map(s => s.trim()), [
    'target/msix-submission/gil-store-upload.msix',
    'target/msix-submission/build-info.json',
    'target/msix-submission/checksums.txt',
  ]);
  assert.match(workflow, /source_commit = \$env:GITHUB_SHA/);
  assert.match(workflow, /msix_sha256 = \$hash/);
});
