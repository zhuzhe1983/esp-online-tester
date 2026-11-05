#!/usr/bin/env node
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { chooseLocale, detectLocale, normalizeLocale, setLocale, t } from '../web/platform/i18n.js';
import { buildRunIssueUrl } from '../web/platform/firmware-run-ui.js';
import { firmwareHistoryAge } from '../web/platform/firmware-history.js';
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

test('every static workbench, mini and agent-guide string has an English entry', () => {
  const han = /[\u4e00-\u9fff]/;
  for (const page of ['index.html', 'embed.html', 'agents.html']) {
    const html = fs.readFileSync(new URL(`../web/platform/${page}`, import.meta.url), 'utf8').replace(/<pre[\s\S]*?<\/pre>|<textarea[\s\S]*?<\/textarea>/g, '');
    const strings = [...html.matchAll(/>([^<>]+)</g)].map(m => m[1].trim()).concat([...html.matchAll(/(?:title|aria-label|placeholder|alt|label)="([^"]+)"/g)].map(m => m[1].trim()),[...html.matchAll(/<meta\s+name="description"\s+content="([^"]+)"/g)].map(m=>m[1].trim()));
    setLocale('en');
    const missing = strings.filter(s => han.test(t(s)));
    assert.deepEqual(missing, [], page);
  }
  for (const value of Object.values(en)) assert.ok(!han.test(value), value);
});

test('dynamic UI templates preserve product data and use natural English word order', () => {
  setLocale('en');
  assert.equal(t('开始烧录 3 台'), 'Flash 3 devices');
  assert.equal(t('家用设备 共用 StopWatch 的固件目录；兼容性待验证。'), '家用设备 uses the firmware catalog for StopWatch; compatibility is unverified.');
  assert.equal(t('自制板 写入进度'), 'Write progress for 自制板');
  assert.equal(t('自制板 $& 写入进度'), 'Write progress for 自制板 $&');
  assert.equal(t('原适配型号说明：启动、暂停是固件自己的文字。'), 'Notes for the original target: 启动、暂停是固件自己的文字。');
  assert.equal(t('左前电机目标占空比'), 'Target duty cycle for the Front left motor');
  assert.equal(t('已选择 3 台 · 成功 1 · 失败 0 · 等待 2'), '3 devices selected · 1 succeeded · 0 failed · 2 waiting');
  assert.equal(t('0/2 · 验证码剩余 60 秒'), '0/2 · Verification expires in 60 seconds');
  assert.equal(t('Stamp 系列 · 11 款'), 'Stamp Series · 11 models');
  assert.equal(t('217 款设备'), '217 Devices');
  assert.equal(t('配置名称'), 'Configuration name');
  assert.equal(t('取消编辑'), 'Cancel editing');
  assert.equal(t('  已固定 · 2 个镜像已校验  '), '  Pinned · 2 images verified  ');
  assert.equal(t('这是一段没有翻译的产品资料'), '这是一段没有翻译的产品资料');
  assert.equal(t('测试 2', {'测试': 'Test'}), 'Test 2');
  setLocale('zh-CN');
});

test('language selection survives unavailable storage and retains the shared scene', () => {
  const savedLocation=globalThis.location,savedStorage=Object.getOwnPropertyDescriptor(globalThis,'localStorage');
  let next;
  try {
    globalThis.location={href:'https://example.test/platform/?autostart=0#scene=example',replace:value=>{next=value;}};
    Object.defineProperty(globalThis,'localStorage',{configurable:true,value:{setItem(){throw Error('Storage blocked');}}});
    chooseLocale('en-US');
    const url=new URL(next);
    assert.equal(url.searchParams.get('lang'),'en');assert.equal(url.searchParams.get('autostart'),'0');assert.equal(url.hash,'#scene=example');
  } finally {
    if(savedLocation===undefined)delete globalThis.location;else globalThis.location=savedLocation;
    if(savedStorage===undefined)delete globalThis.localStorage;else Object.defineProperty(globalThis,'localStorage',savedStorage);
  }
});

test('English issue drafts translate headings while retaining guest output and product identity', () => {
  setLocale('en');
  try {
    const url=new URL(buildRunIssueUrl({product:{id:'custom-board',name:'自制开发板',manufacturer:'我的工厂'},firmware:{id:'custom-fw',name:'中文固件'},source:{},state:{serialByPort:{uart0:'启动\n暂停'},lastError:{code:'CUSTOM',message:'固件自己输出的错误'}}}));
    const body=url.searchParams.get('body');
    assert.match(url.searchParams.get('title'),/^\[Firmware run\] 自制开发板 · 中文固件$/);
    assert.match(body,/## Problem description/);assert.match(body,/Current device: 自制开发板 \(custom-board\)/);
    assert.match(body,/Manufacturer \/ model: 我的工厂/);assert.match(body,/CUSTOM: 固件自己输出的错误/);
    assert.match(body,/\[uart0\]\n启动\n暂停/);assert.ok(url.href.length<=7500);
  } finally { setLocale('zh-CN'); }
});

test('firmware history dates follow the selected UI language', () => {
  const time=Date.UTC(2025,0,1),now=Date.UTC(2026,0,1);
  try {
    for(const language of ['en','zh-CN']) {
      setLocale(language);
      assert.equal(firmwareHistoryAge(time,now),new Date(time).toLocaleDateString(language));
    }
  } finally {setLocale('zh-CN');}
});
