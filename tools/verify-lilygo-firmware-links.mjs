// Verify the immutable upstream bytes against this audit and the production image loader.
// Usage: node tools/verify-lilygo-firmware-links.mjs
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFile} from 'node:fs/promises';
import {pathToFileURL} from 'node:url';
import {chipIdForBoard, isSramOnlyBoard, validateAppImage} from '../web/platform/app-image.js';
import {validateMergedImage} from '../web/platform/runtime.js';
import {createLilygoSparkClient} from '../web/platform/lilygo-spark.js';

const read = async path => JSON.parse(await readFile(new URL('../' + path, import.meta.url)));
export const auditPath = 'docs/platform/research/lilygo-official-firmware-links-2026-11-23.json';

async function pinnedBytes(artifact, browserReadable) {
  const response = await fetch(artifact.url, {signal: AbortSignal.timeout(90000)});
  assert.equal(response.status, 200, artifact.url);
  if (browserReadable) assert.ok(['*', 'https://zhuzhe1983.github.io'].includes(response.headers.get('access-control-allow-origin')), 'Browser origin must be able to read the pinned file');
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (artifact.bytes !== undefined) assert.equal(bytes.length, artifact.bytes, artifact.url);
  assert.equal(createHash('sha256').update(bytes).digest('hex'), artifact.sha256, artifact.url);
  return bytes;
}

export async function checkArtifact(product, artifact) {
  const bytes = await pinnedBytes(artifact, true);
  const {board, flashMb} = product.runtime;
  if (!board) return {reason: '该设备尚无独立的模拟主板。'};
  assert.equal(chipIdForBoard(board), artifact.chipId, product.id + ': wrong chip');
  if (isSramOnlyBoard(board) && artifact.layout === 'merged-flash')
    return {reason: '当前模拟板只能运行 SRAM 程序，此下载包含引导程序和 Flash 分区。'};
  try {
    if (artifact.layout === 'merged-flash') {
      validateMergedImage(bytes, board, flashMb * 1048576);
      return {bootMode: 'rom'};
    }
    if (artifact.layout === 'app-image' && artifact.role !== 'factory') {
      await validateAppImage(bytes, board, flashMb * 1048576);
      return {bootMode: 'app-direct'};
    }
    return {reason: '该文件缺少可供当前模拟板从 ROM 启动的完整 Flash 布局。'};
  } catch (error) {
    if (error.code === 'FLASH_OVERFLOW') return {reason: '镜像或其中的分区超出了当前模拟板配置的 Flash 容量。'};
    if (error.code === 'CHIP_REVISION_UNSUPPORTED') return {reason: '镜像要求的芯片版本与当前模拟板不同。'};
    if (error.code === 'INCOMPLETE_IMAGE') return {reason: '镜像的启动布局与当前模拟板的加载方式不符。'};
    throw error;
  }
}

// Use the reviewed Spark snapshot so the device/version counts can be reproduced.
async function checkSpark(rows, products, firmwares) {
  const url = 'https://raw.githubusercontent.com/Xinyuan-LilyGO/LILYGO-Spark/a88415a33a699dbdf64d90f5d38263f549d53017/firmware_manifest_v2.json';
  const client = createLilygoSparkClient(() => fetch(url, {signal: AbortSignal.timeout(30000)}), {products, firmwares});
  let devices = 0, versions = 0;
  for (const {productId, artifact} of rows) {
    const product = products.find(p => p.id === productId);
    const deviceId = product.providers['lilygo-spark'].productId, seen = new Set();
    let page = 1, received = 0, total;
    do {
      const result = await client.search(deviceId, '', page++);
      total = result.total;
      received += result.rows.length;
      for (const project of result.rows) for (const {source} of project.versions) {
        if (!source || seen.has(source.sha256)) continue;
        seen.add(source.sha256);
        if (artifact?.sha256 === source.sha256 && artifact.url === source.url) continue;
        const bytes = await pinnedBytes(source, true);
        const flashBytes = product.runtime.flashMb * 1048576;
        if (source.bootMode === 'app-direct') await validateAppImage(bytes, product.runtime.board, flashBytes);
        else validateMergedImage(bytes, product.runtime.board, flashBytes);
      }
    } while (received < total);
    if (seen.size) devices++;
    versions += seen.size;
  }
  console.log('Spark matched %d device/version combinations across %d additional devices.', versions, devices);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [audit, products, firmwares] = await Promise.all([read(auditPath), read('web/platform/catalog/products.json'), read('web/platform/catalog/firmwares.json')]);
  const pinned = audit.items.filter(item => item.artifact);
  for (let start = 0; start < pinned.length; start += 4) {
    await Promise.all(pinned.slice(start, start + 4).map(async ({productId, artifact}) => {
      const product = products.items.find(p => p.id === productId);
      const fw = firmwares.items.find(f => f.productIds.includes(productId) && f.sources.some(s => s.url === artifact.url));
      assert.ok(fw, productId + ': missing catalog entry');
      const result = await checkArtifact(product, artifact);
      if (result.bootMode) assert.deepEqual(fw.source, {url: artifact.url, sha256: artifact.sha256, bootMode: result.bootMode});
      else assert.equal(fw.source, null, productId);
    }));
  }
  const packages = audit.items.filter(item => item.package);
  for (const {productId, package: file} of packages) {
    const bytes = await pinnedBytes(file, false);
    const product = products.items.find(p => p.id === productId);
    const fw = firmwares.items.find(f => f.productIds.includes(productId) && f.sources.some(s => s.url === file.url));
    assert.ok(fw && fw.status === 'reference' && !fw.source, productId + ': missing download reference');
    if (file.layout === 'merged-flash') validateMergedImage(bytes, product.runtime.board, product.runtime.flashMb * 1048576);
  }
  await checkSpark(audit.items, products.items, firmwares.items);
  console.log('Verified upstream bytes for %d firmware entries and %d downloadable packages.', pinned.length, packages.length);
}
