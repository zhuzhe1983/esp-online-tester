import test from 'node:test';
import assert from 'node:assert/strict';
import {guestPageUrl,checkGuestPagePolicy,rewriteGuestScript,wrapGuestDocumentTags,createGuestStorage} from '../web/platform/guest-page.js';

if(!process.argv.includes('--browser-fixture')) {
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


test('legacy no-referrer CSP is retained while mixed or additional restrictions fail closed',()=>{
  const policies=['referrer no-referrer',' REFERRER NO-REFERRER; ','\nreferrer\tno-referrer\r'];
  for(const policy of policies){
    assert.equal(checkGuestPagePolicy({'Content-Security-Policy':policy}),policy);
    for(const tag of [`<meta http-equiv="Content-Security-Policy" content="${policy}">`,`<META CONTENT='${policy}' HTTP-EQUIV='CONTENT-SECURITY-POLICY'>`])assert.equal(checkGuestPagePolicy({},tag),'');
  }
  const strict=["referrer no-referrer; script-src 'none'","default-src 'self'; referrer no-referrer","referrer no-referrer; frame-ancestors 'none'",'referrer origin','referrer no-referrer, referrer origin','referrer no-referrerX'];
  for(const policy of strict){
    assert.throws(()=>checkGuestPagePolicy({'Content-Security-Policy':policy}),{code:'GUEST_FRAME_POLICY'});
    assert.throws(()=>checkGuestPagePolicy({},`<meta CONTENT="${policy}" HTTP-EQUIV='Content-Security-Policy'>`),{code:'GUEST_FRAME_POLICY'});
  }
  const legacy='<meta http-equiv="Content-Security-Policy" content="referrer no-referrer">';
  assert.throws(()=>checkGuestPagePolicy({},legacy+`<meta content="connect-src 'none'" http-equiv="content-security-policy">`),{code:'GUEST_FRAME_POLICY'});
  assert.throws(()=>checkGuestPagePolicy({'Content-Security-Policy':"script-src 'none'"},legacy),{code:'GUEST_FRAME_POLICY'});
  assert.throws(()=>checkGuestPagePolicy({'X-Frame-Options':'DENY'},legacy),{code:'GUEST_FRAME_POLICY'});
  assert.equal(checkGuestPagePolicy({},`<meta content="referrer no-referrer" data-description="a > b" http-equiv="Content-Security-Policy">`),'');
  assert.equal(checkGuestPagePolicy({},`<!-- <meta http-equiv="Content-Security-Policy" content="script-src none"> --><script>const text='<meta http-equiv="Content-Security-Policy" content="script-src none">';</script>`),'');
});


test('guest storage implements Web Storage operations without host storage and supports snapshots',()=>{
  const changes=[],first=createGuestStorage({onChange:entries=>changes.push(entries)}),{storage}=first;
  assert.equal(storage.length,0);assert.equal(storage.getItem('language'),null);assert.equal(storage.key(0),null);
  storage.setItem('language','zh-CN');storage.setItem('count',2);storage.setItem('__proto__','guest-only');
  assert.equal(storage.getItem('language'),'zh-CN');assert.equal(storage.getItem('count'),'2');assert.equal(storage.length,3);
  assert.equal(storage.key(0),'language');assert.equal(storage.key(3),null);assert.equal(storage.key(-1),null);
  storage.setItem('language','en-US');assert.equal(storage.length,3);assert.equal(storage.key(0),'language');
  const second=createGuestStorage({entries:first.snapshot()});assert.equal(second.storage.getItem('language'),'en-US');
  second.storage.removeItem('language');assert.equal(second.storage.getItem('language'),null);assert.equal(storage.getItem('language'),'en-US');
  storage.removeItem('count');assert.equal(storage.length,2);assert.equal(changes.length,5);
  storage.clear();assert.equal(storage.length,0);assert.deepEqual(first.snapshot(),[]);assert.equal(changes.length,6);
  assert.equal(second.storage.getItem('__proto__'),'guest-only');
  assert.throws(()=>storage.getItem(),TypeError);assert.throws(()=>storage.setItem('missing-value'),TypeError);
});
test('guest storage is bounded and quota failures leave existing values intact',()=>{
  const {storage}=createGuestStorage();storage.setItem('key','retained');
  assert.throws(()=>storage.setItem('key','x'.repeat(32768)),{name:'QuotaExceededError'});
  assert.equal(storage.getItem('key'),'retained');assert.equal(storage.length,1);
  for(let i=1;i<128;i++)storage.setItem(String(i),'');
  assert.throws(()=>storage.setItem('one-too-many',''),{name:'QuotaExceededError'});assert.equal(storage.length,128);
  storage.removeItem('1');storage.setItem('one-too-many','');assert.equal(storage.length,128);
  assert.throws(()=>createGuestStorage({entries:[['bad',{}]]}),TypeError);
  assert.throws(()=>createGuestStorage({entries:[['oversize','x'.repeat(32768)]]}),{name:'QuotaExceededError'});
});

}

