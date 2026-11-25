import test from 'node:test';
import assert from 'node:assert/strict';
import {isStrictC5Execution} from './c5-lcd273-acceptance.mjs';

test('real pixels or the original image identity cannot turn driver-assisted evidence into strict acceptance',()=>{
  const original={appDirect:false,bootMode:'rom',stubs:[],frame:{width:320,height:320},factorySha256:'cf10a4358d03898458c16390f3f16ceb0306befe2260160de0919d755b920942'};
  assert.equal(isStrictC5Execution({...original,driverCompatibility:null}),true);
  for(const profile of ['waveshare-lcd273-idf553-offline-v1','unknown',false,null])assert.equal(isStrictC5Execution({...original,c5WifiDriverCompat:profile}),false);
  for(const state of [{enabled:true,hookCalls:'0'},{enabled:false,hookCalls:'1'},{profile:'waveshare-lcd273-idf553-offline-v1'}])assert.equal(isStrictC5Execution({...original,driverCompatibility:state}),false);
  assert.equal(isStrictC5Execution(null),false);
});
