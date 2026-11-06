// Maintainer-only SDK acceptance. Never runs in, or modifies, a user's project.
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
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
  const hook = input => {
    const got = spawnSync(resolve(binary), ['write-guard','hook','--auto'], {
      cwd:scratch, env:{...process.env,HOME:home,LOCALAPPDATA:local}, encoding:'utf8',
      input:JSON.stringify({session_id:'sdk-host-session',cwd:home,...input}), timeout:15000});
    assert.equal(got.status,0,got.stderr);
    return JSON.parse(got.stdout);
  };
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
    hook({hook_event_name:'PostToolUse',tool_name:'mcp__gil__gil_start',tool_input:{project_root:root},tool_response:started});
    assert.deepEqual(hook({hook_event_name:'Stop',last_assistant_message:'report complete'}),{});
    assert.equal(hook({hook_event_name:'PreToolUse',tool_name:'Write',tool_input:{file_path:join(root,'report.docx')}}).hookSpecificOutput.permissionDecision,'deny');
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
    const earlyWait = await call('gil_await_user', {project_root:root, question:'What is your goal?'});
    assert.equal(earlyWait.isError, true);
    const opened = await call('gil_open', {project_root:root, kind:'question',
      contract:'objective: clarify the user goal\nnext_action: ask what result the user wants\ndone_when: the user states a goal or remaining uncertainty'});
    assert.equal(opened.structuredContent.ok, true);
    assert.ok(opened.content.some(c => c.type === 'text' && c.text.includes('gil_await_user(project_root, question)')),
      'the actual Question receipt describes the optional compatibility check');
    assert.deepEqual(hook({hook_event_name:'Stop',last_assistant_message:'What is your goal?'}),{},
      'a Question can wait without a handoff call');
    assert.deepEqual(hook({hook_event_name:'PreToolUse',tool_name:'WebSearch',tool_input:{query:'file-free research'}}),{});
    assert.deepEqual(hook({hook_event_name:'PreToolUse',tool_name:'Write',tool_input:{file_path:join(scratch,'unrelated.txt')}}),{});
    const waiting = await call('gil_await_user', {project_root:root, question:'What is your goal?'});
    assert.equal(waiting.structuredContent.ok, true);
    assert.equal(waiting.structuredContent.handoff.status, 'awaiting_user');
    assert.equal(waiting.structuredContent.handoff.question, 'What is your goal?');
    hook({hook_event_name:'PostToolUse',tool_name:'mcp__gil__gil_await_user',tool_input:{project_root:root,question:'What is your goal?'},tool_response:waiting});
    assert.deepEqual(hook({hook_event_name:'Stop',last_assistant_message:'What is your goal?'}),{});
    assert.deepEqual(hook({hook_event_name:'UserPromptSubmit',prompt:'Make my report'}),{});
    assert.deepEqual(hook({hook_event_name:'Stop',last_assistant_message:'Here is the report'}),{});
    assert.deepEqual(hook({hook_event_name:'PreToolUse',tool_name:'mcp__ccd_turn__wrap_up',tool_input:{ask:'Here is the report'}}),{});
    assert.equal(hook({hook_event_name:'PreToolUse',tool_name:'Write',tool_input:{file_path:join(root,'forbidden.txt')}}).hookSpecificOutput.permissionDecision,'deny');
    const again = await call('gil_start', {project_root:root});
    assert.equal(again.isError, true);
    const status = await call('gil_status', {project_root:root});
    assert.equal(status.structuredContent.ok, true);
    assert.match(status.structuredContent.said, /step:C1\/S1/);
    return {passed:true, validator:'MCP SDK Client.callTool',
      scope:'start/recovery, Monitor response, Question guidance and optional read-only handoff, file boundary; no visible Host UI claim'};
  } finally {
    await client?.close();
    await rm(scratch, {recursive:true, force:true});
  }
}
