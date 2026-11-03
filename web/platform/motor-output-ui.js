import {hasMotorOutput,STAMPFLY_MOTORS} from './motor-state.js';

// Read-only guest output. LED numbers identify chain order, not photo positions.
export function mountMotorOutputs({container,runtime}) {
  const panel=document.createElement('details');panel.className='motor-output-panel';panel.hidden=true;
  const title=document.createElement('summary');title.textContent='电机与灯光输出';panel.append(title);
  const rows=document.createElement('div');rows.className='motor-output-grid';panel.append(rows);
  const motorLabels=['左前','右前','左后','右后'];
  const motors=STAMPFLY_MOTORS.map(({id},i)=>{const row=document.createElement('div'),name=document.createElement('span'),value=document.createElement('output');name.textContent=motorLabels[i];value.textContent='等待固件输出';value.dataset.motorOutput=id;row.append(name,value);rows.append(row);return value;});
  const lights=document.createElement('div');lights.className='motor-output-lights';panel.append(lights);
  const leds=['Stamp RGB','载板 RGB 1','载板 RGB 2'].map((label,i)=>{const row=document.createElement('span'),swatch=document.createElement('i'),value=document.createElement('output');row.append(swatch,document.createTextNode(label+' '),value);value.dataset.ledOutput=i;value.textContent='—';lights.append(row);return{swatch,value};});
  const note=document.createElement('p');note.className='helper';note.textContent='占空比来自固件 PWM，频率为电驱动频率。未模拟飞行；载板灯号按电路链序显示。';panel.append(note);
  container.append(panel);
  let active=false;
  const updateMotors=values=>{for(let i=0;i<motors.length;i++){const value=values?.find(m=>m.id===STAMPFLY_MOTORS[i].id);motors[i].textContent=value?`${(value.duty*100).toFixed(1)}% · ${(value.frequencyHz/1000).toFixed(2)} kHz`:'等待固件输出';}};
  const updateLeds=values=>{for(let i=0;i<leds.length;i++){const rgb=values?.[i],{swatch,value}=leds[i];swatch.style.backgroundColor=rgb?`rgb(${rgb.join(',')})`:'transparent';value.textContent=rgb?rgb.join(', '):'—';}};
  const unsubscribe=runtime.subscribe(event=>{
    if(event.type==='state'){active=hasMotorOutput(event.state.product?.runtime?.board);panel.hidden=!active;updateMotors(active?event.state.motors:[]);updateLeds(active?event.state.leds:[]);}
    else if(active&&event.type==='motors')updateMotors(event.motors);
    else if(active&&event.type==='leds')updateLeds(event.leds);
  });
  return{dispose(){unsubscribe();panel.remove();}};
}
