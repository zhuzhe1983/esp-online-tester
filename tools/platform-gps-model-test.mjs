#!/usr/bin/env node
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createGpsUnitModel} from '../web/platform/gps-model.js';
import {createIllustrativeAccessory, connectorFrame, createGroveCable} from '../web/platform/accessory-view.js';
import {validateKnowledge} from './platform-knowledge-test.mjs';
const base = new URL('../web/platform/assets/m5stack-gps-u032-v11/', import.meta.url), sha = value => createHash('sha256').update(value).digest('hex');
const metadata = JSON.parse(await readFile(new URL('model.json', base))), record = JSON.parse(await readFile(new URL('../web/platform/knowledge/m5stack-gps-u032-v11.json', import.meta.url)));
validateKnowledge(record); assert.equal(record.kind, 'accessory'); assert.equal(record.identity.sku, 'U032-V11'); assert.equal(record.identity.controller, 'AT6668');
assert.ok(record.unknowns.some(x => x.id === 'pcb-layout' && x.value === null)); assert.ok(record.unknowns.some(x => x.id === 'schematic-revision'));
assert.equal(metadata.provenance.sourceSha256, 'cfb75ed7e65c36ab005d88beb4a81c293a2f07edffd2a9dd13ec14ce8b1a9b0b');
for (const [file, hash] of Object.entries(metadata.artifacts)) assert.equal(sha(await readFile(new URL(file, base))), hash, `${file} source/artifact hash changed`);
assert.deepEqual(metadata.parts.map(x => x.sourceComponent), [0, 3]); assert.equal(metadata.sourceTransform.scale, 1); assert.equal(metadata.unit, 'mm');
assert.deepEqual(metadata.sourceTransform.matrixRows, [[1, 0, 0, 40], [0, 1, 0, 0], [0, 0, 1, -1.2], [0, 0, 0, 1]]);
const nativeFetch = globalThis.fetch;
try {
 globalThis.fetch = async (url, options) => { assert.ok(options.signal); const bytes = await readFile(url); return new Response(bytes); };
 const root = new THREE.Group(), model = createGpsUnitModel(); root.add(model.group);
 const before = connectorFrame(model.connector, root); await model.ready; const after = connectorFrame(model.connector, root);
 assert.deepEqual(before.position.toArray(), after.position.toArray()); assert.ok(after.position.distanceTo(new THREE.Vector3(0, -24, 1.4)) < 1e-6); assert.ok(after.normal.distanceTo(new THREE.Vector3(0, -1, 0)) < 1e-6);
 const bounds = new THREE.Box3(); for (const id of ['shell-front', 'shell-back']) bounds.union(new THREE.Box3().setFromObject(model.parts[id]));
 const size = bounds.getSize(new THREE.Vector3()); assert.ok(size.distanceTo(new THREE.Vector3(24, 48, 8)) < .0001);
 assert.equal(model.group.getObjectByName('gps-loading-fallback'), undefined); assert.equal(model.parts['pcb-main'].userData.label, 'UNKNOWN'); assert.deepEqual(model.parts['pcb-main'].userData.verifiedComponents, []);
 const anchor = new THREE.Object3D(); anchor.position.set(-20, -45, 1); root.add(anchor); const cable = createGroveCable({type:'HY2.0-4P',allowConnections:true,sourceIds:['test-fixture'],pinOrder:['gnd','vcc','signal1','signal2'],anchor, normal: new THREE.Vector3(0, 1, 0)}, model.connector, root);
 for (const curve of cable.curves) assert.ok(curve.getTangent(1).dot(cable.to.normal) < -.999999);
 let disposed = 0; model.parts['shell-front'].geometry.addEventListener('dispose', () => disposed++); model.dispose(); model.dispose(); assert.equal(disposed, 1);
 const selected = createIllustrativeAccessory({id: 'm5stack-gps-u032-v11', instanceId: 'test-unit', type: 'gps', connector: {type: 'HY2.0-4P'}}); await selected.ready; assert.ok(selected.parts['shell-front']); selected.dispose();
 const legacy = createIllustrativeAccessory({id: 'm5stack-gps-u032', type: 'gps', connector: {type: 'HY2.0-4P'}}); assert.equal(legacy.ready, undefined, 'Legacy U032 must never silently reuse v1.1 CAD');
 globalThis.fetch = async () => new Response('missing', {status: 404}); const missing = createGpsUnitModel(); await assert.rejects(missing.ready, /HTTP 404/); assert.ok(missing.group.getObjectByName('gps-loading-fallback')); assert.ok(missing.connector.anchor); missing.dispose();
 globalThis.fetch = (_url, {signal}) => new Promise((_resolve, reject) => signal.addEventListener('abort', () => reject(new DOMException('Disposed', 'AbortError')), {once: true})); const aborted = createGpsUnitModel(); aborted.dispose(); await assert.rejects(aborted.ready, {name: 'AbortError'});
} finally { globalThis.fetch = nativeFetch; }
const report = JSON.parse(await readFile(new URL('../docs/platform/evidence/gps-model/verification.json', import.meta.url)));
for (const [field, path] of [['modelSha256', '../web/platform/assets/m5stack-gps-u032-v11/model.json'], ['geometrySha256', '../web/platform/assets/m5stack-gps-u032-v11/geometry.bin'], ['connectorRendererSha256','../web/platform/grove-geometry.js'],['rendererSha256', '../web/platform/gps-model.js'], ['reviewSha256', '../web/platform/gps-model-review.js'], ['verificationScriptSha256', './verify-gps-model.py']]) assert.equal(report[field], sha(await readFile(new URL(path, import.meta.url))), `Re-run GPS verification after changing ${path}`);
assert.equal(report.dimensions.length, 10); assert.ok(report.dimensions.every(x => x.pass));
assert.ok(report.photos.filter(x => ['front', 'back'].includes(x.view)).every(x => x.acceptance.pass));
assert.ok(report.photos.filter(x => ['oblique', 'rear'].includes(x.view)).every(x => x.acceptance.status === 'qualitative-only'));
console.log('ok GPS v1.1 pinned sources, knowledge boundaries, 24×48×8 mm assembly, stable Grove anchor/cable, SKU isolation, fallback and disposal');
