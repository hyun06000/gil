// Reviewed packaging contracts; claims must also match the executable's fresh probe.
import assert from 'node:assert/strict';
// 'product' is the one user build per OS: every approved feature, both kinds of new project.
export const PROFILES = ['default', 'visual-authoring', 'approved-plan', 'exploration-flow', 'product'];
// What the executable itself reports for each packaged platform (std::env::consts OS/ARCH).
export const PLATFORMS = {'darwin-arm64': {os: 'macos', arch: 'aarch64'}, 'linux-x64': {os: 'linux', arch: 'x86_64'}};
export function compatibility(profile = 'default') {
  assert.ok(PROFILES.includes(profile), 'unknown runtime profile');
  // The product build READS up to 11; its starts WRITE 10 (tracked scope) or 9 (image reports). See startKinds().
  const storageMax = profile === 'product' ? 11 : ['approved-plan','exploration-flow'].includes(profile) ? 7 : profile === 'default' ? 4 : 5;
  return {protocol:{min:1,max:1}, action_surface:{min:profile==='default'?1:2,max:profile==='default'?1:2},
    storage_format:{min:4,max:storageMax},
    monitor_view_schema:{min:1,max:1}, node_detail_schema:{min:1,max:1}};
}
// The two kinds of NEW project the product build starts, with the storage each one writes. The
// range above is what the build can read; no start writes 11, and nothing is converted by reading.
export function startKinds(profile) {
  return profile === 'product' ? [{start_kind: 'tracked_scope', storage_format: 10}, {start_kind: 'image_reports', storage_format: 9}] : undefined;
}
// The exact Cargo feature list of a profile. Product builds never take runtime-testbed or recovery-auto-approval.
export function cargoFeatures(profile) {
  compatibility(profile);
  return {default: 'product-safety', 'visual-authoring': 'visual-authoring,product-safety',
    'approved-plan': 'approved-plan,visual-authoring,product-safety',
    'exploration-flow': 'exploration-flow,visual-authoring,product-safety',
    product: 'tracking-policy,exploration-flow,visual-authoring,product-safety'}[profile];
}
// The exact `cargo` arguments of a packaged Core build. One list is both what is run and what the
// provenance records, so the record cannot drift from the command. `binOnly`: the Linux archive
// ships the one `gil` executable; the macOS Plugin build keeps its existing invocation.
export function coreBuildArgs(profile, triple, {binOnly = false} = {}) {
  return ['build', '--locked', '--release', '-p', 'gil', ...(binOnly ? ['--bin', 'gil'] : []), '--target', triple, '--features', cargoFeatures(profile)];
}
export function checkCore(core, profile = 'default', platform = 'darwin-arm64') {
  assert.equal(core.schema_version,1); assert.equal(core.product,'gil_core');
  assert.ok(Object.hasOwn(PLATFORMS, platform), 'unknown packaged platform');
  assert.equal(core.os,PLATFORMS[platform].os); assert.equal(core.arch,PLATFORMS[platform].arch);
  const expected=compatibility(profile);
  for(const key of ['protocol','action_surface','storage_format'])
    assert.deepEqual(core[key],expected[key],`Core ${key} does not match package profile`);
  return expected;
}

export function developmentFeatures(profile) {
  compatibility(profile);
  return profile === 'product' ? ['tracking-policy', 'approved-plan', 'visual-authoring', 'exploration-flow']
    : profile === 'exploration-flow' ? ['approved-plan', 'visual-authoring', 'exploration-flow'] : profile === 'default' ? [] : profile === 'approved-plan'
    ? ['approved-plan', 'visual-authoring'] : ['visual-authoring'];
}
export function profileFromFeatures(features = []) {
  for (const profile of PROFILES) {
    if (JSON.stringify(features) === JSON.stringify(developmentFeatures(profile))) return profile;
  }
  throw new Error('unknown development feature set');
}