// Reproducible browser regression with an explicitly in-memory HTTP fixture:
// node tools/platform-guest-page-test.mjs --browser-fixture > web/guest-events-fixture.html
// Serve web/, open the generated file headlessly with the Playwright wrapper, and inspect
// window.regression. It must report passed:true. Close that session and remove the fixture.
if(process.argv.includes('--browser-fixture'))process.stdout.write(`<!doctype html><html><head><meta charset="utf-8"><title>Guest event regression fixture</title></head><body><pre id="result">running</pre><iframe title="guest"></iframe><script type="module">
import {mountGuestPage} from './platform/guest-page.js';
const requests=[],errors=[],checks=[];
const pages={
  '/delegated': '<form id="form" action="/unexpected-form" method="post"><input name="ssid" value="fixture-router"><button>Connect</button></form><script>document.addEventListener("submit",async event=>{event.preventDefault();const response=await fetch("/submit",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({ssid:"fixture-router",password:"fixture-password"})});if(response.ok)location.href="/done";});setTimeout(()=>document.querySelector("form").requestSubmit(),0);<\\/script>',
  '/native': '<form action="/native-submit" method="post"><input name="ssid" value="fixture router"><button>Submit</button></form><script>setTimeout(()=>document.querySelector("form").requestSubmit(),0);<\\/script>',
  '/delegated-link': '<a id="link" href="/unexpected-link">Link</a><script>document.addEventListener("click",async event=>{event.preventDefault();await fetch("/link-submit");location.href="/link-done";});setTimeout(()=>document.getElementById("link").click(),0);<\\/script>',
  '/native-link': '<a id="link" href="/native-linked">Link</a><script>setTimeout(()=>document.getElementById("link").click(),0);<\\/script>',
};
let lastPath='';
const adapter=mountGuestPage({iframe:document.querySelector('iframe'),guestAddress:'192.168.4.1',requestHttp:async request=>{requests.push({path:request.path,method:request.method,body:request.body,bodyBase64:request.bodyBase64});return {status:200,headers:{'content-type':'text/html'},body:pages[request.path]||'<h1>Fixture response</h1>'};},onError:error=>errors.push(error.message),onNavigate:result=>{lastPath=result.path;}});
const ensure=(value,message)=>{if(!value)throw new Error(message);checks.push(message);};
const waitPath=async path=>{const until=Date.now()+5000;while(lastPath!==path&&Date.now()<until)await new Promise(resolve=>setTimeout(resolve,10));ensure(lastPath===path,'Reached '+path);};
try{
  await adapter.open({path:'/delegated'});await waitPath('/done');
  const submitted=requests.find(request=>request.path==='/submit');
  ensure(submitted?.method==='POST','Document submit handler performs POST');
  ensure(atob(submitted.bodyBase64)===JSON.stringify({ssid:'fixture-router',password:'fixture-password'}),'Original JSON submission preserved');
  ensure(!requests.some(request=>request.path==='/unexpected-form'),'Document submit preventDefault suppresses native fallback');
  await adapter.open({path:'/native'});await waitPath('/native-submit');
  const native=requests.find(request=>request.path==='/native-submit');ensure(native.method==='POST'&&native.body==='ssid=fixture+router','Native form fallback remains available');
  await adapter.open({path:'/delegated-link'});await waitPath('/link-done');
  ensure(requests.some(request=>request.path==='/link-submit')&&!requests.some(request=>request.path==='/unexpected-link'),'Document click handler precedes link fallback');
  await adapter.open({path:'/native-link'});await waitPath('/native-linked');
  ensure(errors.length===0,'No guest script or transport errors');
  window.regression={passed:true,checks,requests,errors};
}catch(error){window.regression={passed:false,checks,requests,errors:[...errors,error.message]};}
finally{adapter.dispose();document.getElementById('result').textContent=JSON.stringify(window.regression,null,2);}
</script></body></html>`);
