import * as THREE from './vendor/three.module.min.js';
import {createStopwatchModel} from './stopwatch-model.js';

// Uploaded WebGL textures have fixed storage dimensions. A resized canvas needs a fresh
// texture, even though the canvas object itself is unchanged (e.g. StopWatch -> PaperMono).
export function resizeFramebufferTexture(canvas, texture, width, height, material = null) {
  if (canvas.width === width && canvas.height === height) return texture;
  canvas.width = width; canvas.height = height;
  const replacement = new THREE.CanvasTexture(canvas);
  for (const property of ['colorSpace', 'minFilter', 'magFilter', 'wrapS', 'wrapT', 'anisotropy', 'flipY', 'premultiplyAlpha', 'unpackAlignment', 'generateMipmaps']) replacement[property] = texture[property];
  if (material) { material.map = replacement; material.needsUpdate = true; }
  texture.dispose();
  return replacement;
}

// Human UI convenience only: host-time minimum pulses let polling firmware observe a click.
// This is not cycle-exact input scheduling and does not alter the raw automation input API.
export function createHumanButtonController({send, isRunning, onVisual = () => {}, onError = () => {}, minimumMs = 150, now = () => performance.now(), setTimer = setTimeout, clearTimer = clearTimeout}) {
  let generation = 0, flushing = false;
  const buttons = new Map();
  const current = entry => entry.generation === generation && buttons.get(entry.id) === entry;
  function cancelTimer(entry) { if (entry.timer !== null) clearTimer(entry.timer); entry.timer = null; }
  async function release(entry) {
    if (!current(entry) || entry.sources.size || !isRunning()) return;
    buttons.delete(entry.id); cancelTimer(entry); onVisual(entry.id, false);
    try { await send({type: 'button', id: entry.id, pressed: false}); } catch (error) { if (entry.generation === generation) onError(error); }
  }
  function schedule(entry) {
    if (!current(entry) || entry.sources.size || entry.started === null) return;
    cancelTimer(entry);
    const remaining = minimumMs - (now() - entry.started);
    if (remaining <= 0) void release(entry);
    else entry.timer = setTimer(() => { entry.timer = null; void release(entry); }, remaining);
  }
  async function input({id, pressed}, source = 'scene') {
    if (!isRunning() || flushing) return;
    let entry = buttons.get(id);
    if (pressed) {
      if (entry) { cancelTimer(entry); entry.sources.add(source); return entry.down; }
      entry = {id, generation, sources: new Set([source]), started: null, timer: null, down: null}; buttons.set(id, entry); onVisual(id, true);
      entry.down = Promise.resolve().then(() => {
        if (!current(entry) || !isRunning()) return;
        return send({type: 'button', id, pressed: true});
      }).then(() => { if (current(entry)) { entry.started = now(); schedule(entry); } }).catch(error => {
        if (current(entry)) { buttons.delete(id); cancelTimer(entry); onVisual(id, false); onError(error); }
      });
      return entry.down;
    }
    if (!entry || !entry.sources.delete(source)) return;
    await entry.down; schedule(entry);
  }
  function invalidate() {
    generation++; flushing = false;
    for (const entry of buttons.values()) { cancelTimer(entry); onVisual(entry.id, false); }
    buttons.clear();
  }
  async function releaseAll() {
    const active = [...buttons.values()]; generation++; const token = generation; flushing = true; buttons.clear();
    for (const entry of active) { cancelTimer(entry); onVisual(entry.id, false); }
    try {
      for (const entry of active) {
        await entry.down;
        if (generation !== token || !isRunning()) return;
        await send({type: 'button', id: entry.id, pressed: false});
      }
    } catch (error) { if (generation === token) onError(error); }
    finally { if (generation === token) flushing = false; }
  }
  function suspend() { for (const entry of buttons.values()) cancelTimer(entry); }
  return {input, releaseAll, invalidate, suspend};
}

