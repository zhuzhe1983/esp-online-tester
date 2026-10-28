import test from 'node:test';
import assert from 'node:assert/strict';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {applyRgbLedValues,addRgbLedGlow} from '../web/platform/rgb-led-view.js';

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

test('low PWM stays emissive; diffusion is bounded, depth-tested and off with the LED',()=>{
  const lens=new THREE.Mesh(new THREE.BoxGeometry(2,2,.02),new THREE.MeshStandardMaterial()),texture=addRgbLedGlow(lens);
  applyRgbLedValues([lens],[[0,19,0]]);
  assert.equal(lens.material.emissive.g,19/255,'PWM must not be decoded again as sRGB');
  const glow=lens.userData.ledGlow;assert.equal(glow.visible,true);assert.equal(glow.material.depthTest,true);assert.equal(glow.material.depthWrite,false);
  const dim=glow.material.opacity;applyRgbLedValues([lens],[[0,255,0]]);assert.ok(glow.material.opacity>dim&&glow.material.opacity<=.35);
  applyRgbLedValues([lens],[]);assert.equal(glow.visible,false);assert.equal(glow.material.opacity,0);assert.equal(lens.material.emissive.getHex(),0);
  texture.dispose();glow.geometry.dispose();glow.material.dispose();lens.geometry.dispose();lens.material.dispose();
});
