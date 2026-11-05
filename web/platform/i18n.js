// Lightweight UI localization for the buildless platform pages.
// zh-CN is the source language: UI strings are written in Chinese in HTML/JS and
// translated at render time from locales/<lang>.js (keyed by the source text).
// Machine-read identifiers (API names, message types, catalog ids, share-link
// fields) are never passed through here; only rendered text and a11y attributes are.
import en from './locales/en.js';
import enTemplates from './locales/en-templates.js';

export const LOCALES = Object.freeze({'zh-CN': '中文', en: 'English'});
export const SOURCE_LOCALE = 'zh-CN';
export const STORAGE_KEY = 'esp-online-tester.locale';
const DICTIONARIES = {en};
const HAN = /[\u4e00-\u9fff]/;
const ATTRIBUTES = ['title', 'aria-label', 'placeholder', 'alt', 'label'];
const SKIP = new Set(['SCRIPT', 'STYLE', 'PRE', 'TEXTAREA']);
const translatedText = new WeakMap(), translatedAttributes = new WeakMap();
const escapePattern = text => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
const templates = enTemplates.map(([source, target, localized = []]) => {
  const slots = []; let pattern = '', end = 0;
  for (const match of source.trim().matchAll(/\{(\d+)\}/g)) {
    pattern += escapePattern(source.trim().slice(end, match.index)) + '([\\s\\S]*?)';
    slots.push(Number(match[1])); end = match.index + match[0].length;
  }
  return {pattern: new RegExp('^' + pattern + escapePattern(source.trim().slice(end)) + '$'), target, slots, localized};
});

export function normalizeLocale(value) {
  const tag = String(value || '').trim().toLowerCase();
  if (!tag) return null;
  if (tag.startsWith('zh')) return 'zh-CN';
  if (tag.startsWith('en')) return 'en';
  return null;
}

// Saved choice > ?lang= > navigator languages > fallback.
export function detectLocale({saved, search = '', languages = [], fallback = SOURCE_LOCALE} = {}) {
  const fromSaved = normalizeLocale(saved);
  if (fromSaved) return fromSaved;
  const fromQuery = normalizeLocale(new URLSearchParams(search).get('lang'));
  if (fromQuery) return fromQuery;
  for (const language of languages) { const match = normalizeLocale(language); if (match) return match; }
  return languages.length ? 'en' : fallback;
}

let locale = SOURCE_LOCALE;
let phraseKeys = [];
export const currentLocale = () => locale;

export function setLocale(next) {
  locale = normalizeLocale(next) || SOURCE_LOCALE;
  const dictionary = DICTIONARIES[locale] || {};
  phraseKeys = Object.keys(dictionary).filter(key => key.length > 1).sort((a, b) => b.length - a.length);
  return locale;
}

// Exact strings and declared UI templates win. Templates preserve interpolated
// product data. Otherwise substitute known phrases only when no Chinese remains,
// so unknown catalog prose stays untouched rather than becoming half-translated.
export function t(text, dictionary = DICTIONARIES[locale]) {
  if (!dictionary || typeof text !== 'string' || !HAN.test(text)) return text;
  const trimmed = text.trim();
  if (Object.hasOwn(dictionary, trimmed)) return text.replace(trimmed, () => dictionary[trimmed]);
  if (dictionary === en) for (const template of templates) {
    const match = template.pattern.exec(trimmed); if (!match) continue;
    const values = new Map(template.slots.map((slot, index) => [slot, match[index + 1]]));
    const translated = template.target.replace(/\{(\d+)\}/g, (_, index) => {
      const slot = Number(index), value = values.get(slot) ?? '';
      // Product names, firmware versions and diagnostic text are opaque data.
      // Only explicitly declared UI fragments may be translated recursively.
      return template.localized.includes(slot) ? t(value, dictionary) : value;
    });
    return text.replace(trimmed, () => translated);
  }
  let result = text;
  const keys = dictionary === DICTIONARIES[locale] ? phraseKeys : Object.keys(dictionary).filter(key => key.length > 1).sort((a, b) => b.length - a.length);
  for (const key of keys) if (result.includes(key)) result = result.split(key).join(dictionary[key]);
  return HAN.test(result) ? text : result.replace(/(\S) {2,}(?=\S)/g, "$1 ");
}

