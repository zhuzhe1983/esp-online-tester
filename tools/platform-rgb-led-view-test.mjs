import test from 'node:test';
import assert from 'node:assert/strict';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {applyRgbLedValues,addRgbLedGlow} from '../web/platform/rgb-led-view.js';

test('firmware RGB drives the lens, resets to its own unlit material, and clamps invalid channels', () => {
  const lens = new THREE.Mesh(new THREE.SphereGeometry(1),new THREE.MeshStandardMaterial());
  const shell = new THREE.MeshStandardMaterial({color:0xffd700});
  lens.userData.unlitColor=0x858a85;
  applyRgbLedValues([lens],[[0,255,255]]);
  assert.equal(lens.material.emissive.r,0);assert.equal(lens.material.emissive.g,lens.material.emissive.b);
  assert.ok(lens.material.emissive.g>.5&&lens.material.emissive.g<1,'full brightness keeps display headroom');
  assert.equal(shell.color.getHex(),0xffd700);
  applyRgbLedValues([lens],[]);
  assert.equal(lens.material.color.getHex(),0x858a85); assert.equal(lens.material.emissive.getHex(),0);
  applyRgbLedValues([lens],[[NaN,-2,999]]); assert.deepEqual(lens.userData.rgb,[0,0,255]);
  lens.geometry.dispose();lens.material.dispose();shell.dispose();
});

test('low PWM is visible with bounded emission, while raw values and hue are preserved',()=>{
  const lens=new THREE.Mesh(new THREE.BoxGeometry(2,2,.02),new THREE.MeshStandardMaterial()),raw=[5,2,1];
  applyRgbLedValues([lens],[raw]);
  assert.deepEqual(raw,[5,2,1]);assert.deepEqual(lens.userData.rgb,raw);
  assert.ok(lens.material.emissive.r>.1&&lens.material.emissive.r<.4,'5/255 receives visible but bounded display exposure');
  assert.ok(Math.abs(lens.material.emissive.g/lens.material.emissive.r-2/5)<1e-12);
  assert.ok(Math.abs(lens.material.emissive.b/lens.material.emissive.r-1/5)<1e-12);
  let previous=-1;
  for(let value=0;value<=255;value++){
    applyRgbLedValues([lens],[[value,0,0]]);const intensity=lens.material.emissive.r;
    assert.ok(intensity>previous&&intensity<1,'every PWM level remains ordered without clipping');previous=intensity;
  }
  lens.geometry.dispose();lens.material.dispose();
});

test('diffusion is bounded, depth-tested, ignored by input rays and off with the LED',()=>{
  const lens=new THREE.Mesh(new THREE.BoxGeometry(2,2,.02),new THREE.MeshStandardMaterial()),texture=addRgbLedGlow(lens);
  applyRgbLedValues([lens],[[0,5,0]]);
  const glow=lens.userData.ledGlow;assert.equal(glow.visible,true);assert.equal(glow.material.depthTest,true);assert.equal(glow.material.depthWrite,false);
  const hits=[];glow.raycast(new THREE.Raycaster(),hits);assert.deepEqual(hits,[]);
  const dim=glow.material.opacity;applyRgbLedValues([lens],[[0,255,0]]);assert.ok(glow.material.opacity>dim&&glow.material.opacity<=.2);
  applyRgbLedValues([lens],[]);assert.equal(glow.visible,false);assert.equal(glow.material.opacity,0);assert.equal(lens.material.emissive.getHex(),0);
  texture.dispose();glow.geometry.dispose();glow.material.dispose();lens.geometry.dispose();lens.material.dispose();
});
