import assert from 'node:assert/strict';

// Called before compiling or creating output. Never take GIL_BUILD_VERSION from the caller:
// the packaging invocation selects the version and passes it to Cargo explicitly.
export function buildVersion(preview, development) {
  if (preview !== undefined && preview !== '') {
    assert.ok(typeof preview === 'string' && preview.length < 80
      && /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)-preview\.[1-9]\d*$/.test(preview),
    'candidate build requires an explicit X.Y.Z-preview.N version');
    return preview;
  }
  assert.match(development, /^\d+\.\d+\.\d+\+codex\.[0-9]+$/, 'expected development manifest version');
  return development;
}

export function checkVersionAgreement(expected, cli, probe, mcp) {
  assert.equal(cli.trim(), `gil ${expected}`, 'CLI version must match the selected build/release');
  assert.equal(probe, expected, 'probe version must match the selected build/release');
  assert.equal(mcp, expected, 'MCP version must match the selected build/release');
}

// GitHub exposes workflow_dispatch inputs in its event file. Read only that field;
// PR/push event data never selects a release stamp, and no workflow mutation is needed.
export async function selectedPreview(env, readEvent) {
  if (env.GIL_PREVIEW_VERSION !== undefined) return env.GIL_PREVIEW_VERSION;
  if (env.GITHUB_ACTIONS !== 'true' || env.GITHUB_EVENT_NAME !== 'workflow_dispatch') return undefined;
  assert.ok(env.GITHUB_EVENT_PATH, 'manual CI event path is required');
  const event = JSON.parse(await readEvent(env.GITHUB_EVENT_PATH, 'utf8'));
  return event.inputs?.unsigned_preview_version ?? '';
}
