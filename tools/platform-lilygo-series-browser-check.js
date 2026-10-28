// Run with the repository Playwright wrapper's run-code --filename option on
// an already-open local workbench. Uses its actual catalog; downloads no firmware.
async (page) => {
  const check = (condition, message) => { if (!condition) throw Error(message); };
  const errors = [], onError = error => errors.push(error.message);
  page.on('pageerror', onError);
  const settle = async () => { await page.waitForFunction(() => window.espTester && !document.querySelector('#product-select')?.disabled); };
  const state = () => page.evaluate(async () => ({productId:(await window.espTester.observe()).scene.productId,manufacturer:document.querySelector('#manufacturer-select').value,family:document.querySelector('#product-family-select').value,options:[...document.querySelector('#product-select').options].map(o=>o.value),runHidden:document.querySelector('#start-runtime').hidden,loadHidden:document.querySelector('#load-firmware').hidden,screenHidden:document.querySelector('.flat-display').hidden,specs:document.querySelector('#product-specs').textContent}));
  try {
    await page.setViewportSize({width:1440,height:1000}); await settle();
    const catalog = await page.evaluate(async () => (await (await fetch('./catalog/products.json')).json()).items);
    const lilygo = catalog.filter(p=>p.manufacturer==='LILYGO'), deck = lilygo.filter(p=>p.family?.id==='t-deck-series');
    check(lilygo.length > 6 && deck.length > 1, 'Expanded real Spark product catalog is required.');
    await page.locator('#manufacturer-select').selectOption('M5Stack'); await settle();
    check(await page.locator('#product-family-field').isHidden(), 'M5Stack without declared families retains its flat selector.');
    const m5Count = (await state()).options.length;
    check(m5Count === catalog.filter(p=>p.manufacturer==='M5Stack').length, 'Existing M5Stack models are preserved.');
    await page.locator('#manufacturer-select').selectOption('LILYGO'); await settle();
    check(await page.locator('#product-family-field').isVisible(), 'LILYGO exposes the family selector.');
    check((await state()).family === '', 'Switching manufacturer starts with every family visible.');
    check((await state()).options.length === lilygo.length, 'All families lists concrete products exactly once.');
    const familyCount = await page.locator('#product-select optgroup').count();
    check(familyCount > 1, 'All-family models are grouped by family, without family pseudo-products.');
    await page.locator('#product-family-select').selectOption('t-deck-series'); await settle();
    const deckState = await state();
    check(JSON.stringify([...deckState.options].sort()) === JSON.stringify(deck.map(p=>p.id).sort()), 'T-Deck filter contains every exact T-Deck variant only.');
    check(!deckState.options.includes('t-deck-series'), 'A family is not an executable product.');
    await page.locator('#product-select').selectOption('lilygo-t-deck-plus-1'); await settle();
    const selected = await state();
    check(selected.family === 't-deck-series' && selected.runHidden && selected.loadHidden, 'New Plus remains selectable but unavailable for execution.');
    check(selected.specs.includes('屏幕资料待核对'), 'Unknown display metadata is not presented as a screenless device.');
    await page.locator('#view-2d').click();
    check((await state()).screenHidden, 'Unknown display without model does not fabricate a 2D screen.');
    await page.screenshot({path:'output/lilygo-series-unavailable.png'});
    const shared = await page.evaluate(() => window.espTester.share({mode:'studio',autostart:false}));
    await page.goto(shared); await settle();
    const restored = await state();
    check(restored.productId === selected.productId && restored.family === selected.family, 'Shared scene restores exact local product ID and family.');
    check(restored.runHidden && restored.loadHidden, 'Shared unavailable product stays non-executable.');
    const embedded = await page.evaluate(() => window.espTester.share({mode:'embed',autostart:false}));
    await page.goto(embedded); await page.waitForFunction(() => window.espTester && document.querySelector('#stage-title')?.textContent !== '设备预览');
    const embedState = await page.evaluate(async () => ({productId:(await window.espTester.observe()).scene.productId,runHidden:document.querySelector('#start-runtime').hidden,screenHidden:document.querySelector('.flat-display').hidden}));
    check(embedState.productId === selected.productId && embedState.runHidden && embedState.screenHidden, 'Embedded shared scene keeps exact model and unknown-display execution limits.');
    await page.goto(shared); await settle();
    await page.locator('#search-devices').click();
    await page.locator('#device-search-dialog input').fill('T-Display 系列');
    const results = await page.locator('#device-search-dialog .device-search-result').count();
    check(results === lilygo.filter(p=>p.family?.id==='t-display-series').length, 'Family search crosses the active T-Deck filter and returns its concrete models.');
    await page.locator('#device-search-dialog [data-product-id="lilygo-ttgo-t-display"]').click(); await settle();
    const searched = await state();
    check(searched.family === 't-display-series' && searched.productId === 'lilygo-ttgo-t-display', 'Cross-family search synchronizes selected family and precise model.');
    const firmwareBefore = await page.evaluate(async () => (await window.espTester.observe()).scene.firmware);
    await page.locator('#product-family-select').selectOption(''); await settle();
    const firmwareAfter = await page.evaluate(async () => (await window.espTester.observe()).scene.firmware);
    check(JSON.stringify(firmwareBefore) === JSON.stringify(firmwareAfter), 'Broadening the family filter preserves the current firmware choice.');
    await page.locator('#product-select').selectOption('lilygo-t-display-s3'); await settle();
    check((await state()).family === '', 'Manual all-family browsing stays broad when choosing another model.');
    await page.locator('#product-select').selectOption('lilygo-ttgo-t-display'); await settle();
    check((await state()).family === '', 'All-family choice persists on return to the original model.');
    await page.waitForFunction(() => document.querySelector('#scene-mount').dataset.modelStatus === 'ready');
    check(await page.locator('#scene-mount canvas[data-engine]').isVisible(), 'Known TTGO CAD finishes loading and its canvas is visible.');
    await page.screenshot({path:'output/lilygo-series-all.png'});
    // Stall a test manifest at the metadata request to inspect the real loading
    // state. Its explicit 404 completes the test without fetching a firmware BIN.
    const pendingUrl = new URL('./series-ui-pending.json', page.url()).href;
    let release;
    const waiting = new Promise(resolve => { release = resolve; });
    const route = async request => { await waiting; await request.fulfill({status:404,body:'Intentional UI loading-state fixture'}); };
    await page.route(pendingUrl, route);
    try {
      await page.evaluate(url => { window.seriesUiPending = window.espTester.loadFirmware({manifestUrl:url}).then(()=>null,error=>error.code); }, pendingUrl);
      await page.waitForFunction(() => document.querySelector('#product-family-select').disabled);
      check(await page.locator('#manufacturer-select').isDisabled() && await page.locator('#product-select').isDisabled(), 'All hierarchy controls are disabled during loading.');
      release(); await page.evaluate(() => window.seriesUiPending); await settle();
      check(await page.locator('#product-family-select').isEnabled(), 'Family selection is restored after loading ends.');
    } finally { release(); await page.unroute(pendingUrl,route); }
    check(!errors.length, `Unexpected page errors: ${errors.join('; ')}`);
    return {lilygoProducts:lilygo.length,families:familyCount,deckProducts:deck.length,m5Products:m5Count,sharedProductId:restored.productId,searchProductId:searched.productId,pageErrors:errors,checks:['manufacturer/family/model selection','family-scoped concrete variants','cross-family search','stable local-ID studio and embed sharing','unknown display and unavailable execution','firmware selection preservation','known TTGO CAD ready and visible canvas','busy disable/recovery']};
  } finally { page.off('pageerror',onError); }
}
