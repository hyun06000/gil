// Maintainer-only SDK acceptance. Never runs in, or modifies, a user's project.
import assert from 'node:assert/strict';
import {mkdtemp, mkdir, realpath, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import {Client} from '@modelcontextprotocol/sdk/client/index.js';
import {StdioClientTransport} from '@modelcontextprotocol/sdk/client/stdio.js';

export async function checkNativeOutputContract(binary) {
  const scratch = await realpath(await mkdtemp(join(tmpdir(), 'gil-sdk-output-')));
  const root = join(scratch, 'project');
  const home = join(scratch, 'home');
  const local = join(scratch, 'localappdata');
  for (const dir of [root, home, local]) await mkdir(dir);
  let client;
  const connect = async () => {
    client = new Client({name:'gil-output-contract-check', version:'1'});
    const transport = new StdioClientTransport({command:resolve(binary), args:['mcp','--serve'],
      cwd:scratch, stderr:'pipe', env:{PATH:'/usr/bin:/bin', HOME:home, LOCALAPPDATA:local,
        GIL_MONITOR_STATE_DIR:join(scratch, 'bindings')}});
    await client.connect(transport);
    transport.stderr?.resume();
    // listTools installs the SDK's real output-schema validators. Raw JSON-RPC misses this boundary.
    await client.listTools();
  };
  const call = (name, args) => client.callTool({name, arguments:args}, undefined, {timeout:30000});
  try {
    await connect();
    const started = await call('gil_start', {project_root:root});
    assert.equal(started.structuredContent.ok, true);
    const next = started.structuredContent.next_call;
    assert.equal(next.name, 'show_gil_monitor');
    assert.match(next.arguments.scope_id, /^project:[a-f0-9]{64}$/);
    // Lose the start receipt and connection before showing. Recovery must not restart the project.
    await client.close();
    await connect();
    const context = await call('gil_context', {project_root:root});
    assert.equal(context.structuredContent.ok, true);
    assert.deepEqual(context.structuredContent.next_call, next);
    const shown = await call(next.name, next.arguments);
    assert.equal(shown.isError, false);
    assert.equal(shown.structuredContent.view.timeline.length, 1);
    assert.equal(shown.structuredContent.view.current.step_ref, null);
    const opened = await call('gil_open', {project_root:root, kind:'question',
      contract:'objective: clarify the user goal\nnext_action: ask what result the user wants\ndone_when: the user states a goal or remaining uncertainty'});
    assert.equal(opened.structuredContent.ok, true);
    const again = await call('gil_start', {project_root:root});
    assert.equal(again.isError, true);
    const status = await call('gil_status', {project_root:root});
    assert.equal(status.structuredContent.ok, true);
    assert.match(status.structuredContent.said, /step:C1\/S1/);
    return {passed:true, validator:'MCP SDK Client.callTool',
      scope:'start, lost-receipt context recovery, Monitor response, first Question; no visible Host UI claim'};
  } finally {
    await client?.close();
    await rm(scratch, {recursive:true, force:true});
  }
}
