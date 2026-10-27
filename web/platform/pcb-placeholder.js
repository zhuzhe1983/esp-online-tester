import * as THREE from './vendor/three.module.min.js';

// A known enclosure can constrain an UNKNOWN board envelope without establishing
// the real board outline. Keep that distinction in the model's basis and notes.
export function createPcbPlaceholderGeometry(size, shape = 'box') {
  if (!Array.isArray(size) || size.length !== 3 || !size.every(v => Number.isFinite(v) && v > 0 && v <= 5000)) throw Error('Invalid PCB placeholder dimensions');
  if (shape === 'box') return new THREE.BoxGeometry(...size);
  if (shape === 'disc' && size[0] === size[1]) return new THREE.CylinderGeometry(size[0]/2,size[0]/2,size[2],96).rotateX(Math.PI/2);
  throw Error('Invalid PCB placeholder shape');
}
