// Artifact completeness and verification are independent. A board can try an
// untested image; no UI verdict may substitute for runtime chip/layout checks.
export function firmwareRunPolicy(product, firmware) {
  const boardReady = Boolean(product?.runtime?.board);
  const hasSource = Boolean(firmware?.source?.url || firmware?.source?.manifestUrl);
  const verified = firmware?.simulationVerified === true;
  return {
    canRun: boardReady && hasSource,
    trial: hasSource && !verified,
    label: !hasSource ? '资料入口' : verified ? '已验证操作' : '可尝试运行',
    reason: !boardReady ? 'board-unavailable' : !hasSource ? 'artifact-missing' : null,
  };
}

// Hardware flashing uses the selected source, never the emulator's board or state.
// The downloaded bytes and the connected chip still need the flasher's validation.
export function firmwareFlashPolicy(product, source) {
  let reason = null, message = '';
  const chip = String(product?.chip || '').toLowerCase().replace(/[^a-z0-9]/g, '');
  if (!['esp32s3', 'esp32c3', 'esp32c6'].includes(chip)) {
    reason = 'FLASH_UNSUPPORTED_CHIP'; message = '当前在线刷机支持 ESP32-S3、C3 和 C6。';
  } else if (!Number.isSafeInteger(product?.runtime?.flashMb) || product.runtime.flashMb <= 0 || product.runtime.flashMb > 32) {
    reason = 'FLASH_SIZE'; message = '此型号的 Flash 容量尚未核对，暂不能在线刷机。';
  } else if (!source || !(source.url || source.manifestUrl || source.file || ['build', 'segments'].includes(source.format) && source.files?.length)) {
    reason = 'FIRMWARE_REQUIRED'; message = '请先选择固件文件；只有资料链接的条目不能刷机。';
  } else if (source.archive) {
    reason = 'FLASH_LAYOUT'; message = 'ZIP 固件请先解压，再选择完整 BIN 或烧录清单。';
  } else if (source.bootMode === 'app-direct') {
    reason = 'FLASH_LAYOUT'; message = '应用直启文件不能直接刷机，请补齐 bootloader、分区表和应用文件。';
  }
  return {canFlash: !reason, reason, message};
}
