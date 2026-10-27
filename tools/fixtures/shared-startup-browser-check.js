// Run in a task-owned Playwright CLI session already opened at /platform/.
// The wrapper closes the session after collecting window.sharedStartupChecks.
async (page) => {
  const base = new URL('./', page.url()).href;
  let revision = 0;
  const checks = [];
  const assert = (value, message) => { if (!value) throw Error(message); };
  const visit = async (hash = '', file = 'index.html') => {
    await page.goto(`${base}${file}?startup-check=${++revision}${hash}`);
    await page.waitForFunction(() => Boolean(window.espTester));
    await page.waitForFunction(async () => (await espTester.observe()).status !== 'idle');
  };
  await visit();
  assert(await page.evaluate(async () => (await espTester.observe()).status === 'configured'), 'Plain visit ran firmware');
  checks.push('plain-visit-stays-configured');
  const scenes = await page.evaluate(async () => {
    const {loadCatalog, createDefaultScene, encodeScene} = await import('./catalog.js');
    const c = await loadCatalog();
    const scene = createDefaultScene(c, 'm5stack-stopwatch');
    scene.firmware = {manifestUrl: new URL('../wasm/fw/demo-stopwatch.json', location.href).href};
    const trial = c.firmwares.find(f => f.productIds.includes('m5stack-c126') && f.source && !f.simulationVerified);
    if (!trial) throw Error('AtomS3R trial fixture entry missing');
    const unverified = createDefaultScene(c, 'm5stack-c126'); unverified.firmware = trial.source;
    const unsupported = createDefaultScene(c, c.products.find(p => !p.runtime.board).id); unsupported.firmware = scene.firmware;
    const missing = {...scene, firmware: null};
    const broken = {...scene, firmware: {url: new URL('startup-missing.bin', location.href).href}};
    return {run:encodeScene(scene),trial:encodeScene(unverified),trialSource:trial.source.url||trial.source.manifestUrl,unsupported:encodeScene(unsupported),missing:encodeScene(missing),broken:encodeScene(broken)};
  });
  await visit(`#scene=${scenes.run}&autostart=0`, 'embed.html');
  assert(await page.evaluate(async () => !(await espTester.observe()).firmware), 'Explicit opt-out loaded firmware');
  checks.push('explicit-preview-stays-unloaded');
  await visit('#scene=invalid-scene&autostart=1');
  assert(await page.evaluate(async () => !(await espTester.observe()).firmware), 'Invalid scene ran fallback');
  checks.push('invalid-scene-no-fallback');
  await visit(`#scene=${scenes.run}&recording=invalid`);
  await page.getByText('共享录制无效，自动启动已停止。可手动启动设备或重新获取链接。', {exact: true}).first().waitFor();
  assert(await page.evaluate(async () => !(await espTester.observe()).firmware), 'Invalid recording started firmware');
  checks.push('invalid-recording-stops-with-visible-message');
  await visit(`#scene=${scenes.missing}`);
  await page.getByText('分享链接未包含可加载的固件，请选择远程固件或本地文件。', {exact: true}).first().waitFor();
  assert(await page.evaluate(async () => !(await espTester.observe()).firmware), 'Missing source used default');
  checks.push('missing-firmware-does-not-substitute-default');
  await visit(`#scene=${scenes.unsupported}`);
  await page.getByText('尚未完成适配', {exact: true}).waitFor();
  assert(await page.locator('#start-runtime').isHidden(), 'Unsupported board exposed run button');
  assert(await page.evaluate(async () => !(await espTester.observe()).firmware), 'Unsupported board downloaded firmware');
  checks.push('unsupported-board-no-run');
  let downloads = 0;
  const requested = request => { if (request.url() === scenes.trialSource) downloads++; };
  page.on('request', requested);
  try {
    await visit(`#scene=${scenes.trial}`);
    const dialog = page.getByRole('dialog'); await dialog.waitFor();
    await dialog.getByRole('button', {name: '取消', exact: true}).click();
    await page.getByText('已取消自动启动，可点击“启动”重新开始。', {exact: true}).first().waitFor();
    assert(downloads === 0, 'Trial firmware downloaded before consent');
    assert(await page.evaluate(async () => !(await espTester.observe()).firmware), 'Cancelled trial started');
    checks.push('trial-prompts-on-arrival-cancel-does-not-download');
  } finally { page.off('request', requested); }
  await page.route('**/startup-missing.bin', route => route.fulfill({status:404, body:'not found'}));
  try {
    await visit(`#scene=${scenes.broken}`, 'embed.html');
    await page.waitForFunction(async () => (await espTester.observe()).status === 'error');
    assert(await page.locator('#empty-description').textContent().then(s => s.includes('失败')), 'Mini player hid startup error');
    assert(await page.getByRole('button', {name: '▶ 启动', exact: true}).isEnabled(), 'Failed startup cannot retry');
    checks.push('failed-download-visible-and-retryable-in-mini');
  } finally { await page.unroute('**/startup-missing.bin'); }
  await visit();
  await page.evaluate(({base,scene}) => { const f=document.createElement('iframe'); f.id='startup-check-frame'; f.src=`${base}embed.html#scene=${scene}`; document.body.append(f); }, {base,scene:scenes.run});
  await page.frameLocator('#startup-check-frame').locator('#runtime-badge').filter({hasText:'运行中'}).waitFor({timeout:60000});
  await page.evaluate(async () => {
    const f=document.getElementById('startup-check-frame'), api=f.contentWindow.espTester;
    await api.waitFor({port:'uart0',serialIncludes:'PLATFORM_READY stopwatch',timeoutMs:60000});
    await api.pause(); f.remove();
  });
  checks.push('iframe-starts-real-firmware-without-click');
  await page.evaluate(checks => { window.sharedStartupChecks = {checkedAt:new Date().toISOString(),checks}; }, checks);
}
