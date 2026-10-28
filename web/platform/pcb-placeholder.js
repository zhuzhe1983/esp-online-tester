import * as THREE from './vendor/three.module.min.js';

// A known enclosure can constrain an UNKNOWN board envelope without establishing
// the real board outline. Keep that distinction in the model's basis and notes.
export function createPcbPlaceholderGeometry(size, shape = 'box') {
  if (!Array.isArray(size) || size.length !== 3 || !size.every(v => Number.isFinite(v) && v > 0 && v <= 5000)) throw Error('Invalid PCB placeholder dimensions');
  if (shape === 'box') return new THREE.BoxGeometry(...size);
  if (shape === 'disc' && size[0] === size[1]) return new THREE.CylinderGeometry(size[0]/2,size[0]/2,size[2],96).rotateX(Math.PI/2);
  if (shape === 'capsule' && size[0] >= size[1]) {
    const radius=size[1]/2,halfStraight=(size[0]-size[1])/2,outline=new THREE.Shape();
    outline.moveTo(-halfStraight,-radius);outline.lineTo(halfStraight,-radius);
    outline.absarc(halfStraight,0,radius,-Math.PI/2,Math.PI/2,false);outline.lineTo(-halfStraight,radius);
    outline.absarc(-halfStraight,0,radius,Math.PI/2,Math.PI*1.5,false);outline.closePath();
    return new THREE.ExtrudeGeometry(outline,{depth:size[2],bevelEnabled:false,curveSegments:32,steps:1}).translate(0,0,-size[2]/2);
  }
  throw Error('Invalid PCB placeholder shape');
}
