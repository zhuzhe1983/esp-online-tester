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