// StopWatch uses manufacturer CAD; other products retain explicitly illustrative previews.
export function createSceneView(mount, {onInput = () => {}, onModeChange = () => {}} = {}) {
  const display = document.createElement('canvas');
  display.className = 'flat-display';
  display.setAttribute('aria-label', '设备实际帧缓冲；按住或拖动以发送触摸');
  display.width = 466; display.height = 466;
  const context = display.getContext('2d');
  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(32, 1, .1, 1000);
  const group = new THREE.Group(); scene.add(group);
  scene.add(new THREE.HemisphereLight(0xffffff, 0x64775b, 2.5));
  const light = new THREE.DirectionalLight(0xffffff, 3.1); light.position.set(-55, 95, 140); scene.add(light);
  const rim = new THREE.DirectionalLight(0xc6dece, 1.3); rim.position.set(80, -20, -80); scene.add(rim);
  let texture = new THREE.CanvasTexture(display);
  texture.colorSpace = THREE.SRGBColorSpace; texture.minFilter = THREE.LinearFilter; texture.magFilter = THREE.NearestFilter;
  let renderer, mode = '3d', product, screen, buttons = [], accessoryGroup, accessoryCount = 0, pressed, gesture, zoom = 1, frameWidth = 466, frameHeight = 466;
  let paintPending = false, disposed = false, modelCleanup = null, productGeneration = 0;
  try {
    renderer = new THREE.WebGLRenderer({alpha: true, antialias: true, powerPreference: 'low-power'});
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    renderer.setClearColor(0x000000, 0); renderer.outputColorSpace = THREE.SRGBColorSpace;
    renderer.domElement.setAttribute('aria-label', '三维设备；拖动外壳旋转，滚轮缩放，点击按钮或屏幕交互');
    renderer.domElement.tabIndex = 0;
    mount.append(renderer.domElement);
  } catch { mode = '2d'; }
  mount.append(display); display.hidden = mode !== '2d';
  const raycaster = new THREE.Raycaster(), pointer = new THREE.Vector2();
  function draw() {
    if (paintPending || disposed) return;
    paintPending = true;
    requestAnimationFrame(() => { paintPending = false; if (!disposed && renderer && mode === '3d') renderer.render(scene, camera); });
  }
  function frameCamera() {
    if (accessoryCount) {
      group.updateMatrixWorld(true);
      const bounds = new THREE.Box3().setFromObject(group), size = bounds.getSize(new THREE.Vector3()), center = bounds.getCenter(new THREE.Vector3());
      const tangent = Math.tan(THREE.MathUtils.degToRad(camera.fov / 2));
      const distance = (Math.max(size.y / (2 * tangent), size.x / (2 * tangent * camera.aspect)) * 1.32 + size.z / 2) * zoom;
      camera.position.set(center.x, center.y, center.z + distance); camera.lookAt(center);
    } else {
      const distance = (product?.id?.includes('paper') ? 205 : 158) * Math.max(1, 1 / camera.aspect) * zoom;
      camera.position.set(0, 0, distance); camera.lookAt(0, 0, 0);
    }
    camera.updateProjectionMatrix();
  }
  function resize() {
    const width = mount.clientWidth || 500, height = mount.clientHeight || 450;
    camera.aspect = width / height; frameCamera();
    renderer?.setSize(width, height); draw();
  }
  const resizeObserver = new ResizeObserver(resize); resizeObserver.observe(mount);
  function box(width, height, depth, radius, color, x = 0, y = 0, z = 0) {
    const w = width / 2, h = height / 2, r = Math.min(radius, w, h), shape = new THREE.Shape();
    shape.moveTo(-w + r, -h); shape.lineTo(w - r, -h); shape.quadraticCurveTo(w, -h, w, -h + r);
    shape.lineTo(w, h - r); shape.quadraticCurveTo(w, h, w - r, h); shape.lineTo(-w + r, h);
    shape.quadraticCurveTo(-w, h, -w, h - r); shape.lineTo(-w, -h + r); shape.quadraticCurveTo(-w, -h, -w + r, -h);
    const geometry = new THREE.ExtrudeGeometry(shape, {depth, bevelEnabled: true, bevelThickness: .5, bevelSize: .5, bevelSegments: 2, steps: 1, curveSegments: 10});
    geometry.translate(0, 0, -depth / 2);
    const mesh = new THREE.Mesh(geometry, new THREE.MeshStandardMaterial({color, roughness: .45, metalness: .15}));
    mesh.position.set(x, y, z); group.add(mesh); return mesh;
  }
  function clearGroup() {
    productGeneration++; modelCleanup?.(); modelCleanup = null;
    group.traverse(object => {
      object.geometry?.dispose();
      if (object.material) for (const material of [].concat(object.material)) material.dispose();
    });
    group.clear(); buttons = []; screen = null; accessoryGroup = null; accessoryCount = 0;
  }
  function clearDisplay() {
    frameWidth = product?.display?.framebufferWidth || product?.display?.width || 320;
    frameHeight = product?.display?.framebufferHeight || product?.display?.height || 240;
    texture = resizeFramebufferTexture(display, texture, frameWidth, frameHeight, screen?.material);
    context.fillStyle = product?.id?.includes('paper') ? '#d2d5c6' : '#111a18';
    context.fillRect(0, 0, frameWidth, frameHeight); texture.needsUpdate = true; draw();
  }
  function setProduct(value) {
    product = value; clearGroup(); clearDisplay();
    mount.dataset.modelStatus = 'illustrative';
    const paper = product.id.includes('paper'), generic = !product.display;
    display.classList.toggle('round', product.display?.shape === 'round');
    display.hidden = mode !== '2d' || generic;
    if (generic) {
      box(44, 64, 2, 2, 0x2f6451);
      box(23, 29, 2, 1, 0xb7bcb5, 0, 7, 2);
      box(13, 13, 1, 1, 0x252d29, 0, -15, 2);
      box(10, 7, 4, 1, 0xb7bcb5, 0, -32, 1);
      for (const x of [-18, 18]) for (let y = -24; y <= 24; y += 6) box(3, 2, 2, .2, 0xd2bb71, x, y, 2);
    } else if (paper) {
      box(62, 101, 8, 5, 0xe5e5d9);
      box(55, 91, .8, 2, 0x363b35, 0, 1, 4.5);
      screen = new THREE.Mesh(new THREE.PlaneGeometry(49, 81.7), new THREE.MeshBasicMaterial({map: texture}));
      screen.position.set(0, 2, 5.1); group.add(screen);
      const a = box(11, 3, 3, 1, 0x7a7e70, -14, -48, 4), b = box(11, 3, 3, 1, 0x7a7e70, 14, -48, 4);
      a.userData.control = 'button-a'; b.userData.control = 'button-b'; buttons.push(a, b);
    } else {
      const token = productGeneration, model = createStopwatchModel(texture);
      modelCleanup = model.dispose; group.add(model.group); screen = model.screen; buttons = model.buttons;
      mount.dataset.modelStatus = 'loading';
      model.ready.then(() => {
        if (disposed || productGeneration !== token) return;
        mount.dataset.modelStatus = 'ready'; frameCamera(); draw();
      }).catch(error => {
        if (disposed || productGeneration !== token) return;
        mount.dataset.modelStatus = 'error'; console.error('StopWatch CAD loading failed:', error);
        setMode('2d');
      });
    }
    if (screen) screen.userData.screen = true;
    resetView();
  }
  function setAccessories(accessories = []) {
    if (accessoryGroup) {
      accessoryGroup.traverse(object => { object.geometry?.dispose(); object.material?.dispose(); });
      group.remove(accessoryGroup);
    }
    accessoryCount = Math.min(accessories.length, 3);
    accessoryGroup = new THREE.Group(); group.add(accessoryGroup);
    for (let i = 0; i < accessoryCount; i++) {
      const x = 45 + i * 12, y = -14 - i * 18;
      const shell = new THREE.Mesh(new THREE.BoxGeometry(16, 23, 6), new THREE.MeshStandardMaterial({color: 0xe5e7d9, roughness: .8}));
      shell.position.set(x, y, 0); accessoryGroup.add(shell);
      const chip = new THREE.Mesh(new THREE.BoxGeometry(9, 10, 1), new THREE.MeshStandardMaterial({color: accessories[i].type === 'gps' ? 0xbfc497 : 0x4d715b, roughness: .7}));
      chip.position.set(x, y + 2, 3.7); accessoryGroup.add(chip);
      const curve = new THREE.CatmullRomCurve3([new THREE.Vector3(0, -27, 0), new THREE.Vector3(15, -45 - i * 5, -3), new THREE.Vector3(x, y - 14, 0)]);
      const cable = new THREE.Mesh(new THREE.TubeGeometry(curve, 20, .6, 6, false), new THREE.MeshStandardMaterial({color: 0x71866b}));
      accessoryGroup.add(cable);
    }
    frameCamera(); draw();
  }
  function resetView() { group.rotation.set(-.13, -.25, .025); zoom = 1; resize(); }
  function setMode(next) {
    mode = next === '3d' && renderer ? '3d' : '2d';
    if (renderer) renderer.domElement.hidden = mode !== '3d';
    display.hidden = mode !== '2d' || !product?.display;
    onModeChange(mode, !renderer); draw(); return mode;
  }
  function updateFrame({width, height, pixels}) {
    if (!Number.isInteger(width) || !Number.isInteger(height) || width < 1 || height < 1 || width * height > 4_000_000 || pixels?.length !== width * height * 4) return;
    texture = resizeFramebufferTexture(display, texture, width, height, screen?.material);
    frameWidth = width; frameHeight = height;
    context.putImageData(new ImageData(new Uint8ClampedArray(pixels), width, height), 0, 0);
    texture.needsUpdate = true; draw();
  }
  function hit(event) {
    const rect = mount.getBoundingClientRect();
    pointer.set(((event.clientX - rect.left) / rect.width) * 2 - 1, -((event.clientY - rect.top) / rect.height) * 2 + 1);
    raycaster.setFromCamera(pointer, camera);
    return raycaster.intersectObjects([...buttons, ...(screen ? [screen] : [])], false)[0];
  }
  function touch(hitPoint, down) {
    if (!hitPoint?.uv) return;
    const x = Math.max(0, Math.min(frameWidth - 1, Math.floor(hitPoint.uv.x * frameWidth)));
    const y = Math.max(0, Math.min(frameHeight - 1, Math.floor((1 - hitPoint.uv.y) * frameHeight)));
    gesture.lastTouch = {type: 'touch', x, y, down}; onInput(gesture.lastTouch);
  }
  function down(event) {
    if (event.button !== 0) return;
    const target = event.currentTarget; target.setPointerCapture(event.pointerId);
    gesture = {x: event.clientX, y: event.clientY, pointerId: event.pointerId, type: 'orbit'};
    if (target === display) { gesture.type = 'touch'; flatTouch(event, true); return; }
    const intersection = hit(event);
    if (intersection?.object.userData.control) {
      gesture.type = 'button'; pressed = intersection.object.userData.control;
      onInput({type: 'button', id: pressed, pressed: true});
    } else if (intersection?.object.userData.screen) { gesture.type = 'touch'; touch(intersection, true); }
  }
  function flatTouch(event, isDown) {
    const rect = display.getBoundingClientRect(), border = 10;
    const x = Math.round(((event.clientX - rect.left - border) / Math.max(1, rect.width - border * 2)) * frameWidth);
    const y = Math.round(((event.clientY - rect.top - border) / Math.max(1, rect.height - border * 2)) * frameHeight);
    gesture.lastTouch = {type: 'touch', x: Math.max(0, Math.min(frameWidth - 1, x)), y: Math.max(0, Math.min(frameHeight - 1, y)), down: isDown};
    onInput(gesture.lastTouch);
  }
  function move(event) {
    if (!gesture || gesture.pointerId !== event.pointerId) return;
    if (gesture.type === 'orbit') {
      group.rotation.y += (event.clientX - gesture.x) * .01;
      group.rotation.x = Math.max(-1.3, Math.min(1.3, group.rotation.x + (event.clientY - gesture.y) * .01));
      gesture.x = event.clientX; gesture.y = event.clientY; if (accessoryCount) frameCamera(); draw();
    } else if (gesture.type === 'touch') {
      if (event.currentTarget === display) flatTouch(event, true);
      else { const intersection = hit(event); if (intersection?.object.userData.screen) touch(intersection, true); }
    }
  }
  function up() {
    if (pressed) onInput({type: 'button', id: pressed, pressed: false});
    if (gesture?.lastTouch) onInput({...gesture.lastTouch, down: false});
    gesture = null; pressed = null;
  }
  for (const canvas of [display, renderer?.domElement].filter(Boolean)) {
    canvas.addEventListener('pointerdown', down); canvas.addEventListener('pointermove', move);
    canvas.addEventListener('pointerup', up); canvas.addEventListener('pointercancel', up); canvas.addEventListener('lostpointercapture', up);
  }
  window.addEventListener('blur', up);
  function wheel(event) { if (mode !== '3d') return; event.preventDefault(); zoom = Math.max(.65, Math.min(1.8, zoom + event.deltaY * .001)); resize(); }
  mount.addEventListener('wheel', wheel, {passive: false});
  return {setProduct, setAccessories, setMode, resetView, updateFrame, clearDisplay, releaseInputs: up, get mode() { return mode; },
    dispose() { disposed = true; up(); resizeObserver.disconnect(); window.removeEventListener('blur', up); mount.removeEventListener('wheel', wheel); clearGroup(); texture.dispose(); renderer?.dispose(); mount.replaceChildren(); },
  };
}
