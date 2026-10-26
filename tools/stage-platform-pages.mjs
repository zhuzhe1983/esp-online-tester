#!/usr/bin/env node
// Tests use the repository's fixtures; Pages receives a separate public tree.
import {cp, mkdir, readdir, rm, access, readFile} from 'node:fs/promises';
import {resolve, join, relative} from 'node:path';
const [sourceArg = 'web', destinationArg = 'target/pages'] = process.argv.slice(2);
const source = resolve(sourceArg), destination = resolve(destinationArg);
if (destination === source || !relative(source, destination).startsWith('..')) throw new Error('Publish destination must be outside the source tree.');
let exists = false;
try { await access(destination); exists = true; } catch (error) { if (error.code !== 'ENOENT') throw error; }
if (exists) throw new Error('Use a new staging directory; this command does not overwrite an existing tree.');
for (const [name, forbidden] of [['products', ['generic-esp32s3']], ['firmwares', ['platform-stopwatch-diagnostic', 'esp32sim-s3-hello']]]) {
  const catalog = JSON.parse(await readFile(join(source, 'platform/catalog', `${name}.json`), 'utf8'));
  if (catalog.items.some(item => forbidden.includes(item.id))) throw new Error(`Internal test fixtures are still listed in public ${name}.`);
}
await mkdir(destination, {recursive: true});
await cp(source, destination, {recursive: true});
const excluded = ['wasm/fw/demo-stopwatch.json'];
for (const name of await readdir(join(destination, 'wasm/fw/public'))) if (name.startsWith('platform-stopwatch')) excluded.push(`wasm/fw/public/${name}`);
for (const path of excluded) await rm(join(destination, path), {force: true});
for (const path of excluded) {
  let present = false;
  try { await access(join(destination, path)); present = true; } catch (error) { if (error.code !== 'ENOENT') throw error; }
  if (present) throw new Error(`Test fixture leaked into public output: ${path}`);
}
console.log(JSON.stringify({publicSite: destinationArg, excludedInternalFixtures: excluded}, null, 2));
