// Run with the Playwright CLI wrapper in a task-owned headless session.
async page => {
  await page.waitForFunction(() => !!window.espTester);
  const workers = [];
  page.on('worker', worker => workers.push(new URL(worker.url()).pathname));
  const observe = () => page.evaluate(() => espTester.observe());

  async function waitForState(check, label) {
    const deadline = Date.now() + 120000;
    while (Date.now() < deadline) {
      const state = await observe();
      if (state.status === 'error') throw Error(JSON.stringify(state.lastError));
      if (check(state)) return;
      await new Promise(resolve => setTimeout(resolve, 50));
    }
    throw Error(`Timed out waiting for ${label}`);
  }

  function until(target) {
    return waitForState(state => state.stats.time >= target, `guest second ${target}`);
  }

  async function advance(seconds) {
    await until((await observe()).stats.time + seconds);
  }

  async function frame(label) {
    return page.evaluate(async label => {
      const f = await espTester.capture({format: 'rgba'});
      if (f.width !== 135 || f.height !== 240) throw Error('Expected the K016-P LCD');
      let nonblack = 0;
      for (let i = 0; i < f.pixels.length; i += 4) {
        if (f.pixels[i] || f.pixels[i + 1] || f.pixels[i + 2]) nonblack++;
      }
      const hash = await crypto.subtle.digest('SHA-256', f.pixels);
      return {
        label,
        nonblack,
        frame: f.frameNumber,
        guestTime: (await espTester.observe()).stats.time,
        sha256: Array.from(new Uint8Array(hash), n => n.toString(16).padStart(2, '0')).join(''),
      };
    }, label);
  }

  function readBuzzer() {
    return page.evaluate(async () => {
      const {chunks} = await espTester.readAudio();
      let activeSamples = 0;
      let signChanges = 0;
      let previous = 0;
      for (const chunk of chunks) {
        if (chunk.sampleRate !== 16000 || chunk.channels !== 1) {
          throw Error('Expected mono 16 kHz G2 PCM');
        }
        for (const sample of chunk.samples) {
          if (sample !== 0) activeSamples++;
          if (previous && sample && (previous < 0) !== (sample < 0)) signChanges++;
          previous = sample;
        }
      }
      const estimatedHz = signChanges * 16000 / (2 * activeSamples);
      if (activeSamples < 500 || !(estimatedHz > 3800 && estimatedHz < 4200)) {
        throw Error(`Expected the factory 4 kHz buzzer, got ${activeSamples} samples at ${estimatedHz} Hz`);
      }
      return {activeSamples, estimatedHz, channels: 1, sampleRate: 16000};
    });
  }

  async function hold(key, {buzzer = false} = {}) {
    if (buzzer) await page.evaluate(async () => { await espTester.readAudio(); });
    await page.keyboard.down(key);
    try {
      await advance(0.8);
      if (buzzer) return await readBuzzer();
    } finally {
      await page.keyboard.up(key);
      await advance(1.5);
    }
  }

  try {
    await waitForState(state => state.scene, 'initial scene');
    const started = await page.evaluate(async () => {
      const {loadCatalog, createDefaultScene} = await import('./catalog.js');
      const catalog = await loadCatalog();
      const product = catalog.products.find(item => item.id === 'm5stack-k016-p');
      const factory = catalog.firmwares.find(item => item.id === product.defaultFirmwareId);
      const scene = createDefaultScene(catalog, product.id);
      scene.firmware = factory.source;
      await espTester.configure(scene);
      const capabilities = await espTester.capabilities();
      if (capabilities.audio.input.available) throw Error('Unexpected public PCM input');
      if (!capabilities.audio.output.available || capabilities.audio.output.source !== 'guest-routed-ledc-g2') {
        throw Error('Expected the routed G2 output capability');
      }
      const {firmware} = await espTester.loadFirmware();
      const flash = firmware.artifacts.find(artifact => artifact.kind === 'flash');
      const rom = firmware.artifacts.find(artifact => artifact.kind === 'rom');
      if (flash?.sha256 !== factory.source.sha256 || !rom || firmware.appDirect || firmware.stubs.length) {
        throw Error('Expected the pinned full-ROM factory image');
      }
      await espTester.start();
      return {firmwareSha256: flash.sha256, romSha256: rom.sha256, mode: firmware.executionMode};
    });
    await until(11);
    if (!workers.some(path => path.endsWith('/wasm/worker.js'))) {
      throw Error('Expected the normal Wasm Worker');
    }
    const boot = await frame('no-input IMU');
    if (boot.nonblack === 0) throw Error('Expected the factory IMU screen');
    const tilts = [];
    for (const [x, y, z] of [[.5, .5, Math.sqrt(.5)], [-.5, .25, Math.sqrt(.6875)], [0, 0, 1]]) {
      await page.evaluate(([x, y, z]) => espTester.setImu({
        enabled: true,
        source: 'agent',
        quaternion: [-z, 0, x, 1 + y],
        angularVelocityDps: [0, 0, 0],
      }), [x, y, z]);
      await advance(1);
      tilts.push(await frame('IMU tilt'));
    }
    if (new Set(tilts.map(f => f.sha256)).size !== 3) throw Error('Expected distinct tilt responses');
    await page.evaluate(() => espTester.setImu({enabled: false, quaternion: [0, 0, 0, 1]}));

    const buzzer = await hold('a', {buzzer: true});
    const rtc = await frame('A to RTC');
    await advance(1.1);
    const next = await frame('RTC after another second');
    if (rtc.nonblack === 0 || next.nonblack === 0 || rtc.sha256 === next.sha256) {
      throw Error('Expected visible RTC progression');
    }
    await hold('b');
    const microphone = await frame('B to silent FFT');
    await advance(0.6);
    const refresh = await frame('silent FFT refresh');
    if (microphone.nonblack || refresh.nonblack || refresh.frame <= microphone.frame) {
      throw Error('Expected the guest to keep drawing its silent FFT');
    }
    return {started, workers, buzzer, frames: [boot, ...tilts, rtc, next, microphone, refresh]};
  } catch (error) {
    const state = await observe();
    throw Error(`${error.message} at guest ${state.stats.time}s (${state.status})\n${state.serial.slice(-1200)}`);
  } finally {
    await page.evaluate(() => espTester.dispose());
  }
}
