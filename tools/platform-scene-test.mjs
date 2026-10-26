import assert from 'node:assert/strict';
import * as THREE from '../web/platform/vendor/three.module.min.js';
import {pickVisibleSurface, createScenePointerRouter, createSurfaceHalo} from '../web/platform/scene-interaction.js';
import {createIllustrativeAccessory, createGroveCable, connectorFrame} from '../web/platform/accessory-view.js';
import {normalizeShellAppearance, applyShellAppearance} from '../web/platform/shell-appearance.js';
import {GROVE_INSERTION, GROVE_PITCH, createGroveSocket} from '../web/platform/grove-geometry.js';
import {SHELL_FINISHES, SHELL_PRESETS, shellPreset} from '../web/platform/appearance.js';
const checks = [];
function check(name, fn) { fn(); checks.push(name); console.log(`ok ${name}`); }
const near = (a, b) => assert.ok(Math.abs(a - b) < 1e-6, `${a} != ${b}`);
const cube = (z, color = 'white') => { const m = new THREE.Mesh(new THREE.BoxGeometry(4, 4, 1), new THREE.MeshStandardMaterial({color})); m.position.z = z; return m; };
check('full assembly picking blocks hidden buttons, including an optically invisible physical shell', () => {
  const root = new THREE.Group(), shell = cube(2), button = cube(0); button.userData.control = 'button-a'; root.add(button, shell);
  const ray = new THREE.Raycaster(new THREE.Vector3(0, 0, 10), new THREE.Vector3(0, 0, -1));
  assert.equal(pickVisibleSurface(ray, root).object, shell);
  shell.material.opacity = 0; shell.userData.inputOccluder = true; assert.equal(pickVisibleSurface(ray, root).object, shell);
  shell.visible = false; assert.equal(pickVisibleSurface(ray, root).object, button);
  button.parent.visible = false; assert.equal(pickVisibleSurface(ray, root), null);
});
check('local hover contours leave material and framebuffer untouched and are excluded from picking', () => {
  const root = new THREE.Group(), screen = cube(0), halo = createSurfaceHalo(); root.add(screen);
  const material = screen.material; halo.show(screen, 'touch'); assert.equal(halo.kind, 'touch'); assert.equal(screen.material, material); assert.equal(screen.children[0].userData.pickIgnore, true);
  const ray = new THREE.Raycaster(new THREE.Vector3(0, 0, 10), new THREE.Vector3(0, 0, -1)); assert.equal(pickVisibleSurface(ray, root).object, screen);
  halo.clear(); assert.equal(screen.children.length, 0);
});
check('A and B can stay down together; each pointer releases only its own input', () => {
  const events = [], router = createScenePointerRouter({onInput: (e, source) => events.push({...e, source})});
  router.down({pointerId: 1, x: 0, y: 0, surface: {kind: 'button', id: 'button-a'}});
  router.down({pointerId: 2, x: 0, y: 0, surface: {kind: 'button', id: 'button-b'}});
  assert.equal(router.activeCount, 2); assert.deepEqual(events.map(e => [e.id, e.pressed, e.source]), [['button-a', true, 'scene:1'], ['button-b', true, 'scene:2']]);
  router.up(1); assert.equal(router.kind(2), 'button'); assert.equal(events.at(-1).id, 'button-a'); router.releaseAll(); assert.equal(events.at(-1).id, 'button-b'); assert.equal(events.at(-1).pressed, false);
});
check('touch and orbit keep separate ownership for their entire gesture', () => {
  const events = [], moves = [], router = createScenePointerRouter({onInput: e => events.push(e), onOrbit: (...v) => moves.push(v)});
  router.down({pointerId: 1, x: 0, y: 0, surface: {kind: 'touch'}, point: {x: 20, y: 30}});
  assert.equal(router.down({pointerId: 2, x: 0, y: 0, surface: {kind: 'touch'}, point: {x: 0, y: 0}}), 'ignored');
  router.move({pointerId: 1, x: 100, y: 50, point: {x: 99, y: 50}}); router.up(1); assert.equal(moves.length, 0); assert.equal(events.at(-1).down, false);
  const count = events.length; router.down({pointerId: 3, x: 0, y: 0, surface: {kind: 'orbit'}}); router.move({pointerId: 3, x: 25, y: 15, point: {x: 10, y: 10}}); router.up(3);
  assert.equal(events.length, count); assert.deepEqual(moves, [[25, 15]]);
});
check('unsupported power control explains the limitation without emitting GPIO', () => {
  const events = [], notices = [], router = createScenePointerRouter({onInput: e => events.push(e), onUnsupported: x => notices.push(x)});
  router.down({pointerId: 1, surface: {kind: 'unsupported', message: 'power unavailable'}}); router.up(1);
  assert.deepEqual(notices, ['power unavailable']); assert.equal(events.length, 0);
});
check('Grove plugs mate in connector coordinates and wires depart along both outward normals', () => {
  const root = new THREE.Group(); root.rotation.set(.3, -.4, .2); const socket = new THREE.Group(); socket.position.set(17.55, -17.55, .55);
  socket.setRotationFromMatrix(new THREE.Matrix4().makeBasis(new THREE.Vector3(Math.SQRT1_2, Math.SQRT1_2, 0), new THREE.Vector3(0, 0, 1), new THREE.Vector3(Math.SQRT1_2, -Math.SQRT1_2, 0)));
  const anchor = new THREE.Object3D(); anchor.position.z = .65; socket.add(anchor); root.add(socket);
  const host = {type:'HY2.0-4P',allowConnections:true,sourceIds:['test-fixture'],pinOrder:['gnd','vcc','signal1','signal2'],anchor, normal: new THREE.Vector3(0, 0, 1)}, accessory = createIllustrativeAccessory({id: 'gps', type: 'gps', connector: {type: 'HY2.0-4P'}}); root.add(accessory.group);Object.assign(accessory.connector,{allowConnections:true,sourceIds:['test-fixture'],pinOrder:['gnd','vcc','signal1','signal2']});
  const frame = connectorFrame(host, root); near(frame.position.x, 17.55 + .65 * Math.SQRT1_2); near(frame.position.y, -17.55 - .65 * Math.SQRT1_2); near(frame.normal.x, Math.SQRT1_2); near(frame.normal.y, -Math.SQRT1_2);
  const cable = createGroveCable(host, accessory.connector, root); root.add(cable.group);
  const plug = cable.group.getObjectByName('plug:device'),nose=plug.getObjectByName('inserted-nose'),stop=plug.getObjectByName('plug-stop');nose.geometry.computeBoundingBox();stop.geometry.computeBoundingBox();near(nose.geometry.boundingBox.min.z,-GROVE_INSERTION);near(nose.geometry.boundingBox.max.z,0);assert.ok(stop.geometry.boundingBox.min.z+stop.position.z>=0,'stop remains outside socket');
  const cavity=createGroveSocket();const ray=new THREE.Raycaster(new THREE.Vector3(0,0,1),new THREE.Vector3(0,0,-1));cavity.updateMatrixWorld();assert.ok(ray.intersectObject(cavity,true)[0].point.z<-4.7,'no dark block across the mating cavity');
  for (const curve of cable.curves) { near(curve.getTangent(0).dot(cable.from.normal), 1); near(curve.getTangent(1).dot(cable.to.normal), -1); for (const point of curve.getPoints(80)) assert.ok(Math.hypot(point.x, point.y) > 28, 'wire must not cut through the round host shell'); }
});
check('Grove wire order follows each socket label independently after rotation', () => {
  const root = new THREE.Group(); root.rotation.set(.4, -.3, .2);
  const a = new THREE.Object3D(), b = new THREE.Object3D(); a.position.set(30, -30, 0); a.rotation.z = .7; b.position.set(75, -25, 4); b.rotation.y = Math.PI; root.add(a,b);
  const host = {type:'HY2.0-4P',allowConnections:true,sourceIds:['test-fixture'],anchor:a,normal:new THREE.Vector3(0,0,1),pinOrder:['signal2','signal1','vcc','gnd']}, accessory = {type:'HY2.0-4P',allowConnections:true,sourceIds:['test-fixture'],anchor:b,normal:new THREE.Vector3(0,0,1),pinOrder:['gnd','vcc','signal1','signal2']};
  const cable = createGroveCable(host,accessory,root), signals = ['gnd','vcc','signal1','signal2'];
  for (const [i,signal] of signals.entries()) {
    const wire = cable.group.getObjectByName(`wire:${signal}`), curve = cable.curves[i];
    assert.equal(wire.userData.fromPinIndex,3-i); assert.equal(wire.userData.toPinIndex,i);
    near(curve.getPoint(0).sub(cable.from.position).dot(cable.from.tangent),(3-i-1.5)*GROVE_PITCH);
    near(curve.getPoint(1).sub(cable.to.position).dot(cable.to.tangent),(i-1.5)*GROVE_PITCH);
  }
});
check('unverified connectors never fall back to an assumed Grove wire order', () => {
  const root=new THREE.Group(),anchor=new THREE.Object3D();root.add(anchor);const known={type:'HY2.0-4P',anchor,normal:new THREE.Vector3(0,0,1),allowConnections:true,sourceIds:['test'],pinOrder:['gnd','vcc','signal1','signal2']};
  for(const bad of [{...known,allowConnections:false},{...known,pinOrder:undefined},{...known,sourceIds:[]},{...known,pinOrder:['gnd','gnd','signal1','signal2']}]){assert.equal(createGroveCable(bad,known,root),null);assert.equal(createGroveCable(known,bad,root),null);}
});
check('appearance changes only shell and restores opaque occlusion; invalid values reject', () => {
  const shell = cube(0), screen = cube(1, 'red'), button = cube(2, 'yellow'), occluder = cube(-1), before = [screen.material.color.getHex(), button.material.color.getHex()];
  applyShellAppearance(shell, normalizeShellAppearance({shell: {color: '#663399', opacity: .25, finish: 'plastic'}}), {occluder});
  near(shell.material.opacity, .25); assert.equal(shell.material.transparent, true); assert.equal(occluder.visible, false); assert.equal(shell.userData.inputOccluder, true); assert.deepEqual([screen.material.color.getHex(), button.material.color.getHex()], before);
  applyShellAppearance(shell, normalizeShellAppearance(null), {occluder}); assert.equal(shell.material.color.getHexString(), '080a0b'); assert.equal(occluder.visible, true); assert.equal(shell.material.transparent, false);
  applyShellAppearance(shell, normalizeShellAppearance({shell: {color: '#C8A65A', finish: 'metal'}})); assert.equal(shell.material.metalness, .78); assert.equal(shell.material.color.getHexString(), 'c8a65a');
  for (const shell of [{color: 'red'}, {opacity: -1}, {opacity: Infinity}, {finish: 'glass'}]) assert.throws(() => normalizeShellAppearance({shell}));
});
check('eight finishes have distinct optical responses while color and opacity remain independent', () => {
  const shell = cube(0), occluder = cube(-1), signatures = new Set(), materials = {};
  for (const {id} of SHELL_FINISHES) {
    const value = normalizeShellAppearance({shell: {color: '#A376CA', opacity: .37, finish: id}});
    applyShellAppearance(shell, value, {occluder});
    const m = shell.material;
    assert.equal(m.isMeshPhysicalMaterial, true); assert.equal(m.color.getHexString(), 'a376ca'); near(m.opacity, .37);
    assert.equal(value.shell.finish, id); assert.equal(occluder.visible, false); assert.equal(shell.userData.inputOccluder, true);
    materials[id] = {roughness: m.roughness, metalness: m.metalness, transmission: m.transmission, clearcoat: m.clearcoat, specularIntensity: m.specularIntensity};
    signatures.add(JSON.stringify(materials[id]));
  }
  assert.equal(signatures.size, 8);
  assert.ok(materials['glossy-plastic'].roughness < materials.plastic.roughness);
  assert.ok(materials.rubber.roughness > materials.plastic.roughness && materials.rubber.specularIntensity < materials.plastic.specularIntensity);
  assert.ok(materials['clear-resin'].transmission > materials['frosted-plastic'].transmission && materials['frosted-plastic'].roughness > materials['clear-resin'].roughness);
  assert.ok(materials['brushed-metal'].metalness > .7 && materials['brushed-metal'].roughness > materials.metal.roughness);
  assert.ok(materials.ceramic.clearcoat > 0 && materials.ceramic.metalness === 0);
  for (const finish of ['clear-resin', 'frosted-plastic']) {
    applyShellAppearance(shell, normalizeShellAppearance({shell: {finish, opacity: 1}}), {occluder});
    assert.equal(shell.material.opacity, 1); assert.ok(shell.material.transmission > 0); assert.equal(occluder.visible, false);
  }
  applyShellAppearance(shell, normalizeShellAppearance(null), {occluder});
  near(shell.material.roughness, .66); near(shell.material.metalness, 0); near(shell.material.transmission, 0); near(shell.material.clearcoat, 0);
  assert.equal(shell.material.depthWrite, true); assert.equal(shell.material.side, THREE.FrontSide); assert.equal(occluder.visible, true);
  for (const [id, preset] of Object.entries(SHELL_PRESETS)) assert.equal(shellPreset(preset), id);
  shell.material.dispose();
});
check('shell material isolation preserves shared screen/button/port/halo resources and original geometry', () => {
  const root = new THREE.Group(), shell = cube(0), screen = cube(1), button = cube(2), port = cube(3), halo = cube(4);
  screen.material = shell.material; root.add(shell, screen, button, port, halo);
  const geometry = shell.geometry, source = shell.material, surfaces = [screen, button, port, halo];
  const originals = surfaces.map(mesh => ({material: mesh.material, color: mesh.material.color.getHex(), roughness: mesh.material.roughness}));
  let sourceDisposals = 0; source.addEventListener('dispose', () => sourceDisposals++);
  let owned;
  for (const {id} of SHELL_FINISHES) {
    applyShellAppearance(shell, normalizeShellAppearance({shell: {finish: id}}));
    owned ||= shell.material; assert.equal(shell.material, owned); assert.notEqual(owned, source); assert.equal(shell.geometry, geometry);
    surfaces.forEach((mesh, i) => { assert.equal(mesh.material, originals[i].material); assert.equal(mesh.material.color.getHex(), originals[i].color); assert.equal(mesh.material.roughness, originals[i].roughness); });
  }
  owned.dispose(); assert.equal(sourceDisposals, 0, 'shared input material belongs to other surfaces');
});
check('procedural brushing needs no UVs or textures and releases its owned material hooks', () => {
  const shell = cube(0); shell.geometry.deleteAttribute('uv');
  let originalDisposals = 0; shell.material.addEventListener('dispose', () => originalDisposals++);
  applyShellAppearance(shell, normalizeShellAppearance({shell: {finish: 'brushed-metal'}}));
  const material = shell.material, brushedKey = material.customProgramCacheKey();
  assert.equal(originalDisposals, 1, 'replaced exclusive material is released');
  assert.equal(shell.geometry.getAttribute('uv'), undefined); assert.equal(material.roughnessMap, null); assert.equal(material.normalMap, null);
  const pristine = {vertexShader: THREE.ShaderLib.physical.vertexShader, fragmentShader: THREE.ShaderLib.physical.fragmentShader};
  const brushed = {...pristine}; material.onBeforeCompile(brushed);
  assert.notEqual(brushed.vertexShader, pristine.vertexShader); assert.notEqual(brushed.fragmentShader, pristine.fragmentShader);
  applyShellAppearance(shell, normalizeShellAppearance(null));
  assert.equal(shell.material, material); assert.notEqual(material.customProgramCacheKey(), brushedKey);
  const restored = {...pristine}; material.onBeforeCompile(restored); assert.deepEqual(restored, pristine);
  applyShellAppearance(shell, normalizeShellAppearance({shell: {finish: 'brushed-metal'}}));
  material.dispose(); const afterDispose = {...pristine}; material.onBeforeCompile(afterDispose); assert.deepEqual(afterDispose, pristine);
});
check('direct shell normalization rejects unknown fields and invalid shapes instead of accepting textures or scripts', () => {
  for (const value of [[], 1, 'plastic', {shell: null}, {shell: []}, {other: 1}, {shell: {finish: null}}, {shell: {opacity: null}}, {shell: {color: null}}, {shell: {texture: 'remote.png'}}]) assert.throws(() => normalizeShellAppearance(value));
  assert.deepEqual(normalizeShellAppearance({}, {shell: {color: '#E5E5D9', opacity: 1, finish: 'plastic'}}), {shell: {color: '#e5e5d9', opacity: 1, finish: 'plastic'}});
});
console.log(`Passed ${checks.length} scene interaction/geometry checks (synthetic geometry, no firmware execution claims).`);
