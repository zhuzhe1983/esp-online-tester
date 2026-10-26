#!/usr/bin/env node
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { validateWatchlist, probeFirmwareUrl, checkWatchEntry, buildFirmwareIssueUrl } from '../web/platform/watchlist.js';

const document = validateWatchlist(JSON.parse(await readFile(new URL('../web/platform/catalog/firmware-watchlist.json', import.meta.url), 'utf8')));
assert.ok(document.entries.length >= 6);
assert.ok(document.entries.every(entry => entry.compatibility.status === 'reference-only' && entry.storage === 'remote-links-only'));
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
const report = await checkWatchEntry(entry, { fetchImpl: async (url, options) => url === entry.metadataUrl
  ? new Response(JSON.stringify({ tag_name: 'new-release', html_url: entry.releaseUrl, assets: [] }))
  : new Response(null, { status: 404 }) });
assert.equal(report.updateAvailable, true); assert.equal(report.state, 'attention'); assert.ok(report.issueUrl.startsWith('https://github.com/zhuzhe1983/esp-online-tester/issues/new?'));
const issue = decodeURIComponent(buildFirmwareIssueUrl({ url: 'https://user:secret@example.com/fw.bin?token=private#secret', reason: '404', status: 404 }));
assert.equal(issue.includes('private'), false); assert.equal(issue.includes('user:secret'), false);
assert.ok(issue.includes('https://example.com/fw.bin'));
assert.throws(() => validateWatchlist({ ...document, entries: [{ ...entry, compatibility: { status: 'assumed-compatible', productIds: [] } }] }));
console.log('ok watchlist metadata schema, update detection, HEAD/range cancellation without body reads, 404/CORS distinction and private issue URL redaction');
