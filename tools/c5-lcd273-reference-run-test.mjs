// Pure reference-evidence logic: synthetic UART/FAT32 fixtures, no runtime/guest.
import test from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {SD_PAYLOAD,SOURCE,parseOptions,sdUartFacts,inspectTestFile,referenceFactsObserved} from './c5-lcd273-reference-run.mjs';
const sha=b=>createHash('sha256').update(b).digest('hex');
const success=()=>['I (1) main_task: Calling app_main()','I tf_card_test: TF card mounted at /sdcard','I tf_card_test: Starting TF card read/write test',
  'I tf_card_test: Test file: /sdcard/test.txt','I tf_card_test: Write success: 71 bytes','I tf_card_test: Read success: 71 bytes',
  'I tf_card_test: Read content:\n'+SD_PAYLOAD,'I tf_card_test: ========== TF CARD TEST PASS =========='].join('\r\n');
function fatFixture({mbr=false,activeSecondFat=false}={}){
  const volume=Buffer.alloc(1024*1024),fats=activeSecondFat?2:1,dataSector=32+fats*16,root=dataSector*512,file=root+512,fat=(32+(activeSecondFat?16:0))*512;
  volume[13]=1;volume.writeUInt16LE(512,11);volume.writeUInt16LE(32,14);volume[16]=fats;volume.writeUInt32LE(2048,32);volume.writeUInt32LE(16,36);volume.writeUInt16LE(activeSecondFat?0x81:0,40);volume.writeUInt32LE(2,44);volume.writeUInt16LE(0xaa55,510);
  volume.writeUInt32LE(0x0fff_ffff,fat+8);volume.writeUInt32LE(0x0fff_ffff,fat+12);volume.write('TEST    TXT',root,'ascii');volume[root+11]=0x20;volume.writeUInt16LE(3,root+26);volume.writeUInt32LE(Buffer.byteLength(SD_PAYLOAD),root+28);volume.write(SD_PAYLOAD,file,'ascii');
  if(!mbr)return {bytes:volume,root,file,fat};const disk=Buffer.alloc(volume.length+512);volume.copy(disk,512);disk[450]=0x0c;disk.writeUInt32LE(1,454);disk.writeUInt32LE(2048,458);disk.writeUInt16LE(0xaa55,510);return {bytes:disk,root:root+512,file:file+512,fat:fat+512};
}

