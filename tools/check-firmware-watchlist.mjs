#!/usr/bin/env node
// Fetch release JSON and probe headers only; never download or save firmware bodies.
import { readFile, writeFile } from 'node:fs/promises';
import { validateWatchlist, checkWatchEntry } from '../web/platform/watchlist.js';

const args = process.argv.slice(2), offline = args.includes('--offline'), json = args.includes('--json');
const id = args.includes('--id') ? args[args.indexOf('--id') + 1] : null;
const output = args.includes('--output') ? args[args.indexOf('--output') + 1] : null;
const watchlist = validateWatchlist(JSON.parse(await readFile(new URL('../web/platform/catalog/firmware-watchlist.json', import.meta.url), 'utf8')));
const entries = id ? watchlist.entries.filter(entry => entry.id === id) : watchlist.entries;
if (!entries.length) throw new Error('No matching watchlist entry.');
if (offline) {
  console.log(JSON.stringify({ valid: true, entries: entries.length, policy: 'remote-links-only', checkedAt: new Date().toISOString() }));
} else {
  const reports = [];
  for (const entry of entries) {
    const report = await checkWatchEntry(entry, {fetchImpl:async (url, options) => new URL(url).protocol === 'file:' ? new Response(await readFile(new URL(url))) : fetch(url, options)});
    reports.push(report);
    if (!json) {
      console.log(`${entry.id}: ${entry.version} → ${report.latestVersion || 'unknown'}; ${report.state}${report.updateAvailable ? '; update available' : ''}`);
      for (const asset of report.assets) console.log(`  HEAD/range probe: HTTP ${asset.status ?? '?'} ${asset.state}; ${asset.bytesRead} body bytes read; ${asset.url}`);
      if (report.error) console.log(`  ${report.error.message}`);
      if (report.issueUrl) console.log(`  Review and submit an issue if needed: ${report.issueUrl}`);
    }
  }
  if (json) console.log(JSON.stringify({ checkedAt: new Date().toISOString(), reports }, null, 2));
  if (output) await writeFile(output, JSON.stringify({checkedAt:new Date().toISOString(),reports},null,2)+'\n');
  if (reports.some(report => report.state !== 'ok')) process.exitCode = 1;
}
