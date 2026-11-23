// Run against the workbench. Removing this product's local entries exercises
// Spark discovery even when the local catalog has not caught up with a new board.
async page => {
  const base = new URL('./', page.url());
  const emptyProduct = 'lilygo-t-dongle-c5', referenceProduct = 'm5stack-c124', referenceId = 'm5stack-c124-factory-reference';
  const check = (ok, message) => { if (!ok) throw Error(message); };
  const waitForScene = () => page.waitForFunction(() => window.espTester && document.querySelector('#flash-device'));
  await page.route('**/catalog/firmwares.json', async route => {
    const response = await route.fetch(), catalog = await response.json();
    catalog.items = catalog.items.filter(item => !item.productIds.includes(emptyProduct));
    await route.fulfill({response, json:catalog});
  });
  const configure = id => page.evaluate(async id => {
    const {loadCatalog, createDefaultScene} = await import('./catalog.js');
    await espTester.configure(createDefaultScene(await loadCatalog(), id));
  }, id);
  await page.goto(base.href); await waitForScene();
  await configure(emptyProduct);
  check(await page.locator('#firmware-select option[value]:not([value=""])').count() === 0, 'Empty catalog fixture');
  await page.locator('#browse-spark').click();
  check(await page.locator('#firmware-kind').inputValue() === 'lilygo-spark' && await page.locator('#firmware-spark').isVisible(), 'One click opens the Spark selector');

  await configure(referenceProduct);
  check(await page.locator('#browse-spark').isHidden(), 'Spark discovery belongs to LILYGO products');
  await page.locator('[data-firmware-tab="library"]').click();
  await page.locator('#firmware-select').selectOption(referenceId);
  const expected = await page.evaluate(async id => {
    const {loadCatalog} = await import('./catalog.js');
    const entry = (await loadCatalog()).firmwares.find(item => item.id === id);
    return [...new Set([...entry.sources.map(source => source.url), entry.publicationUrl])];
  }, referenceId);
  const readLinks = locator => locator.evaluateAll(items => items.map(link => link.href));
  const workbench = await readLinks(page.locator('#firmware-updates .firmware-update-actions a'));
  check(expected.every(href => workbench.includes(href)) && workbench.length === new Set(workbench).size, 'Reference files and publisher page have distinct clickable links');
  check(await page.locator('#load-firmware').isDisabled() && await page.locator('#flash-device').isDisabled(), 'Reference cannot run or flash');

  const mini = await page.evaluate(async () => {
    const {encodeScene} = await import('./catalog.js'), {scene} = await espTester.observe();
    return './embed.html#' + new URLSearchParams({scene:encodeScene(scene),autostart:'0'});
  });
  await page.goto(new URL(mini, base).href); await waitForScene();
  await page.locator('#firmware-references summary').click();
  const miniLinks = await readLinks(page.locator('#firmware-references a'));
  check(expected.every(href => miniLinks.includes(href)), 'Mini lists references for this exact product');
  check(await page.locator('#flash-device').isDisabled() && await page.locator('#start-runtime').isDisabled(), 'Preview has no runnable or flashable selection');
  const edit = await page.locator('#edit-scene').getAttribute('href');
  await page.goto(edit); await waitForScene();
  const restored = await page.evaluate(async () => (await espTester.observe()).scene);
  check(restored.productId === referenceProduct && !restored.firmware, 'Open in workbench preserves the empty firmware selection');
}
