import * as THREE from './vendor/three.module.min.js';

// Millimetres. +Z points out of the socket; z=0 is the rim, not its rear wall.
// 2 mm pitch is specified. Hidden moulding/depth values remain photo estimates;
// see docs/platform/grove-connectors.md, not a manufacturing-fit CAD claim.
export const GROVE_PITCH = 2;
export const GROVE_INSERTION = 3;
export const GROVE_WIRE_EXIT = 4.8;
export const GROVE_SIGNALS = ['gnd', 'vcc', 'signal1', 'signal2'];
export function verifiedGrove(connector) {
  return connector?.allowConnections === true && connector.type === 'HY2.0-4P' &&
    Array.isArray(connector.pinOrder) && connector.pinOrder.length === 4 &&
    GROVE_SIGNALS.every(signal => connector.pinOrder.filter(pin => pin === signal).length === 1) &&
    Array.isArray(connector.sourceIds) && connector.sourceIds.length > 0;
}
const material = (color, metalness=0) => new THREE.MeshStandardMaterial({color,metalness,roughness:metalness?.35:.65});
const addBox = (group,name,size,position,mat) => { const mesh=new THREE.Mesh(new THREE.BoxGeometry(...size),mat);mesh.name=name;mesh.position.set(...position);group.add(mesh);return mesh; };
export function createGroveSocket({color=0xe4e5dc,housingVariant}={}) {
  const group=new THREE.Group(),white=material(color),dark=material(0x41463f),metal=material(0xb8a572,.65);
  // A real open cavity: no dark plate immediately across the insertion path.
  for(const x of [-4.7,4.7])addBox(group,'socket-side',[.8,5.2,4.8],[x,0,-2.4],white);
  if(housingVariant==='open-roof') {
    // U136 official front/oblique photos show the pin channel through this roof opening.
    addBox(group,'socket-top-rear',[8.6,.7,1.4],[0,2.25,-4.1],white);
    for(const x of [-3.95,3.95])addBox(group,'socket-top-rail',[.7,.7,3.4],[x,2.25,-1.7],white);
  } else addBox(group,'socket-top',[8.6,.7,4.8],[0,2.25,-2.4],white);
  for(const x of [-3.2,3.2])addBox(group,'socket-bottom-rail',[2.2,.7,4.8],[x,-2.25,-2.4],white);
  addBox(group,'socket-bottom',[4.2,.7,3.2],[0,-2.25,-3.2],white);
  addBox(group,'socket-rear',[8.6,3.8,.3],[0,0,-4.95],white);
  // Recess shading faces into the cavity only; its exterior is white plastic.
  const recess=new THREE.Mesh(new THREE.PlaneGeometry(8.6,3.8),dark);recess.name='socket-interior';recess.position.z=-4.79;group.add(recess);
  for(let i=0;i<4;i++)addBox(group,`socket-pin-${i}`,[.4,.4,3.2],[-3+i*GROVE_PITCH,-.15,-3.1],metal);
  group.userData={kind:'connector-visual',mouthPlaneMm:0,insertionDepthMm:GROVE_INSERTION,dimensionsBasis:'photo-estimate',pinPitchMm:GROVE_PITCH};
  return group;
}
export function createGrovePlug(frame,name) {
  const group=new THREE.Group();group.name=name;group.matrixAutoUpdate=false;group.matrix.copy(frame.matrix);
  const white=material(0xe9e9df);
  const outline=new THREE.Shape();outline.moveTo(-4.1,-1.7);outline.lineTo(4.1,-1.7);outline.lineTo(4.1,1.7);outline.lineTo(-4.1,1.7);outline.closePath();
  for(let i=0;i<4;i++){const x=-3+i*GROVE_PITCH,hole=new THREE.Path();hole.moveTo(x-.4,-.55);hole.lineTo(x-.4,.25);hole.lineTo(x+.4,.25);hole.lineTo(x+.4,-.55);hole.closePath();outline.holes.push(hole);}
  const nose=new THREE.Mesh(new THREE.ExtrudeGeometry(outline,{depth:GROVE_INSERTION,bevelEnabled:false}),white);nose.geometry.translate(0,0,-GROVE_INSERTION);nose.name='inserted-nose';group.add(nose);
  addBox(group,'plug-body',[8.2,3.4,GROVE_WIRE_EXIT],[0,0,GROVE_WIRE_EXIT/2],white);
  addBox(group,'plug-stop',[9.4,4.5,.8],[0,0,.45],white);
  // Four crimp windows, separated by moulded plastic; never a solid black cap.
  const metal=material(0x90958c,.5);
  for(let i=0;i<4;i++)addBox(group,`crimp-window-${i}`,[.7,.02,1.15],[-3+i*GROVE_PITCH,1.71,3.1],metal);
  group.userData={matingFace:[0,0,0],outwardNormal:[0,0,1],insertionDepthMm:GROVE_INSERTION,wireExitMm:GROVE_WIRE_EXIT,dimensionsBasis:'photo-estimate'};
  return group;
}
