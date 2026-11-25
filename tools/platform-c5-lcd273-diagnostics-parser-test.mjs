// Pure parser checks only: no runtime, worker, WASM, guest or browser startup.
import test from 'node:test';
import assert from 'node:assert/strict';
import {parseHardwareDiagnostics} from '../web/platform/hardware-diagnostics.js';
import {c5Lcd273Snapshot} from './fixtures/c5-lcd273-diagnostics.mjs';

test('C5 LCD273 diagnostics retain exact u64 decimal strings and independent copies',()=>{
  const input=c5Lcd273Snapshot();input.state.display.pixelsWritten='18446744073709551615';input.state.sd.card.storageBlocksLoaded='9007199254740993';
  const parsed=parseHardwareDiagnostics(input);assert.deepEqual(parsed,input);
  assert.equal(BigInt(parsed.state.sd.card.storageBlocksLoaded),9007199254740993n);
  input.state.sd.card.storageBlocksLoaded='0';input.state.display.visible=false;input.limits[0]='changed';
  assert.equal(parsed.state.sd.card.storageBlocksLoaded,'9007199254740993');assert.equal(parsed.state.display.visible,true);assert.notEqual(parsed.limits[0],'changed');
  parsed.state.sd.card.storageBlocksWritten='9';assert.equal(input.state.sd.card.storageBlocksWritten,'1');
});

test('C5 LCD273 parser rejects lossy numbers, overflow and noncanonical counters',()=>{
  const paths=[['state','display','pixelsWritten'],['state','display','ramWriteCommands'],['state','display','resetCount'],['state','boardRejectedTransactions'],
    ['state','sd','card','capacityBlocks'],['state','sd','card','nativeCommandCalls'],['state','sd','card','storageBlocksLoaded'],['state','sd','card','storageBlocksWritten']];
  for(const path of paths)for(const invalid of [0,9007199254740992,1n,null,undefined,'','00','01','-1','1.0','1e3',' 1','1 ','18446744073709551616','9'.repeat(21)]){
    const input=c5Lcd273Snapshot();let at=input;for(const key of path.slice(0,-1))at=at[key];at[path.at(-1)]=invalid;
    assert.equal(parseHardwareDiagnostics(input),null,`${path.join('.')} must reject ${String(invalid)}`);
  }
});

test('C5 LCD273 presence is attachment only, with a nullable card snapshot',()=>{
  const absent=c5Lcd273Snapshot();absent.state.sd.present=false;absent.state.sd.card=null;
  assert.deepEqual(parseHardwareDiagnostics(absent),absent);
  const input=c5Lcd273Snapshot();input.state.sd.card.spiMode=false;input.state.sd.card.cardState='Idle';input.state.sd.card.nativeCommandCalls='0';
  assert.deepEqual(parseHardwareDiagnostics(input),input,'an attached idle card must remain idle, not be upgraded to initialized or mounted');
  for(const change of [s=>s.state.sd.present=false,s=>s.state.sd.card=null,s=>delete s.state.sd.card,s=>s.state.sd.card=Object.assign([],s.state.sd.card),s=>s.state.sd.card.cardState='Mounted',s=>s.state.sd.card.spiMode=1,s=>s.state.sd.transport='sdmmc']){
    const invalid=c5Lcd273Snapshot();change(invalid);assert.equal(parseHardwareDiagnostics(invalid),null);
  }
});

test('C5 LCD273 parser discards invented mount/completed-read capability fields',()=>{
  const input=c5Lcd273Snapshot();input.t='hardware-diagnostics';input.acceptancePassed=true;input.state.sd.mounted=true;
  input.state.sd.card.guestCompletedReads='4';input.state.sd.card.guestFilesystemMounted=true;input.state.display.verifiedOnHardware=true;
  const parsed=parseHardwareDiagnostics(input);assert.deepEqual(parsed,c5Lcd273Snapshot());
  assert.equal(parsed.state.sd.mounted,undefined);assert.equal(parsed.state.sd.card.guestCompletedReads,undefined);
});

test('C5 LCD273 diagnostics reject malformed latches, schema and unbounded limits',()=>{
  const changes=[s=>s.schemaVersion=2,s=>s.counterEncoding='number',s=>s.state=null,s=>s.state=Object.assign([],s.state),s=>s.state.display=null,s=>s.state.display=Object.assign([],s.state.display),s=>s.state.sd=null,s=>s.state.sd=Object.assign([],s.state.sd),
    s=>s.state.display.resetHigh=1,s=>s.state.display.sleepOut=null,s=>s.state.display.displayOn=1,s=>s.state.display.visible='true',
    s=>s.state.display.brightness=256,s=>s.state.display.brightness=-1,s=>s.state.display.pixelFormat=NaN,s=>s.state.display.madctl=1.5,
    s=>s.state.sd.present='true',s=>s.limits=Array(9).fill('x'),s=>s.limits=['x'.repeat(513)],s=>s.limits=[null]];
  for(const change of changes){const input=c5Lcd273Snapshot();change(input);assert.equal(parseHardwareDiagnostics(input),null);}
  for(const board of ['waveshare-c5-lcd147','waveshare-c5-touch35','esp32c5','stamp-c5','unknown']){
    const input=c5Lcd273Snapshot();input.board=board;assert.equal(parseHardwareDiagnostics(input),null,'other C5 boards must not inherit these observations');
  }
});

test('existing StopWatch and CoreMatrix parser contracts remain unchanged',()=>{
  const stopwatch={schemaVersion:1,board:'stopwatch',state:{powerLedEnabled:true,downloadLocked:false,pm1ConfiguredHz:100000,pm1LastBusHz:null,
    display:{sleepOut:false,displayOn:false,visible:false,brightness:0,brightnessControlEnabled:null}},findings:[],limits:[]};
  const corematrix={schemaVersion:1,board:'corematrix',state:{controllerPowered:true,matrix:{powered:false,enabled:true,pwmSteps:0,frames:0,bytesWritten:0},
    buzzer:{frequencyHz:0,duty4095:0,supplyEnabled:false},grove5vEnabled:false,sdPresent:true}};
  assert.deepEqual(parseHardwareDiagnostics(stopwatch),stopwatch);assert.deepEqual(parseHardwareDiagnostics(corematrix),corematrix);
});