test('fixed source identity and 71-byte original payload are independent of firmware filename',()=>{
  assert.equal(Buffer.byteLength(SD_PAYLOAD),71);assert.equal(sha(SD_PAYLOAD),'b0944679cf35c8ac6566d654a769567cc9292e0812cdf471e4cae1f2cd45a91c');
  assert.equal(SOURCE.revision,'418de818f4f40c5a04c4292dbe293bfaa1351068');assert.equal(SOURCE.sdkVersion,'5.5.3');assert.notEqual(SOURCE.sd.mainSha256,SOURCE.display.mainSha256);
});
test('source SD UART requires ordered write/read/memcmp branch evidence, not a lone PASS',()=>{
  const good=sdUartFacts('\x1b[0;32m'+success()+'\x1b[0m');assert.equal(good.guestReportedWriteReadComparePass,true);assert.equal(good.appMain,true);assert.equal(good.transportCompletionProven,false);
  assert.equal(sdUartFacts('TF CARD TEST PASS').guestReportedWriteReadComparePass,false);
  for(const raw of [success().replace('Write success: 71','Write success: 70'),success().replace(SD_PAYLOAD,'other data'),success()+'\nTF CARD TEST FAIL',success().replace('Read success: 71 bytes','')])assert.equal(sdUartFacts(raw).guestReportedWriteReadComparePass,false);
  assert.equal(sdUartFacts(success()+'\nGuru Meditation Error').fatalRuntimeOutputObserved,true);
});
test('FAT32 root test.txt inspection checks exact content without mutating the disk',()=>{
  for(const options of [{},{mbr:true},{activeSecondFat:true}]){const f=fatFixture(options),before=sha(f.bytes),r=inspectTestFile(f.bytes);assert.equal(r.status,'observed');assert.equal(r.bytes,71);assert.equal(r.matchesExpectedPayload,true);assert.equal(r.guestMountInferred,false);assert.equal(sha(f.bytes),before);}
  const other=fatFixture();other.bytes[other.file]^=1;assert.equal(inspectTestFile(other.bytes).matchesExpectedPayload,false);
  const absent=fatFixture();absent.bytes[absent.root]=0;assert.equal(inspectTestFile(absent.bytes).status,'not-found');
});
test('FAT32 short-name matching folds only ASCII lowercase and rejects every high-bit alias',()=>{
  const lower=fatFixture();lower.bytes.write('test    txt',lower.root,'ascii');assert.equal(inspectTestFile(lower.bytes).matchesExpectedPayload,true);
  for(let i=0;i<11;i++){const alias=fatFixture();alias.bytes[alias.root+i]|=0x80;const before=sha(alias.bytes),r=inspectTestFile(alias.bytes);
    assert.equal(r.status,'not-found',`high-bit short-name byte ${i} must not match TEST.TXT`);assert.notEqual(r.matchesExpectedPayload,true);assert.equal(sha(alias.bytes),before);}
});
test('FAT32 root/file chains preserve exact bytes and reject duplicate names across root clusters',()=>{
  const next=fatFixture(),entry=Buffer.from(next.bytes.subarray(next.root,next.root+32));next.bytes.writeUInt32LE(4,next.fat+8);next.bytes.writeUInt32LE(0x0fff_ffff,next.fat+16);
  for(let at=0;at<512;at+=32)next.bytes[next.root+at]=0xe5;entry.copy(next.bytes,next.root+1024);assert.equal(inspectTestFile(next.bytes).matchesExpectedPayload,true);
  entry.copy(next.bytes,next.root);assert.equal(inspectTestFile(next.bytes).code,'FAT_TARGET_AMBIGUOUS');
  const file=fatFixture();file.bytes.writeUInt32LE(5,file.fat+12);file.bytes.writeUInt32LE(0x0fff_ffff,file.fat+20);file.bytes.writeUInt32LE(600,file.root+28);file.bytes.fill(0xa5,file.root+1536,file.root+1536+88);
  const parsed=inspectTestFile(file.bytes);assert.equal(parsed.status,'observed');assert.equal(parsed.bytes,600);assert.equal(parsed.matchesExpectedPayload,false);
  assert.equal(parsed.sha256,sha(Buffer.concat([file.bytes.subarray(file.file,file.file+512),file.bytes.subarray(file.root+1536,file.root+1536+88)])));
});
test('FAT32 inspector rejects cyclic/truncated/ambiguous or malformed structures',()=>{
  const cycle=fatFixture();cycle.bytes.writeUInt32LE(3,cycle.fat+12);assert.equal(inspectTestFile(cycle.bytes).code,'FAT_CHAIN');
  const short=fatFixture();short.bytes.writeUInt32LE(600,short.root+28);assert.equal(inspectTestFile(short.bytes).code,'FAT_FILE_TRUNCATED');
  const duplicate=fatFixture();duplicate.bytes.copy(duplicate.bytes,duplicate.root+32,duplicate.root,duplicate.root+32);assert.equal(inspectTestFile(duplicate.bytes).code,'FAT_TARGET_AMBIGUOUS');
  const geometry=fatFixture();geometry.bytes[13]=3;assert.equal(inspectTestFile(geometry.bytes).code,'FAT_GEOMETRY');
  assert.equal(inspectTestFile(Buffer.alloc(512)).status,'unavailable');assert.equal(inspectTestFile(Buffer.alloc(2)).status,'unavailable');
});
test('reference-observed gates combine actual-scope facts and never infer transport completion',()=>{
  const f={executionStarted:true,abiVerified:true,uart:{...sdUartFacts(success()),truncated:false},sd:{after:{testFile:{matchesExpectedPayload:true}},counterDelta:{status:'observed',storageBlocksLoaded:'9007199254740993',storageBlocksWritten:'1'}},
    display:{realNonzeroFrame:true,panelVisible:true,panelPixelsWritten:'307200',panelRamWriteCommands:'1'}};
  assert.equal(referenceFactsObserved('sd',f),true);assert.equal(referenceFactsObserved('display',f),true);assert.equal(f.uart.transportCompletionProven,false);
  for(const mutate of [v=>v.abiVerified=false,v=>v.executionStarted=false,v=>v.uart.truncated=true,v=>v.uart.fatalRuntimeOutputObserved=true,v=>v.sd.after.testFile.matchesExpectedPayload=false,v=>v.sd.counterDelta.status='not-comparable',v=>v.sd.counterDelta.storageBlocksWritten='0']){const bad=structuredClone(f);mutate(bad);assert.equal(referenceFactsObserved('sd',bad),false);}
  for(const mutate of [v=>v.display.realNonzeroFrame=false,v=>v.display.panelVisible=false,v=>v.display.panelPixelsWritten=1,v=>v.display.panelRamWriteCommands='0']){const bad=structuredClone(f);mutate(bad);assert.equal(referenceFactsObserved('display',bad),false);}
});
test('CLI requires explicit run/check, workload kind and independently supplied artifact hashes',()=>{
  assert.deepEqual(parseOptions(['--help']),{help:true});assert.throws(()=>parseOptions([]),{code:'ARGUMENT'});
  const args=['--check-inputs','--kind','sd'];for(const key of ['wasm','rom','merged','sd'])args.push('--'+key,'caller-'+key,'--'+key+'-sha256','a'.repeat(64));
  assert.equal(parseOptions(args).kind,'sd');assert.throws(()=>parseOptions([...args,'--run']),{code:'ARGUMENT'});assert.throws(()=>parseOptions([...args,'--build-receipt','receipt.json']),{code:'ARGUMENT'});
});
