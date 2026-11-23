import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {createDefaultScene, validateScene, encodeScene, decodeScene} from '../web/platform/catalog.js';

const read = async name => JSON.parse(await readFile(new URL(`../${name}`, import.meta.url), 'utf8'));
const base = 'docs/platform/research/lilygo-spark-product-';
const inventory = await read(`${base}source-2026-10-28.json`);
const receipt = await read(`${base}source-receipt-2026-10-28.json`);
const audit = await read(`${base}mapping-2026-10-28.json`);
const firmwareAudit = await read('docs/platform/research/lilygo-official-firmware-links-2026-11-23.json');
const catalog = Object.fromEntries(await Promise.all(['products', 'accessories', 'firmwares'].map(async name => [name, (await read(`web/platform/catalog/${name}.json`)).items])));
const lilygo = catalog.products.filter(p => p.manufacturer === 'LILYGO');
const byProvider = new Map(lilygo.map(p => [p.providers?.['lilygo-spark']?.productId, p]));
const curated = {
  't-display': ['lilygo-ttgo-t-display', 4, 0, 'lilygo-ttgo-t-display-factory'],
  't-display-s3': ['lilygo-t-display-s3', 16, 8, 'lilygo-t-display-s3-factory'],
  't-deck': ['lilygo-t-deck', 16, 8, 'lilygo-t-deck-factory'],
  't-embed-cc1101': ['lilygo-t-embed-cc1101', 16, 8, 'lilygo-t-embed-cc1101-factory'],
  't-beam': ['lilygo-t-beam-v1-2', 4, 8, 'lilygo-t-beam-v1-2-factory'],
  't-watch-s3': ['lilygo-t-watch-s3', 16, 8, 'lilygo-t-watch-s3-factory'],
};

test('the pinned source accounts for every Spark product once and excludes non-device entries', () => {
  assert.equal(inventory.groups.length, 23);
  const rows = inventory.groups.flatMap(group => group.products.map(p => ({...p, groupId: group.id})));
  assert.equal(rows.length, 151);
  assert.equal(new Set(rows.map(p => p.product_id)).size, 151);
  assert.deepEqual(audit.counts, {groups: 23, sourceProducts: 151, included: 127, excluded: 24});
  assert.deepEqual(new Set(audit.products.map(p => p.productId)), new Set(rows.map(p => p.product_id)));
  const included = rows.filter(p => p.groupId !== 'general-esp32-series' && p.mcu.includes('ESP32'));
  assert.equal(byProvider.size, 127);
  assert.equal(lilygo.length, 127);
  assert.deepEqual(new Set(byProvider.keys()), new Set(included.map(p => p.product_id)));
  for (const row of audit.products) {
    if (row.status !== 'included') {
      assert.ok(row.reason);
      assert.equal(byProvider.has(row.productId), false);
    }
  }
  for (const product of lilygo) {
    const provider = product.providers['lilygo-spark'];
    assert.equal(provider.groupId, product.family.id);
    assert.equal(provider.catalogRevision, receipt.source.revision.commit);
    assert.ok(!Object.hasOwn(provider, 'uuid'));
    assert.ok(!inventory.groups.some(g => g.id === product.id), 'Series must not become executable products');
  }
});

test('six curated devices keep stable identities, capacities and exact factory defaults', () => {
  for (const [pid, [id, flash, psram, factory]] of Object.entries(curated)) {
    const product = byProvider.get(pid);
    assert.equal(product.id, id);
    assert.equal(product.runtime.flashMb, flash);
    assert.equal(product.runtime.psramMb, psram);
    assert.equal(product.defaultFirmwareId, factory);
    assert.equal(product.providers['lilygo-spark'].mappingScope, 'hardware-variant');
  }
  assert.equal(byProvider.get('t-display').runtime.board, 'ttgo-tdisplay');
  assert.equal(byProvider.get('t-display-s3').display.bus, 'I8080 8-bit');
  assert.match(byProvider.get('t-deck').hardwareRevision, /not Plus/);
  assert.match(byProvider.get('t-beam').hardwareRevision, /1\.2/);
});

test('T-Deck is a six-item family with independent Plus, Pro and software editions', () => {
  const deck = lilygo.filter(p => p.family.id === 't-deck-series');
  assert.deepEqual(deck.map(p => p.providers['lilygo-spark'].productId).sort(), ['t-deck', 't-deck-meshtastic', 't-deck-plus-1', 't-deck-plus-meshtastic', 't-deck-pro', 't-deck-pro-meshtastic'].sort());
  for (const p of deck.filter(p => p.id !== 'lilygo-t-deck')) {
    assert.ok(p.model?.url, `${p.id}: exact preview exists`);
    assert.notEqual(p.runtime.board, 'tdeck', 'A sibling must not inherit original T-Deck hardware');
    if(p.runtime.board) assert.equal(p.runtime.board,p.id,'Explicitly adapted variant retains an independent board identity');
    assert.notEqual(p.defaultFirmwareId, curated['t-deck'][3], 'Only the original Deck can inherit its exact default');
  }
  for (const pid of ['t-deck-meshtastic', 't-deck-plus-meshtastic', 't-deck-pro-meshtastic']) assert.equal(byProvider.get(pid).firmwareEdition, 'Meshtastic');
});

