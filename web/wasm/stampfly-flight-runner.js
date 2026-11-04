import {createFlightPlant} from './stampfly-flight.js';
// Optional guest-time coupling. Not constructed on disabled/non-StampFly boards.
// Holds the last sampled guest PWM for each 1 ms interval. Physics uses 250 us
// steps; freshly read PWM can affect only future intervals, never past motion.
export function createFlightRunner({wasm,emu,clockHz,epoch=0,publish=()=>{}}){
  if(!wasm.esp32sim_flight_motors||!wasm.esp32sim_flight_sample)throw new Error('Flight sensor bridge unavailable');
  const plant=createFlightPlant({clockHz,epoch}),quantum=clockHz/1000,p=wasm.esp32sim_alloc(64);
  if(!Number.isSafeInteger(quantum))throw new Error('Unsupported flight coupling clock');
  let next=quantum,lastPublished=-Infinity,disposed=false;
  function inject(){const s=plant.sensors();if(wasm.esp32sim_flight_sample(emu,...s.rawAccelG,...s.rawGyroDps,s.bottomDistanceM===null?-1:s.bottomDistanceM*1000,s.frontDistanceM===null?-1:s.frontDistanceM*1000)!==0)throw new Error('Flight sensor input rejected');}
  function samplePwm(){if(wasm.esp32sim_flight_motors(emu,p)!==1)throw new Error('Board has no flight motor routes');const v=new DataView(wasm.memory.buffer,p,64);plant.setMotors(Array.from({length:4},(_,i)=>({duty:v.getFloat64(i*16,true),frequencyHz:v.getFloat64(i*16+8,true)})));}
  try{samplePwm();inject();}catch(e){wasm.esp32sim_free(p,64);throw e;}
  function publishNow(force=false){const time=performance.now();if(force||time-lastPublished>=20){lastPublished=time;publish(plant.snapshot());}}
  return {
    run(_emu,budget,wall){if(disposed)throw new Error('Flight runner disposed');let at=wasm.esp32sim_cycles(emu);const target=at+budget;let rc=0;
      while(at<target){const previous=at;rc=wasm.esp32sim_run(emu,Math.max(1,Math.min(target-at,next-at)),wall);at=wasm.esp32sim_cycles(emu);
        if(at<previous)throw new Error('Flight guest clock moved backwards');
        if(at>=next){plant.advanceTo(at);samplePwm();inject();next=(Math.floor(at/quantum)+1)*quantum;}
        if(rc!==0||at===previous)break;
      }
      publishNow();return rc;
    },
    snapshot:()=>plant.snapshot(),publish:publishNow,
    dispose(){if(!disposed){wasm.esp32sim_free(p,64);disposed=true;}},
  };
}
