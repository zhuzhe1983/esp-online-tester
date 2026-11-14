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
    const html = fs.readFileSync(new URL(`../web/platform/${page}`, import.meta.url), 'utf8').replace(/(<(pre|textarea)\b[^>]*>)[\s\S]*?<\/\2>/g, '$1</$2>');
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

test('catalog control and connector labels have English UI translations', () => {
  const products=JSON.parse(fs.readFileSync(new URL('../web/platform/catalog/products.json',import.meta.url),'utf8')).items;
  setLocale('en');
  try {
    const labels=new Set(products.flatMap(product=>[...(product.controls||[]),...(product.interfaces||[])].map(item=>item.name).filter(Boolean)));
    assert.deepEqual([...labels].filter(label=>/[\u4e00-\u9fff]/.test(t(label))),[]);
  } finally {setLocale('zh-CN');}
});

test('device tools translate composed states and destructive-action scope accurately', () => {
  setLocale('en');
  try {
    assert.equal(t('TF 卡已准备 · 64 MiB · FAT32 · 尚未接入固件'), 'TF card prepared · 64 MiB · FAT32 · Not connected to firmware yet');
    assert.equal(t('将用 128 MiB exFAT 空卡替换当前设备与固件的卡，原卡文件会被清空。'), 'Replace the card for the current device and firmware with an empty 128 MiB exFAT card. All files on the old card will be erased.');
    assert.equal(t('将用 64 MiB FAT32 空卡替换此设备准备的卡，原卡文件会被清空。'), 'Replace the card prepared for this device with an empty 64 MiB FAT32 card. All files on the old card will be erased.');
    assert.equal(t('最多 32 张，仅保存在本机；分享链接不会包含图片。'), 'Up to 32 stickers, stored only on this device. Shared links do not include the images.');
    assert.equal(t('每款设备最多保存 32 张贴纸。'), 'Each device can store up to 32 stickers.');
    assert.equal(t('已添加 2 张；点选模型外壳放置当前贴纸。'), 'Stickers added: 2. Select the model shell to place the current sticker.');
    assert.equal(t('关闭设备存储'), 'Close device storage');
    assert.equal(t('收起外壳编辑'), 'Close shell editor');
    assert.equal(t('关闭麦克风'), 'Disable microphone');
    assert.equal(t('运行固件后开启扬声器'), 'Start the firmware to enable the speaker');
    assert.equal(t('此设备尚不支持麦克风'), 'Microphone input is not supported on this device yet');
    assert.equal(t('启动 · Wi-Fi：暂停 · 仅连接网络'), '启动 · Wi-Fi: 暂停 · Network only');
    assert.equal(t('虚拟 Flash 配置需要明确选择 UIFlow2 配置。'), 'Select a UIFlow2 configuration to write virtual Flash settings.');
  } finally {setLocale('zh-CN');}
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