test('every LilyGO preview round-trips and new protocol support stays separate from firmware compatibility', async () => {
  for (const product of lilygo) {
    const scene = createDefaultScene(catalog, product.id);
    assert.equal(validateScene(scene, catalog).valid, true, product.id);
    assert.deepEqual(decodeScene(encodeScene(scene, catalog), catalog), scene);
    if (Object.hasOwn(curated, product.providers['lilygo-spark'].productId)) continue;
    assert.ok(product.model?.url, `${product.id}: missing debug/CAD/photo preview`);
    assert.ok(product.chronology && Object.hasOwn(product.chronology,'date'));
    assert.ok(['diagnostic','unavailable'].includes(product.runtime.status));
    if(!product.runtime.board){assert.equal(product.runtime.status,'unavailable');assert.equal(product.runtime.flashMb,null);assert.equal(product.runtime.psramMb,null);}
    if(product.displayStatus==='unknown')assert.equal(product.display,null,'An unknown display cannot be a guessed physical screen');
    const knowledge = await read(`web/platform/knowledge/${product.id}.json`);
    if (product.defaultFirmwareId) {
      const factory = catalog.firmwares.find(f => f.id === product.defaultFirmwareId);
      assert.equal(factory?.role, 'factory', product.id);
      assert.deepEqual(factory.productIds, [product.id]);
      assert.equal(knowledge.facts.find(f => f.id === 'factory-firmware')?.value.firmwareId, factory.id);
    } else assert.ok(knowledge.unknowns.some(f => f.id === 'factory-firmware' && f.value === null));
    assert.equal(knowledge.sources.find(s => s.id === 'spark-product-catalog').sha256, receipt.source.sha256);
    assert.ok(knowledge.factoryFirmwareInvestigation,'Each product has a recorded factory-source investigation');
    assert.ok(knowledge.sources.filter(s => s.status === 'linked').every(s => !s.fetchedAt && !s.sha256));
    assert.ok(knowledge.sources.filter(s => s.status === 'fetched').every(s => /^[a-f0-9]{64}$/.test(s.sha256) && s.fetchedAt));
    if(product.runtime.board)assert.equal(knowledge.runtimeSupport.status,'diagnostic');
  }
});

test('the curated source projection has exact retained byte identity and no invented UUID', async () => {
  const bytes = await readFile(new URL(`../${base}source-2026-10-28.json`, import.meta.url));
  assert.equal(bytes.length, receipt.projection.bytes);
  assert.equal(createHash('sha256').update(bytes).digest('hex'), receipt.projection.sha256);
  assert.equal(receipt.identity.uuidFound, false);
  assert.equal(receipt.identity.productKey, 'product_list[].products[].product_id');
  assert.equal(receipt.identity.groupKey, 'product_list[].id');
});

test('firmware receipts cover the expansion and pin the exact public download for each mapped product', () => {
  const expansion = lilygo.filter(p => !Object.hasOwn(curated, p.providers['lilygo-spark'].productId));
  assert.deepEqual(firmwareAudit.items.map(r => r.productId).sort(), expansion.map(p => p.id).sort());
  for (const row of firmwareAudit.items) {
    const {productId, artifact} = row;
    const product = lilygo.find(p => p.id === productId);
    assert.ok(row.notes, productId);
    if (row.package) {
      const file = row.package;
      const entry = catalog.firmwares.find(f => f.productIds.includes(productId) && f.sources.some(s => s.url === file.url));
      assert.equal(entry?.status, 'reference', productId);
      assert.equal(entry.source, null);
      assert.match(file.sha256, /^[a-f0-9]{64}$/);
      if (file.role === 'factory') assert.equal(product.defaultFirmwareId, entry.id);
    }
    if (!artifact) {
      if (row.package?.role !== 'factory') assert.equal(product.defaultFirmwareId, undefined, productId);
      continue;
    }
    const fw = catalog.firmwares.find(f => f.productIds.includes(productId) && f.sources.some(s => s.url === artifact.url));
    assert.deepEqual(fw?.productIds, [productId], productId);
    assert.equal(fw.role, artifact.role);
    assert.match(artifact.sha256, /^[a-f0-9]{64}$/);
    assert.equal(fw.simulationVerified, false);
    assert.equal(fw.hardwareValidated, false);
    if (artifact.role === 'factory') assert.equal(product.defaultFirmwareId, fw.id);
    if (fw.source) {
      assert.equal(fw.status, 'trial');
      assert.equal(fw.source.url, artifact.url);
      assert.equal(fw.source.sha256, artifact.sha256);
    } else assert.equal(fw.status, 'reference');
  }
});
