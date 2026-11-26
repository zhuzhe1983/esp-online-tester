import test from 'node:test';
import assert from 'node:assert/strict';
import {networkInterfaceView} from '../web/platform/network-interface-view.js';
import {createNetworkSession} from '../web/platform/network-session.js';
const state=()=>({primaryInterface:null,softApSupported:true,router:{ssid:'test-wifi-only-name'},interfaces:{},softAp:null});

test('RMII and W5500 present Ethernet without inventing Wi-Fi, SoftAP, or an address',()=>{
  for(const primaryInterface of ['native-rmii','w5500-macraw']){
    const s={...state(),primaryInterface,softAp:{ssid:'stale-previous-wifi'},interfaces:{1:{leaseReady:true,guestAddress:'192.168.4.1'}}};
    const view=networkInterfaceView(s);assert.equal(view.wired,true);assert.equal(view.softApAvailable,false);assert.equal(view.serviceOption,'以太网应用服务');
    assert.match(view.addresses,/未取得地址/);assert(!/STA|SoftAP|test-wifi|192\.168|stale/.test(view.addresses));assert(!/STA|SoftAP/.test(view.description));
    s.interfaces[0]={leaseReady:false,guestAddress:'10.0.2.15'};assert(!networkInterfaceView(s).addresses.includes('10.0.2.15'));
    s.interfaces[0].leaseReady=true;assert.match(networkInterfaceView(s).addresses,/10\.0\.2\.15/);
  }
});
test('switching back to Wi-Fi restores STA and its supported SoftAP labels',()=>{
  const s=state();s.interfaces={1:{leaseReady:true,guestAddress:'192.168.4.1'}};s.softAp={ssid:'actual-ap'};
  const view=networkInterfaceView(s);assert.equal(view.wired,false);assert.equal(view.softApAvailable,true);assert.equal(view.serviceOption,'STA 应用服务');assert.match(view.addresses,/test-wifi-only-name/);assert.match(view.addresses,/actual-ap.*192\.168\.4\.1/);
  s.softApSupported=false;assert.equal(networkInterfaceView(s).softApAvailable,false);assert.match(networkInterfaceView(s).description,/STA 网络中继/);
});
test('public network snapshots track the selected physical interface before attach and after switch',()=>{
  let physical='native-rmii';const n=createNetworkSession({emit(){},command:async()=>{},sendFrame(){},primaryInterface:()=>physical,supportsSoftAp:()=>!physical});
  assert.equal(n.snapshot().primaryInterface,'native-rmii');assert.equal(n.snapshot().softApSupported,false);assert.deepEqual(n.snapshot().interfaces,{});
  physical='w5500-macraw';assert.equal(n.snapshot().primaryInterface,'w5500-macraw');assert.equal(n.snapshot().softApSupported,false);
  physical=null;assert.equal(n.snapshot().primaryInterface,null);assert.equal(n.snapshot().softApSupported,true);n.detach();
});
