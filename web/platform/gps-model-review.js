import * as THREE from './vendor/three.module.min.js';
import {createGpsUnitModel} from './gps-model.js';
const base = new URL('./assets/m5stack-gps-u032-v11/', import.meta.url);
const views = {
  front: {photo: 7, label: '正面', position: [0, 0, 180], up: [0, 1, 0], target: [.2, 0, 0], span: 72.45},
  back: {photo: 8, label: '背面', position: [0, 0, -180], up: [0, 1, 0], target: [0, 0, 0], span: 72.45},
  oblique: {photo: 6, label: '插座斜视（定性）', position: [65, -160, 95], roll: .53, up: [0, 1, 0], target: [0, 0, 0], span: 73},
  rear: {photo: 5, label: '背面斜视（定性）', position: [-45, -75, -160], roll: -.45, up: [0, 1, 0], target: [0, 0, 0], span: 73}
};
const scene = new THREE.Scene(); scene.background = new THREE.Color('white'); scene.add(new THREE.HemisphereLight(0xffffff, 0xa6aaa6, 2.5));
const light = new THREE.DirectionalLight(0xffffff, 3.1); scene.add(light);
const camera = new THREE.OrthographicCamera(-40, 40, 40, -40, .1, 1000);
const renderer = new THREE.WebGLRenderer({antialias: true, preserveDrawingBuffer: true}); renderer.setPixelRatio(1); renderer.setSize(800, 800, false); renderer.outputColorSpace = THREE.SRGBColorSpace;
document.querySelector('#render').append(renderer.domElement);
const overlay = document.createElement('canvas'); overlay.width = overlay.height = 800; document.querySelector('#overlay').append(overlay);
const model = createGpsUnitModel(); scene.add(model.group); let selected = 'front', inspecting = false;
function showView(id) {
 const v = views[id]; if (!v) throw new Error('Unknown GPS view'); selected = id;
 camera.left = -v.span / 2; camera.right = v.span / 2; camera.top = v.span / 2; camera.bottom = -v.span / 2; camera.position.set(...v.position); camera.up.set(...v.up); camera.lookAt(...v.target); if (v.roll) camera.rotateZ(v.roll); camera.updateProjectionMatrix();
 light.position.copy(camera.position).add(new THREE.Vector3(-60, 90, 30));
 const url = new URL(`reference-photo-${v.photo}.webp`, base).href; document.querySelector('#reference').src = document.querySelector('#overlay-reference').src = url;
 document.querySelector('#source').href = `https://m5stack.oss-cn-shenzhen.aliyuncs.com/resource/docs/products/unit/Unit-GPS%20v1.1/${v.photo}.webp`;
 document.querySelector('#camera').textContent = JSON.stringify({view: id, ...v, alignment: 'Fixed scale and camera only; no geometry deformation. Oblique photo pose is approximate, not a dimensional acceptance test.'}, null, 2);
 renderer.render(scene, camera); overlay.getContext('2d').clearRect(0, 0, 800, 800); overlay.getContext('2d').drawImage(renderer.domElement, 0, 0); document.body.dataset.view = id;
 return {id, ...v};
}
for (const [id, v] of Object.entries(views)) { const b = document.createElement('button'); b.textContent = v.label; b.onclick = () => showView(id); document.querySelector('#views').append(b); }
document.querySelector('#unknown').onclick = () => { inspecting = !inspecting; for (const id of ['shell-front', 'shell-back', 'front-label', 'back-label']) if (model.parts[id]) model.parts[id].visible = !inspecting; showView(selected); };
window.gpsModelReview = {showView, capture: () => renderer.domElement.toDataURL('image/png'), model, views, renderer, scene, camera};
model.ready.then(() => { showView('front'); document.querySelector('#status').textContent = '厂家 CAD 已加载；PCB 与内部布局未验证。'; document.body.dataset.ready = 'true'; }).catch(e => { showView('front'); document.querySelector('#status').textContent = `${e.message}；仍展示尺寸占位。`; });
window.addEventListener('pagehide', () => { model.dispose(); renderer.dispose(); });
