import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createDefaultScene} from '../web/platform/catalog.js';
import {createPlatformRuntime, validateMergedImage} from '../web/platform/runtime.js';
import {chipIdForBoard} from '../web/platform/app-image.js';
import {imuModelForBoard} from '../web/platform/imu.js';
import {firstFirmware} from '../web/platform/catalog-selection.js';

const readJson = async path => JSON.parse(await readFile(new URL(path, import.meta.url), 'utf8'));
const catalog = {};
for (const name of ['products', 'accessories', 'firmwares']) {
  catalog[name] = (await readJson(`../web/platform/catalog/${name}.json`)).items;
}
const plus = catalog.products.find(product => product.id === 'm5stack-k016-p');

test('K016-P preserves its PICO-D4 identity and pinned factory trial', async () => {
  assert.equal(plus.module, 'ESP32-PICO-D4');
  assert.deepEqual(plus.runtime, {board: 'stickc-plus', flashMb: 4, psramMb: 0, status: 'partial'});
  assert.equal(plus.display.width, 135);
  assert.equal(plus.display.height, 240);
  assert.deepEqual(
    plus.controls.filter(control => Number.isInteger(control.pin)).map(({id, pin}) => [id, pin]),
    [['button-a', 37], ['button-b', 39]],
  );
  assert.equal(plus.controls.find(control => control.id === 'power').transport, 'board');
  assert.equal(plus.defaultFirmwareId, 'm5stack-k016-p-factory-reference');

  const receipt = await readJson('../docs/platform/evidence/stick-k016-p/factory-inputs.json');
  const factory = firstFirmware(catalog.firmwares, plus);
  assert.equal(factory.id, plus.defaultFirmwareId);
  assert.equal(factory.status, 'trial');
  assert.equal(factory.simulationVerified, false);
  assert.equal(factory.source.url, receipt.url);
  assert.equal(factory.source.sha256, receipt.sha256);
});

test('K016-P routes guest buttons and virtual MPU6886 with browser PCM input disabled', async () => {
  const runtime = createPlatformRuntime({catalog});
  try {
    const scene = createDefaultScene(catalog, plus.id);
    await runtime.configure(scene);
    const capabilities = await runtime.capabilities();
    assert.equal(capabilities.executable, true);
    assert.deepEqual(capabilities.input.buttons, ['button-a', 'button-b', 'power']);
    assert.equal(capabilities.input.touch, false);
    assert.equal(capabilities.imu.available, true);
    assert.equal(capabilities.imu.physicalMountingVerified, false);
    assert.equal(capabilities.audio.input.available, false);
    assert.equal(imuModelForBoard('stickc-plus'), 'mpu6886-register-subset');
    const sample = await runtime.setImu({quaternion: [0, 0, 0, 1]});
    assert.equal(sample.delivery, 'configured');
  } finally {
    await runtime.dispose();
  }
});

test('K016-P accepts classic merged images at bootloader offset 0x1000', () => {
  assert.equal(chipIdForBoard('stickc-plus'), 0);
  assert.equal(chipIdForBoard('m5stack-k016-p'), 0);

  const bytes = new Uint8Array(0x11000).fill(255);
  const view = new DataView(bytes.buffer);
  for (const offset of [0x1000, 0x10000]) {
    bytes.fill(0, offset, offset + 64);
    bytes[offset] = 0xe9;
    bytes[offset + 1] = 1;
    view.setUint32(offset + 28, 4, true);
  }
  bytes.fill(0, 0x8000, 0x8020);
  view.setUint16(0x8000, 0x50aa, true);
  view.setUint32(0x8004, 0x10000, true);
  view.setUint32(0x8008, 0x1000, true);
  assert.equal(validateMergedImage(bytes, 'stickc-plus', 4 << 20), true);

  const wrongChip = bytes.slice();
  wrongChip[0x100c] = 9;
  assert.throws(() => validateMergedImage(wrongChip, 'stickc-plus', 4 << 20), {code: 'CHIP_MISMATCH'});
  const s3Offset = bytes.slice();
  s3Offset[0x1000] = 255;
  s3Offset[0] = 0xe9;
  assert.throws(() => validateMergedImage(s3Offset, 'stickc-plus', 4 << 20), {code: 'INCOMPLETE_IMAGE'});
  assert.throws(() => validateMergedImage(bytes, 'stickc-plus', 0x10000), {code: 'FLASH_OVERFLOW'});
});

test('K016-P 3D A/B regions retain their photo provenance', async () => {
  const model = await readJson('../web/platform/assets/m5stack-k016-p/model.json');
  const plan = await readJson('../web/platform/assets/m5stack-k016-p/model-plan.json');
  assert.deepEqual(model.presentation.buttons, plan.presentation.buttons);
  assert.deepEqual(model.presentation.buttons.map(button => button.id), ['button-a', 'button-b']);
  for (const button of model.presentation.buttons) {
    assert.ok(button.sourceIds.includes('product-photo-bc8190a43b'));
    assert.match(button.basis, /Photo-informed/);
    assert.ok(button.sizeMm.every(value => value > 0 && value < 15));
  }
  assert.ok(model.presentation.buttons[0].positionMm[2] > 6.75);
  assert.ok(model.presentation.buttons[1].positionMm[0] > 12);
});
