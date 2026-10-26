// Node-only test fixtures. Website catalog loaders must never import this file.
import {readFile} from 'node:fs/promises';

export const HELLO_PRODUCT_ID = 'test-esp32s3-hello';
export const UNAVAILABLE_PRODUCT_ID = 'test-unimplemented-board';
export const DIAGNOSTIC_FIRMWARE_ID = 'test-stopwatch-diagnostic';
export async function withPlatformTestFixtures(publicCatalog, baseUrl) {
  const hello = JSON.parse(await readFile(new URL('./platform-hello.json', import.meta.url), 'utf8'));
  const diagnostic = JSON.parse(await readFile(new URL('./platform-stopwatch.json', import.meta.url), 'utf8'));
  const catalog = structuredClone(publicCatalog);
  for (const firmware of [hello.firmware, diagnostic]) {
    firmware.source.manifestUrl = new URL(firmware.source.manifestUrl, baseUrl).href;
    catalog.firmwares.push(firmware);
  }
  catalog.products.push(hello.product);
  catalog.products.push({...structuredClone(hello.product),id:UNAVAILABLE_PRODUCT_ID,name:'Test-only unimplemented board',runtime:{...hello.product.runtime,board:null,status:'unavailable'}});
  return catalog;
}
