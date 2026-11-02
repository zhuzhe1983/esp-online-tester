// esptool-js v0.7.0 HardReset omits the assertion of RTS. Keep the vendor pinned
// and supply its documented resetConstructors hook instead.
// Timing: espressif/esptool v5.3.0 esptool/reset.py HardReset.reset (100 ms;
// USB-OTG 200 ms before and after release). DTR false leaves the boot pin high.
// https://github.com/espressif/esptool/blob/v5.3.0/esptool/reset.py#L153-L190
export function createFlashHardReset(transport,usingUsbOtg=false,wait=ms=>new Promise(resolve=>setTimeout(resolve,ms))) {
  return {async reset(){
    await transport.setDTR(false);
    await transport.setRTS(true);
    await wait(usingUsbOtg?200:100);
    await transport.setRTS(false);
    if(usingUsbOtg)await wait(200);
  }};
}

// ESP32-S3 only. Addresses/bits checked against ESP-IDF v5.4.2:
// components/soc/esp32s3/register/soc/{reg_base,gpio_reg,rtc_cntl_reg}.h.
// The WDT key, stage/timeout/pulse values and 500 ms wait match esptool v5.3.0
// targets/esp32s3.py watchdog_reset. Its automatic branch covers USB-OTG; here
// the same sequence also handles confirmed native USB-Serial/JTAG manual boot.
// https://github.com/espressif/esptool/blob/v5.3.0/esptool/targets/esp32s3.py
// https://github.com/espressif/esp-idf/issues/13287
// No eFuse, Flash, other chip, or unconfirmed serial adapter registers are written.
export async function resetFlashedDevice(loader,port,{wait=ms=>new Promise(resolve=>setTimeout(resolve,ms))}={}) {
  const chip=loader.chip;
  if(chip?.CHIP_NAME!=='ESP32-S3')return loader.after('hard_reset');
  const info=port?.getInfo?.() || {};
  const nativeUsb=info.usbVendorId===0x303a && info.usbProductId===0x1001
    || (typeof chip.usesUsbJtagSerial==='function'?await chip.usesUsbJtagSerial(loader):chip.usesUsbJtagSerial)===true;
  if(!nativeUsb)return loader.after('hard_reset');
  const strapReg=0x60004038,optionReg=0x6000812c,forceDownload=1;
  await loader.writeReg(optionReg,0,forceDownload);
  const strap=await loader.readReg(strapReg),option=await loader.readReg(optionReg);
  if((strap&8)!==0 || (option&forceDownload)!==0)return loader.after('hard_reset');
  await loader.transport.setDTR(false);
  await loader.writeReg(0x600080b0,0x50d83aa1);
  await loader.writeReg(0x6000809c,2000);
  // RTC WDT enable + stage0=5 + whole-chip reset + pulse width=2.
  await loader.writeReg(0x60008098,0xd0000102);
  await loader.writeReg(0x600080b0,0);
  await wait(500);
}
