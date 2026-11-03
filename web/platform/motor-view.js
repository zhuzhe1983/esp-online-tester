// Indicative rotor motion from sampled guest PWM and guest time, not measured RPM.
import * as THREE from './vendor/three.module.min.js';
import {STAMPFLY_MOTORS} from './motor-state.js';
const triplet=value=>Array.isArray(value)&&value.length===3&&value.every(Number.isFinite);
const visualRadiansPerSecond=18;
export function createMotorRotors(group,definitions=[]) {
  if(!Array.isArray(definitions)||definitions.length>4)throw Error('Invalid motor rotors');
  const rotors=[],ids=new Set(),claimed=new Set(),originalParts=[...group.children];
  for(const definition of definitions) {
    const {id,axis,pivotMm,parts,direction=1}=definition||{};
    if(!STAMPFLY_MOTORS.some(m=>m.id===id)||ids.has(id)||!triplet(axis)||!Number.isFinite(Math.hypot(...axis))||Math.hypot(...axis)<.000001||
      !triplet(pivotMm)||pivotMm.some(v=>Math.abs(v)>5000)||![-1,1].includes(direction)||!Array.isArray(parts)||!parts.length)throw Error('Invalid motor rotor');
    const meshes=parts.map(name=>{const matches=originalParts.filter(child=>child.name===name);if(typeof name!=='string'||claimed.has(name)||matches.length!==1)throw Error('Invalid rotor part membership');claimed.add(name);return matches[0];});
    const pivot=new THREE.Group();pivot.name=`rotor:${id}`;pivot.position.set(...pivotMm);
    pivot.userData={motorId:id,axis:new THREE.Vector3(...axis).normalize(),direction,angle:0,guestTime:null,duty:0};
    group.add(pivot);for(const mesh of meshes){mesh.position.sub(new THREE.Vector3(...pivotMm));pivot.add(mesh);}ids.add(id);rotors.push(pivot);
  }
  group.userData.motorRotors=rotors;return rotors;
}
export function applyMotorValues(rotors,values,output,{running=false}={}) {
  for(const rotor of rotors) {
    const data=rotor.userData,state=values?.find(value=>value.id===data.motorId),time=output?.guestTime;
    if(!state||!Number.isFinite(time)||time<0){data.angle=0;data.guestTime=null;data.duty=0;rotor.quaternion.identity();continue;}
    if(running&&data.guestTime!==null&&time>=data.guestTime)data.angle=(data.angle+(time-data.guestTime)*data.duty*visualRadiansPerSecond*data.direction)%(Math.PI*2);
    data.guestTime=running?time:null;data.duty=state.frequencyHz>0?state.duty:0;
    rotor.quaternion.setFromAxisAngle(data.axis,data.angle);
  }
}
