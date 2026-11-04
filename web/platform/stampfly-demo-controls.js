import {hasMotorOutput, STAMPFLY_MOTORS} from './motor-state.js';
import {flightAttitudeDegrees} from './flight-view.js';

const READY = 'SF1 READY stampfly-uart-demo 0.1.0';
const modes = new Set(['STOP','MOTOR','AUTO','LAND']);
const finite = (value, min, max) => Number.isFinite(value) && value >= min && value <= max;
export function isStampflyDemo(state) {
  const metadata = state?.firmware?.metadata;
  return hasMotorOutput(state?.product?.runtime?.board) && metadata?.source === 'esp-app-description' && metadata.name === 'stampfly_uart_demo' && metadata.version === '0.1.0';
}
export function parseStampflyDemoLine(line) {
  if (line === READY) return {type:'ready'};
  const words = line.split(' ');
  if (words[0] !== 'SF1') return null;
  if (words.length === 3 && words[1] === 'ACK' && ['STOP','MOTOR','TAKEOFF','LAND','TARGET'].includes(words[2])) return {type:'ack',command:words[2]};
  if (words.length === 3 && words[1] === 'ERR' && ['FORMAT','RANGE','NOT_READY','MODE','LINE_TOO_LONG'].includes(words[2])) return {type:'error',code:words[2]};
  if (words.length !== 14 || words[1] !== 'STATE' || !modes.has(words[2])) return null;
  const n = words.slice(3).map(Number);
  if (words.slice(3).some(v=>!v.length) || !n.every(Number.isFinite) || !Number.isSafeInteger(n[0]) || n[0]<0 || n.slice(5,9).some(v=>!finite(v,0,1)) || n.slice(9).some(v=>![0,1].includes(v))) return null;
  return {type:'telemetry',mode:words[2],milliseconds:n[0],heightM:n[1],rollDeg:n[2],pitchDeg:n[3],yawRateDps:n[4],duties:n.slice(5,9),imuValid:!!n[9],tofValid:!!n[10]};
}
export function stampflyCommand(command, values = []) {
  const valid = command === 'STOP' || command === 'LAND' ? values.length === 0
    : command === 'MOTOR' ? values.length === 4 && values.every(v=>finite(v,0,1))
    : command === 'TAKEOFF' ? values.length === 1 && finite(values[0],.1,1.5)
    : command === 'TARGET' ? values.length === 4 && finite(values[0],.1,1.5) && values.slice(1,3).every(v=>finite(v,-10,10)) && finite(values[3],-45,45) : false;
  if (!valid) throw Error('控制值超出范围。');
  return `SF1 ${command}${values.length?' '+values.map(v=>Number(v.toFixed(4))).join(' '):''}\n`;
}