function translateElement(element) {
  for (const name of ATTRIBUTES) {
    const value = element.getAttribute?.(name);
    if (value && HAN.test(value) && translatedAttributes.get(element)?.get(name) !== value) {
      const next = t(value);
      if (next !== value) {
        let values = translatedAttributes.get(element); if (!values) translatedAttributes.set(element, values = new Map());
        values.set(name, next); element.setAttribute(name, next);
      }
    }
  }
}

function translateText(node) {
  if (skipped(node) || translatedText.get(node) === node.nodeValue) return;
  const next = t(node.nodeValue);
  if (next !== node.nodeValue) { translatedText.set(node, next); node.nodeValue = next; }
}

function skipped(node) {
  for (let el = node.nodeType === 1 ? node : node.parentElement; el; el = el.parentElement) {
    if (SKIP.has(el.tagName) || el.hasAttribute?.('data-i18n-skip')) return true;
  }
  return false;
}

export function translateTree(root) {
  if (!root || locale === SOURCE_LOCALE) return;
  if (root.nodeType === 3) { translateText(root); return; }
  if (root.nodeType !== 1 && root.nodeType !== 9 && root.nodeType !== 11) return;
  if (root.nodeType === 1) { if (skipped(root)) return; translateElement(root); }
  const walker = root.ownerDocument ? root.ownerDocument.createTreeWalker(root, 5) : root.createTreeWalker(root, 5);
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    if (node.nodeType === 1) { if (skipped(node)) continue; translateElement(node); }
    else if (HAN.test(node.nodeValue)) translateText(node);
  }
}

function readSaved() { try { return localStorage.getItem(STORAGE_KEY); } catch { return null; } }

export function chooseLocale(next) {
  next = normalizeLocale(next) || SOURCE_LOCALE;
  try { localStorage.setItem(STORAGE_KEY, next); } catch {}
  const url = new URL(location.href);
  url.searchParams.set('lang', next);
  location.replace(url.href);
}

function mountSwitcher(doc) {
  const host = doc.querySelector('[data-language-mount]') || doc.querySelector('.topbar-actions') || doc.querySelector('.view-controls');
  if (!host || doc.getElementById('language-select')) return;
  const select = doc.createElement('select');
  select.id = 'language-select'; select.className = 'language-select'; select.setAttribute('data-i18n-skip', '');
  select.setAttribute('aria-label', locale === 'en' ? 'Language' : '语言');
  for (const [value, label] of Object.entries(LOCALES)) select.append(new Option(label, value, false, value === locale));
  select.addEventListener('change', () => chooseLocale(select.value));
  host.prepend(select);
}

export function startI18n(doc = document) {
  setLocale(detectLocale({saved: readSaved(), search: location.search, languages: navigator.languages?.length ? navigator.languages : [navigator.language].filter(Boolean)}));
  doc.documentElement.lang = locale;
  if (locale !== SOURCE_LOCALE) {
    doc.title = t(doc.title);
    const description=doc.querySelector('meta[name="description"]'); if(description)description.content=t(description.content);
    for(const link of doc.querySelectorAll('head link[title]'))translateElement(link);
    translateTree(doc.body);
    new MutationObserver(records => {
      for (const record of records) {
        if (record.type === 'characterData') translateTree(record.target);
        else if (record.type === 'attributes') { if (!skipped(record.target)) translateElement(record.target); }
        else for (const node of record.addedNodes) translateTree(node);
      }
    }).observe(doc.body, {subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: ATTRIBUTES});
  }
  mountSwitcher(doc);
}

if (typeof document !== 'undefined' && typeof location !== 'undefined') startI18n();
