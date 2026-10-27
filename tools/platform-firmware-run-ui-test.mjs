import test from 'node:test';
import assert from 'node:assert/strict';
import {buildRunIssueUrl,redactIssueText} from '../web/platform/firmware-run-ui.js';

test('issue draft carries product, firmware and current errors without credentials or raw user paths',()=>{
  const url=new URL(buildRunIssueUrl({product:{id:'paper',name:'Paper',manufacturer:'M5Stack',sku:'C153',chip:'esp32s3',runtime:{board:'papermono'}},firmware:{id:'reader',name:'Reader',revision:'1.6.5'},source:{url:'https://user:secret@example.org/app.bin?token=private',sha256:'a'.repeat(64)},state:{version:'1',status:'error',lastError:{code:'FETCH_FAILED',message:'Network failed'},serialByPort:{uart0:'Boot failed\npassword=hidden\nSSID=home-network\nMAC=12:34:56:78:90:ab\n192.168.1.2\n/Users/example/private/log.txt\nAuthorization: Bearer hidden'}}}));
  assert.equal(url.origin,'https://github.com');assert.equal(url.pathname,'/zhuzhe1983/esp-online-tester/issues/new');
  const body=url.searchParams.get('body');
  for(const value of ['C153','1.6.5','FETCH_FAILED','Boot failed','papermono'])assert.ok(body.includes(value));
  for(const value of ['private','secret','hidden','home-network','12:34:56','192.168.1.2','/Users/example'])assert.ok(!body.includes(value),value);
  assert.ok(body.includes('请补充'));
});
test('long serial output is bounded and local firmware content stays out of draft URLs',()=>{
  const url=buildRunIssueUrl({product:{id:'paper',name:'Paper',runtime:{}},source:{local:true},state:{serial:'旧日志'.repeat(10000)+'latest tail'}});
  assert.ok(url.length<=7500);
  assert.ok(new URL(url).searchParams.get('body').includes('latest tail'));
  assert.ok(new URL(url).searchParams.get('body').includes('本地文件（不包含文件内容）'));
  assert.ok(!redactIssueText('https://example.org/file?secret=x#token').includes('secret'));
});
test('long non-ASCII device names and errors also respect the issue URL budget',()=>{
  const url=buildRunIssueUrl({product:{name:'设备'.repeat(2000)},firmware:{name:'固件'.repeat(2000)},state:{lastError:{code:'ERROR',message:'错误详情'.repeat(2000)}}});
  assert.ok(url.length<=7500);
  assert.ok(new URL(url).searchParams.get('body').includes('ERROR'));
});
