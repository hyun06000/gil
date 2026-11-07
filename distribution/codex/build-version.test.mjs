import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {buildVersion, checkVersionAgreement, selectedPreview} from './build-version.mjs';

test('packaging selects an explicit candidate stamp or the unchanged development manifest version', () => {
  const dev = '0.2.0+codex.20260927174145';
  assert.equal(buildVersion(undefined, dev), dev);
  assert.equal(buildVersion('', dev), dev);
  assert.equal(buildVersion('0.2.1-preview.25', dev), '0.2.1-preview.25');
  for (const bad of ['0.2.1', '0.2.1-preview.0', '0.2.1-preview.01', ' 0.2.1-preview.25', '0.2.1-preview.25\n', '0.2.1-preview.25/escape', '01.2.1-preview.25', null])
    assert.throws(() => buildVersion(bad, dev));
});
test('a mislabeled binary or stale MCP connection cannot pass release version acceptance', () => {
  const v = '0.2.1-preview.25';
  checkVersionAgreement(v, `gil ${v}\n`, v, v);
  for (const values of [['gil 0.1.0\n',v,v], [`gil ${v}`, '0.1.0', v], [`gil ${v}`, v, '0.2.1-preview.24']])
    assert.throws(() => checkVersionAgreement(v, ...values), /version must match/);
});
test('manual CI input selects the precompile stamp; PR events cannot label a candidate', async () => {
  const ci = {GITHUB_ACTIONS:'true', GITHUB_EVENT_NAME:'workflow_dispatch', GITHUB_EVENT_PATH:'fixture-event.json'};
  let reads = 0;
  const read = async path => { assert.equal(path, 'fixture-event.json'); reads++; return JSON.stringify({inputs:{unsigned_preview_version:'0.2.1-preview.25'}, unrelated:'ignored'}); };
  assert.equal(await selectedPreview(ci, read), '0.2.1-preview.25');
  assert.equal(await selectedPreview({...ci, GITHUB_EVENT_NAME:'pull_request'}, read), undefined);
  assert.equal(await selectedPreview({...ci, GITHUB_ACTIONS:'false'}, read), undefined);
  assert.equal(await selectedPreview({...ci, GIL_PREVIEW_VERSION:'0.2.1-preview.26'}, read), '0.2.1-preview.26');
  assert.equal(reads, 1);
  await assert.rejects(selectedPreview({...ci, GITHUB_EVENT_PATH:''}, read));
  await assert.rejects(selectedPreview(ci, async () => 'invalid JSON'));
  assert.equal(await selectedPreview(ci, async () => '{}'), '');
  const script = await readFile(new URL('./build-preview.mjs', import.meta.url), 'utf8');
  assert.match(script, /GIL_BUILD_VERSION: coreVersion/);
  assert.match(script, /checkNativeOutputContract\([^\n]+coreVersion/);
});
