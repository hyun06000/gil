import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { normalizeLocale, t, tIn, fill, setLocale, getLocale, LOCALES, TABLES, applyStaticText } from '../ui/locale.js';

const shared = JSON.parse(readFileSync(new URL('../src/i18n/locale-cases.json', import.meta.url), 'utf8'));

test('the shared case table normalizes on the JS side exactly as in Rust', () => {
  assert.ok(shared.cases.length >= 40, 'the case table shrank');
  for (const [input, expected] of shared.cases) assert.equal(normalizeLocale(input), expected, JSON.stringify(input));
  for (const odd of [undefined, 12, {}, [], true]) assert.equal(normalizeLocale(odd), 'en');
});

// Languages whose tables must carry every English key. Extended as translations land.
const COMPLETE = ['en', 'ko', 'zh-Hans', 'zh-Hant', 'ja'];

test('every language table has exactly the English keys with the same placeholders', () => {
  const english = TABLES.en;
  const keys = Object.keys(english);
  assert.ok(keys.length > 0);
  assert.ok(COMPLETE.every((locale) => LOCALES.includes(locale)));
  const slots = (s) => [...String(s).matchAll(/\{([a-z_][a-z0-9_]*)\}/g)].map((m) => m[1]).sort();
  for (const locale of COMPLETE) {
    if (locale === 'en') continue;
    const table = TABLES[locale];
    const missing = keys.filter((k) => !Object.prototype.hasOwnProperty.call(table, k));
    const extra = Object.keys(table).filter((k) => !Object.prototype.hasOwnProperty.call(english, k));
    assert.deepEqual(missing, [], `${locale} lacks ${missing.length} key(s)`);
    assert.deepEqual(extra, [], `${locale} has keys unknown to en`);
    for (const key of keys) {
      assert.ok(String(table[key]).trim().length > 0, `${locale}:${key} is empty`);
      assert.deepEqual(slots(table[key]), slots(english[key]), `${locale}:${key} placeholders differ`);
    }
  }
});

test('lookup falls back to English and then to the key; fill replaces named slots only', () => {
  assert.equal(fill('{a} and {b} and {a} {c}', { a: 1, b: 'two' }), '1 and two and 1 {c}');
  assert.equal(tIn('ja', 'no.such.key'), 'no.such.key');
  assert.equal(tIn('en', 'no.such.key'), 'no.such.key');
  setLocale('zh-TW'); assert.equal(getLocale(), 'zh-Hant');
  setLocale(null); assert.equal(getLocale(), 'en', 'a Host without a locale gets English');
  setLocale('ko-KR'); assert.equal(getLocale(), 'ko');
  assert.equal(t('locale.self_test', { n: 3 }), TABLES.ko['locale.self_test'].replace('{n}', '3'));
  setLocale('en');
});

test('applyStaticText fills data-i18n text, titles and aria labels and tolerates a missing root', () => {
  const made = (attrs) => ({ attrs, textContent: '', getAttribute: (k) => attrs[k], setAttribute(k, v) { attrs[k] = v; } });
  const a = made({ 'data-i18n': 'locale.self_test' }), b = made({ 'data-i18n-title': 'locale.self_test' }), c = made({ 'data-i18n-aria': 'locale.self_test' });
  const root = { querySelectorAll: (q) => q === '[data-i18n]' ? [a] : q === '[data-i18n-title]' ? [b] : [c] };
  setLocale('ja');
  applyStaticText(root);
  assert.equal(a.textContent, TABLES.ja['locale.self_test']);
  assert.equal(b.attrs.title, TABLES.ja['locale.self_test']);
  assert.equal(c.attrs['aria-label'], TABLES.ja['locale.self_test']);
  applyStaticText(null);
  setLocale('en');
});
