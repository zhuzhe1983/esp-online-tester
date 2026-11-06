import {encodeUIFlow2,inspectFlashConfiguration,validateFlashConfiguration} from './flash-configuration.js';

const check=(condition,message)=>{if(!condition)throw Object.assign(new Error(message),{code:'FLASH_CONFIG_INVALID'});};
export const UIFLOW2_PARTITION=Object.freeze({name:'nvs',offset:0x9000,bytes:0x6000});

// Inspect the *current* full Flash snapshot, including any guest/persisted changes.
// This plan is private operation data; never expose it through events or receipts.
export function virtualConfigurationPlan(bytes,{flashBytes,partitionTableOffset=0x8000,appDirect=false}={}) {
  check(!appDirect,'应用直启固件不支持写入 UIFlow2 NVS 配置。');
  check(bytes instanceof Uint8Array && Number.isSafeInteger(flashBytes) && flashBytes>0 && bytes.length===flashBytes,'当前虚拟 Flash 容量与设备不一致。');
  check(Number.isSafeInteger(partitionTableOffset) && partitionTableOffset>=0x8000 && partitionTableOffset%4096===0 && partitionTableOffset+4096<=flashBytes,'当前虚拟 Flash 分区表位置无效。');
  const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength);let count=0,terminated=false;
  for(let at=partitionTableOffset;at<partitionTableOffset+4096;at+=32) {
    const magic=view.getUint16(at,true);
    if(magic===0xffff || magic===0xebeb){terminated=true;break;}
    check(magic===0x50aa,'当前虚拟 Flash 分区表损坏。');count++;
    check(view.getUint32(at+8,true)>0,'当前虚拟 Flash 包含空分区。');
    check((view.getUint32(at+28,true)&1)===0,'不支持加密分区的虚拟配置写入。');
  }
  check(count>0 && terminated,'当前虚拟 Flash 缺少完整分区表。');
  const plan={receipt:{format:'merged',flashBytes,partitionTableOffset},files:[{address:0,data:bytes}]};
  check(inspectFlashConfiguration(plan).uiflow2,'此固件没有已支持的 UIFlow2 NVS 布局（0x9000 / 24 KiB），未修改固件。');
  return plan;
}

export function encodeVirtualConfiguration(plan,profile) {
  check(profile?.type==='uiflow2','虚拟 Flash 配置需要明确选择 UIFlow2 配置。');
  return encodeUIFlow2(validateFlashConfiguration(plan,profile));
}

// Build a new dirty-page set without changing the old session or original image.
export function replaceConfigurationPages(baseline,pages,configuration) {
  check(baseline instanceof Uint8Array && baseline.length>=0xf000 && configuration instanceof Uint8Array && configuration.length===0x6000,'虚拟 Flash 配置长度无效。');
  const next=new Map(pages);
  for(let at=0;at<configuration.length;at+=4096) {
    const offset=UIFLOW2_PARTITION.offset+at,page=configuration.slice(at,at+4096);
    if(page.every((value,index)=>value===baseline[offset+index]))next.delete(offset);else next.set(offset,page.buffer);
  }
  return next;
}