// Commands only enter UART RX. Readiness and acknowledgements come from the guest;
// the independent motor/flight stream remains the sole source of physical output.
export function createStampflyDemoController({runtime, onChange = () => {}, setTimer = setTimeout, clearTimer = clearTimeout}) {
  let state = {}, ready = false, telemetry = null, fragment = '', pending = null, feedback = '', disposed = false, generation = 0, timer = null;
  let first = true;
  const snapshot = () => ({active:hasMotorOutput(state.product?.runtime?.board),matched:isStampflyDemo(state),status:state.status,ready,telemetry,flight:state.flight || null,motors:state.motors || [],motorOutput:state.motorOutput || null,pending:pending ? {command:pending.command,acknowledged:pending.acknowledged} : null,feedback});
  const publish = () => {if (!disposed) onChange(snapshot());};
  const cancel = () => {clearTimer(timer);timer=null;pending=null;generation++;};
  function reset() {cancel();ready=false;telemetry=null;fragment='';feedback='';}
  function checkOutput() {
    if (!pending?.acknowledged) return;
    const {command,values,cycle} = pending;
    let confirmed = false;
    if (command === 'STOP' || command === 'MOTOR') {
      const expected = command === 'STOP' ? [0,0,0,0] : values;
      confirmed = Number.isFinite(state.motorOutput?.cycle) && state.motorOutput.cycle > cycle && STAMPFLY_MOTORS.every(({id},i)=>{
        const motor=state.motors?.find(m=>m.id===id);return motor && Math.abs(motor.duty-expected[i])<=1/255;
      });
    } else if (command === 'TAKEOFF') confirmed = state.flight?.enabled === true && state.flight.cycle > pending.flightCycle && state.flight.contact === false && state.flight.heightM > .05;
    else if (command === 'LAND') confirmed = state.flight?.enabled === true && state.flight.cycle > pending.flightCycle && state.flight.contact === true && state.flight.heightM <= .01;
    else if (command === 'TARGET') confirmed = telemetry?.mode === 'AUTO' && telemetry.milliseconds > pending.telemetryTime;
    if (confirmed) {feedback = command === 'STOP' ? '已收到 STOP 回执，实际 PWM 已归零。' : command === 'MOTOR' ? '已收到 MOTOR 回执，实际 PWM 已更新。' : command === 'TAKEOFF' ? '已收到 TAKEOFF 回执，物理状态已离地。' : command === 'LAND' ? '已收到 LAND 回执，物理状态已接地。' : '已收到 TARGET 回执及新的机载遥测；实际姿态见观测值。';cancel();}
  }
  function serial(data, historical = false) {
    if (!isStampflyDemo(state)) return;
    fragment += data;
    const lines=fragment.split('\n');fragment=lines.pop().slice(-512);
    for (const raw of lines) {
      const line=parseStampflyDemoLine(raw.replace(/\r$/,''));if(!line)continue;
      if(line.type==='ready'){const tail=fragment;reset();fragment=tail;ready=true;feedback='演示固件已就绪，等待手动操作。';}
      else if(line.type==='telemetry')telemetry=line;
      else if(!historical && line.type==='ack' && pending?.command===line.command){pending.acknowledged=true;feedback=`已收到 ${line.command} 回执，等待实际输出。`;}
      else if(!historical && line.type==='error' && pending){feedback=`固件拒绝命令：${line.code}。`;cancel();}
    }
    checkOutput();publish();
  }
  function accept(event) {
    if(disposed)return;
    if(event.type==='state') {
      const next=event.state || {}, changed=next.scene?.productId!==state.scene?.productId || JSON.stringify(next.firmware)!==JSON.stringify(state.firmware);
      if(changed || ['idle','configured','loading','ready','error','disposed'].includes(next.status))reset();
      else if(next.status==='paused' && state.status!=='paused'){cancel();feedback='已暂停，PWM 观测值保留；螺旋桨与飞行状态冻结。';}
      else if(next.status==='running' && state.status==='paused')feedback='已继续运行，观测值来自固件与物理状态。';
      state=next;
      if(first && ['running','paused'].includes(state.status))serial(state.serialByPort?.uart0 || '',true);
      first=false;checkOutput();publish();
    } else if(event.type==='serial' && event.source==='uart0')serial(event.data);
    else if(event.type==='motors'){state={...state,motors:event.motors,motorOutput:event.motorOutput};checkOutput();publish();}
    else if(event.type==='flight'){state={...state,flight:event.flight};checkOutput();publish();}
  }
  const unsubscribe=runtime.subscribe(accept);
  const observedGeneration=generation;
  Promise.resolve(runtime.observe()).then(value=>{if(first && generation===observedGeneration)accept({type:'state',state:value});}).catch(()=>{});
  async function send(command, values=[]) {
    const data=stampflyCommand(command,values);
    if(!isStampflyDemo(state) || !ready || state.status!=='running')throw Error('请启动网页演示固件，并等待 READY。');
    if(pending && command!=='STOP')throw Error('请等待当前命令的固件回执与实际输出。');
    if(['TAKEOFF','LAND','TARGET'].includes(command) && state.flight?.enabled!==true)throw Error('请先暂停并启用近似物理飞行环境。');
    if(command==='TARGET' && telemetry?.mode!=='AUTO')throw Error('请先起飞，再调整目标。');
    cancel();const token=generation;
    pending={command,values:[...values],acknowledged:false,cycle:state.motorOutput?.cycle ?? -1,flightCycle:state.flight?.cycle ?? -1,telemetryTime:telemetry?.milliseconds ?? -1};
    feedback=`已发送 ${command}，等待固件回执。`;publish();
    timer=setTimer(()=>{if(token===generation){feedback='等待超时：未确认命令回执及实际输出。可查看串口或点击停止。';cancel();publish();}},30000);
    try {await runtime.input({type:'serial',port:'uart0',data});}
    catch(error){if(token===generation){feedback=error.message;cancel();publish();}throw error;}
  }
  async function stop() {
    if(!isStampflyDemo(state))throw Error('当前固件不是网页演示固件。');
    if(state.status==='running' && ready)return send('STOP');
    if(['running','paused','ready'].includes(state.status)){reset();publish();await runtime.reset();return;}
    throw Error('当前没有可停止的演示固件。');
  }
  async function configureFlight(enabled) {
    if(!isStampflyDemo(state) || !['ready','paused'].includes(state.status))throw Error('请先加载演示固件，并暂停运行。');
    if(typeof runtime.configureFlight!=='function')throw Error('当前运行时尚未提供飞行环境。');
    await runtime.configureFlight({enabled});
  }
  return {snapshot,send,stop,configureFlight,dispose(){disposed=true;cancel();unsubscribe();}};
}

