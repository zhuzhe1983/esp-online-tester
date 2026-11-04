import * as THREE from './vendor/three.module.min.js';
import {hasMotorOutput} from './motor-state.js';

// StampFly CAD: +X right, +Y forward, +Z up. Runtime: NED world / FRD body.
// The same orthogonal basis on each side leaves the native assembly untouched.
const basis = new THREE.Quaternion(Math.SQRT1_2,Math.SQRT1_2,0,0);
const vector = (value,length) => Array.isArray(value) && value.length===length && value.every(Number.isFinite);
export function flightViewPose(board, flight) {
  if (!hasMotorOutput(board) || flight?.enabled!==true || flight.model!=='stampfly-rigid-body-v1' || flight.worldFrame!=='NED' || flight.bodyFrame!=='FRD' ||
      !vector(flight.positionM,3) || !vector(flight.quaternionXyzw,4) || !Number.isFinite(flight.guestTime) || flight.guestTime<0 ||
      flight.positionM.some(v=>Math.abs(v)>10000)) return null;
  const q=new THREE.Quaternion(...flight.quaternionXyzw),norm=q.length();
  if(Math.abs(norm-1)>.001)return null;
  q.normalize().premultiply(basis).multiply(basis.clone().invert());
  return {positionMm:[flight.positionM[1]*1000,flight.positionM[0]*1000,-flight.positionM[2]*1000],quaternion:q.toArray(),guestTime:flight.guestTime,epoch:flight.epoch};
}
export function applyFlightPose(group, board, flight) {
  const pose=flightViewPose(board,flight);
  if(!pose){group.position.set(0,0,0);group.quaternion.identity();return false;}
  group.position.fromArray(pose.positionMm);group.quaternion.fromArray(pose.quaternion);return true;
}
export function createFlightGround() {
  const ground=new THREE.Group();ground.name='flight-ground-reference';ground.visible=false;
  const grid=new THREE.GridHelper(1000,20,0x7b9685,0xb6c6bc);grid.rotation.x=Math.PI/2;
  for(const material of [].concat(grid.material)){material.transparent=true;material.opacity=.45;material.depthWrite=false;}
  ground.add(grid);
  return ground;
}
