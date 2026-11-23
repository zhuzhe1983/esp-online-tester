import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {resolve} from 'node:path';
import {validateWatchlist} from '../web/platform/watchlist.js';

const root = new URL('../web/platform/', import.meta.url);
const json = async path => JSON.parse(await readFile(new URL(path, root), 'utf8'));
const digest = data => createHash('sha256').update(data).digest('hex');
const products = (await json('catalog/products.json')).items.filter(p => p.id.startsWith('seeed-'));
const firmwares = (await json('catalog/firmwares.json')).items;
validateWatchlist(await json('catalog/firmware-watchlist.json'));
const audit = JSON.parse(await readFile(new URL('../docs/platform/research/seeed-display-factory-audit-2026-11-22.json', import.meta.url)));
assert.equal(products.length, 48);
assert.equal(new Set(products.map(p => p.id)).size, products.length);
const dated = products.filter(p => p.releaseDate);
assert.deepEqual(dated.map(p => p.releaseDate), dated.map(p => p.releaseDate).sort().reverse());
assert.equal(products.find(p => p.id === 'seeed-sensecap-indicator').sku, '114993068');
assert.equal(products.find(p => p.id === 'seeed-sensecap-watcher').sku, '113991275');
assert.equal(products.find(p => p.id === 'seeed-xiao-epaper-75').chip, 'esp32c3');
assert.equal(products.find(p => p.id === 'seeed-reterminal-d1001').chip, 'esp32p4');
assert.equal(products.find(p => p.id === 'seeed-reterminal-e1002').display.controller, 'ED2208');
assert.ok(!firmwares.some(f => f.productIds.includes('seeed-sensecap-watcher') && /xiaozhi/i.test(f.id)), 'W1-B AI Vision must not inherit the separate Xiaozhi edition target');

const bareBoards = products.filter(p => /^seeed-xiao-epaper-ee0[2-5]$/.test(p.id));
const selections = products.filter(p => p.configuration?.kind === 'commercial-kit-selection');
assert.equal(bareBoards.length, 4);
assert.equal(selections.length, 25);
const allowed = {
  EE02: ['P133_SP6'], EE03: ['P103_MONO'],
  EE04: ['P0154_MONO','P0213_MONO','P0213_QUAD','P0213_FLEX','P029_MONO','P029_QUAD','P029_FLEX','P042_MONO','P0426_MONO','P0583_MONO','P073_SP6','P075_MONO'],
  EE05: ['P0154_MONO','P0213_MONO','P0213_QUAD','P0213_FLEX','P029_MONO','P029_QUAD','P029_FLEX','P042_MONO','P0426_MONO','P0583_MONO','P075_MONO'],
};
for (const board of bareBoards) {
  assert.equal(board.display, null, 'A bare driver board does not contain a panel');
  assert.equal(board.runtime.board, board.id);
  assert.equal(board.runtime.status, 'partial');
  assert.equal(board.runtime.touch, false);
  const id = board.id.slice(-4).toUpperCase();
  const kits = selections.filter(p => p.configuration.boardProductId === board.id);
  assert.deepEqual(kits.map(p => p.configuration.panelId).sort(), [...allowed[id]].sort());
  for (const kit of kits) {
    assert.equal(kit.sku, kit.configuration.parentSku, 'Configurations retain the real parent commercial SKU');
    assert.equal(kit.configuration.boardSku, board.sku);
    assert.equal(kit.runtime.board, kit.id, 'Each panel has an exact registered runtime board');
    assert.equal(kit.runtime.status, 'partial');
    assert.ok(kit.configuration.panelSku && kit.configuration.sourceUrl);
    assert.equal(kit.runtime.touch, false, 'A storefront title alone does not establish a touch input path');
  }
}
for (const product of products.filter(p => p.hardwareEquivalence)) {
  assert.equal(product.hardwareEquivalence.runtimeBoard, product.runtime.board);
  assert.ok(product.hardwareEquivalence.sourceUrls.length >= 2, 'A shared display board requires SKU and hardware-family evidence');
  assert.ok(product.hardwareEquivalence.referenceProductId !== product.id);
  for (const firmware of firmwares.filter(f => f.productIds.includes(product.id))) {
    assert.equal(firmware.simulationVerified, false, 'New editions do not inherit another SKU execution result');
    assert.equal(firmware.hardwareValidated, false);
  }
}
assert.equal(products.find(p => p.id === 'seeed-sensecap-watcher-xiaozhi').firmwareEdition, 'XiaoZhi EN');
assert.ok(!firmwares.some(f => f.productIds.includes('seeed-sensecap-indicator-meshtastic') && f.source?.url?.includes('Default_Factory_Firmware_ESP32-S3')), 'The dedicated Meshtastic edition must not receive the ordinary D1L recovery image');
assert.equal(audit.history[0].products, 13, 'Initial execution evidence remains scoped to its original batch');
assert.equal(audit.summary.explicitKitConfigurations, 25);
assert.equal(audit.summary.independentCommercialDevicesAndBareBoards, 23);

