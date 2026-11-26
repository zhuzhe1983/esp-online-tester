import assert from 'node:assert/strict';
import {existsSync} from 'node:fs';
import {readFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {test} from 'node:test';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {createManufacturerModel} from '../web/platform/manufacturer-model.js';
import {applyShellAppearance} from '../web/platform/shell-appearance.js';

const root = new URL('../web/platform/', import.meta.url);
const {items} = JSON.parse(await readFile(new URL('catalog/products.json', root)));
const ids = (process.env.ESP32SIM_M5_PAPER_PRODUCT_IDS || 'm5stack-k131,m5stack-k132,m5stack-k132-v11,m5stack-k152').split(',');
for (const id of ids) {
  const definitionUrl = new URL(`assets/${id}/paper-label-repair.json`, root);
  if (!existsSync(fileURLToPath(definitionUrl))) continue; // Each SKU is activated in its own commit.
  test(`${id} real paper meshes retain their materials across shell presets`, async () => {
    const previous = {fetch:globalThis.fetch, document:globalThis.document, load:THREE.TextureLoader.prototype.loadAsync};
    let model;
    try {
      globalThis.fetch = async url => {try {return new Response(await readFile(fileURLToPath(url)));} catch {return new Response('', {status:404});}};
      globalThis.document = {createElement:() => ({getContext:() => ({fillText(){}})})};
      THREE.TextureLoader.prototype.loadAsync = async url => {await readFile(fileURLToPath(url)); return new THREE.Texture();};
      const definition = JSON.parse(await readFile(definitionUrl));
      model = createManufacturerModel(items.find(p => p.id === id), new THREE.Texture());
      await model.ready;
      const shells = new Set(model.group.userData.shellMeshes);
      const labels = definition.labels.map(({part}) => model.parts[part.id]);
      assert.ok(labels.every(mesh => mesh?.isMesh), 'Every repaired label must use the real production loader');
      for (const mesh of labels) {
        assert.equal(shells.has(mesh), false, 'Printed paper cannot follow shell tint/opacity');
        assert.ok(mesh.material.map, 'Photo paper has a source texture');
        assert.ok(mesh.material.alphaTest > 0, 'Openings keep their source alpha cutouts');
      }
      const snapshots = labels.map(mesh => ({mesh, color:mesh.material.color.getHex(), opacity:mesh.material.opacity, map:mesh.material.map}));
      for (const preset of [{color:'#c4c9c2',opacity:1,finish:'plastic'},{color:'#d4af37',opacity:1,finish:'metal'},{color:'#8054b3',opacity:.35,finish:'plastic'}]) {
        for (const shell of shells) applyShellAppearance(shell, {shell:preset});
        for (const {mesh,color,opacity,map} of snapshots) {
          assert.equal(mesh.material.color.getHex(), color);
          assert.equal(mesh.material.opacity, opacity);
          assert.equal(mesh.material.map, map);
        }
      }
    } finally {
      model?.dispose();
      globalThis.fetch = previous.fetch;
      globalThis.document = previous.document;
      THREE.TextureLoader.prototype.loadAsync = previous.load;
    }
  });
}
