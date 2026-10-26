import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {resolve} from 'node:path';

const root = new URL('../web/platform/', import.meta.url);
const readJson = async url => JSON.parse(await readFile(url, 'utf8'));
const utc = value => typeof value === 'string' && /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$/.test(value) && Number.isFinite(Date.parse(value));
const digest = bytes => createHash('sha256').update(bytes).digest('hex');

export function validateKnowledge(record) {
  assert.equal(record.schemaVersion, '1');
  assert.ok(['product', 'accessory', 'firmware'].includes(record.kind));
  assert.ok(record.id && record.catalogId && record.title && record.summary);
  assert.ok(utc(record.updatedAt));
  const sources = new Map(record.sources.map(item => [item.id, item]));
  assert.equal(sources.size, record.sources.length, 'Source IDs must be unique');
  for (const source of sources.values()) {
    assert.ok(source.title && source.kind);
    const url = new URL(source.url);
    assert.equal(url.protocol, 'https:'); assert.equal(url.username, ''); assert.equal(url.password, '');
    assert.ok(['fetched', 'linked', 'failed'].includes(source.status));
    if (source.status === 'fetched') {
      assert.ok(utc(source.fetchedAt), `${source.id}: fetched source needs a UTC receipt`);
      assert.match(source.sha256, /^[a-f0-9]{64}$/);
      assert.ok(Number.isInteger(source.bytes) && source.bytes > 0);
      assert.match(source.archiveName, /^[a-zA-Z0-9][a-zA-Z0-9._-]*$/);
      assert.ok(Date.parse(source.fetchedAt) <= Date.parse(record.updatedAt));
    } else {
      for (const key of ['fetchedAt', 'sha256', 'bytes', 'archiveName']) assert.equal(Object.hasOwn(source, key), false, `${source.id}: ${key} would invent a successful download receipt`);
      if (source.status === 'failed') assert.ok(utc(source.attemptedAt) && source.error);
    }
    if (source.discoveredFrom) assert.ok(sources.has(source.discoveredFrom), 'Discovery source must exist');
  }
  const ids = new Set();
  for (const fact of [...record.facts, ...record.unknowns]) {
    assert.ok(fact.id && !ids.has(fact.id)); ids.add(fact.id);
    assert.ok(['official', 'image-measurement', 'cad-measurement', 'unknown'].includes(fact.basis));
    assert.ok(Array.isArray(fact.sourceIds));
    for (const id of fact.sourceIds) assert.ok(sources.has(id), `${fact.id}: dangling source reference`);
    if (fact.basis === 'unknown') assert.equal(fact.value, null, 'Unknown values must not look like established facts');
    else {
      assert.ok(fact.sourceIds.length && fact.sourceIds.some(id => sources.get(id).status === 'fetched'), `${fact.id}: evidence must have been fetched`);
      assert.notEqual(fact.value, null);
    }
    if (fact.basis.endsWith('-measurement')) assert.ok(fact.method, 'Measurements need a reproducible method');
    if (fact.basis === 'image-measurement') assert.ok(fact.uncertainty, 'Pixel measurements need an uncertainty statement');
  }
  for (const id of record.summarySourceIds) assert.ok(sources.has(id));
  for (const revision of record.revisions) for (const id of revision.sourceIds) assert.ok(sources.has(id));
  for (const asset of record.assets) {
    assert.ok(sources.has(asset.sourceId));
    assert.equal(asset.url, sources.get(asset.sourceId).url, 'Asset URL must identify its recorded source');
  }
  return true;
}

const index = await readJson(new URL('knowledge/index.json', root));
assert.equal(index.schemaVersion, '1'); assert.ok(utc(index.updatedAt));
assert.equal(new Set(index.entries.map(item => item.id)).size, index.entries.length);
const catalogs = Object.fromEntries(await Promise.all(['products', 'accessories', 'firmwares'].map(async name => [name, (await readJson(new URL(`catalog/${name}.json`, root))).items])));
const records = [];
for (const entry of index.entries) {
  assert.match(entry.url, /^\.\/[a-z0-9-]+\.json$/);
  const record = await readJson(new URL(entry.url, new URL('knowledge/index.json', root)));
  validateKnowledge(record); records.push(record);
  assert.equal(record.catalogId, entry.catalogId); assert.equal(record.kind, entry.kind); assert.equal(record.updatedAt, entry.updatedAt);
  const category = {product:'products', accessory:'accessories', firmware:'firmwares'}[record.kind];
  const item = catalogs[category].find(item => item.id === record.catalogId);
  assert.ok(item, 'Knowledge must point to an actual catalog entry');
  assert.equal(new URL(item.knowledgeUrl, root).href, new URL(entry.url, new URL('knowledge/index.json', root)).href);
}
const stopwatch = records.find(item => item.id === 'm5stack-stopwatch');
assert.equal(stopwatch.hardwareRevision, 'unspecified', 'Do not infer the user’s hardware revision');
assert.equal(stopwatch.revisions.find(item => item.id === 'v1.0').powerContact.actualFunction, '5V IN');
assert.equal(stopwatch.revisions.find(item => item.id === 'v1.0.1').powerContact.actualFunction, 'BAT');
assert.equal(stopwatch.revisions.find(item => item.id === 'v1.0').powerContact.batteryConnection, false);
const envelope = stopwatch.facts.find(item => item.id === 'product-envelope');
assert.deepEqual(envelope.value, {width:52,height:52,depth:15.5});
assert.equal(stopwatch.facts.find(item => item.id === 'body-diameter-drawing').value, 51.95);
assert.equal(stopwatch.facts.find(item => item.id === 'cad-mesh').basis, 'cad-measurement');
const product = catalogs.products.find(item => item.id === stopwatch.catalogId);
assert.deepEqual([product.dimensions.width,product.dimensions.height,product.dimensions.depth],[52,52,15.5]);
assert.ok(stopwatch.sources.some(source => source.id === product.dimensions.sourceId));

const invalidLink = structuredClone(stopwatch);
invalidLink.sources.find(source => source.status === 'linked').fetchedAt = stopwatch.updatedAt;
assert.throws(() => validateKnowledge(invalidLink), /invent/);
const missingEvidence = structuredClone(stopwatch); missingEvidence.facts[0].sourceIds = ['missing'];
assert.throws(() => validateKnowledge(missingEvidence), /dangling/);
const fakeUnknown = structuredClone(stopwatch); fakeUnknown.unknowns[0].value = 'v1.0.1';
assert.throws(() => validateKnowledge(fakeUnknown), /Unknown/);
const falsePrecision = structuredClone(stopwatch); falsePrecision.facts[0].basis = 'image-measurement'; falsePrecision.facts[0].method = 'Measure pixels';
assert.throws(() => validateKnowledge(falsePrecision), /uncertainty/);

let verified = 0;
const arg = process.argv.indexOf('--source-dir');
if (arg !== -1) {
  assert.ok(process.argv[arg + 1], '--source-dir needs the temporary download directory');
  for (const record of records) for (const source of record.sources.filter(item => item.status === 'fetched')) {
    const bytes = await readFile(resolve(process.argv[arg + 1], source.archiveName));
    assert.equal(bytes.length, source.bytes, `${source.id}: byte count`);
    assert.equal(digest(bytes), source.sha256, `${source.id}: SHA-256`); verified++;
  }
}
console.log(`platform-knowledge: ${records.length} record(s), evidence/reference/revision boundaries passed; ${verified} downloaded source hashes verified${arg === -1 ? ' (use --source-dir to verify originals)' : ''}.`);
