import * as THREE from './vendor/three.module.min.js';
import {createStopwatchModel} from './stopwatch-model.js';
import {createPapermonoModel} from './papermono-model.js';
import {createManufacturerModel} from './manufacturer-model.js';
import {pickVisibleSurface, createScenePointerRouter, createSurfaceHalo} from './scene-interaction.js';
import {createIllustrativeAccessory, createGroveCable} from './accessory-view.js';
import {applyRgbLedValues} from './rgb-led-view.js';
import {normalizeShellAppearance, applyShellAppearance, DEFAULT_STOPWATCH_APPEARANCE} from './shell-appearance.js';
import {normalizeOrientation} from './imu.js';

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

// Product-specific CAD stays separate from runtime support; missing models stay empty.
export function createSceneView(mount, {onInput = () => {}, onModeChange = () => {}, onUnsupported = () => {}, onOrientation = () => {}, onModelReady = () => {}} = {}) {
  const display = document.createElement('canvas');
  display.className = 'flat-display';
  display.setAttribute('aria-label', '设备实际帧缓冲；按住或拖动以发送触摸');
  display.width = 466; display.height = 466;
  const context = display.getContext('2d');
  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(32, 1, 2, 3000); camera.layers.enable(1);
  const group = new THREE.Group(); scene.add(group);
  scene.add(new THREE.HemisphereLight(0xffffff, 0x64775b, 2.5));
  const light = new THREE.DirectionalLight(0xffffff, 3.1); light.position.set(-55, 95, 140); scene.add(light);
  const rim = new THREE.DirectionalLight(0xc6dece, 1.3); rim.position.set(80, -20, -80); scene.add(rim);
  let texture = new THREE.CanvasTexture(display);
  texture.colorSpace = THREE.SRGBColorSpace; texture.minFilter = THREE.LinearFilter; texture.magFilter = THREE.NearestFilter;
  let renderer, mode = '3d', product, screen, buttons = [], accessoryGroup, accessoryCount = 0, zoom = 1, frameWidth = 466, frameHeight = 466;
  let paintPending = false, disposed = false, modelCleanup = null, productGeneration = 0, accessoryGeneration = 0, accessoryCleanups = [];
  let orientationTimer = null, orientationIdle = null, lastOrientationAt = -Infinity;
  const getOrientation = () => group.quaternion.toArray();
  function clearOrientationTimers() { clearTimeout(orientationTimer); clearTimeout(orientationIdle); orientationTimer = orientationIdle = null; }
  function publishOrientation(rotating = false) {
    if (disposed) return;
    lastOrientationAt = performance.now();
    Promise.resolve(onOrientation({quaternion: getOrientation(), rotating})).catch(error => console.error('Scene orientation failed:', error));
  }
  function orientationChanged() {
    clearTimeout(orientationIdle);
    if (orientationTimer === null) orientationTimer = setTimeout(() => { orientationTimer = null; publishOrientation(true); }, Math.max(0, 1000 / 30 - (performance.now() - lastOrientationAt)));
    orientationIdle = setTimeout(() => { clearTimeout(orientationTimer); orientationTimer = null; publishOrientation(false); }, 100);
  }
  function setOrientation(value, {notify = true} = {}) {
    const normalized = normalizeOrientation(value); clearOrientationTimers(); group.quaternion.fromArray(normalized);
    if (notify) publishOrientation(false); if (accessoryCount) frameCamera(); draw(); return getOrientation();
  }
  let parts = {}, connectors = {}, shellMeshes = [], customizedAppearance = false, appearanceDefaults = DEFAULT_STOPWATCH_APPEARANCE, appearance = normalizeShellAppearance(null), accessoryDefinitions = [];
  let rgbLedMeshes = [], rgbValues = [], accessoryOutputs = [];
  let accessoryPositions=[];
  function updateAccessoryPositions(values) {accessoryPositions=values||[];for(const child of accessoryGroup?.children || []){const state=accessoryPositions.find(item=>child.name===`accessory:${item.accessoryId}`);for(const pivot of child.userData.potentiometers||[]){const value=Number.isFinite(state?.value)?state.value:.5;pivot.userData.value=value;pivot.rotation.z=(.5-value)*pivot.userData.visualSweepRadians;}}draw();}
  function updateAccessoryOutputs(values) {accessoryOutputs=values||[];for(const child of accessoryGroup?.children || []){const state=accessoryOutputs.find(item=>child.name===`accessory:${item.accessoryId}`);applyRgbLedValues(child.userData.rgbLedMeshes||[],state?.leds);}draw();}
  function updateLeds(values) { rgbValues = values || []; applyRgbLedValues(rgbLedMeshes, rgbValues); draw(); }
  const halo = createSurfaceHalo();
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
    if (accessoryCount || product?.model && product.id !== 'm5stack-papermono') {
      group.updateMatrixWorld(true);
      const bounds = new THREE.Box3().setFromObject(group), size = bounds.getSize(new THREE.Vector3()), center = bounds.getCenter(new THREE.Vector3());
      const tangent = Math.tan(THREE.MathUtils.degToRad(camera.fov / 2));
      const distance = Math.max(20, (Math.max(size.y / (2 * tangent), size.x / (2 * tangent * camera.aspect)) * 1.32 + size.z / 2) * zoom);
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
  function clearGroup() {
    accessoryGeneration++; for (const cleanup of accessoryCleanups) cleanup(); accessoryCleanups = [];
    clearOrientationTimers();
    productGeneration++; modelCleanup?.(); modelCleanup = null;
    group.traverse(object => {
      object.geometry?.dispose();
      if (object.material) for (const material of [].concat(object.material)) material.dispose();
    });
    group.clear(); buttons = []; screen = null; parts = {}; connectors = {}; shellMeshes = []; accessoryGroup = null; accessoryCount = 0;
    rgbLedMeshes = []; rgbValues = [];
  }
  function clearDisplay() {
    frameWidth = product?.display?.framebufferWidth || product?.display?.width || 320;
    frameHeight = product?.display?.framebufferHeight || product?.display?.height || 240;
    texture = resizeFramebufferTexture(display, texture, frameWidth, frameHeight, screen?.material);
    context.fillStyle = /paper|ink/i.test(product?.display?.type || '') ? '#d2d5c6' : '#111a18';
    context.fillRect(0, 0, frameWidth, frameHeight); texture.needsUpdate = true; draw();
  }
  function setProduct(value) {
    releaseInputs(); product = value; clearGroup(); clearDisplay();
    accessoryDefinitions = [];
    appearanceDefaults = product.id === 'm5stack-papermono' ? {shell: {color: '#888d8e', opacity: 1, finish: 'plastic'}} : DEFAULT_STOPWATCH_APPEARANCE;
    appearance = normalizeShellAppearance(null, appearanceDefaults); customizedAppearance = false;
    mount.dataset.modelStatus = 'illustrative';
    const generic = !product.display;
    display.classList.toggle('round', product.display?.shape === 'round');
    display.hidden = mode !== '2d' || generic;
    if (!product.model && !['m5stack-stopwatch','m5stack-papermono'].includes(product.id)) {
      mount.dataset.modelStatus = 'unavailable'; display.hidden = true; onModelReady();
    } else {
      const token = productGeneration, model = product.id === 'm5stack-papermono' ? createPapermonoModel(texture) : product.id === 'm5stack-stopwatch' ? createStopwatchModel(texture) : createManufacturerModel(product, texture);
      modelCleanup = model.dispose; group.add(model.group); screen = model.screen; buttons = model.buttons; parts = model.parts; connectors = model.connectors;
      mount.dataset.modelStatus = 'loading';
      model.ready.then(() => {
        if (disposed || productGeneration !== token) return;
        screen = model.screen; if(screen)screen.userData.screen=true;
        shellMeshes = model.group.userData.shellMeshes || [parts.shell].filter(Boolean);
        rgbLedMeshes = model.group.userData.rgbLedMeshes || []; applyRgbLedValues(rgbLedMeshes, rgbValues);
        for(const mesh of shellMeshes)mesh.userData.originalAppearance ||= {shell:{color:`#${mesh.material.color.getHexString()}`,opacity:1,finish:'plastic'}};
        if(product.model && product.id !== 'm5stack-papermono') { appearanceDefaults = shellMeshes[0]?.userData.originalAppearance || DEFAULT_STOPWATCH_APPEARANCE; if(!customizedAppearance)appearance=normalizeShellAppearance(null,appearanceDefaults); }
        mount.dataset.modelStatus = 'ready'; onModelReady(); applyAppearance(); setAccessories(accessoryDefinitions); frameCamera(); draw();
      }).catch(error => {
        if (disposed || productGeneration !== token) return;
        mount.dataset.modelStatus = 'error'; console.error(`${product.name} CAD loading failed:`, error);
        setMode('2d');
      });
    }
    if (screen) screen.userData.screen = true;
    resetView();
  }
  function setAccessories(accessories = []) {
    releaseInputs();
    const token = ++accessoryGeneration;
    for (const cleanup of accessoryCleanups) cleanup(); accessoryCleanups = [];
    clearHover(); accessoryDefinitions = accessories;
    if (accessoryGroup) {
      accessoryGroup.traverse(object => { object.geometry?.dispose(); if (object.material) for (const material of [].concat(object.material)) material.dispose(); });
      group.remove(accessoryGroup);
    }
    accessoryCount = Math.min(accessories.length, 32);
    accessoryGroup = new THREE.Group(); accessoryGroup.name = 'accessory-assembly'; group.add(accessoryGroup);
    const occupiedSockets = new Set();
    const loadedAccessories = [];
    const attachCables = () => {
      // Rebuild in scene order, never in network completion order. The socket
      // anchors of manufacturer assets exist only after their CAD has loaded.
      if (disposed || token !== accessoryGeneration) return;
      for (const child of [...accessoryGroup.children]) if (child.name === 'grove-cable-visual-only') {
        child.traverse(object => { object.geometry?.dispose(); object.material?.dispose(); }); accessoryGroup.remove(child);
      }
      occupiedSockets.clear();
      for (const {definition, accessory} of [...loadedAccessories].sort((a,b)=>Number(!!b.definition.physicalSupported)-Number(!!a.definition.physicalSupported))) {
        const connectorMatches = definition.connector?.type === 'HY2.0-4P' && (!definition.wiring || ['logical','visual'].includes(definition.wiring.mode) || definition.wiring.connectorId === 'grove' || definition.wiring.interfaceId === 'grove');
        const parent=definition.wiring?.viaAccessoryId?loadedAccessories.find(item=>item.definition.instanceId===definition.wiring.viaAccessoryId):null;
        const socketKey=parent?`${definition.wiring.viaAccessoryId}:${definition.wiring.portId}`:'grove';
        const hostConnector=parent?parent.accessory.connectors?.[definition.wiring.portId]:connectors.grove;
        const hasRoute=parent?definition.physicalSupported:connectorMatches;
        if (hostConnector && hasRoute && definition.connected !== false && !occupiedSockets.has(socketKey)) {
          const cable = createGroveCable(hostConnector, accessory.connector, group);
          if (cable) { accessoryGroup.add(cable.group); occupiedSockets.add(socketKey); accessory.group.userData.connection = definition.physicalSupported ? 'grove-physical' : 'grove-visual-only'; }
          else accessory.group.userData.connection = 'unverified-connector-preview';
        } else accessory.group.userData.connection = 'unconnected-preview';
      }
      updateAccessoryOutputs(accessoryOutputs);updateAccessoryPositions(accessoryPositions);frameCamera(); draw();
    };
    for (let i = 0; i < accessoryCount; i++) {
      const definition = accessories[i], accessory = createIllustrativeAccessory(definition, i); accessoryGroup.add(accessory.group);
      if(definition.wiring?.viaAccessoryId){const port=Number(definition.wiring.portId?.split('-')[1]||1);accessory.group.position.set(112,(2-port)*42,0);accessory.group.rotation.z=0;}else if(definition.model?.id==='passive-hub-u006'){accessory.group.position.set(64,0,0);accessory.group.rotation.z=0;}
      loadedAccessories.push({definition, accessory});
      if (accessory.dispose) accessoryCleanups.push(() => accessory.dispose());
      accessory.ready?.then(attachCables).catch(error => { if (!disposed && token === accessoryGeneration) { console.error('Accessory model loading failed:', error); draw(); } });
    }
    attachCables();
  }
  function applyAppearance() { for(const mesh of shellMeshes.length?shellMeshes:[parts.shell].filter(Boolean))applyShellAppearance(mesh, customizedAppearance?appearance:mesh.userData.originalAppearance || appearance, {occluder: parts.occluder}); draw(); }
  function setAppearance(value) { customizedAppearance = value != null; appearance = normalizeShellAppearance(value, appearanceDefaults); applyAppearance(); return getAppearance(); }
  function getAppearance() { return {shell: {...appearance.shell}}; }
  function resetView() { releaseInputs(); group.rotation.set(-.13, -.25, .025); zoom = 1; publishOrientation(false); resize(); }
  function setMode(next) {
    releaseInputs();
    mode = next === '3d' && renderer ? '3d' : '2d';
    if (renderer) renderer.domElement.hidden = mode !== '3d';
    display.hidden = mode !== '2d' || !product?.display || (!product.model && !product.runtime.board);
    onModeChange(mode, !renderer); draw(); return mode;
  }
  function updateFrame({width, height, pixels}) {
    if (!Number.isInteger(width) || !Number.isInteger(height) || width < 1 || height < 1 || width * height > 4_000_000 || pixels?.length !== width * height * 4) return;
    texture = resizeFramebufferTexture(display, texture, width, height, screen?.material);
    frameWidth = width; frameHeight = height;
    context.putImageData(new ImageData(new Uint8ClampedArray(pixels), width, height), 0, 0);
    texture.needsUpdate = true; draw();
  }
  function setRay(event) {
    const rect = renderer.domElement.getBoundingClientRect();
    pointer.set(((event.clientX - rect.left) / rect.width) * 2 - 1, -((event.clientY - rect.top) / rect.height) * 2 + 1);
    camera.updateMatrixWorld(); group.updateMatrixWorld(true); raycaster.setFromCamera(pointer, camera);
  }
  function hit(event) { setRay(event); return pickVisibleSurface(raycaster, group); }
  function nativePoint(u, v) { return {x: Math.max(0, Math.min(frameWidth - 1, Math.floor(u * frameWidth))), y: Math.max(0, Math.min(frameHeight - 1, Math.floor(v * frameHeight)))}; }
  function flatPoint(event) {
    const rect = display.getBoundingClientRect(), style = getComputedStyle(display), xBorder = parseFloat(style.borderLeftWidth) || 0, yBorder = parseFloat(style.borderTopWidth) || 0;
    const u = (event.clientX - rect.left - xBorder) / Math.max(1, rect.width - xBorder * 2), v = (event.clientY - rect.top - yBorder) / Math.max(1, rect.height - yBorder * 2);
    return {u, v, ...nativePoint(u, v)};
  }
  function screenPoint(event, intersection) {
    if (event.currentTarget === display) return flatPoint(event);
    if (intersection?.object === screen && intersection.uv) return nativePoint(intersection.uv.x, 1 - intersection.uv.y);
    if (!screen) return null;
    // Continue a captured screen gesture on its plane, even beyond its outline. It cannot become orbit.
    setRay(event); screen.updateWorldMatrix(true, false);
    const normal = new THREE.Vector3(0, 0, 1).transformDirection(screen.matrixWorld);
    const plane = new THREE.Plane().setFromNormalAndCoplanarPoint(normal, screen.getWorldPosition(new THREE.Vector3()));
    const point = raycaster.ray.intersectPlane(plane, new THREE.Vector3()); if (!point) return null;
    screen.worldToLocal(point); screen.geometry.computeBoundingBox(); const bounds = screen.geometry.boundingBox;
    return nativePoint((point.x - bounds.min.x) / (bounds.max.x - bounds.min.x), (bounds.max.y - point.y) / (bounds.max.y - bounds.min.y));
  }
  function surface(intersection) {
    const object = intersection?.object;
    if(object?.userData.potentiometer){const knob=object.userData.potentiometer;return {kind:'potentiometer',id:knob.id,accessoryId:knob.accessoryId,value:knob.pivot.userData.value,sweep:knob.pivot.userData.visualSweepRadians,message:'拖动旋钮调整官方标称曲线；显示转角未按实物校准。'};}
    if (object?.userData.encoder) {
      // A push encoder can use the same CAD component for both actions. Only
      // the visible front-center is a press; the outer rim remains draggable.
      const press=object.userData.encoderPress;
      if(press && object.userData.control && intersection.face?.normal.z>.7){const local=object.worldToLocal(intersection.point.clone());if(Math.hypot(local.x-press.centerMm[0],local.y-press.centerMm[1])<=press.radiusMm)return {kind:'button',id:object.userData.control};}
      const control = product?.controls?.find(c => c.id === object.userData.encoder && c.type === 'encoder');
      return product?.runtime?.encoder && control ? {kind:'encoder',id:control.id,detents:control.detents,message:'沿旋钮圆周拖动；每格发送一个编码器步进。'} : {kind:'unsupported',message:'此设备尚未实现旋钮输入。'};
    }
    if (object?.userData.control) return {kind: 'button', id: object.userData.control};
    if (object?.userData.unsupported) return {kind: 'unsupported', message: object.userData.unsupported};
    if (object?.userData.screen) return product?.runtime?.touch ? {kind: 'touch'} : product?.display?.touch === false ? {kind:'orbit'} : {kind: 'unsupported', message: '此设备尚未实现触摸输入。'};
    return {kind: 'orbit'};
  }
  function clearHover() {
    halo.clear(); delete mount.dataset.hover; mount.style.cursor = ''; mount.removeAttribute('title'); display.style.outline = ''; draw();
  }
  function hover(event) {
    if (router.activeCount) return;
    if (event.currentTarget === display) {
      const point = flatPoint(event), inside = point.u >= 0 && point.u <= 1 && point.v >= 0 && point.v <= 1 && (product?.display?.shape !== 'round' || (point.u - .5) ** 2 + (point.v - .5) ** 2 <= .25);
      clearHover(); if (inside && product?.runtime?.touch) { display.style.outline = '2px solid #ee857f55'; mount.dataset.hover = 'touch'; mount.style.cursor = 'crosshair'; } return;
    }
    const intersection = hit(event), role = surface(intersection);
    if (!intersection) { clearHover(); return; }
    halo.show(intersection.object, role.kind); mount.dataset.hover = role.kind;
    mount.style.cursor = {button: 'pointer', touch: 'crosshair', unsupported: 'not-allowed', orbit: 'grab', encoder:'grab',potentiometer:'grab'}[role.kind];
    if (role.message) mount.title = role.message; else mount.removeAttribute('title'); draw();
  }
  const router = createScenePointerRouter({onInput, onUnsupported, onError: error => console.error('Scene input failed:', error), onOrbit(dx, dy) {
    group.rotation.y += dx * .01; group.rotation.x += dy * .01;
    orientationChanged();
    if (accessoryCount) frameCamera(); draw();
  }});
  const encoderMeshes = new Map();
  function encoderPoint(event, mesh) {
    if (!mesh) return null;
    hit(event);
    if(mesh.userData.potentiometer){const pivot=mesh.userData.potentiometer.pivot,parent=pivot.parent;parent.updateWorldMatrix(true,false);const world=parent.localToWorld(pivot.position.clone()),normal=new THREE.Vector3(0,0,1).transformDirection(parent.matrixWorld),point=raycaster.ray.intersectPlane(new THREE.Plane().setFromNormalAndCoplanarPoint(normal,world),new THREE.Vector3());if(!point)return null;parent.worldToLocal(point).sub(pivot.position);return point.lengthSq()<.001?null:{angle:Math.atan2(point.y,point.x)};}
    mesh.updateWorldMatrix(true,false); mesh.geometry.computeBoundingBox();
    const center = mesh.geometry.boundingBox.getCenter(new THREE.Vector3()), world = mesh.localToWorld(center.clone());
    const normal = new THREE.Vector3(0,0,1).transformDirection(mesh.matrixWorld);
    const point = raycaster.ray.intersectPlane(new THREE.Plane().setFromNormalAndCoplanarPoint(normal,world),new THREE.Vector3());
    if (!point) return null; mesh.worldToLocal(point).sub(center);
    return point.lengthSq() < .001 ? null : {angle:Math.atan2(point.y,point.x)};
  }
  function down(event) {
    if (event.button !== 0) return;
    const flat = event.currentTarget === display, intersection = flat ? {object: screen} : hit(event), role = surface(intersection);
    const point = role.kind === 'touch' ? screenPoint(event, intersection) : ['encoder','potentiometer'].includes(role.kind) ? encoderPoint(event,intersection.object) : null;
    if (['encoder','potentiometer'].includes(role.kind)) encoderMeshes.set(event.pointerId,intersection.object);
    if (flat && point && product?.display?.shape === 'round' && (point.u - .5) ** 2 + (point.v - .5) ** 2 > .25) return;
    clearHover(); event.preventDefault(); event.currentTarget.focus?.({preventScroll: true});
    const kind = router.down({pointerId: event.pointerId, x: event.clientX, y: event.clientY, surface: role, point});
    if (!['ignored', 'unsupported'].includes(kind)) { event.currentTarget.setPointerCapture(event.pointerId); mount.style.cursor = kind === 'orbit' ? 'grabbing' : 'pointer'; }
  }
  function move(event) {
    const kind = router.kind(event.pointerId);
    if (!kind) { hover(event); return; }
    router.move({pointerId: event.pointerId, x: event.clientX, y: event.clientY, point: kind === 'touch' ? screenPoint(event) : ['encoder','potentiometer'].includes(kind) ? encoderPoint(event,encoderMeshes.get(event.pointerId)) : null});
  }
  function up(event) { const orbit = router.kind(event.pointerId) === 'orbit'; router.up(event.pointerId); encoderMeshes.delete(event.pointerId); if (orbit) { clearOrientationTimers(); publishOrientation(false); } if (!router.activeCount) clearHover(); }
  function releaseInputs() { const hadMotion = orientationTimer !== null || orientationIdle !== null; clearOrientationTimers(); router.releaseAll(); encoderMeshes.clear(); if (hadMotion) publishOrientation(false); clearHover(); }
  for (const canvas of [display, renderer?.domElement].filter(Boolean)) {
    canvas.addEventListener('pointerdown', down); canvas.addEventListener('pointermove', move); canvas.addEventListener('pointerleave', clearHover); canvas.addEventListener('blur', releaseInputs);
    canvas.addEventListener('pointerup', up); canvas.addEventListener('pointercancel', up); canvas.addEventListener('lostpointercapture', up);
  }
  window.addEventListener('blur', releaseInputs);
  function wheel(event) { if (mode !== '3d') return; event.preventDefault(); clearHover(); zoom = Math.max(.65, Math.min(1.8, zoom + event.deltaY * .001)); resize(); }
  mount.addEventListener('wheel', wheel, {passive: false});
  return {setProduct, setAccessories, setMode, resetView, updateFrame, updateLeds, updateAccessoryOutputs, updateAccessoryPositions, clearDisplay, releaseInputs, setAppearance, getAppearance, setOrientation, getOrientation, get mode() { return mode; }, get hasShell() { return shellMeshes.length > 0 || !!parts.shell; },
    dispose() { disposed = true; releaseInputs(); resizeObserver.disconnect(); window.removeEventListener('blur', releaseInputs); mount.removeEventListener('wheel', wheel); clearGroup(); texture.dispose(); renderer?.dispose(); mount.replaceChildren(); },
  };
}
