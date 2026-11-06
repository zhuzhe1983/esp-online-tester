// Read-only preflight; the caller must run this before any Flash erase/write.
// ESP32 addresses/masks: ESP-IDF v4.4.8 and v5.5.2 efuse_reg.h, plus the
// v4.4.8 esp_efuse_table.csv for UART_DOWNLOAD_DIS (bit 27 on revision >= 3).
// esptool v5.3.0 targets/esp32.py independently identifies the counter and
// ABS_DONE bits. Exact links and the deliberately stricter policy are in
// docs/platform/flash-security.md. These are classic ESP32 registers only.
const ESP32_EFUSE_WORD0=0x3ff5a000,ESP32_EFUSE_WORD6=0x3ff5a018;
const FLASH_CRYPT_CNT=0x7f00000,UART_DOWNLOAD_DIS=0x8000000;
const ABS_DONE=0x30,DISABLE_DL_ENCRYPT_DECRYPT_CACHE=0x380;
const normalized=value=>String(value || '').toLowerCase().replace(/[^a-z0-9]/g,'');
const problem=(code,message)=>Object.assign(new Error(message),{code});
const invalid=()=>problem('FLASH_SECURITY_UNKNOWN','无法确认设备安全状态；未写入 Flash，请重新连接后重试。');
const secured=()=>problem('SECURED_DEVICE','设备已启用安全启动、Flash 加密或下载保护；本平台不会写入此设备。');
const uint8=value=>Number.isInteger(value)&&value>=0&&value<=255;
const uint32=value=>Number.isInteger(value)&&value>=0&&value<=0xffffffff;
function registerWord(value) {
  // esptool-js 0.7.0 readReg returns a signed int32; accept that representation
  // as well as unsigned words, but never coerce undefined, strings or NaN to 0.
  if(!Number.isInteger(value)||value< -0x80000000||value>0xffffffff)throw invalid();
  return value>>>0;
}

export async function assertFlashSecurity(loader,chip=loader?.chip?.CHIP_NAME) {
  const target=normalized(chip);
  if(!['esp32','esp32s3','esp32c3','esp32c6','esp32c61'].includes(target))throw problem('FLASH_UNSUPPORTED_CHIP','此芯片尚无可用的烧录安全检查。');
  if(normalized(loader?.chip?.CHIP_NAME)!==target)throw invalid();
  if(loader.secureDownloadMode===true)throw secured();
  if(loader.secureDownloadMode!==undefined&&typeof loader.secureDownloadMode!=='boolean')throw invalid();
  if(target==='esp32') {
    if(typeof loader.readReg!=='function')throw invalid();
    // Sequential reads: esptool's command/response stream is not multiplexed.
    const word0=registerWord(await loader.readReg(ESP32_EFUSE_WORD0));
    const word6=registerWord(await loader.readReg(ESP32_EFUSE_WORD6));
    // Reject every nonzero encryption counter, including an even-popcount
    // development state. Do not attempt encrypted writes or change eFuses.
    // Reject ABS_DONE_1 / UART_DOWNLOAD_DIS even on pre-v3 parts: no revision
    // guess can turn a set security-related bit into permission to overwrite.
    if((word0&(FLASH_CRYPT_CNT|UART_DOWNLOAD_DIS))||(word6&(ABS_DONE|DISABLE_DL_ENCRYPT_DECRYPT_CACHE)))throw secured();
    return;
  }
  if(typeof loader.getSecurityInfo!=='function')throw invalid();
  const info=await loader.getSecurityInfo(),flags=info?.parsedFlags;
  if(!uint8(info?.flashCryptCnt)||typeof flags?.SECURE_BOOT_EN!=='boolean'||typeof flags?.SECURE_DOWNLOAD_ENABLE!=='boolean')throw invalid();
  // The vendored loader also exposes raw flags. If present, they must agree
  // with the two parsed booleans; an absent raw field is allowed for adapters.
  if(info.flags!==undefined&&(!uint32(info.flags)||!!(info.flags&1)!==flags.SECURE_BOOT_EN||!!(info.flags&4)!==flags.SECURE_DOWNLOAD_ENABLE))throw invalid();
  if(flags.SECURE_BOOT_EN||flags.SECURE_DOWNLOAD_ENABLE||info.flashCryptCnt!==0)throw secured();
}
