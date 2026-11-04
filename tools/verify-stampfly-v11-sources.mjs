// Read-only evidence reproduction. Never executes an installer or downloads firmware.
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {resolve} from 'node:path';
import {validateMergedImage} from '../web/platform/runtime.js';

const root = new URL('../', import.meta.url);
const json = async path => JSON.parse(await readFile(new URL(path, root), 'utf8'));
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const knowledge = await json('web/platform/knowledge/m5stack-k138-v11.json');
const audit = await json('docs/platform/evidence/m5stack-k138-v11/source-audit.json');
const factory = await json('docs/platform/evidence/m5stack-k138-v11/factory-firmware-audit.json');
const sources = new Map(knowledge.sources.map(source => [source.id, source]));
assert.deepEqual(audit.sources, knowledge.sources, 'Source audit and knowledge must identify the same retrieved bytes');

for (const asset of audit.retainedAssets) {
  const bytes = await readFile(new URL(`web/platform/${asset.path.replace(/^\.\//, '')}`, root));
  assert.equal(bytes.length, asset.bytes);
  assert.equal(digest(bytes), asset.sha256);
}

const args = process.argv.slice(2);
assert.ok(args.length === 0 || args.length === 2 && args[0] === '--source-dir', 'Usage: node tools/verify-stampfly-v11-sources.mjs [--source-dir <temporary-download-directory>]');
if (args.length) {
  for (const source of sources.values()) {
    if (source.status !== 'fetched') continue;
    const bytes = await readFile(resolve(args[1], source.archiveName));
    assert.equal(bytes.length, source.bytes, `${source.id}: byte count`);
    assert.equal(digest(bytes), source.sha256, `${source.id}: SHA-256`);
  }
  const installer = await readFile(resolve(args[1], sources.get('factory-easyloader').archiveName));
  const firmware = await readFile(resolve(args[1], sources.get('factory-raw-v100').archiveName));
  const offset = installer.indexOf(firmware);
  assert.equal(offset, factory.factoryIdentity.payloadOffsetInInstaller);
  assert.equal(installer.indexOf(firmware, offset + 1), -1, 'Expected a unique complete payload in the installer');
  assert.equal(firmware.length, factory.factoryIdentity.payloadBytes);
  assert.equal(digest(firmware), factory.factoryIdentity.payloadSha256);
  assert.equal(validateMergedImage(firmware, 'esp32s3', 8 * 1024 * 1024), true);
  console.log(`StampFly v1.1: ${sources.size} retrieved source hashes, ${audit.retainedAssets.length} retained assets, unique installer payload and complete ESP32-S3 image layout verified. No code executed or hardware flashed.`);
} else {
  console.log(`StampFly v1.1: ${audit.retainedAssets.length} retained asset hashes and source receipt consistency verified. Use --source-dir for downloaded source/firmware identity checks.`);
}
