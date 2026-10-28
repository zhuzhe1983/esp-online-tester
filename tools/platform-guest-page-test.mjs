import test from 'node:test';
import assert from 'node:assert/strict';
import {guestPageUrl,checkGuestPagePolicy,rewriteGuestScript,wrapGuestDocumentTags} from '../web/platform/guest-page.js';

test('guest page requests remain on the selected firmware origin',()=>{
  const base='http://192.168.4.1:8080/config/';
  assert.equal(guestPageUrl('../scan?q=1',base).href,'http://192.168.4.1:8080/scan?q=1');
  for(const url of ['https://192.168.4.1:8080/','http://192.168.4.1/','http://localhost/','//example.com/','http://user:pass@192.168.4.1:8080/','javascript:alert(1)'])assert.throws(()=>guestPageUrl(url,base),{code:'GUEST_URL'});
});
test('embedding retains or rejects the firmware security policy instead of removing it',()=>{
  assert.equal(checkGuestPagePolicy({},'<h1>Firmware</h1>'),'');
  for(const headers of [{'X-Frame-Options':'DENY'},{'x-frame-options':'SAMEORIGIN'},{'Content-Security-Policy':"script-src 'none'"},{'Content-Security-Policy':"connect-src 'none'"},{'Content-Security-Policy':'img-src data:'},{'Content-Security-Policy':"frame-ancestors 'self'"}])assert.throws(()=>checkGuestPagePolicy(headers),{code:'GUEST_FRAME_POLICY'});
  assert.throws(()=>checkGuestPagePolicy({},'<meta http-equiv="Content-Security-Policy" content="default-src none">'),{code:'GUEST_FRAME_POLICY'});
});
test('virtual navigation rewrite preserves strings, comments, regex and unrelated properties',()=>{
  const source=`location.href='/done.html'; window.location.assign('/next'); document.location='/last'; self.location.reload();
const text="location.href='/secret'"; const regex=/location.href/g; // location.href='/comment'
/* location */ other.location.href='unchanged'; const template=\`location\`;`;
  const result=rewriteGuestScript(source);
  assert.match(result,/__espGuestLocation\.href='\/done.html'/);
  assert.match(result,/window\.__espGuestLocation\.assign/);
  assert.match(result,/document\.__espGuestLocation='\/last'/);
  assert.match(result,/self\.__espGuestLocation\.reload/);
  assert.ok(result.includes('"location.href=\'/secret\'"'));
  assert.ok(result.includes('/location.href/g'));assert.ok(result.includes("// location.href='/comment'"));
  assert.ok(result.includes("other.location.href='unchanged'"));assert.ok(result.includes('`location`'));
});

test('inert parsing preserves document wrapper attributes and raw script text',()=>{
  const html=`<!doctype html><html lang="zh"><head><title>Original</title><script>const text='<body onclick="fail()">';</script></head><body class="settings" onload="start()"><!-- <body> --><p>Ready</p></body></html>`;
  const wrapped=wrapGuestDocumentTags(html);
  assert.ok(wrapped.includes('<esp-guest-html lang="zh">'));
  assert.ok(wrapped.includes('<esp-guest-body class="settings" onload="start()">'));
  assert.ok(wrapped.includes("const text='<body onclick=\"fail()\">';"));
  assert.ok(wrapped.includes('<!-- <body> -->'));
  assert.ok(wrapped.endsWith('</esp-guest-body></esp-guest-html>'));
});