export function mountStampflyDemoControls({container,runtime}) {
  container.hidden=true;
  const element=(tag,text,className)=>{const n=document.createElement(tag);if(text)n.textContent=text;if(className)n.className=className;return n;};
  const panel=element('section',null,'stampfly-demo');panel.setAttribute('aria-label','StampFly 网页演示');container.append(panel);
  const heading=element('div',null,'stampfly-demo-heading'),title=element('strong','StampFly UART 遥控演示'),stop=element('button','停止','button stampfly-stop');stop.type='button';stop.dataset.demoAction='stop';heading.append(title,stop);panel.append(heading);
  const message=element('p',null,'stampfly-demo-message');message.setAttribute('role','status');panel.append(message);
  const body=element('div',null,'stampfly-demo-body');panel.append(body);
  const motorFields=element('div',null,'stampfly-motor-fields');body.append(motorFields);
  const inputs=STAMPFLY_MOTORS.map(({id},i)=>{const label=element('label',['左前','右前','左后','右后'][i]),input=element('input'),value=element('output','0%');input.type='range';input.min='0';input.max='100';input.step='1';input.value='0';input.dataset.demoMotor=id;input.setAttribute('aria-label',`${label.textContent}电机目标占空比`);input.addEventListener('input',()=>value.textContent=`${input.value}%`);label.append(input,value);motorFields.append(label);return input;});
  const actions=element('div',null,'stampfly-demo-actions');body.append(actions);
  const button=(label,action,parent=actions)=>{const n=element('button',label,'button');n.type='button';n.dataset.demoAction=action;parent.append(n);return n;};
  const motor=button('应用电机占空比','motor'),preset=button('四桨 20%','preset');
  const actual=element('p',null,'stampfly-demo-actual');actual.dataset.demoOutput='motors';body.append(actual);
  const flight=element('details',null,'stampfly-flight-options');flight.append(element('summary','近似物理飞行'));body.append(flight);
  const limits=element('p','使用未校准的简化物理模型；不代表真机性能。电机演示也会产生推力。启用或关闭环境后需手动启动。','helper');flight.append(limits);
  const enable=button('启用飞行环境','flight-toggle',flight);
  const fields=element('div',null,'stampfly-flight-fields');flight.append(fields);
  const field=(label,min,max,step,value)=>{const row=element('label',label),input=element('input');Object.assign(input,{type:'number',min:String(min),max:String(max),step:String(step),value:String(value)});row.append(input);fields.append(row);return input;};
  const height=field('目标高度 / m',.1,1.5,.05,.3),roll=field('横滚 / °',-10,10,1,0),pitch=field('俯仰 / °',-10,10,1,0),yaw=field('偏航速率 / °/s',-45,45,5,0);
  const flightActions=element('div',null,'stampfly-demo-actions');flight.append(flightActions);
  const takeoff=button('起飞并保持高度','takeoff',flightActions),land=button('降落','land',flightActions),target=button('应用目标','target',flightActions);
  const flightOutput=element('p',null,'stampfly-demo-actual');flightOutput.dataset.demoOutput='flight';flight.append(flightOutput);
  const note=element('p','螺旋桨转动仅按真实客体 PWM 示意，不是实测转速。停止按钮切断演示固件电机输出；暂停时点击停止会复位。','helper');body.append(note);
  let current={},dead=false;
  const controller=createStampflyDemoController({runtime,onChange:value=>{
    if(dead)return;
    if((!value.ready && current.ready) || value.matched!==current.matched || value.active!==current.active)for(const input of inputs){input.value='0';input.dispatchEvent(new Event('input'));}
    current=value;container.hidden=!value.active;body.hidden=!value.matched;
    const running=value.matched && value.ready && value.status==='running';
    message.textContent=!value.matched?'在固件列表选择「UART 遥控演示」并运行，即可控制四个电机。原厂固件使用其自己的控制方式。':value.feedback || (value.status==='running'?'等待演示固件 READY…':'请点击运行；启动和复位时电机保持关闭。');
    stop.disabled=!value.matched || !['running','paused','ready'].includes(value.status);stop.textContent=value.status==='paused'?'停止并复位':'停止';
    for(const input of inputs)input.disabled=!running;
    motor.disabled=preset.disabled=!running || !!value.pending;
    enable.disabled=!['ready','paused'].includes(value.status) || typeof runtime.configureFlight!=='function';enable.textContent=value.flight?.enabled?'关闭飞行环境':'启用飞行环境';
    const flying=running && value.flight?.enabled===true && !value.pending;
    takeoff.disabled=land.disabled=!flying;target.disabled=!flying || value.telemetry?.mode!=='AUTO';
    actual.textContent='实际 PWM：'+STAMPFLY_MOTORS.map(({id},i)=>`${['左前','右前','左后','右后'][i]} ${Number.isFinite(value.motors.find(m=>m.id===id)?.duty)?(value.motors.find(m=>m.id===id).duty*100).toFixed(1)+'%':'—'}`).join(' · ');
    const angles=flightAttitudeDegrees(value.flight),attitude=angles?` · 横滚 ${angles[0].toFixed(1)}° / 俯仰 ${angles[1].toFixed(1)}° / 偏航 ${angles[2].toFixed(1)}°`:'';
    const position=value.flight?.positionM,translation=Array.isArray(position)&&position.slice(0,2).every(Number.isFinite)?` · 北 ${position[0].toFixed(2)} / 东 ${position[1].toFixed(2)} m`:'';
    flightOutput.textContent=value.flight?.enabled?`实际高度 ${Number.isFinite(value.flight.heightM)?value.flight.heightM.toFixed(3):'—'} m${translation}${attitude} · ${value.flight.contact?'已接地':'未接地'} · ${value.status==='paused'?'已冻结':'由真实 PWM 驱动物理模型'}`:'飞行环境未启用；当前仅显示电机 PWM。';
  }});
  const act=work=>{Promise.resolve().then(work).catch(error=>{if(!dead)message.textContent=error.message;});};
  motor.onclick=()=>act(()=>controller.send('MOTOR',inputs.map(input=>Number(input.value)/100)));
  preset.onclick=()=>{for(const input of inputs){input.value='20';input.dispatchEvent(new Event('input'));}act(()=>controller.send('MOTOR',[.2,.2,.2,.2]));};
  stop.onclick=()=>act(()=>controller.stop());enable.onclick=()=>act(()=>controller.configureFlight(!current.flight?.enabled));
  takeoff.onclick=()=>act(()=>controller.send('TAKEOFF',[Number(height.value)]));land.onclick=()=>act(()=>controller.send('LAND'));
  target.onclick=()=>act(()=>controller.send('TARGET',[height,roll,pitch,yaw].map(input=>Number(input.value))));
  return {dispose(){dead=true;controller.dispose();panel.remove();}};
}
