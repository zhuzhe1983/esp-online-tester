// Rebuild the C147 preview from the M5Stack orthographic drawing and photos.
// Pass a scratch directory to compare the deployed model without overwriting it.
import {createHash} from 'node:crypto';
import {mkdir, readFile, writeFile} from 'node:fs/promises';
import {resolve} from 'node:path';
import {Box3, BoxGeometry, ExtrudeGeometry, Path, Shape, Vector3} from '../web/platform/vendor/three.module.min.js';

const out = resolve(process.argv[2] ?? 'web/platform/assets/m5stack-c147');
const sources = JSON.parse(await readFile(new URL('../docs/platform/evidence/chain-dualkey/sources.json', import.meta.url)));
const geometryParts = [];
const parts = [];
const bounds = new Box3();
let byteOffset = 0;

function exportPart(name, role, color, geometry, extra = {}) {
  const {position, normal} = geometry.attributes;
  const bytes = Buffer.alloc(position.count * 24);
  for (let i = 0; i < position.count; i++) {
    const point = new Vector3().fromBufferAttribute(position, i);
    bounds.expandByPoint(point);
    for (const [index, number] of [...point.toArray(), normal.getX(i), normal.getY(i), normal.getZ(i)].entries()) {
      bytes.writeFloatLE(number === 0 ? 0 : number, (i * 6 + index) * 4);
    }
  }
  geometryParts.push(bytes);
  parts.push({name, role, color, byteOffset, vertexCount: position.count, ...extra});
  byteOffset += bytes.length;
}

function roundedOutline(width, height, radius) {
  const x = width / 2, y = height / 2, r = radius;
  const outline = new Shape();
  outline.moveTo(x, -y + r);
  outline.absarc(x - r, y - r, r, 0, Math.PI / 2);
  outline.absarc(-x + r, y - r, r, Math.PI / 2, Math.PI);
  outline.absarc(-x + r, -y + r, r, Math.PI, 3 * Math.PI / 2);
  outline.absarc(x - r, -y + r, r, 3 * Math.PI / 2, 2 * Math.PI);
  outline.closePath();
  return outline;
}

function slab(outline, low, high, x = 0, y = 0) {
  return new ExtrudeGeometry(outline, {depth: high - low, bevelEnabled: false, curveSegments: 8}).translate(x, y, low);
}

function plate(name, role, color, width, height, radius, low, high, x = 0, y = 0, extra) {
  exportPart(name, role, color, slab(roundedOutline(width, height, radius), low, high, x, y), extra);
}

function circleHole(shape, x, y, radius) {
  const hole = new Path();
  hole.absarc(x, y, radius, 0, 2 * Math.PI, true);
  shape.holes.push(hole);
}

// Model axes: X from KEY2 to KEY1 as viewed from the slide-switch edge (-Y);
// Z from the base to the key tops. The drawing sets overall 47.9 x 23.9 x 34.35.
const bottom = roundedOutline(47.9, 23.9, .75);
for (const x of [-20, 20]) for (const y of [-8, 8]) circleHole(bottom, x, y, 1.25);
for (const y of [-8, 8]) circleHole(bottom, 11.95, y, 2.425);
for (const x of [-1, 1]) {
  const recess = new Path();
  const inner = x * 16.9;
  const outer = x * 23.8;
  recess.moveTo(inner, -5.6);
  recess.lineTo(inner, 5.6);
  recess.lineTo(outer, 5.6);
  recess.lineTo(outer, -5.6);
  recess.closePath();
  bottom.holes.push(recess);
}
exportPart('shell-bottom', 'shell', '#d6d4d1', slab(bottom, -17.175, -15.775));

// The lower shell leaves a recess at both Chain sockets. Nearby exposed parts
// are approximations traced from the product's side and underside views.
const sides = [[-23.95,-11.95],[23.95,-11.95],[23.95,-6],[17,-6],[17,6],[23.95,6],
  [23.95,11.95],[-23.95,11.95],[-23.95,6],[-17,6],[-17,-6],[-23.95,-6]];
const lower = new Shape(sides.map(([x,y])=>({x,y})));
lower.holes.push(new Path([[-16.4,-10],[-16.4,10],[16.4,10],[16.4,-10]].map(([x,y])=>({x,y}))));
exportPart('shell-lower', 'shell', '#d8d7d4', slab(lower, -15.775, -5.72));

const upper = new Shape([[-23.95,-11.95],[-4.3,-11.95],[-4.3,-9],[4.3,-9],[4.3,-11.95],
  [23.95,-11.95],[23.95,11.95],[4.6,11.95],[4.6,11.25],[-4.6,11.25],[-4.6,11.95],[-23.95,11.95]]
  .map(([x,y])=>({x,y})));
