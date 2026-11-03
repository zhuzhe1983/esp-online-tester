import assert from 'node:assert/strict';
import test from 'node:test';
import {assertFlashSecurity} from '../web/platform/flash-security.js';

const base=0x3ff5a000;
function classic(words={}) {
  const reads=[];
  return {reads,chip:{CHIP_NAME:'ESP32'},secureDownloadMode:false,
    async readReg(address){reads.push(address);assert.ok([base,base+24].includes(address));return words[address]??0;},
    getSecurityInfo(){throw Error('Classic ESP32 has no GET_SECURITY_INFO command');},
    writeReg(){throw Error('Security checks must not write eFuses or registers');},
    writeFlash(){throw Error('Security checks must not write Flash');}};
}
const securityInfo=()=>({flags:0,flashCryptCnt:0,parsedFlags:{SECURE_BOOT_EN:false,SECURE_DOWNLOAD_ENABLE:false}});
function modern(chip='ESP32-S3',info=securityInfo()) {
  return {chip:{CHIP_NAME:chip},secureDownloadMode:false,async getSecurityInfo(){return info;},readReg(){throw Error('Do not use classic ESP32 registers on other chips');}};
}

test('classic ESP32 checks the exact two eFuse read words and leaves unrelated fields alone',async()=>{
  const loader=classic({[base]:0x00008000,[base+24]:0x40}); // write-protect / JTAG-disable alone
  await assertFlashSecurity(loader,'esp32');assert.deepEqual(loader.reads,[0x3ff5a000,0x3ff5a018]);
  // The vendor transport sign-extends 32-bit register responses.
  await assertFlashSecurity(classic({[base]:-2147483648,[base+24]:-2147483648}));
});

test('classic ESP32 refuses all 127 nonzero encryption counter states, including even popcounts',async()=>{
  for(let count=1;count<=127;count++)await assert.rejects(()=>assertFlashSecurity(classic({[base]:count<<20})),{code:'SECURED_DEVICE'});
});

test('classic ESP32 refuses secure boot V1/V2 and each protected UART-download setting',async()=>{
  for(const bit of [4,5,7,8,9])await assert.rejects(()=>assertFlashSecurity(classic({[base+24]:1<<bit})),{code:'SECURED_DEVICE'});
  await assert.rejects(()=>assertFlashSecurity(classic({[base]:1<<27})),{code:'SECURED_DEVICE'});
});

test('classic ESP32 never treats missing, malformed or failed reads as an unprotected device',async()=>{
  for(const value of [undefined,null,NaN,Infinity,0.5,'0',false,0x100000000,-0x80000001])for(const failingAddress of [base,base+24]) {
    const loader=classic();loader.readReg=async address=>address===failingAddress?value:0;
    await assert.rejects(()=>assertFlashSecurity(loader),{code:'FLASH_SECURITY_UNKNOWN'});
  }
  const failure=Error('serial read failed'),loader=classic();loader.readReg=async()=>{throw failure;};
  await assert.rejects(()=>assertFlashSecurity(loader),failure);
});

test('S3/C3/C6 keep GET_SECURITY_INFO and reject encryption or either secure-mode flag',async()=>{
  for(const chip of ['ESP32-S3','ESP32-C3','ESP32-C6']) {
    await assertFlashSecurity(modern(chip));
    for(const flag of ['SECURE_BOOT_EN','SECURE_DOWNLOAD_ENABLE']) {
      const info=securityInfo();info.parsedFlags[flag]=true;info.flags=flag==='SECURE_BOOT_EN'?1:4;
      await assert.rejects(()=>assertFlashSecurity(modern(chip,info)),{code:'SECURED_DEVICE'});
    }
    for(const count of [1,3,127,255])await assert.rejects(()=>assertFlashSecurity(modern(chip,{...securityInfo(),flashCryptCnt:count})),{code:'SECURED_DEVICE'});
  }
});

test('modern security replies fail closed for missing fields, malformed values and inconsistent flags',async()=>{
  for(const info of [null,{}, {...securityInfo(),flashCryptCnt:undefined},{...securityInfo(),flashCryptCnt:'0'},
    {...securityInfo(),flashCryptCnt:256},{...securityInfo(),flashCryptCnt:-1},{...securityInfo(),flashCryptCnt:NaN},
    {...securityInfo(),parsedFlags:{}},{...securityInfo(),parsedFlags:{SECURE_BOOT_EN:0,SECURE_DOWNLOAD_ENABLE:false}},
    {...securityInfo(),flags:1},{...securityInfo(),flags:4},{...securityInfo(),flags:'0'}, {...securityInfo(),flags:-1}]) {
    await assert.rejects(()=>assertFlashSecurity(modern('ESP32-S3',info)),{code:'FLASH_SECURITY_UNKNOWN'});
  }
  const adapter=securityInfo();delete adapter.flags;await assertFlashSecurity(modern('ESP32-C3',adapter));
  const failure=Error('unsupported command'),loader=modern();loader.getSecurityInfo=async()=>{throw failure;};
  await assert.rejects(()=>assertFlashSecurity(loader),failure);
});

test('unknown/mismatched chip and secure-download mode stop before register or security commands',async()=>{
  await assert.rejects(()=>assertFlashSecurity(classic(),'ESP32-S3'),{code:'FLASH_SECURITY_UNKNOWN'});
  await assert.rejects(()=>assertFlashSecurity(modern('ESP32-S31')),{code:'FLASH_UNSUPPORTED_CHIP'});
  for(const loader of [classic(),modern()]) {
    loader.secureDownloadMode=true;
    loader.readReg=loader.getSecurityInfo=()=>{throw Error('No commands permitted');};
    await assert.rejects(()=>assertFlashSecurity(loader),{code:'SECURED_DEVICE'});
    loader.secureDownloadMode='false';await assert.rejects(()=>assertFlashSecurity(loader),{code:'FLASH_SECURITY_UNKNOWN'});
  }
});
