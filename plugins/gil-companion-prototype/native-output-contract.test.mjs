import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {AjvJsonSchemaValidator} from '@modelcontextprotocol/sdk/validation/ajv';
import {checkNativeOutputContract} from './native-output-contract.mjs';

const tools = JSON.parse(readFileSync(new URL('../../src/mcp/tools.json', import.meta.url)));
test('strict start/context outputs declare only the prepared Monitor handoff', () => {
  for (const name of ['gil_start', 'gil_context']) {
    const schema = tools.find(t => t.name === name).outputSchema;
    const valid = new AjvJsonSchemaValidator().getValidator(schema);
    const receipt = {ok:true, exit_code:0, said:'receipt', problem:''};
    const next = {name:'show_gil_monitor', arguments:{scope_id:`project:${'a'.repeat(64)}`}};
    assert.equal(valid(receipt).valid, true);
    assert.equal(valid({...receipt, next_call:next}).valid, true);
    for (const bad of [null, {}, {...next, name:'gil_start'}, {...next, arguments:{}},
      {...next, arguments:{scope_id:'invented'}}, {...next, extra:true}]) {
      assert.equal(valid({...receipt, next_call:bad}).valid, false);
    }
    assert.equal(valid({...receipt, undeclared:true}).valid, false);
    // This is the preview.15 regression: a valid handoff rejected by the advertised schema.
    const prior = structuredClone(schema); delete prior.properties.next_call;
    assert.equal(new AjvJsonSchemaValidator().getValidator(prior)({...receipt, next_call:next}).valid, false);
  }
});

test('real SDK validates native start, lost-receipt recovery, Monitor and first Question',
  {skip:!process.env.GIL_NATIVE_MCP, timeout:120000}, async () => {
    assert.equal((await checkNativeOutputContract(process.env.GIL_NATIVE_MCP)).passed, true);
  });