exportPart('shell-upper', 'shell', '#dedbd8', slab(upper, -5.60, 1.34));
exportPart('switch-slider', 'hardware', '#adaeaa', new BoxGeometry(2.1,2.75,6).toNonIndexed().translate(0,-10,-1.18));

for (const {x, id, ledIndex} of [{x:-11.975,id:'key2',ledIndex:0},{x:11.975,id:'key1',ledIndex:1}]) {
  plate(`${id}-switch`, 'hardware', '#d0d7d9', 14.8,14.8,1.05,1.34,6.92,x,0,
    {material:{opacity:.74,roughness:.42}});
  plate(`${id}-red-cap`, 'keycap', '#d54a5d', 18.6,18.6,2.65,6.92,14.20,x,0,{controlId:id});
  plate(`${id}-light-window`, 'keycap', '#dd596c', 15.8,15.8,2.55,14.20,16.64,x,0,{controlId:id,ledIndex});
  plate(`${id}-clear-cap`, 'keycap', '#f2dfe1', 17.05,17.05,2.7,14.20,17.175,x,0,
    {controlId:id,material:{opacity:.44,roughness:.27}});
}

const ground = Buffer.concat(geometryParts);
const file = 'geometry.f32';
const hash = createHash('sha256').update(ground).digest('hex');
const origin = ['dimensions','annotated-photo','front-photo','end-photo','top-photo','rear-photo','schematic','m5-hardware-tree'];
const connector = (id, type, centerMm, rotation) => ({
  id, kind:'connector-visual', connectorType:type, allowConnections:false,
  ...(type==='HY2.0-4P'?{housingColor:'#247c8d'}:{}),
  basis:'image-measurement', sourceIds:['dimensions','annotated-photo','rear-photo'],
  geometry:{centerMm,rotation},
});
const metadata = {
  schemaVersion:'1', modelRevision:'2026-11-23', productId:'m5stack-c147',
  unit:'mm', binary:file, strideFloats:6, artifacts:{[file]:hash}, parts,
  derivedParts:[
    connector('chain-uart1','HY2.0-4P',[-23.4,0,-11.5],[0,-Math.PI/2,0]),
    connector('chain-uart2','HY2.0-4P',[23.4,0,-11.5],[0,Math.PI/2,0]),
    connector('usb-c','USB-C',[0,11.77,-3.12],[-Math.PI/2,0,0]),
    {id:'pcb-main',kind:'pcb-placeholder',basis:'unknown',sourceIds:['dimensions','annotated-photo'],
      color:'#859188',geometry:{sizeMm:[32.6,19,1.2],centerMm:[0,0,-9.0]}},
  ],
  presentation:{
    inspectionGroups:[{id:'top-assembly',parts:['shell-upper','switch-slider','usb-c',
      'key1-switch','key1-red-cap','key1-light-window','key1-clear-cap',
      'key2-switch','key2-red-cap','key2-light-window','key2-clear-cap'],
      offsetMm:[0,0,19],sourceIds:['dimensions','annotated-photo'],
      notes:'Preview separates the shell and switches so the UNKNOWN board placeholder can be seen.'}],
  },
  provenance:{
    kind:'photo-derived', sources:sources.filter(entry=>origin.includes(entry.id)),
    notes:[
      'Exterior reconstructed from the orthographic drawing and M5Stack photos. No C147 CAD was found in the checked manufacturer tree.',
      'Key centres, shell seam, recess depth, keycap geometry and connector heights are approximate image measurements.',
      'The RGB cap regions indicate the guest LED output. Optical scattering through the caps has not been measured.',
      'The PCB is an UNKNOWN placeholder. Ports are visual recesses with no active cable routing.',
    ],
  },
  verification:{
    officialDimensionsMm:[47.9,23.9,34.35],
    surfaceBoundsMm: [bounds.min.toArray(), bounds.max.toArray()].map(values=>values.map(value=>Math.round(value*100000)/100000)),
    sourceDrawing:'dimensions',
    report:'../../../docs/platform/evidence/chain-dualkey/model-verification.json',
  },
};

await mkdir(out,{recursive:true});
await writeFile(resolve(out,file),ground);
await writeFile(resolve(out,'model.json'),JSON.stringify(metadata,null,2)+'\n');
console.log(JSON.stringify({sha256:hash,bytes:ground.length,bounds:metadata.verification.surfaceBoundsMm}));
