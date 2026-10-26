// U032-V11 only. Manufacturer CAD in mm; provenance and limitations live beside the asset.
import * as THREE from './vendor/three.module.min.js';
import {createGroveSocket} from './grove-geometry.js';
const base = new URL('./assets/m5stack-gps-u032-v11/', import.meta.url);
const material = (color, options = {}) => new THREE.MeshStandardMaterial({color, roughness: .62, ...options});
function box(w, h, d, mat, x = 0, y = 0, z = 0) { const mesh = new THREE.Mesh(new THREE.BoxGeometry(w, h, d), mat); mesh.position.set(x, y, z); return mesh; }
export function createGpsUnitModel() {
  const group = new THREE.Group(); group.name = 'm5stack-gps-u032-v11';
  group.userData.description = 'U032-V11 厂家外壳 CAD；PCB UNKNOWN。配件功能为逻辑 UART1 NMEA 输入。';
  const parts = {}, textures = new Set(), aborter = new AbortController(); let disposed = false;
  const fallback = box(24, 48, 8, material(0xe8e9e4)); fallback.name = 'gps-loading-fallback'; fallback.userData.description = 'U032-V11 外形尺寸占位；厂家 CAD 尚未加载。'; group.add(fallback);
  const pcb = new THREE.Group(); pcb.name = 'pcb-main'; pcb.position.set(0, 6, 0); pcb.userData = {kind: 'pcb-placeholder', label: 'UNKNOWN', estimated: {dimensionsMm: [20, 28, 1], centerMm: [0, 6, 0]}, verifiedComponents: []};
  pcb.add(box(20, 28, 1, material(0x89928c))); parts['pcb-main'] = pcb; group.add(pcb);
  if (typeof document !== 'undefined') {
    const canvas = document.createElement('canvas'); canvas.width = 512; canvas.height = 128; const context = canvas.getContext('2d');
    context.fillStyle = '#ffffff'; context.font = 'bold 76px monospace'; context.textAlign = 'center'; context.textBaseline = 'middle'; context.fillText('UNKNOWN', 256, 64);
    const texture = new THREE.CanvasTexture(canvas); texture.colorSpace = THREE.SRGBColorSpace; textures.add(texture);
    for (const back of [false, true]) { const label = new THREE.Mesh(new THREE.PlaneGeometry(18, 4.5), new THREE.MeshBasicMaterial({map: texture, transparent: true, depthWrite: false})); label.name = 'pcb-unknown-label'; label.position.z = back ? -.515 : .515; if (back) label.rotation.y = Math.PI; pcb.add(label); }
  }
  // Opening 10 x 5.2 mm is dimensioned. Hidden insert details are photo-informed estimates.
  const socket = createGroveSocket({color:0x00a9c4}); socket.name = 'grove-insert'; socket.position.set(0, -24, 1.4); socket.rotation.x = Math.PI / 2;
  const metal = material(0xb7b8a8, {metalness:.7,roughness:.3});
  const anchor = new THREE.Object3D(); anchor.name = 'connector:accessory:grove:mouth'; anchor.position.z = 0; anchor.userData = {kind: 'connector-anchor', type: 'HY2.0-4P', normal: [0, 0, 1], visualOnly: true}; socket.add(anchor); group.add(socket); parts.grove = socket;
  // Back label left-to-right is white TXD, yellow RXD, red 5V, black GND; back view reverses world X.
  const connector = {type: 'HY2.0-4P', anchor, normal: new THREE.Vector3(0, 0, 1),allowConnections:true,sourceIds:['official-docs','photo-6'],pinOrder:['gnd','vcc','signal1','signal2']};
  const screw = new THREE.Mesh(new THREE.CylinderGeometry(1.85, 1.85, .28, 32), metal); screw.rotation.x = Math.PI / 2; screw.position.set(0, -12, -3.9); screw.name = 'rear-fastener'; group.add(screw); parts.fastener = screw;
  const recess = new THREE.Mesh(new THREE.CircleGeometry(.8, 6), material(0x343a3b)); recess.rotation.y = Math.PI; recess.position.set(0, -12, -4.055); group.add(recess);
  const releaseObject = object => object.traverse(child => { child.geometry?.dispose(); if (child.material) for (const m of [].concat(child.material)) m.dispose(); });
  const ready = (async () => {
    const response = await fetch(new URL('model.json', base), {signal: aborter.signal}); if (!response.ok) throw new Error(`GPS model: HTTP ${response.status}`);
    const metadata = await response.json();
    if (metadata.id !== 'm5stack-gps-u032-v11' || metadata.unit !== 'mm') throw new Error('GPS model identity/unit mismatch');
    const geometryResponse = await fetch(new URL(metadata.binary, base), {signal: aborter.signal}); if (!geometryResponse.ok) throw new Error(`GPS geometry: HTTP ${geometryResponse.status}`);
    const binary = await geometryResponse.arrayBuffer(); if (disposed) return;
    for (const part of metadata.parts) {
      const geometry = new THREE.BufferGeometry(), buffer = new THREE.InterleavedBuffer(new Float32Array(binary, part.byteOffset, part.vertexCount * 6), 6);
      geometry.setAttribute('position', new THREE.InterleavedBufferAttribute(buffer, 3, 0)); geometry.setAttribute('normal', new THREE.InterleavedBufferAttribute(buffer, 3, 3)); geometry.computeBoundingSphere();
      const mesh = new THREE.Mesh(geometry, material(part.color)); mesh.name = part.name; mesh.userData = {kind: 'manufacturer-shell', inputOccluder: true}; group.add(mesh); parts[part.name] = mesh;
    }
    group.remove(fallback); releaseObject(fallback); group.userData.provenance = metadata;
    // Texture failure leaves honest CAD geometry visible; the caller can display the rejected ready status.
    if (typeof document !== 'undefined') for (const item of metadata.textures) {
      const texture = await new THREE.TextureLoader().loadAsync(new URL(item.file, base).href); if (disposed) { texture.dispose(); return; }
      textures.add(texture); texture.colorSpace = THREE.SRGBColorSpace; texture.anisotropy = 4;
      const label = new THREE.Mesh(new THREE.PlaneGeometry(...item.dimensionsMm), new THREE.MeshBasicMaterial({map: texture,polygonOffset:true,polygonOffsetFactor:-2,polygonOffsetUnits:-4})); label.name = item.name; label.position.set(...item.centerMm); if (item.name.startsWith('back')) label.rotation.y = Math.PI; group.add(label); parts[item.name] = label;
    }
    return metadata;
  })();
  // Mark handled until a viewer installs its own error/fallback UI; still return the rejecting promise.
  ready.catch(() => {});
  return {group, parts, connector, ready, dispose() { if (disposed) return; disposed = true; aborter.abort(); for (const texture of textures) texture.dispose(); textures.clear(); releaseObject(group); }};
}
