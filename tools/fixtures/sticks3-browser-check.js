// Run on a loopback workbench with the locally built StickS3 diagnostic.
async page => {
  const origin = new URL(page.url());
  if (!['127.0.0.1', 'localhost'].includes(origin.hostname)) throw Error('Use a local workbench for this fixture.');
  const check = (ok, message) => { if (!ok) throw Error(message); };
  const serial = () => page.evaluate(async () => (await window.espTester.observe()).serialByPort.uart0 ?? '');
  const waitFor = (line, offset = 0) => page.evaluate(async ([line, offset]) => {
    const deadline = performance.now() + 45_000;
    do {
      const state = await window.espTester.observe();
      if (state.status === 'error') throw Error(state.lastError?.message || 'StickS3 worker failed.');
      if (state.serialByPort.uart0?.slice(offset).includes(line)) return;
      await new Promise(resolve => setTimeout(resolve, 30));
    } while (performance.now() < deadline);
    throw Error('No guest output for ' + line);
  }, [line, offset]);
  const capture = async expected => page.evaluate(async expected => {
    const deadline = performance.now() + 10_000;
    const pixel = (120 * 135 + 67) * 4;
    do {
      let frame;
      try { frame = await window.espTester.capture({format: 'rgba'}); }
      catch (error) { if (error.code !== 'NO_FRAME') throw error; }
      if (frame) {
        if (frame.width !== 135 || frame.height !== 240) throw Error('Expected 135 × 240 guest pixels.');
        const center = [...frame.pixels.slice(pixel, pixel + 4)];
        if (String(center) === String(expected)) {
          const digest = new Uint8Array(await crypto.subtle.digest('SHA-256', frame.pixels));
          return {center, sha256: [...digest].map(v => v.toString(16).padStart(2, '0')).join('')};
        }
      }
      await new Promise(resolve => setTimeout(resolve, 30));
    } while (performance.now() < deadline);
    throw Error('Missing expected screen pixel ' + expected.join(','));
  }, expected);
  const frames = [];
  const errors = [], onError = error => errors.push(error.message);
  page.on('pageerror', onError);
  try {
    await page.setViewportSize({width: 1440, height: 1000});
    await page.waitForFunction(() => !!window.espTester);
    await page.locator('#manufacturer-select').selectOption('M5Stack');
    await page.locator('#product-select').selectOption('m5stack-k150');
    await page.waitForFunction(() => document.querySelector('#stage-title').textContent.includes('StickS3')
      && document.querySelector('#scene-mount').dataset.modelStatus === 'ready');
    const initial = await page.evaluate(async () => {
      const api = window.espTester;
      const state = await api.observe();
      const caps = await api.capabilities();
      await api.configure({...state.scene, firmware: null});
      const shared = await api.share({mode: 'embed', autostart: false, includeRecording: false});
      await api.configureNetwork({enabled: false, autoProvision: false});
      return {product: state.scene.productId, defaultFirmwareId: state.product.defaultFirmwareId,
        imu: caps.imu.model, buttons: caps.input.buttons, touch: caps.input.touch, shared};
    });
    check(initial.product === 'm5stack-k150' && !initial.defaultFirmwareId, 'Expected K150 with no factory default.');
    check(initial.imu === 'bmi270-register-subset' && String(initial.buttons) === 'btnA,btnB' && !initial.touch,
      'Expected physical A/B and pollable BMI270.');
    await page.locator('[data-firmware-tab="local"]').click();
    const bytes = await page.evaluate(async () => {
      const response = await fetch('../wasm/fw/local/sticks3-fixture.bin');
      if (!response.ok) throw Error('Build and install the loopback-only K150 fixture first.');
      return [...new Uint8Array(await response.arrayBuffer())];
    });
    await page.locator('#firmware-file').setInputFiles({
      name: 'sticks3-fixture.bin', mimeType: 'application/octet-stream', buffer: Buffer.from(bytes)
    });
    await page.locator('#load-firmware').click();
    await waitFor('STICKS3 BUTTONS 0 COLOR ffff');
    await waitFor('STICKS3 ACCEL_X 0');
    const boot = await serial();
    for (const line of ['ESP-ROM', 'STICKS3 PSRAM 8388608', 'STICKS3 IDS PM1=50 BMI270=24 ES8311=83/11', 'STICKS3 BMI_INIT 01'])
      check(boot.includes(line), 'Missing guest boot evidence ' + line);
    const observeFirmware = () => page.evaluate(async () => (await window.espTester.observe()).firmware);
    const firmware = await observeFirmware();
    check(firmware.source === 'local' && !firmware.appDirect, 'Expected local merged ROM boot.');
    frames.push(await capture([255, 255, 255, 255]));
    const canvas = await page.locator('#scene-mount canvas[data-engine]').boundingBox();
    check(canvas, 'Expected the K150 interactive 3D model.');
    const hover = async (x, y) => {
      await page.mouse.move(canvas.x + canvas.width / 2 + x * canvas.height, canvas.y + y * canvas.height);
      return page.locator('#scene-mount').getAttribute('data-hover');
    };
    check(await hover(0, 0.5) === 'orbit', 'The front screen must remain a rotation surface.');
    const pointer = async (x, y, line, color) => {
      check(await hover(x, y) === 'button', 'Expected an exposed button hit area.');
      let offset = (await serial()).length;
      await page.mouse.down();
      await waitFor(line, offset);
      const buttonFrame = await capture(color);
      offset = (await serial()).length;
      await page.mouse.up();
      await waitFor('STICKS3 BUTTONS 0 COLOR ffff', offset);
      await capture([255, 255, 255, 255]);
      return buttonFrame.sha256;
    };
    const pointerFrames = {
      btnA: await pointer(-0.025, 0.73, 'STICKS3 BUTTONS 1 COLOR f800', [255, 0, 0, 255]),
      btnB: await pointer(0.195, 0.50, 'STICKS3 BUTTONS 2 COLOR 07e0', [0, 255, 0, 255])
    };
    await page.locator('#view-2d').click();
    await page.locator('.flat-display').waitFor({state: 'visible'});
    const key = async (name, down, line, color) => {
      const offset = (await serial()).length;
      await page.keyboard[down ? 'down' : 'up'](name);
      await waitFor(line, offset);
      frames.push(await capture(color));
    };
    await key('a', true, 'STICKS3 BUTTONS 1 COLOR f800', [255, 0, 0, 255]);
    await key('b', true, 'STICKS3 BUTTONS 3 COLOR 001f', [0, 0, 255, 255]);
    await key('a', false, 'STICKS3 BUTTONS 2 COLOR 07e0', [0, 255, 0, 255]);
    await key('b', false, 'STICKS3 BUTTONS 0 COLOR ffff', [255, 255, 255, 255]);
    await page.evaluate(() => window.espTester.setImu({quaternion: [0, 0, Math.SQRT1_2, Math.SQRT1_2]}));
    await waitFor('STICKS3 ACCEL_X 4096', boot.length);
    await page.evaluate(() => window.espTester.pause());
    await page.goto(initial.shared);
    await page.waitForFunction(() => !!window.espTester && document.querySelector('#stage-title').textContent.includes('StickS3')
      && document.querySelector('#scene-mount').dataset.modelStatus === 'ready');
    const embedded = await page.evaluate(async () => {
      const state = await window.espTester.observe();
      return {product: state.scene.productId, firmware: state.scene.firmware, mode: state.scene.view.mode};
    });
    check(embedded.product === 'm5stack-k150' && !embedded.firmware && embedded.mode === 'embed',
      'Expected a configuration-only K150 embed.');
    check(errors.length === 0, 'Unexpected page errors: ' + errors.join('; '));
    return {boot: boot.split('\n').filter(line => line.includes('ESP-ROM') || line.startsWith('STICKS3')),
      firmware: firmware.artifacts.map(({kind, sha256, bytes}) => ({kind, sha256, bytes})), frames, pointerFrames,
      imuSample: 4096, embedded};
  } finally {
    page.off('pageerror', onError);
  }
}
