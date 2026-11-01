import test from 'node:test';
import assert from 'node:assert/strict';
import { createInventory } from '../src/inventory.mjs';

test('reserve refuses more than available and accepts exactly the available amount', () => {
  const inv = createInventory({ pen: 3 });
  assert.equal(inv.reserve('pen', 4), false);
  assert.equal(inv.reserve('pen', 3), true);
  assert.equal(inv.available('pen'), 0);
});

test('commit reduces stock once and frees the reservation', () => {
  const inv = createInventory({ pen: 5 });
  assert.equal(inv.reserve('pen', 2), true);
  assert.equal(inv.commit('pen'), 3);
  assert.equal(inv.available('pen'), 3, 'after commit nothing is reserved any more');
  assert.equal(inv.reserve('pen', 3), true, 'the remaining stock is reservable again');
});

test('release never goes negative and rejects bad quantities', () => {
  const inv = createInventory({ pen: 1 });
  inv.reserve('pen', 1);
  inv.release('pen', 5);
  assert.equal(inv.available('pen'), 1);
  assert.throws(() => inv.release('pen', 0), RangeError);
});
