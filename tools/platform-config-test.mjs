import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {resolve, basename} from 'node:path';
import {pathToFileURL} from 'node:url';
import {withPlatformTestFixtures,DIAGNOSTIC_FIRMWARE_ID} from './fixtures/platform-catalog.mjs';
import {SHELL_FINISHES, SHELL_PRESETS} from '../web/platform/appearance.js';
import {
  loadCatalog, createDefaultScene, validateScene, encodeScene, decodeScene,
  createLocalAccessory, validateLocalAccessory, importLocalAccessory, exportLocalAccessory,
  listLocalAccessories, saveLocalAccessory, removeLocalAccessory, MAX_SCENE_BYTES, firmwareSourceMatches,
} from '../web/platform/catalog.js';

// Use repository catalog bytes while retaining fetch's URL/error semantics.
const originalFetch = globalThis.fetch;
globalThis.fetch = async url => new Response(await readFile(url, 'utf8'));
let catalog;
try { catalog = await loadCatalog(); } finally { globalThis.fetch = originalFetch; }
const sceneSchema = JSON.parse(await readFile(new URL('../web/platform/schemas/scene.schema.json', import.meta.url), 'utf8'));
const checks = [];
function test(name, run) { run(); checks.push(name); }
function has(result, code, kind = 'errors') { return result[kind].some(item => item.code === code); }
function base() { return createDefaultScene(catalog); }
function gps() { return {id: 'gps-1', catalogId: 'm5stack-gps-u032', parameters: {latitude: 31.2, longitude: 121.5}, wiring: {mode: 'logical', port: 'uart1'}}; }
const memoryStorage = () => { const items = new Map(); return {getItem: key => items.get(key) ?? null, setItem: (key, value) => items.set(key, value)}; };