for (const product of products) {
  const knowledge = await json(product.knowledgeUrl);
  assert.equal(knowledge.identity.sku, product.sku, 'Knowledge identity must retain the exact commercial SKU');
  for (const fact of knowledge.facts.filter(f => f.id === 'commercial-sku')) {
    assert.equal(fact.value, product.sku, 'A hardware-family variant must not inherit the base product SKU fact');
  }
  const entry = audit.entries.find(e => e.productId === product.id);
  assert.ok(entry && entry.uiflow1 && entry.uiflow2 && entry.xiaozhi, product.id);
  const list = firmwares.filter(f => f.productIds.includes(product.id));
  if (entry.factoryFirmwareIds.length) {
    assert.equal(list[0].role, 'factory');
    assert.ok(entry.factoryFirmwareIds.includes(list[0].id), 'Official factory reference remains first for its exact SKU');
    assert.equal(entry.defaultFirmwareId,product.defaultFirmwareId);
    if(list[0].id!==product.defaultFirmwareId){
      assert.ok(entry.defaultReason, 'An explicit alternative default needs a documented reason');
      assert.ok(list.find(f=>f.id===product.defaultFirmwareId)?.source?.sha256, 'Alternative defaults need a pinned executable image');
    }
  }
  if (entry.status === 'image-pinned') assert.ok(list[0].source?.sha256, 'Factory images and manifests must be pinned');
  for (const firmware of list) {
    if (firmware.source?.manifestUrl) {
      const path = firmware.source.manifestUrl;
      assert.equal(digest(await readFile(new URL(path, root))), firmware.source.sha256, path);
      const plan = await json(path);
      assert.equal(plan.board, product.runtime.board, 'Manifest identity matches the actual runtime board; sourced shared hardware does not need a fabricated alias');
      assert.equal(plan.app_direct, undefined, 'Factory defaults must retain full ROM boot layouts');
      for (const entry of Object.values(plan.flash_at || {})) {
        assert.match(entry.url, /^https:\/\//);
        assert.match(entry.sha256, /^[a-f0-9]{64}$/);
      }
      if (product.id.startsWith('seeed-xiao-ips-')) {
        assert.deepEqual(Object.keys(plan.flash_at), ['0x0','0x8000','0xe000','0x10000']);
        const entries = Object.values(plan.flash_at);
        assert.equal(new Set(entries.map(e => e.archive.entry)).size, 4);
        assert.equal(new Set(entries.map(e => e.url)).size, 1);
        for (const entry of entries) {
          assert.equal(entry.archive.format, 'zip-entry');
          assert.match(entry.archive.sha256, /^[a-f0-9]{64}$/);
          assert.match(entry.archive.entry, /^[a-zA-Z0-9_.-]+\.bin$/);
        }
      }
      if (/seeed-reterminal-e100[1-4]/.test(product.id)) {
        assert.ok(plan.flash_at['0x90000'], 'SenseCraft HMI application offset is 0x90000, not the generic Arduino 0x10000');
        assert.equal(plan.flash_at['0x10000'], undefined);
      }
      if (product.configuration) {
        assert.deepEqual(Object.keys(plan.flash_at), ['0x0','0x8000','0xe000','0x90000']);
        assert.equal(plan.flash_mb, 16);
        assert.equal(plan.psram_mb, 8);
        assert.equal(plan.boot_mode, undefined, 'Use the supported full flash-plan contract without an invented boot mode');
      }
      if (product.runtime.board === 'seeed-sensecap-watcher') {
        assert.ok(plan.flash_at['0x110000']);
        assert.equal(plan.flash_at['0x9000'], undefined, 'Never replace the device-specific factory identity partition');
      }
    }
  }
}

assert.equal(selections.filter(p => firmwares.find(f => f.id === p.defaultFirmwareId)?.source?.manifestUrl).length, 17);
for (const p of selections.filter(p => p.configuration.boardId === 'EE05')) {
  const knowledge = await json(p.knowledgeUrl);
  const conflict = knowledge.facts.find(f => f.id === 'adc-source-conflict');
  assert.deepEqual(conflict.value, {hardwareAdcEnable: 39, sensecraftCurrentConfig: 6});
}
for (const p of selections.filter(p => p.configuration.panelId.startsWith('P0213_'))) {
  assert.equal(p.display.width, p.configuration.panelId === 'P0213_FLEX' ? 104 : 122);
  assert.equal(p.display.height, p.configuration.panelId === 'P0213_FLEX' ? 212 : 250);
}
for (const p of selections.filter(p => p.configuration.panelId.startsWith('P029_'))) {
  assert.deepEqual([p.display.width, p.display.height], [128,296]);
}

// Optional integration gate against actual downloaded HTTP bodies, not generated
// snippets. No binary is included in this test or committed with the catalog.
let verified = 0;
const at = process.argv.indexOf('--source-dir');
if (at !== -1) {
  assert.ok(process.argv[at + 1]);
  const checked = new Set();
  for (const product of products) {
    const knowledge = await json(product.knowledgeUrl);
    for (const source of knowledge.sources.filter(s => s.status === 'fetched')) {
      if (checked.has(source.archiveName)) continue;
      const body = await readFile(resolve(process.argv[at + 1], source.archiveName));
      assert.equal(body.length, source.bytes, source.url);
      assert.equal(digest(body), source.sha256, source.url);
      checked.add(source.archiveName); verified++;
    }
  }
}
console.log(`seeed-catalog: 48 product identities (23 independent devices/boards + 25 explicit kit selections), firmware boundaries and layouts passed; ${verified} original HTTP bodies hash-verified.`);
