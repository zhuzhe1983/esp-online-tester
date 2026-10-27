import * as THREE from './vendor/three.module.min.js';

const alphaMasks = new WeakMap();
function textureAlpha(map, uv) {
  const image = map?.image;
  if (!image || !uv || typeof document === 'undefined') return 1;
  if (!alphaMasks.has(image)) {
    try {
      const canvas = document.createElement('canvas'); canvas.width = image.naturalWidth || image.width; canvas.height = image.naturalHeight || image.height;
      const context = canvas.getContext('2d', {willReadFrequently: true}); context.drawImage(image, 0, 0);
      alphaMasks.set(image, {width: canvas.width, height: canvas.height, pixels: context.getImageData(0, 0, canvas.width, canvas.height).data});
    } catch { alphaMasks.set(image, null); }
  }
  const mask = alphaMasks.get(image); if (!mask) return 1;
  map.updateMatrix(); const point = map.transformUv(uv.clone());
  const x = Math.max(0, Math.min(mask.width - 1, Math.floor(point.x * mask.width))), y = Math.max(0, Math.min(mask.height - 1, Math.floor(point.y * mask.height)));
  return mask.pixels[(y * mask.width + x) * 4 + 3] / 255;
}

// Raycast the complete assembly, not just controls: an opaque shell must occlude a button.
export function pickVisibleSurface(raycaster, root) {
  root.updateWorldMatrix(true, true);
  for (const hit of raycaster.intersectObject(root, true)) {
    let visible = true;
    for (let object = hit.object; object; object = object.parent) if (!object.visible || object.userData.pickIgnore) { visible = false; break; }
    if (!visible || !hit.object.isMesh) continue;
    const material = Array.isArray(hit.object.material) ? hit.object.material[hit.face?.materialIndex || 0] : hit.object.material;
    if (!material || !material.visible || (material.opacity === 0 && !hit.object.userData.inputOccluder)) continue;
    if (material.alphaTest > 0 && textureAlpha(material.map, hit.uv) < material.alphaTest) continue;
    return hit;
  }
  return null;
}

// A pointer owns its role from down to up. A screen swipe never turns into an orbit,
// and independent pointers can hold A and B without releasing each other's inputs.
export function createScenePointerRouter({onInput = () => {}, onOrbit = () => {}, onUnsupported = () => {}, onError = () => {}} = {}) {
  const pointers = new Map(); let touchOwner = null;
  const emit = (event, pointerId) => { try { Promise.resolve(onInput(event, `scene:${pointerId}`)).catch(onError); } catch (error) { onError(error); } };
  function up(pointerId) {
    const gesture = pointers.get(pointerId); if (!gesture) return;
    pointers.delete(pointerId);
    if (gesture.kind === 'button') emit({type: 'button', id: gesture.control, pressed: false}, pointerId);
    if (gesture.kind === 'touch') { touchOwner = null; if (gesture.lastTouch) emit({...gesture.lastTouch, down: false}, pointerId); }
  }
  function touch(gesture, point) {
    if (!point) return;
    gesture.lastTouch = {type: 'touch', x: point.x, y: point.y, down: true}; emit(gesture.lastTouch, gesture.pointerId);
  }
  function down({pointerId, x, y, surface, point}) {
    up(pointerId);
    const kind = surface?.kind || 'orbit';
    if (kind === 'unsupported') { onUnsupported(surface.message); return 'unsupported'; }
    const gesture = {pointerId, x, y, kind};
    if (kind === 'button') { gesture.control = surface.id; pointers.set(pointerId, gesture); emit({type: 'button', id: surface.id, pressed: true}, pointerId); }
    else if (kind === 'encoder') {
      if (pointers.size) return 'ignored';
      gesture.control = surface.id; gesture.angle = point?.angle; gesture.detentAngle = 2 * Math.PI / (surface.detents || 16); gesture.remainder = 0;
      pointers.set(pointerId, gesture);
    } else if (kind === 'touch') {
      // The board currently exposes a single-contact touch controller.
      if (touchOwner !== null || [...pointers.values()].some(item => item.kind === 'orbit')) return 'ignored';
      touchOwner = pointerId; pointers.set(pointerId, gesture); touch(gesture, point);
    } else {
      if (pointers.size) return 'ignored';
      pointers.set(pointerId, gesture);
    }
    return kind;
  }
  function move({pointerId, x, y, point}) {
    const gesture = pointers.get(pointerId); if (!gesture) return null;
    if (gesture.kind === 'orbit') { onOrbit(x - gesture.x, y - gesture.y); gesture.x = x; gesture.y = y; }
    else if (gesture.kind === 'touch') touch(gesture, point);
    else if (gesture.kind === 'encoder' && Number.isFinite(point?.angle)) {
      if (Number.isFinite(gesture.angle)) {
        const delta = Math.atan2(Math.sin(gesture.angle - point.angle), Math.cos(gesture.angle - point.angle));
        gesture.remainder += delta;
        const steps = Math.trunc(gesture.remainder / gesture.detentAngle);
        if (steps) { gesture.remainder -= steps * gesture.detentAngle; emit({type:'encoder',id:gesture.control,steps:Math.max(-64,Math.min(64,steps))}, pointerId); }
      }
      gesture.angle = point.angle;
    }
    return gesture.kind;
  }
  return {down, move, up, releaseAll() { for (const id of [...pointers.keys()]) up(id); }, kind: id => pointers.get(id)?.kind || null, get activeCount() { return pointers.size; }};
}

export function createSurfaceHalo() {
  let target = null, overlay = null, kind = null;
  function clear() {
    if (overlay) { overlay.removeFromParent(); overlay.traverse(object => { object.geometry?.dispose(); object.material?.dispose(); }); }
    overlay = null; target = null; kind = null;
  }
  function show(mesh, nextKind) {
    if (mesh === target && kind === nextKind) return;
    clear();
    if (!mesh?.geometry || !['button', 'orbit', 'touch', 'encoder'].includes(nextKind)) return;
    target = mesh; kind = nextKind;
    const colors = {button: 0x59dca1, orbit: 0x72b9ef, encoder: 0x72b9ef, touch: 0xee857f};
    overlay = new THREE.Group(); overlay.name = `hover:${nextKind}`; overlay.userData.pickIgnore = true;
    mesh.geometry.computeBoundingBox(); const center = mesh.geometry.boundingBox.getCenter(new THREE.Vector3());
    // Only contour geometry is added. No highlight is drawn into the live framebuffer.
    for (const [scale, opacity] of [[1.008, .28], [1.025, .09]]) {
      const geometry = new THREE.EdgesGeometry(mesh.geometry, 28);
      geometry.translate(-center.x, -center.y, -center.z); geometry.scale(scale, scale, scale); geometry.translate(center.x, center.y, center.z);
      const contour = new THREE.LineSegments(geometry, new THREE.LineBasicMaterial({color: colors[nextKind], transparent: true, opacity, depthTest: true, depthWrite: false}));
      contour.layers.set(1); contour.userData.pickIgnore = true; overlay.add(contour);
    }
    if (nextKind === 'touch') overlay.position.z = .035;
    overlay.layers.set(1); mesh.add(overlay);
  }
  return {show, clear, get target() { return target; }, get kind() { return kind; }};
}
