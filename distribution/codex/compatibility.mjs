// Reviewed packaging contracts; claims must also match the executable's fresh probe.
import assert from 'node:assert/strict';
export function compatibility(profile = 'default') {
  assert.ok(['default', 'visual-authoring'].includes(profile), 'unknown runtime profile');
  return {protocol:{min:1,max:1}, action_surface:{min:profile==='default'?1:2,max:profile==='default'?1:2},
    storage_format:{min:4,max:profile==='default'?4:5},
    monitor_view_schema:{min:1,max:1}, node_detail_schema:{min:1,max:1}};
}
export function checkCore(core, profile = 'default') {
  assert.equal(core.schema_version,1); assert.equal(core.product,'gil_core');
  assert.equal(core.os,'macos'); assert.equal(core.arch,'aarch64');
  const expected=compatibility(profile);
  for(const key of ['protocol','action_surface','storage_format'])
    assert.deepEqual(core[key],expected[key],`Core ${key} does not match package profile`);
  return expected;
}
