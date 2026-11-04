async page => {
 const check=(v,m)=>{if(!v)throw Error(m);};
 await page.evaluate(async()=>{
  const {mountStampflyDemoControls}=await import('./stampfly-demo-controls.js');
  const listeners=new Set(),state={status:'running',scene:{productId:'m5stack-k138'},product:{runtime:{board:'stampfly'}},firmware:{metadata:{source:'esp-app-description',name:'stampfly_uart_demo',version:'0.1.0'}},flight:{enabled:true,cycle:1,heightM:0,contact:true},motors:[],motorOutput:{cycle:1}};
  const panel=document.createElement('div');panel.id='demo-mode-probe';Object.assign(panel.style,{position:'fixed',inset:'0',zIndex:'99999',background:'white'});document.body.append(panel);
  const inputs=[],runtime={subscribe(fn){listeners.add(fn);return()=>listeners.delete(fn);},observe:async()=>state,input:async e=>{inputs.push(e);return{delivery:'queued'};},configureFlight:async()=>{},reset:async()=>{}};
  const controls=mountStampflyDemoControls({container:panel,runtime});
  const emit=e=>listeners.forEach(fn=>fn(e));emit({type:'state',state});
  window.modeProbe={panel,controls,emit,inputs};
 });
 const buttons=()=>page.evaluate(()=>Object.fromEntries([...modeProbe.panel.querySelectorAll('[data-demo-action]')].map(e=>[e.dataset.demoAction,e.disabled])));
 try{
  const initial=await buttons();check(initial.motor&&initial.takeoff&&!initial.stop,'initial READY gate');
  await page.evaluate(()=>modeProbe.emit({type:'serial',source:'uart0',data:'SF1 READY stampfly-uart-demo 0.1.0\n'}));
  const ready=await buttons();check(!ready.motor&&!ready.stop&&ready.takeoff&&ready.land,'READY without STATE enables manual motors and Stop only');
  await page.evaluate(()=>modeProbe.emit({type:'serial',source:'uart0',data:'SF1 STATE STOP 1 0 0 0 0 0 0 0 0 1 0\n'}));
  const stopped=await buttons();check(!stopped.takeoff&&stopped.land&&stopped.target,'actual STOP allows takeoff only');
  await page.locator('#demo-mode-probe summary').click();
  await page.locator('#demo-mode-probe [data-demo-action=takeoff]').click();
  const pending=await buttons();check(pending.motor&&pending.takeoff&&!pending.stop,'Stop stays enabled while takeoff awaits output');
  await page.locator('#demo-mode-probe [data-demo-action=stop]').click();
  const input=await page.evaluate(()=>modeProbe.inputs);check(input.length===2&&input[0].data==='SF1 TAKEOFF 0.3\n'&&input[1].data==='SF1 STOP\n','Stop supersedes pending command through UART only');
  return {kind:'Explicit transport-double UI test; not firmware evidence',initial,ready,stopped,pending,input};
 }finally{await page.evaluate(()=>{modeProbe.controls.dispose();modeProbe.panel.remove();delete window.modeProbe;});}
}
