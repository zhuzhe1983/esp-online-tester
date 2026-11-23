// Run with the Playwright CLI wrapper in a task-owned headless session.
async page => {
  await page.waitForFunction(() => !!window.espTester);
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
    const loaded = await espTester.loadFirmware();
    await espTester.start();
    return {factory: factory.source, loaded};
  });
  const observe = () => page.evaluate(() => espTester.observe());

  async function until(target) {
    await page.waitForFunction(async target => {
      const state = await espTester.observe();
      if (state.status === 'error') throw Error(JSON.stringify(state.lastError));
      return state.stats.time >= target;
    }, target, {timeout: 120000});
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

  async function hold(key) {
    await page.keyboard.down(key);
    try {
      await advance(0.8);
    } finally {
      await page.keyboard.up(key);
    }
    await advance(1.5);
  }

  try {
    await until(11);
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

    await hold('a');
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
    return {started, frames: [boot, ...tilts, rtc, next, microphone, refresh]};
  } finally {
    await page.evaluate(() => espTester.dispose());
  }
}
