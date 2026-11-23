#!/usr/bin/env node
// Check tracked evidence, including compressed profiles, without printing personal values.
import {execFileSync} from 'node:child_process';
import {readFileSync} from 'node:fs';
import {gunzipSync} from 'node:zlib';

const roots=process.argv.slice(2);
const files = execFileSync('git', ['ls-files', '-z', '--', ...(roots.length?roots:['docs/evidence'])], {encoding: 'utf8'}).split('\0').filter(Boolean);
if(!files.length)throw new Error('No tracked evidence matched the requested paths.');
const rules = [
  ['personal home-directory label', /\/(?:Users|home)\/(?!alice(?:[/\s"'`\\<>]|$))[^/\s"'`\\<>]+/],
  ['email or personal SSH target', /\b[\w.+-]+@[\w.-]+\.[a-z]{2,}\b/i],
  ['local hostname', /\b[\w-]+\.local\b/],
  ['personal application inventory', /Discord|Telegram|Signal(?: Beta)?\.app|zoom\.us/i],
  ['raw process inventory', /^\s*\d+\s+\d+\.\d+\s+\/(?:Applications|System)\//m],
  ['host identity field', /User ID:\s*\d+|HostKeyAlias=|\b(?:HostName|ComputerName|Serial Number|Hardware UUID):/i],
];
let failed = 0;
let compressed = 0;
for (const file of files) {
  let bytes = readFileSync(file);
  if (file.endsWith('.gz')) { bytes = gunzipSync(bytes); compressed++; }
  if (bytes.includes(0)) continue; // Binary artifacts still require manual review.
  const text = bytes.toString('utf8');
  for (const [label, pattern] of rules) {
    if (pattern.test(text)) { console.error(`${file}: ${label}`); failed++; }
  }
}
if (failed) process.exitCode = 1;
else console.log(`PASS: ${files.length} tracked evidence files checked (${compressed} gzip files); no configured privacy patterns found. Manual review is still required.`);
