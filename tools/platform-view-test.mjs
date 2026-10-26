#!/usr/bin/env node
// Resource-lifecycle regression without a GPU. Browser QA also checks actual WebGL uploads.
import assert from 'node:assert/strict';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import { resizeFramebufferTexture } from '../web/platform/scene-view.js';

const canvas = { width: 466, height: 466 };
let texture = new THREE.CanvasTexture(canvas);
texture.colorSpace = THREE.SRGBColorSpace;
texture.minFilter = THREE.LinearFilter;
texture.magFilter = THREE.NearestFilter;
texture.generateMipmaps = false;
texture.flipY = false;
const material = new THREE.MeshBasicMaterial({ map: texture });
let disposed = 0;
texture.addEventListener('dispose', () => { disposed++; });

const initial = texture;
texture = resizeFramebufferTexture(canvas, texture, 466, 466, material);
assert.equal(texture, initial);
assert.equal(disposed, 0);
console.log('ok same-size frames reuse their texture and do not dispose GPU storage');

texture = resizeFramebufferTexture(canvas, texture, 468, 466, material);
assert.notEqual(texture, initial);
assert.equal(disposed, 1);
assert.equal(material.map, texture);
assert.equal(texture.image, canvas);
assert.deepEqual([canvas.width, canvas.height], [468, 466]);
assert.equal(texture.colorSpace, THREE.SRGBColorSpace);
assert.equal(texture.minFilter, THREE.LinearFilter);
assert.equal(texture.magFilter, THREE.NearestFilter);
assert.equal(texture.generateMipmaps, false);
assert.equal(texture.flipY, false);
console.log('ok first native StopWatch frame replaces storage and preserves texture settings');

const stopwatch = texture;
stopwatch.addEventListener('dispose', () => { disposed++; });
texture = resizeFramebufferTexture(canvas, texture, 480, 800, material);
assert.notEqual(texture, stopwatch);
assert.equal(disposed, 2);
assert.equal(material.map, texture);
assert.deepEqual([canvas.width, canvas.height], [480, 800]);
const paper = texture;
for (let frame = 0; frame < 100; frame++) texture = resizeFramebufferTexture(canvas, texture, 480, 800, material);
assert.equal(texture, paper);
assert.equal(disposed, 2);
console.log('ok PaperMono switching replaces the texture once; following frames reuse it');

paper.addEventListener('dispose', () => { disposed++; });
texture = resizeFramebufferTexture(canvas, texture, 468, 466, material);
assert.notEqual(texture, paper);
assert.equal(material.map, texture);
assert.equal(disposed, 3);
assert.deepEqual([canvas.width, canvas.height], [468, 466]);
texture.dispose(); material.dispose();
console.log('ok returning to StopWatch replaces the larger texture and updates the screen material');
console.log('Passed 4 framebuffer texture lifecycle checks.');