test('catalogs retain provenance, distinct GPS defaults and truthful support', () => {
  for (const items of Object.values(catalog)) {
    assert.equal(new Set(items.map(item => item.id)).size, items.length);
    for (const item of items) {
      assert.ok(item.name && item.sources.length && Array.isArray(item.limitations));
      for (const source of item.sources) { assert.match(source.url, /^https:\/\//); assert.match(source.checkedAt, /^\d{4}-\d{2}-\d{2}$/); }
    }
  }
  for(const id of ['m5stack-papermono','m5stack-c153-lite']) { const paper=catalog.products.find(item=>item.id===id);assert.equal(paper.runtime.board,'papermono');assert.equal(paper.runtime.status,'partial');assert.equal(paper.runtime.touch,true);const factory=catalog.firmwares.find(item=>item.id===paper.defaultFirmwareId);assert.ok(factory.productIds.includes(id)&&factory.source?.sha256); }
  assert.equal(catalog.products.find(item=>item.id==='m5stack-c149').runtime.board,null);
  const watch = catalog.products.find(item => item.id === 'm5stack-stopwatch');
  assert.equal(watch.runtime.status, 'partial');
  assert.deepEqual(watch.interfaces.find(item => item.id === 'grove').pins.filter(item => item.gpio !== undefined).map(item => item.gpio), [10, 11]);
  assert.equal(watch.display.width, 466); assert.equal(watch.display.framebufferWidth, 468);
  assert.equal(catalog.accessories.find(item => item.id === 'm5stack-gps-u032').parameters.baudRate, 9600);
  assert.equal(catalog.accessories.find(item => item.id === 'm5stack-gps-u032-v11').parameters.baudRate, 115200);
  for (const firmware of catalog.firmwares) {
    assert.ok(firmware.productIds.every(id => catalog.products.some(item => item.id === id)));
    if (firmware.status === 'reference') assert.equal(firmware.source, null);
  }
  assert.ok(['m5stack-papermono', 'm5stack-stopwatch'].every(id => catalog.products.some(item => item.id === id)));
  assert.ok(catalog.products.every(item => !item.id.includes('hello') && !item.id.startsWith('test-')), 'Diagnostic devices stay out of the public catalog');
  assert.ok(catalog.firmwares.every(item => !['platform-stopwatch-diagnostic', 'esp32sim-s3-hello'].includes(item.id)), 'Internal firmware fixtures must not return to the public catalog');
  assert.deepEqual(catalog.firmwares.filter(item => item.productIds.includes('m5stack-stopwatch')).map(item => item.id), ['m5stack-stopwatch-official']);
  assert.equal(catalog.firmwares.some(item => item.id.startsWith('test-')), false);
  assert.ok(catalog.firmwares.find(item => item.id === 'm5stack-stopwatch-official').source.url.startsWith('https://m5burner-cdn.m5stack.com/'));
});

test('configuration validity is separate from execution readiness', () => {
  const result = validateScene(base(), catalog);
  assert.equal(result.valid, true);
  assert.ok(has(result, 'FIRMWARE_MISSING', 'warnings'));
  assert.ok(has(result, 'PARTIAL_MODEL', 'warnings'));
  const paper = validateScene(createDefaultScene(catalog, 'm5stack-papermono'), catalog);
  assert.equal(paper.valid, true); assert.ok(has(paper, 'PARTIAL_MODEL', 'warnings'));
  assert.ok(has(validateScene(createDefaultScene(catalog,'m5stack-c149'),catalog),'MODEL_UNAVAILABLE','warnings'));
  assert.throws(() => createDefaultScene(catalog, 'invented'), {code: 'UNKNOWN_PRODUCT'});
});

test('visual Units share their exact SKU without pretending to inject UART data', () => {
  for (const id of ['m5stack-unit-button-u027','m5stack-unit-angle-u005','m5stack-unit-rgb-u003']) {
    const scene=base();scene.accessories=[{id:'unit-1',catalogId:id,parameters:{},wiring:{mode:'visual'}}];
    const validation=validateScene(scene,catalog);assert.equal(validation.valid,true);
    assert.ok(has(validation,'VISUAL_ACCESSORY','warnings'));
    assert.equal(has(validation,'LOGICAL_UART','warnings'),false);
    assert.equal(decodeScene(encodeScene(scene,catalog),catalog).accessories[0].catalogId,id);
    scene.accessories[0].wiring.port='uart1';assert.equal(validateScene(scene,catalog).valid,false);
  }
});

test('canonical Unicode URL roundtrip and object order independence', () => {
  const scene = base();
  const accessory = createLocalAccessory({id: 'local:test', name: '自制定位 🛰️', description: '自己的配置', modelId: 'uart-nmea'});
  scene.localAccessories = [accessory];
  scene.accessories = [{...gps(), catalogId: accessory.id}];
  const encoded = encodeScene(scene);
  assert.match(encoded, /^[A-Za-z0-9_-]+$/);
  assert.deepEqual(decodeScene(encoded), scene);
  assert.equal(encodeScene(Object.fromEntries(Object.entries(scene).reverse())), encoded);
  assert.equal(validateScene(decodeScene(encoded), catalog).valid, true);
});

test('shell presentation survives sharing while legacy scenes remain valid', () => {
  const original = base(); assert.equal(validateScene(original, catalog).valid, true);
  const scene = base(); scene.view.appearance = {shell: {color: '#8456cf', opacity: 0.36, finish: 'plastic'}};
  const restored = decodeScene(encodeScene(scene));
  assert.equal(validateScene(restored, catalog).valid, true);
  assert.deepEqual(restored.view.appearance, scene.view.appearance);
  for (const patch of [{opacity: -0.1}, {opacity: 1.1}, {opacity: '0.5'}, {color: 'red'}, {color: '#123'}, {finish: 'script'}, {src: 'https://example.test/texture'}]) {
    const invalid = structuredClone(scene); Object.assign(invalid.view.appearance.shell, patch);
    assert.equal(validateScene(invalid, catalog).valid, false);
  }
  delete restored.view.appearance; assert.deepEqual(restored, original);
});

test('all shell finishes share through schema v1 without changing independent color or opacity', () => {
  const ids = SHELL_FINISHES.map(item => item.id);
  assert.equal(ids.length, 8); assert.equal(new Set(ids).size, 8);
  assert.ok(SHELL_FINISHES.every(item => typeof item.label === 'string' && item.label.length));
  assert.deepEqual(sceneSchema.$defs.appearance.properties.shell.properties.finish.enum, ids);
  for (const finish of ids) for (const opacity of [0, .37, 1]) {
    const scene = base(); scene.view.appearance = {shell: {color: '#a376ca', opacity, finish}};
    const restored = decodeScene(encodeScene(scene));
    assert.equal(restored.schemaVersion, '1'); assert.deepEqual(restored, scene);
    assert.equal(validateScene(restored, catalog).valid, true, `${finish} / ${opacity}`);
  }
  for (const finish of ['glass', 'Plastic', '', null, {}, ['metal']]) {
    const scene = base(); scene.view.appearance = {shell: {color: '#a376ca', opacity: .37, finish}};
    assert.ok(has(validateScene(scene, catalog), 'SHELL_FINISH'));
  }
  assert.deepEqual(SHELL_PRESETS, {
    original: {color: '#080a0b', opacity: 1, finish: 'plastic'},
    'black-clear': {color: '#171923', opacity: .32, finish: 'plastic'},
    'purple-clear': {color: '#8456cf', opacity: .36, finish: 'plastic'},
    gold: {color: '#d7ab52', opacity: 1, finish: 'metal'},
  });
});

test('firmware matching does not mistake raw URLs or empty references for manifests', () => {
  const a = {url: 'https://example.test/a.bin', sha256: 'a'.repeat(64)};
  assert.equal(firmwareSourceMatches(null, null), false);
  assert.equal(firmwareSourceMatches(a, {url: 'https://example.test/b.bin'}), false);
  assert.equal(firmwareSourceMatches(a, {manifestUrl: a.url}), false);
  assert.equal(firmwareSourceMatches(a, {...a, sha256: 'b'.repeat(64)}), false);
  assert.equal(firmwareSourceMatches(a, {...a, sha256: 'A'.repeat(64)}), true);
  assert.equal(firmwareSourceMatches(a, {url: a.url}), true);
});

test('malformed, unsafe, oversized, cyclic and nonfinite payloads reject', () => {
  for (const encoded of ['', 'a', '%%%', '_w', 'a'.repeat(MAX_SCENE_BYTES * 2)]) assert.throws(() => decodeScene(encoded));
  assert.throws(() => decodeScene(btoa('{"schemaVersion":"2"}')), {code: 'SCHEMA_VERSION'});
  assert.throws(() => decodeScene(btoa('{"schemaVersion":"1","__proto__":{"polluted":true}}')), {code: 'UNSAFE_KEY'});
  assert.equal({}.polluted, undefined);
  assert.throws(() => encodeScene({...base(), extra: 'x'.repeat(MAX_SCENE_BYTES)}), {code: 'INPUT_TOO_LARGE'});
  const cycle = base(); cycle.view = cycle;
  assert.equal(validateScene(cycle, catalog).valid, false);
  assert.equal(validateScene({...base(), extra: NaN}, catalog).valid, false);
  assert.equal(validateScene({...base(), extra: undefined}, catalog).valid, false);
});

test('remote firmware URLs, exclusivity and hash pins are checked', () => {
  for (const url of ['javascript:alert(1)', 'data:application/octet-stream,test', 'file:///tmp/a.bin', 'http://example.com/fw.bin', 'https://user:secret@example.com/fw.bin', '/relative.bin', ' https://example.com/fw.bin']) {
    assert.ok(has(validateScene({...base(), firmware: {url}}, catalog), 'FIRMWARE_URL'));
  }
  const source = {url: 'https://example.com/固件.bin', sha256: 'a'.repeat(64)};
  assert.equal(validateScene({...base(), firmware: source}, catalog).valid, true);
  assert.equal(validateScene({...base(), firmware: {url: 'http://localhost:8000/fw.bin'}}, catalog).valid, true);
  assert.ok(has(validateScene({...base(), firmware: {...source, manifestUrl: 'https://example.com/fw.json'}}, catalog), 'FIRMWARE_SOURCE'));
  assert.ok(has(validateScene({...base(), firmware: {...source, sha256: 'bad'}}, catalog), 'FIRMWARE_HASH'));
  assert.ok(has(validateScene({...base(), firmware: {url: source.url}}, catalog), 'UNPINNED_FIRMWARE', 'warnings'));
});

test('unknown scene fields, products and controls reject', () => {
  assert.ok(has(validateScene({...base(), productId: 'made-up'}, catalog), 'UNKNOWN_PRODUCT'));
  assert.ok(has(validateScene({...base(), arbitraryScript: 'test'}, catalog), 'UNKNOWN_FIELD'));
  assert.ok(has(validateScene({...base(), keyMapping: {KeyQ: 'unknown'}}, catalog), 'UNKNOWN_CONTROL'));
  assert.ok(has(validateScene({...base(), view: {mode: 'fullscreen', deviceView: '3d'}}, catalog), 'VIEW_MODE'));
});

test('logical GPS requires UART1 and explicitly warns about bypassed hardware', () => {
  const scene = {...base(), accessories: [gps()]};
  const result = validateScene(scene, catalog);
  assert.equal(result.valid, true); assert.ok(has(result, 'LOGICAL_UART', 'warnings'));
  scene.accessories[0].wiring.port = 'uart2';
  assert.ok(has(validateScene(scene, catalog), 'UART_PORT'));
  scene.accessories[0].wiring = {mode: 'logical', port: 'uart1', rx: 10};
  assert.ok(has(validateScene(scene, catalog), 'LOGICAL_WIRING'));
  scene.accessories = [gps(), gps()];
  assert.ok(has(validateScene(scene, catalog), 'ACCESSORY_ID'));
  scene.accessories = [{...gps(), catalogId: 'm5stack-gps-unknown'}];
  assert.ok(has(validateScene(scene, catalog), 'UNKNOWN_ACCESSORY'));
  scene.accessories = [{...gps(), parameters: {latitude: 91}}];
  assert.ok(has(validateScene(scene, catalog), 'PARAMETER_RANGE'));
});

test('physical UART descriptions validate only exposed pins and remain unsupported', () => {
  const scene = {...base(), accessories: [{...gps(), wiring: {mode: 'physical', port: 'uart1', connectorId: 'grove', rx: 10, tx: 11}}]};
  const result = validateScene(scene, catalog);
  assert.equal(result.valid, true); assert.ok(has(result, 'PHYSICAL_WIRING_UNSUPPORTED', 'warnings'));
  scene.accessories[0].wiring.tx = 10;
  assert.ok(has(validateScene(scene, catalog), 'PIN_CONFLICT'));
  scene.accessories[0].wiring = {mode: 'physical', port: 'uart1', connectorId: 'internal-i2c', rx: 47, tx: 48};
  assert.ok(has(validateScene(scene, catalog), 'CONNECTOR'));
  scene.productId = 'm5stack-papermono';
  scene.accessories[0].wiring.connectorId = 'grove';
  assert.ok(has(validateScene(scene, catalog), 'CONNECTOR'));
});

test('DIY import, export, persistence and portable definitions work without a public catalog mutation', () => {
  const storage = memoryStorage();
  const before = JSON.stringify(catalog);
  const definition = createLocalAccessory({id: 'local:temperature', name: 'DIY 温度计', description: '个人设备', connector: {type: 'custom', protocol: 'i2c', voltage: 3.3, pins: [{name: 'SDA', direction: 'bidirectional'}, {name: 'VCC', direction: 'power'}]}});
  assert.equal(definition.model.status, 'unavailable');
  assert.deepEqual(importLocalAccessory(exportLocalAccessory(definition)), definition);
  saveLocalAccessory(definition, storage);
  assert.deepEqual(listLocalAccessories(storage), [definition]);
  saveLocalAccessory({...definition, name: 'Updated'}, storage);
  assert.equal(listLocalAccessories(storage).length, 1);
  assert.equal(listLocalAccessories(storage)[0].name, 'Updated');
  const scene = {...base(), localAccessories: [definition], accessories: [{...gps(), catalogId: definition.id}]};
  assert.equal(validateScene(decodeScene(encodeScene(scene)), catalog).valid, true);
  assert.ok(has(validateScene(scene, catalog), 'ACCESSORY_MODEL_UNAVAILABLE', 'warnings'));
  assert.equal(removeLocalAccessory(definition.id, storage), true);
  assert.equal(removeLocalAccessory(definition.id, storage), false);
  assert.deepEqual(listLocalAccessories(storage), []);
  assert.equal(JSON.stringify(catalog), before);
});

test('DIY scripts, namespace collisions, duplicate pins and corrupted storage reject', () => {
  assert.throws(() => createLocalAccessory({name: 'Bad', modelId: 'javascript'}), {code: 'LOCAL_ACCESSORY'});
  assert.throws(() => createLocalAccessory({id: 'm5stack-gps-u032', name: 'Bad'}), {code: 'LOCAL_ACCESSORY'});
  const definition = createLocalAccessory({name: 'Test'});
  assert.equal(validateLocalAccessory({...definition, script: 'alert(1)'}).valid, false);
  assert.equal(validateLocalAccessory({...definition, connector: {type: 'custom', voltage: 3.3, pins: [{name: 'A', direction: 'input'}, {name: 'A', direction: 'output'}]}}).valid, false);
  assert.throws(() => importLocalAccessory('{"__proto__":{}}'), {code: 'UNSAFE_KEY'});
  assert.throws(() => listLocalAccessories({getItem: () => '{broken'}), {code: 'INVALID_JSON'});
  const scene = {...base(), localAccessories: [definition, definition]};
  assert.ok(has(validateScene(scene, catalog), 'DUPLICATE_DEFINITION'));
});

for (const name of ['scene', 'local-accessory', 'catalog']) {
  const schema = JSON.parse(await readFile(new URL(`../web/platform/schemas/${name}.schema.json`, import.meta.url), 'utf8'));
  assert.equal(schema.$schema, 'https://json-schema.org/draft/2020-12/schema');
}
let pinnedArtifacts = 0;
// Artifact pin checks deliberately use private fixtures, independently of public entries.
const fixtureCatalog = await withPlatformTestFixtures(catalog, new URL('../web/platform/index.html', import.meta.url));
for (const firmware of fixtureCatalog.firmwares.filter(item => item.status === 'available' && item.source?.manifestUrl?.startsWith('file:'))) {
  const bytes = await readFile(new URL(firmware.source.manifestUrl));
  assert.equal(createHash('sha256').update(bytes).digest('hex'), firmware.source.sha256, `${firmware.id} manifest hash must match its catalog pin`);
  const manifest = JSON.parse(bytes);
  if (firmware.id === DIAGNOSTIC_FIRMWARE_ID) {
    for (const entry of Object.values(manifest.files).flat()) assert.match(manifest.artifact_sha256[entry], /^[a-f0-9]{64}$/, 'Every diagnostic artifact must have a hash pin');
  }
  for (const [entry, expected] of Object.entries(manifest.artifact_sha256 || {})) {
    const artifactUrl = new URL(entry, firmware.source.manifestUrl);
    let artifact;
    try { artifact = await readFile(artifactUrl); }
    catch (error) {
      if (error.code !== 'ENOENT' || !entry.endsWith('_rom.elf')) throw error;
      if (!process.env.ESP32SIM_ROM_DIR) {
        console.log(`platform-config: ROM hash not verified (fetch demo assets or set ESP32SIM_ROM_DIR): ${entry}`);
        continue;
      }
      artifact = await readFile(pathToFileURL(resolve(process.env.ESP32SIM_ROM_DIR, basename(artifactUrl.pathname))));
    }
    assert.equal(createHash('sha256').update(artifact).digest('hex'), expected, `${firmware.id}: ${entry} must match its artifact pin`);
    pinnedArtifacts++;
  }
}
console.log(`platform-config: ${checks.length} behavior groups passed; manifest pins and ${pinnedArtifacts} artifact hashes verified.`);
