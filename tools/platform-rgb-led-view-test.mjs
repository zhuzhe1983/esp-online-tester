import test from 'node:test';
import assert from 'node:assert/strict';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {applyRgbLedValues} from '../web/platform/rgb-led-view.js';

test('firmware RGB drives the lens, resets to its own unlit material, and clamps invalid channels', () => {
  const lens = new THREE.Mesh(new THREE.SphereGeometry(1),new THREE.MeshStandardMaterial());
  const shell = new THREE.MeshStandardMaterial({color:0xffd700});
  lens.userData.unlitColor=0x858a85;
  applyRgbLedValues([lens],[[0,255,255]]);
  assert.equal(lens.material.color.getHex(),0x00ffff); assert.equal(lens.material.emissive.getHex(),0x00ffff);
  assert.equal(shell.color.getHex(),0xffd700);
  applyRgbLedValues([lens],[]);
  assert.equal(lens.material.color.getHex(),0x858a85); assert.equal(lens.material.emissive.getHex(),0);
  applyRgbLedValues([lens],[[NaN,-2,999]]); assert.deepEqual(lens.userData.rgb,[0,0,255]);
  lens.geometry.dispose();lens.material.dispose();shell.dispose();
});
