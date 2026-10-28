import test from 'node:test';
import assert from 'node:assert/strict';
import {sharedStartup, setSharedPlayback, shareFirmwareSource} from '../web/platform/shared-startup.js';
import {installAutomation} from '../web/platform/automation.js';

const source = {url: 'https://example.org/factory.bin'};
const restored = {productId: 'device', firmware: source};
const product = {runtime: {board: 'stopwatch'}};
const plan = (query = '', overrides = {}) => sharedStartup(new URLSearchParams(query), {restored, product, ...overrides});

test('shared scenes run by default; plain visits, invalid scenes and explicit opt-out do not', () => {
  assert.equal(plan(), 'run'); // Includes existing scene links made before autostart was added.
  assert.equal(plan('', {restored: null}), 'manual');
  assert.equal(plan('autostart=1', {restored: null}), 'manual');
  assert.equal(plan('autostart=0'), 'manual');
  assert.equal(plan('autostart=0&autoplay=1', {recording: {events: []}}), 'manual');
  assert.equal(plan('', {restored: {...restored, firmware: null}}), 'missing-firmware');
  assert.equal(plan('', {product: {runtime: {status: 'unavailable'}}}), 'unavailable');
});

test('recordings replay by default but invalid recordings stop automatic execution', () => {
  assert.equal(plan('', {recording: {events: []}}), 'replay');
  assert.equal(plan('autoplay=0', {recording: {events: []}}), 'run');
  assert.equal(plan('', {invalidRecording: true}), 'invalid-recording');
  assert.equal(plan('autoplay=0', {invalidRecording: true}), 'run');
  assert.equal(plan('', {restored: {...restored, firmware: null}, recording: {events: []}}), 'missing-firmware');
});

test('automation share carries playback choices and preserves repository subpaths', async () => {
  const scene = {schemaVersion: '1', productId: 'device', firmware: source, accessories: [], localAccessories: [], keyMapping: {}, view: {mode: 'studio'}};
  const runtime = {observe: async () => ({scene}), subscribe: () => () => {}};
  const api = installAutomation(runtime, {target: {location: {href: 'https://example.org/repo/platform/index.html'}}});
  const shared = new URL(await api.share({mode: 'embed'}));
  assert.equal(shared.pathname, '/repo/platform/embed.html');
  const params = new URLSearchParams(shared.hash.slice(1));
  assert.equal(params.get('autostart'), '1'); assert.equal(params.get('autoplay'), '1');
  const manual = new URLSearchParams(new URL(await api.share({autostart: false})).hash.slice(1));
  assert.equal(manual.get('autostart'), '0'); assert.equal(manual.get('autoplay'), '0');
  const withoutReplay = new URLSearchParams(new URL(await api.share({autoplay: false})).hash.slice(1));
  assert.equal(withoutReplay.get('autostart'), '1'); assert.equal(withoutReplay.get('autoplay'), '0');
  await assert.rejects(api.share({autostart: '0'}), {code: 'INVALID_INPUT'});
  assert.throws(() => setSharedPlayback(new URLSearchParams(), {autoplay: 1}), {code: 'INVALID_INPUT'});
});

test('sharing keeps the loaded firmware while browsing another Spark entry',()=>{
 const scene={firmware:{...source,sha256:'a'.repeat(64),bootMode:'rom'}};
 for(const status of ['running','paused','ready'])for(const selectedSource of [null,{url:'https://example.org/other.bin'}])assert.deepEqual(shareFirmwareSource({scene,status,firmware:{source:'remote'},selectedSource}),scene.firmware);
 assert.equal(shareFirmwareSource({scene,status:'running',firmware:{source:'local'},selectedSource:source}),null);
 assert.deepEqual(shareFirmwareSource({scene,status:'configured',firmware:null,selectedSource:source}),source);
});
test('a source-less API share is explicitly a preview, never an autostart promise',async()=>{
 const scene={schemaVersion:'1',productId:'device',firmware:null,accessories:[],keyMapping:{},view:{mode:'studio'}};
 const api=installAutomation({observe:async()=>({scene}),subscribe:()=>()=>{}},{target:{location:{href:'https://example.org/repo/platform/'}}});
 const params=new URLSearchParams(new URL(await api.share({mode:'embed'})).hash.slice(1));assert.equal(params.get('autostart'),'0');assert.equal(params.get('autoplay'),'0');
});
