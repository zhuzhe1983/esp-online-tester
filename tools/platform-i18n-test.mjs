#!/usr/bin/env node
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { detectLocale, normalizeLocale, setLocale, t } from '../web/platform/i18n.js';
import en from '../web/platform/locales/en.js';

test('locale priority: saved > ?lang= > navigator > fallback', () => {
  assert.equal(detectLocale({saved: 'en', search: '?lang=zh', languages: ['zh-CN']}), 'en');
  assert.equal(detectLocale({saved: null, search: '?lang=en', languages: ['zh-CN']}), 'en');
  assert.equal(detectLocale({saved: 'xx', search: '?lang=zh-TW', languages: ['en-US']}), 'zh-CN');
  assert.equal(detectLocale({languages: ['de-DE', 'en-GB']}), 'en');
  assert.equal(detectLocale({languages: ['zh-Hans-CN']}), 'zh-CN');
  assert.equal(detectLocale({languages: ['fr']}), 'en');
  assert.equal(detectLocale({}), 'zh-CN');
  assert.equal(normalizeLocale(''), null);
});

test('translation: exact, interpolated, and untouched when partial', () => {
  setLocale('en');
  assert.equal(t('分享场景'), 'Share scene');
  assert.equal(t('  启动 '), '  Start ');
  assert.equal(t('已选择 3 个文件'), 'Selected 3 files');
  assert.equal(t('API v1'), 'API v1');
  assert.equal(t('这是一段未收录的目录描述'), '这是一段未收录的目录描述');
  setLocale('zh-CN');
  assert.equal(t('分享场景'), '分享场景');
});

test('every static index/embed string has an English entry', () => {
  const han = /[\u4e00-\u9fff]/;
  for (const page of ['index.html', 'embed.html']) {
    const html = fs.readFileSync(new URL(`../web/platform/${page}`, import.meta.url), 'utf8').replace(/<pre[\s\S]*?<\/pre>|<textarea[\s\S]*?<\/textarea>/g, '');
    const strings = [...html.matchAll(/>([^<>]+)</g)].map(m => m[1].trim()).concat([...html.matchAll(/(?:title|aria-label|placeholder)="([^"]+)"/g)].map(m => m[1].trim()));
    setLocale('en');
    const missing = strings.filter(s => han.test(t(s)));
    assert.deepEqual(missing, [], page);
  }
  for (const value of Object.values(en)) assert.ok(!han.test(value), value);
});
