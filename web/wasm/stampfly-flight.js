import {PARAMETERS as P,ROUT_OHM} from './stampfly-parameters.js';
// A deliberately bounded ideal plant: guest PWM is its only actuator input.
// FRD body -> NED world. Fixed integration steps are independent of wall time.
export const FLIGHT_MODEL='stampfly-rigid-body-v1';
const IDS=['front-left','front-right','rear-left','rear-right'],PINS=[5,42,10,41];
const cross=(a,b)=>[a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]];
const dot=(a,b)=>a[0]*b[0]+a[1]*b[1]+a[2]*b[2];
const add=(a,b)=>a.map((v,i)=>v+b[i]);
const scale=(a,k)=>a.map(v=>v*k);
export function rotate(q,v){const t=scale(cross(q,v),2);return add(v,add(scale(t,q[3]),cross(q,t)));}
const inverse=q=>[-q[0],-q[1],-q[2],q[3]];
const multiply=(a,b)=>[a[3]*b[0]+a[0]*b[3]+a[1]*b[2]-a[2]*b[1],a[3]*b[1]-a[0]*b[2]+a[1]*b[3]+a[2]*b[0],a[3]*b[2]+a[0]*b[1]-a[1]*b[0]+a[2]*b[3],a[3]*b[3]-a[0]*b[0]-a[1]*b[1]-a[2]*b[2]];
export function motorResistance(duty){const f=Math.max(0,Math.min(100,duty*100)),i=Math.floor(f);return ROUT_OHM[i]+(ROUT_OHM[Math.min(i+1,100)]-ROUT_OHM[i])*(f-i);}
export function motorDerivative(omega,duty){const w=Math.max(0,omega),current=duty<0.01?0:(P.voltage-P.ke*w)/motorResistance(duty);return (P.kt*current-P.cq*w*w-P.kt*(P.i0a+P.i0b*w*w))/P.rotorJ;}
export function hoverDuty(){const w=Math.sqrt(P.mass*P.gravity/(4*P.efficiency*P.ct));let lo=0,hi=1;for(let i=0;i<60;i++){const mid=(lo+hi)/2;if(motorDerivative(w,mid)<0)lo=mid;else hi=mid;}return (lo+hi)/2;}
// Flat-floor ray intersection from the actual assumed sensor mount, not COM height.
export function floorDistance(position,q,mount,beam){const start=add(position,rotate(q,mount)),direction=rotate(q,beam);if(direction[2]<P.minimumBeamDown)return null;const d=(P.floorDownM-start[2])/direction[2];return d>=P.minimumTofM&&d<=P.maximumTofM?d:null;}
export function createFlightPlant({clockHz=240000000,epoch=0}={}){
  if(!Number.isSafeInteger(clockHz)||clockHz<=0||clockHz*P.stepSeconds%1!==0)throw new Error('Unsupported flight clock');
  const stepCycles=clockHz*P.stepSeconds;
  let cycle=0,integratedCycle=0,position=[0,0,0],velocity=[0,0,0],q=[0,0,0,1],omega=[0,0,0],accel=[0,0,0],contact=true;
  const speeds=[0,0,0,0],thrusts=[0,0,0,0];let pwm=Array.from({length:4},()=>({duty:0,frequencyHz:0}));
  const mounts=[[P.arm,-P.arm,P.floorDownM],[P.arm,P.arm,P.floorDownM],[-P.arm,-P.arm,P.floorDownM],[-P.arm,P.arm,P.floorDownM]];
  function step(){
    const dt=P.stepSeconds,torqueRotor=[];
    for(let i=0;i<4;i++){
      const d=pwm[i].frequencyHz>0?pwm[i].duty:0,w=speeds[i],f=x=>motorDerivative(x,d);
      const a=f(w),b=f(w+dt*a/2),c=f(w+dt*b/2),d4=f(w+dt*c);
      speeds[i]=Math.max(0,w+dt*(a+2*b+2*c+d4)/6);thrusts[i]=P.efficiency*P.ct*speeds[i]**2;
      torqueRotor[i]=P.cq*speeds[i]**2+P.rotorJ*(speeds[i]-w)/dt;
    }
    const [fl,fr,rl,rr]=thrusts;
    const torque=[P.arm*(fl+rl-fr-rr),P.arm*(fl+fr-rl-rr),-torqueRotor[0]+torqueRotor[1]+torqueRotor[2]-torqueRotor[3]];
    const gyro=cross(omega,omega.map((v,i)=>v*P.inertia[i]));
    omega=omega.map((v,i)=>v+dt*(torque[i]-gyro[i]-P.angularDrag*v)/P.inertia[i]);
    const wnorm=Math.hypot(...omega),angle=wnorm*dt/2,s=wnorm>0?Math.sin(angle)/wnorm:dt/2;
    q=multiply(q,[...scale(omega,s),Math.cos(angle)]);q=scale(q,1/Math.hypot(...q));
    const before=velocity.slice(),force=rotate(q,[0,0,-(fl+fr+rl+rr)]);
    velocity=velocity.map((v,i)=>v+dt*(force[i]/P.mass+(i===2?P.gravity:0)-P.linearDrag*v/P.mass));
    position=add(position,scale(velocity,dt));
    // Inelastic planar contact at four assumed feet. Sequential impulses oppose
    // penetration; approximate viscous ground friction dissipates motion. No target/pose controller.
    const contacts=mounts.filter(m=>position[2]+rotate(q,m)[2]>=P.floorDownM-1e-5);
    contact=contacts.length>0;const penetration=Math.max(0,...contacts.map(m=>position[2]+rotate(q,m)[2]-P.floorDownM));
    const impulses=contacts.map(()=>0);
    for(let iteration=0;iteration<16;iteration++)for(const [index,mount] of contacts.entries()){
      const at=add(velocity,rotate(q,cross(omega,mount)));
      const n=[0,0,-1],bodyN=rotate(inverse(q),n),lever=cross(mount,bodyN);
      const effective=1/P.mass+dot(lever,lever.map((v,i)=>v/P.inertia[i]));
      const next=Math.max(0,impulses[index]+at[2]/effective),impulse=next-impulses[index];impulses[index]=next;
      velocity=add(velocity,scale(n,impulse/P.mass));omega=add(omega,lever.map((v,i)=>v*impulse/P.inertia[i]));
    }
    if(contact){const friction=Math.exp(-20*P.friction*dt);velocity[0]*=friction;velocity[1]*=friction;omega[2]*=friction;}
    position[2]-=penetration;
    accel=velocity.map((v,i)=>(v-before[i])/dt);
    if(![...position,...velocity,...q,...omega,...speeds].every(Number.isFinite))throw new Error('Flight model diverged');
  }
  function sensors(){const body=rotate(inverse(q),[accel[0]/P.gravity,accel[1]/P.gravity,accel[2]/P.gravity-1]);const gyro=scale(omega,180/Math.PI);return {rawAccelG:[body[1],body[0],-body[2]],rawGyroDps:[gyro[1],gyro[0],-gyro[2]],bottomDistanceM:floorDistance(position,q,P.bottomMountFRD,[0,0,1]),frontDistanceM:floorDistance(position,q,P.frontMountFRD,[1,0,0])};}
  return {
    advanceTo(nextCycle){if(!Number.isSafeInteger(nextCycle)||nextCycle<cycle)throw new Error('Flight clock cannot go backwards');cycle=nextCycle;while(integratedCycle+stepCycles<=cycle){step();integratedCycle+=stepCycles;}},
    setMotors(values){if(!Array.isArray(values)||values.length!==4||values.some(v=>!Number.isFinite(v.duty)||v.duty<0||v.duty>1||!Number.isFinite(v.frequencyHz)||v.frequencyHz<0||v.frequencyHz>clockHz||v.frequencyHz===0&&v.duty!==0))throw new Error('Invalid routed motor PWM');pwm=values.map(({duty,frequencyHz})=>({duty,frequencyHz}));},
    sensors,
    snapshot(){return {enabled:true,model:FLIGHT_MODEL,calibrated:false,source:'guest-pwm-rigid-body',epoch,cycle,guestTime:cycle/clockHz,worldFrame:'NED',bodyFrame:'FRD',positionM:position.slice(),velocityMps:velocity.slice(),quaternionXyzw:q.slice(),angularVelocityDpsFRD:scale(omega,180/Math.PI),heightM:-position[2]||0,contact,motors:pwm.map((p,i)=>({id:IDS[i],gpio:PINS[i],...p,omegaRadS:speeds[i],thrustN:thrusts[i]})),sensors:sensors()};},
  };
}
