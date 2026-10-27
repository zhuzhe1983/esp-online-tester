#!/usr/bin/env node
// Prove the optional U136 roof did not alter pre-existing default connector geometry/materials.
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {readFile} from 'node:fs/promises';
import {createGroveSocket} from '../web/platform/grove-geometry.js';
const base=execFileSync('git',['show','d3c7820f:web/platform/grove-geometry.js'],{encoding:'utf8'});
const previous=await import('data:text/javascript,'+encodeURIComponent(base.replace("'./vendor/three.module.min.js'",JSON.stringify(new URL('../web/platform/vendor/three.module.min.js',import.meta.url).href))));
const describe=group=>group.children.map(x=>({name:x.name,position:x.position.toArray(),rotation:x.rotation.toArray(),attributes:Object.fromEntries(Object.entries(x.geometry.attributes).map(([k,a])=>[k,Array.from(a.array)])),index:x.geometry.index?Array.from(x.geometry.index.array):null,material:{color:x.material.color.getHex(),metalness:x.material.metalness,roughness:x.material.roughness,side:x.material.side}}));
for(const options of [undefined,{color:0xb5212d}])assert.deepEqual(describe(createGroveSocket(options)),describe(previous.createGroveSocket(options)));
const open=createGroveSocket({housingVariant:'open-roof'});assert.equal(open.getObjectByName('socket-top'),undefined);assert.equal(open.children.filter(x=>x.name==='socket-top-rail').length,2);assert.equal(open.getObjectByName('socket-top-rear').geometry.parameters.depth,1.4);
const sha=x=>createHash('sha256').update(x).digest('hex');
console.log(JSON.stringify({base:'d3c7820f',previousRendererSha256:sha(base),rendererSha256:sha(await readFile(new URL('../web/platform/grove-geometry.js',import.meta.url))),check:'Default and custom-color closed-roof child geometry, indices, transforms and materials are identical to the previous implementation; only explicit open-roof uses new geometry.'},null,2));
