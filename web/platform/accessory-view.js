import * as THREE from './vendor/three.module.min.js';
import {createGpsUnitModel} from './gps-model.js';
import {createGrovePlug, verifiedGrove, GROVE_PITCH, GROVE_WIRE_EXIT, GROVE_SIGNALS} from './grove-geometry.js';

const material = (color, options = {}) => new THREE.MeshStandardMaterial({color, roughness: .72, ...options});
function box(w, h, d, mat, x = 0, y = 0, z = 0) { const mesh = new THREE.Mesh(new THREE.BoxGeometry(w, h, d), mat); mesh.position.set(x, y, z); return mesh; }

export function connectorFrame(connector, relativeTo) {
  connector.anchor.updateWorldMatrix(true, false); relativeTo.updateWorldMatrix(true, false);
  const matrix = relativeTo.matrixWorld.clone().invert().multiply(connector.anchor.matrixWorld);
  return {matrix, position: new THREE.Vector3().setFromMatrixPosition(matrix), normal: connector.normal.clone().transformDirection(matrix), tangent: new THREE.Vector3(1, 0, 0).transformDirection(matrix)};
}

export function createIllustrativeAccessory(definition, index = 0) {
  if (definition.id === 'm5stack-gps-u032-v11' || definition.catalogId === 'm5stack-gps-u032-v11') {
    const model = createGpsUnitModel(); model.group.name = `accessory:${definition.instanceId || definition.id || index}`;
    model.group.position.set(64 + index * 34, -9 - index * 5, 0); model.group.rotation.z = -Math.PI / 4; return model;
  }
  const group = new THREE.Group(); group.name = `accessory:${definition.instanceId || definition.id || index}`;
  group.userData.description = 'GPS/配件为示意外观；逻辑 UART 输入不模拟物理接线、供电或波特率。';
  group.position.set(62 + index * 30, -9 - index * 5, 0); group.rotation.z = -Math.PI / 4;
  const shell = box(16, 23, 6, material(0xe4e5d9)); shell.name = 'accessory-shell'; group.add(shell);
  const antenna = box(9, 10, 1, material(definition.type === 'gps' ? 0xbfc497 : 0x4d715b), 0, 2, 3.7); antenna.name = 'illustrative-component'; group.add(antenna);
  if (definition.connector?.type !== 'HY2.0-4P') return {group, connector: null};
  const socket = new THREE.Group(); socket.name = 'accessory-hy2-4p'; socket.position.set(0, -11.5, 0); socket.rotation.x = Math.PI / 2;
  const white = material(0xe9e8e0), dark = material(0x202722);
  socket.add(box(7.8, 3.4, .15, dark, 0, 0, .05), box(8.3, .6, .7, white, 0, 1.9, 0), box(8.3, .6, .7, white, 0, -1.9, 0), box(.6, 4.2, .7, white, -4, 0, 0), box(.6, 4.2, .7, white, 4, 0, 0));
  const anchor = new THREE.Object3D(); anchor.name = 'connector:accessory:grove:mouth'; anchor.position.z = .4;
  anchor.userData = {kind: 'connector-anchor', type: 'HY2.0-4P', normal: [0, 0, 1], visualOnly: true}; socket.add(anchor); group.add(socket);
  return {group, connector: {type: 'HY2.0-4P', anchor, normal: new THREE.Vector3(0, 0, 1)}};
}

export function createGroveCable(host, accessory, relativeTo) {
  if (!verifiedGrove(host) || !verifiedGrove(accessory)) return null;
  const from = connectorFrame(host, relativeTo), to = connectorFrame(accessory, relativeTo);
  const group = new THREE.Group(); group.name = 'grove-cable-visual-only';
  group.userData.description = 'Grove 插头按模型口位置装配；线色仅作外观示意，不证明电气连接。';
  group.add(createGrovePlug(from, 'plug:device'), createGrovePlug(to, 'plug:accessory'));
  const colors = [0x272b29, 0xbb594e, 0xd3b549, 0xd9dcd3], ids = GROVE_SIGNALS, curves = [];
  const hostOrder = host.pinOrder, accessoryOrder = accessory.pinOrder;
  for (let i = 0; i < 4; i++) {
    const fromOffset = (hostOrder.indexOf(ids[i]) - 1.5) * GROVE_PITCH, toOffset = (accessoryOrder.indexOf(ids[i]) - 1.5) * GROVE_PITCH;
    const a = from.position.clone().addScaledVector(from.normal, GROVE_WIRE_EXIT).addScaledVector(from.tangent, fromOffset);
    const d = to.position.clone().addScaledVector(to.normal, GROVE_WIRE_EXIT).addScaledVector(to.tangent, toOffset);
    const standOff = Math.min(15, Math.max(8, a.distanceTo(d) * .5));
    const curve = new THREE.CubicBezierCurve3(a, a.clone().addScaledVector(from.normal, standOff), d.clone().addScaledVector(to.normal, standOff), d);
    const wire = new THREE.Mesh(new THREE.TubeGeometry(curve, 320, .45, 12, false), material(colors[i])); wire.name = `wire:${ids[i]}`; wire.userData={signal:ids[i],fromPinIndex:hostOrder.indexOf(ids[i]),toPinIndex:accessoryOrder.indexOf(ids[i])};group.add(wire); curves.push(curve);
  }
  group.userData.endpoints = {from: from.position.toArray(), to: to.position.toArray(), fromNormal: from.normal.toArray(), toNormal: to.normal.toArray(), plugLength: GROVE_WIRE_EXIT};
  return {group, curves, from, to};
}
