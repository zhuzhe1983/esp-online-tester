import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {analyticsPage,installAnalytics,MEASUREMENT_ID} from '../web/platform/analytics.js';
const site='https://zhuzhe1983.github.io/esp-online-tester/';
const fixture=(href=site+'platform/',referrer='')=>{
  const appended=[],win={location:{href},navigator:{}},doc={referrer,createElement:()=>({}),head:{append:script=>appended.push(script)}};
  win.self=win.top=win;return {win,doc,appended};
};
test('only the published site pages collect; local, forks, redirects and reviews do not',()=>{
  for(const href of ['http://localhost:8000/platform/','https://example.com/esp-online-tester/platform/',site+'platform/model-review.html',site,site+'emulator.html?fw=private.bin'])assert.equal(analyticsPage(href),null);
  assert.equal(analyticsPage(site+'platform/index.html').page_location,site+'platform/');
});
test('page URL, title and referrer never include scene, firmware, identity or query values',()=>{
  const secret='PRIVATE_SENTINEL',f=fixture(site+'platform/?firmware='+secret+'&ssid='+secret+'&utm_campaign='+secret+'#scene='+secret,'https://user:password@example.com/private/'+secret+'?q='+secret+'#'+secret);
  assert.equal(installAnalytics(f.win,f.doc),true);
  const events=f.win.dataLayer.map(args=>Array.from(args));assert.equal(events.length,4);
  const config=events.find(([type])=>type==='config')[2],view=events.find(([type])=>type==='event');
  assert.deepEqual(events[0],['set',config]); // Must precede any container initialization event.
  assert.equal(config.page_location,site+'platform/');assert.equal(config.page_referrer,'https://example.com/');
  assert.equal(config.page_title,'ESP Workbench');assert.equal(config.send_page_view,false);
  assert.equal(config.allow_google_signals,false);assert.equal(config.allow_ad_personalization_signals,false);
  assert.equal(config.cookie_path,'/esp-online-tester/');assert.equal(view[1],'page_view');assert.equal(view[2].send_to,MEASUREMENT_ID);
  assert.ok(!JSON.stringify(events).includes(secret));assert.ok(!JSON.stringify(events).includes('password'));
  assert.equal(f.appended.length,1);assert.equal(f.appended[0].async,true);assert.equal(f.appended[0].referrerPolicy,'strict-origin');
  assert.equal(installAnalytics(f.win,f.doc),false);assert.equal(f.win.dataLayer.length,4);
});
test('DNT, GPC, Google opt-out and embedded contexts do not load Google or queue data',()=>{
  for(const block of [f=>f.win.navigator.doNotTrack='1',f=>f.win.navigator.globalPrivacyControl=true,f=>f.win[`ga-disable-${MEASUREMENT_ID}`]=true,f=>f.win.top={}]){
    const f=fixture();block(f);assert.equal(installAnalytics(f.win,f.doc),false);assert.equal(f.appended.length,0);assert.equal(f.win.dataLayer,undefined);
  }
});
test('every public entry point loads the shared local module once, redirect does not',async()=>{
  for(const name of ['platform/index.html','platform/embed.html','platform/agents.html','platform/integration.html','emulator.html','run.html']){
    const html=await readFile(new URL('../web/'+name,import.meta.url),'utf8');assert.equal((html.match(/src="\.\/(?:platform\/)?analytics\.js"/g)||[]).length,1,name);
  }
  assert.ok(!(await readFile(new URL('../web/index.html',import.meta.url),'utf8')).includes('analytics.js'));
});
