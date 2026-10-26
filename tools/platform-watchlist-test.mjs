#!/usr/bin/env node
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { validateWatchlist, probeFirmwareUrl, checkWatchEntry, buildFirmwareIssueUrl } from '../web/platform/watchlist.js';

const document = validateWatchlist(JSON.parse(await readFile(new URL('../web/platform/catalog/firmware-watchlist.json', import.meta.url), 'utf8')));
assert.ok(document.entries.length >= 6);
assert.ok(document.entries.every(entry => entry.storage === 'remote-links-only'));
const products=JSON.parse(await readFile(new URL('../web/platform/catalog/products.json',import.meta.url),'utf8')).items;
const firmwares=JSON.parse(await readFile(new URL('../web/platform/catalog/firmwares.json',import.meta.url),'utf8')).items;
for(const entry of document.entries.filter(entry=>entry.compatibility.status==='product-verified')) {
  assert.equal(entry.compatibility.simulationVerified,true);
  assert.ok(entry.assets.length&&entry.assets.every(asset=>/^[a-f0-9]{64}$/.test(asset.sha256)));
  for(const id of entry.compatibility.productIds) {
    assert.ok(products.find(product=>product.id===id)?.runtime?.board,'Verified firmware needs a runnable product board');
    assert.ok(firmwares.some(f=>f.productIds.includes(id)&&entry.assets.some(a=>f.source?.url===a.url&&f.source.sha256===a.sha256)),'Verified release needs a pinned catalog artifact for the same product');
  }
}
const official = document.entries.find(entry => entry.id === 'm5stack-stopwatch-official');
assert.equal(official.compatibility.status, 'product-verified');
assert.equal(official.compatibility.simulationVerified, true);
assert.equal(official.compatibility.hardwareValidated, false);
assert.ok(official.assets.every(asset => /^[a-f0-9]{64}$/.test(asset.sha256)));
let calls = [], cancels = 0;
const headers = new Headers({ 'content-length': '99000000' });
const fetchImpl = async (url, options) => {
  calls.push({ url, options });
  assert.equal(options.cache, 'no-store'); assert.equal(options.credentials, 'omit');
  if (options.method === 'HEAD') return { status: 405, ok: false, headers, body: { async cancel() { cancels++; } } };
  assert.equal(options.method, 'GET'); assert.equal(options.headers.Range, 'bytes=0-0');
  return { status: 200, ok: true, headers, body: { async cancel() { cancels++; } }, arrayBuffer() { throw new Error('Firmware body must not be read'); } };
};
const result = await probeFirmwareUrl('https://example.com/large.bin', { fetchImpl });
assert.equal(result.available, true); assert.equal(result.bytesRead, 0); assert.equal(cancels, 2); assert.equal(calls.length, 2);
const missing = await probeFirmwareUrl('https://example.com/missing.bin', { fetchImpl: async () => new Response(null, { status: 404 }) });
assert.equal(missing.state, 'missing');
const cors = await probeFirmwareUrl('https://example.com/cors.bin', { fetchImpl: async () => { throw new TypeError('Failed to fetch'); } });
assert.equal(cors.state, 'network-or-cors'); assert.equal(cors.available, null);
const entry = document.entries[0];
const report = await checkWatchEntry(entry, { fetchImpl: async (url, options) => url.endsWith('firmware-release-snapshot.json')
  ? new Response(JSON.stringify({schemaVersion:'1',entries:[{id:entry.id,upstreamRepo:entry.upstreamRepo,checkedAt:new Date().toISOString(),release:{tag_name:'new-release',html_url:entry.releaseUrl,assets:[]}}]}))
  : new Response(null, { status: 404 }) });
assert.equal(report.updateAvailable, true); assert.equal(report.state, 'attention'); assert.ok(report.issueUrl.startsWith('https://github.com/zhuzhe1983/esp-online-tester/issues/new?'));
assert.equal(report.metadata.source, 'site-snapshot'); assert.equal(report.metadata.stale, false);
const limited = await checkWatchEntry(entry, {fetchImpl:async url => {
  assert.equal(new URL(url).hostname === 'api.github.com', false, 'Visitors must not consume anonymous GitHub API quota');
  return new Response(null,{status:403});
}});
assert.equal(limited.issueUrl,null); assert.equal(limited.latestVersion,entry.version); assert.equal(limited.metadata.stale,true);
const stale = await checkWatchEntry(entry, {fetchImpl:async (url,options) => options.method === 'HEAD' ? new Response(null,{status:200}) : new Response(JSON.stringify({schemaVersion:'1',entries:[{id:entry.id,upstreamRepo:entry.upstreamRepo,checkedAt:'2020-01-01T00:00:00Z',release:{tag_name:'previous-known',html_url:entry.releaseUrl,assets:[]},error:{message:'API rate limit; retained'}}]}))});
assert.equal(stale.latestVersion,'previous-known'); assert.equal(stale.metadata.stale,true); assert.equal(stale.issueUrl,null);
const issue = decodeURIComponent(buildFirmwareIssueUrl({ url: 'https://user:secret@example.com/fw.bin?token=private#secret', reason: '404', status: 404 }));
assert.equal(issue.includes('private'), false); assert.equal(issue.includes('user:secret'), false);
assert.ok(issue.includes('https://example.com/fw.bin'));
assert.throws(() => validateWatchlist({ ...document, entries: [{ ...entry, compatibility: { status: 'assumed-compatible', productIds: [] } }] }));
console.log('ok watchlist metadata schema, update detection, HEAD/range cancellation without body reads, 404/CORS distinction and private issue URL redaction');

const {releaseRequest,publicRelease}=await import('./firmware-release-source.mjs');
const burner=document.entries.find(item=>item.metadataProvider==='m5burner');
assert.ok(burner);
assert.equal(releaseRequest(burner,'private-ci-token').headers.Authorization,undefined,'GitHub credentials never go to M5Burner');
const release=publicRelease(burner,[{fid:burner.firmwareId,author:{email:'must-not-publish@example.com'},versions:[
 {version:'2',published_at:'2026-10-01',published:true,file:'second.bin'},
 {version:'99',published_at:'2026-10-02',published:false,file:'draft.bin'},
 {version:'1',published_at:'2026-09-01',published:true,file:'first.bin'}]}]);
assert.equal(release.tag_name,'2');assert.equal(release.assets[0].browser_download_url,'https://m5burner-cdn.m5stack.com/firmware/second.bin');assert.equal(JSON.stringify(release).includes('author'),false);
assert.throws(()=>publicRelease(burner,[]));
assert.throws(()=>publicRelease(burner,[{fid:burner.firmwareId,versions:[{version:'x',published_at:'2026-10-01',published:true,file:'../private.bin'}]}]));
console.log('ok official M5Burner metadata: published versions only, fixed CDN, no GitHub token or author metadata retained');
