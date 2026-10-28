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
    assert.equal(p.runtime.board, null);
    assert.equal(p.model, undefined);
    assert.equal(p.defaultFirmwareId, undefined);
    assert.equal(p.displayStatus, 'unknown');
    assert.equal(p.display, null);
  }
  for (const pid of ['t-deck-meshtastic', 't-deck-plus-meshtastic', 't-deck-pro-meshtastic']) assert.equal(byProvider.get(pid).firmwareEdition, 'Meshtastic');
});

test('new products remain explicit unknowns and round-trip as valid preview scenes', async () => {
  for (const product of lilygo) {
    const scene = createDefaultScene(catalog, product.id);
    assert.equal(validateScene(scene, catalog).valid, true, product.id);
    assert.deepEqual(decodeScene(encodeScene(scene, catalog), catalog), scene);
    if (Object.hasOwn(curated, product.providers['lilygo-spark'].productId)) continue;
    assert.deepEqual(product.runtime, {board: null, flashMb: null, psramMb: null, status: 'unavailable'});
    assert.equal(product.displayStatus, 'unknown');
    assert.equal(product.display, null);
    assert.deepEqual(product.controls, []);
    assert.deepEqual(product.interfaces, []);
    assert.equal(product.defaultFirmwareId, undefined);
    const knowledge = await read(`web/platform/knowledge/${product.id}.json`);
    assert.ok(knowledge.unknowns.some(f => f.id === 'factory-firmware' && f.value === null));
    assert.equal(knowledge.sources.find(s => s.id === 'spark-product-catalog').sha256, receipt.source.sha256);
    assert.ok(knowledge.sources.filter(s => s.id !== 'spark-product-catalog').every(s => s.status === 'linked' && !s.fetchedAt && !s.sha256));
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
