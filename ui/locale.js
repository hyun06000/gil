// User-facing language of the shared Monitor UI.
//
// The same normalization rule as the Rust Core (`src/locale.rs`): one BCP 47 tag from the
// Host becomes one of five supported languages. Both sides are checked against
// `src/i18n/locale-cases.json`. The MCP App chooses from the Host context's `locale` and
// falls back to English when the Host says nothing; the native Companion window, which has
// no Host context, follows the browser's own language. Nothing here reads a Host's name.
//
// Lookup falls back from the chosen language to English and finally to the key itself, so a
// missing translation shows a readable key instead of breaking the screen.

import en from './i18n/en.js';
import ko from './i18n/ko.js';
import zhHans from './i18n/zh-Hans.js';
import zhHant from './i18n/zh-Hant.js';
import ja from './i18n/ja.js';

export const LOCALES = ['en', 'ko', 'zh-Hans', 'zh-Hant', 'ja'];
export const TABLES = { en, ko, 'zh-Hans': zhHans, 'zh-Hant': zhHant, ja };

export function normalizeLocale(tag) {
  if (typeof tag !== 'string') return 'en';
  const parts = tag.trim().split(/[-_]/).filter(Boolean);
  if (parts.length === 0) return 'en';
  const language = parts[0].toLowerCase();
  if (language === 'en') return 'en';
  if (language === 'ko') return 'ko';
  if (language === 'ja') return 'ja';
  if (language === 'zh') {
    const rest = parts.slice(1).map((p) => p.toLowerCase());
    const script = rest.find((p) => p.length === 4 && /^[a-z]+$/.test(p));
    if (script) return script === 'hant' ? 'zh-Hant' : 'zh-Hans';
    const region = rest.find((p) => p.length === 2 && /^[a-z]+$/.test(p));
    return ['tw', 'hk', 'mo'].includes(region) ? 'zh-Hant' : 'zh-Hans';
  }
  return 'en';
}

let current = 'en';
let chosen = false;

/** Choose the language from a Host tag. `null`/`undefined` means "the Host said nothing" → en. */
export function setLocale(tag) {
  current = normalizeLocale(tag);
  chosen = true;
  if (typeof document !== 'undefined' && document.documentElement) {
    document.documentElement.setAttribute('lang', current);
  }
  return current;
}
export function getLocale() { return current; }
/** True once something explicitly chose a language (the App always does; the Companion may not). */
export function localeChosen() { return chosen; }

/** Replace `{name}` slots. Unknown slots stay as written. */
export function fill(template, args) {
  if (!args) return template;
  return template.replace(/\{([a-z_][a-z0-9_]*)\}/g, (whole, name) =>
    Object.prototype.hasOwnProperty.call(args, name) ? String(args[name]) : whole);
}

/** One sentence in the current language, English as fallback, the key as the last resort. */
export function t(key, args) {
  const table = TABLES[current] || en;
  const found = Object.prototype.hasOwnProperty.call(table, key) ? table[key]
    : Object.prototype.hasOwnProperty.call(en, key) ? en[key] : key;
  return fill(found, args);
}
export function tIn(locale, key, args) {
  const table = TABLES[normalizeLocale(locale)] || en;
  const found = Object.prototype.hasOwnProperty.call(table, key) ? table[key]
    : Object.prototype.hasOwnProperty.call(en, key) ? en[key] : key;
  return fill(found, args);
}

/** Fill static markup: `data-i18n` (text), `data-i18n-title`, `data-i18n-aria` (aria-label), `data-i18n-html` is never used. */
export function applyStaticText(root) {
  if (!root || typeof root.querySelectorAll !== 'function') return;
  for (const el of root.querySelectorAll('[data-i18n]')) el.textContent = t(el.getAttribute('data-i18n'));
  for (const el of root.querySelectorAll('[data-i18n-title]')) el.setAttribute('title', t(el.getAttribute('data-i18n-title')));
  for (const el of root.querySelectorAll('[data-i18n-aria]')) el.setAttribute('aria-label', t(el.getAttribute('data-i18n-aria')));
  if (typeof document !== 'undefined' && root === document && TABLES[current]['page.title']) {
    document.title = t('page.title');
  }
}
