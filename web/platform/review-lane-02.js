import * as THREE from './vendor/three.module.min.js';
import {createManufacturerModel} from './manufacturer-model.js';
import {applyShellAppearance} from './shell-appearance.js';
import {createIllustrativeAccessory,createGroveCable} from './accessory-view.js';
const ids = ["m5stack-c153-lite", "m5stack-k124-b", "m5stack-c149", "m5stack-s007-pin127", "m5stack-k049-b", "m5stack-c139", "m5stack-s007-pin254-v033", "m5stack-k051", "m5stack-s015-dip", "m5stack-k131", "m5stack-k130-v11", "m5stack-k152", "m5stack-k132", "m5stack-c126-m12", "m5stack-c126", "m5stack-c124", "m5stack-c008-c", "m5stack-k034", "m5stack-k016-p", "m5stack-k010-aws", "m5stack-k006-v27", "m5stack-k128", "m5stack-k010-v13", "m5stack-c122-b", "m5stack-k151", "m5stack-k138", "m5stack-k001-v27"];
const scene = new THREE.Scene(); scene.background = new THREE.Color('white'); scene.add(new THREE.HemisphereLight(0xffffff, 0x70756e, 1.4)); const light = new THREE.DirectionalLight(0xffffff, 2.1); scene.add(light);
const camera = new THREE.OrthographicCamera(-60, 60, 60, -60, .1, 1000); const renderer = new THREE.WebGLRenderer({antialias: true, preserveDrawingBuffer: true}); renderer.setPixelRatio(1); renderer.setSize(800, 800, false); renderer.outputColorSpace = THREE.SRGBColorSpace; document.querySelector('#render').append(renderer.domElement);
const overlay = document.createElement('canvas'); overlay.width = overlay.height = 800; document.querySelector('#overlay').append(overlay);
let current, token = 0, view = 'front', hidden = false;
function dispose() { if (!current) return; current.accessory?.dispose(); current.model?.dispose(); current.group.traverse(o => { o.geometry?.dispose(); for (const m of [].concat(o.material || [])) m.dispose(); }); for (const t of current.textures.values()) t.dispose(); scene.remove(current.group); current = null; }
async function load(id) {
 if (!/^m5stack-[a-z0-9-]+$/.test(id)) throw new Error('Invalid product id'); const mine = ++token; dispose(); hidden = false; document.body.dataset.ready = 'false'; document.querySelector('#product').value = id;
 const base = new URL(`./assets/${id}/`, import.meta.url), response = await fetch(new URL('model.json', base)); if (!response.ok) throw new Error(`Model HTTP${response.status}`); const meta = await response.json();
 const pixels = new Uint8Array([17,23,25,255]), blank = new THREE.DataTexture(pixels,1,1); blank.colorSpace = THREE.SRGBColorSpace; blank.needsUpdate = true;
 const catalog = await fetch(new URL('./catalog/products.json',import.meta.url)).then(r=>r.json());
 const product = catalog.items.find(p=>p.id===id);
 const model = createManufacturerModel(product || {id, model:{url:`./assets/${id}/model.json`}, display:meta.presentation?.screen ? {width:1,height:1} : null}, blank);
 current = {id,group:model.group,meta,textures:new Map([['blank',blank]]),base,model}; scene.add(model.group); await model.ready;
 for (const child of model.group.children) if (child.name !== 'pcb-main') child.traverse(object => { if(object.isMesh)object.userData.exterior=true; });
 if (mine !== token) return; document.body.dataset.ready = 'true'; document.querySelector('#status').textContent = `${id} · 厂家 CAD / ${product?.runtime?.board ? "板级模型范围见产品目录" : "板级模拟未实现"}`; show('front'); return meta;
}
function show(id, options = {}) {
 if (!current) return; view = id; const {meta, base} = current, def = meta.reviewViews?.[id] || {}, body = meta.presentation?.bodySizeMm || [50, 50, 20], span = options.span || def.span || Math.max(...body) * 1.45, target = options.target || def.target || [0, 0, 0]; const position = options.position || def.position || (id === 'front' ? [0, 0, 250] : id === 'back' ? [0, 0, -250] : [250, 0, 0]), up = options.up || def.up || (id === 'side' ? [0, 0, 1] : [0, 1, 0]);
 camera.left = -span / 2; camera.right = span / 2; camera.top = span / 2; camera.bottom = -span / 2; camera.position.set(...position); camera.up.set(...up); camera.lookAt(...target); camera.updateProjectionMatrix(); light.position.copy(camera.position).add(new THREE.Vector3(-90, 100, 60)); renderer.render(scene, camera); overlay.getContext('2d').clearRect(0, 0, 800, 800); overlay.getContext('2d').drawImage(renderer.domElement, 0, 0);
 const reference = meta.referencePhotos.find(p => p.view === id); if (reference) document.querySelector('#reference').src = document.querySelector('#overlay-reference').src = new URL(reference.file, base).href;
 const receipt = {id: current.id, view, span, position, target, up}; document.querySelector('#details').textContent = JSON.stringify(receipt, null, 2); return receipt;
}
for (const id of ids) document.querySelector('#product').add(new Option(id, id)); document.querySelector('#product').onchange = e => load(e.target.value).catch(e => document.querySelector('#status').textContent = e.message); document.querySelectorAll('[data-view]').forEach(b => b.onclick = () => show(b.dataset.view)); document.querySelector('#unknown').onclick = () => { hidden = !hidden; current.group.traverse(o => { if (o.userData.exterior) o.visible = !hidden; }); show(view); };
async function cable() { const accessory=createIllustrativeAccessory({id:'m5stack-gps-u032-v11'}); current.group.add(accessory.group);await accessory.ready;const connection=createGroveCable(current.model.connectors.grove,accessory.connector,current.group);if(!connection)throw Error('No verified cable mapping');current.group.add(connection.group);current.accessory=accessory;current.cable=connection;show(view);return connection.group.userData.endpoints;}
function appearance(value) { for(const mesh of current.group.userData.shellMeshes || []) applyShellAppearance(mesh,value); return show(view); }
let maskMaterials=[];
function mask(enabled) { if(enabled){ current.group.traverse(o=>{if(!o.isMesh)return;const original=o.material;const replacement=new THREE.MeshBasicMaterial({color:0x000000,map:original.map,transparent:original.transparent,alphaTest:original.alphaTest,side:original.side});maskMaterials.push([o,original,replacement]);o.material=replacement;}); }else{for(const[o,original,replacement]of maskMaterials){o.material=original;replacement.dispose();}maskMaterials=[];}show(view);}
window.laneReview = {load, show, appearance, mask, cable, get current() { return current; }, renderer}; window.addEventListener('pagehide', () => { dispose(); renderer.dispose(); }); load(new URL(location.href).searchParams.get('id') || ids[0]).catch(e => { document.querySelector('#status').textContent = e.message; });
