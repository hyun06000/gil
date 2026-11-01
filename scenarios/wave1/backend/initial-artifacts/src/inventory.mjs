// Synthetic inventory module for the Wave 1 backend scenario. Node standard library only.
export function createInventory(initial = {}) {
  const stock = new Map(Object.entries(initial));
  const reserved = new Map();
  const get = (map, sku) => map.get(sku) ?? 0;
  return {
    available(sku) { return get(stock, sku) - get(reserved, sku); },
    reserve(sku, qty) {
      if (!Number.isInteger(qty) || qty <= 0) throw new RangeError('qty must be a positive integer');
      if (qty > this.available(sku)) return false;
      reserved.set(sku, get(reserved, sku) + qty);
      return true;
    },
    release(sku, qty) {
      if (!Number.isInteger(qty) || qty <= 0) throw new RangeError('qty must be a positive integer');
      reserved.set(sku, Math.max(0, get(reserved, sku) - qty));
    },
    commit(sku) {
      stock.set(sku, get(stock, sku) - get(reserved, sku));
      return get(stock, sku);
    },
    snapshot() { return { stock: Object.fromEntries(stock), reserved: Object.fromEntries(reserved) }; },
  };
}
