// Run through playwright_cli.sh run-code after opening the caller-supplied platform URL.
// Uses a test-only metadata route; no catalog/model assets are modified.
async page => {
 const base=new URL('./',page.url());if(!['http:','https:'].includes(base.protocol))throw Error('Open the caller-supplied platform URL before running this check');
 const context=page.context(),errors=[],result={fixture:'S007 geometry with test-only declared cover group; no firmware execution',checks:[]};
 const pattern='**/assets/m5stack-s007/model.json';
 await context.route(pattern,async route=>{const response=await route.fetch(),data=await response.json();data.presentation.inspectionGroups=[{id:'cover',parts:['cad-component-0','cover-screw','cover-key:button'],offsetMm:[12,0,10],sourceIds:[data.provenance.sources.find(source=>source.id.startsWith('manufacturer-cad')).id],notes:'UI fixture only: declare a visible viewing offset for the existing cover, its photographic twin and control hit surface.'}];await route.fulfill({response,json:data});});
 const onError=error=>errors.push(error.message);page.on('pageerror',onError);let mini;
 const ready=async p=>{await p.waitForFunction(()=>!!window.espTester);await p.waitForFunction(()=>document.querySelector('#scene-mount')?.dataset.modelStatus==='ready');};
 const configure=async(p,id)=>{await p.evaluate(async id=>{const m=await import('./catalog.js'),catalog=await m.loadCatalog();await espTester.configure(m.createDefaultScene(catalog,id));},id);await ready(p);};
 const state=async p=>Number(await p.locator('#inspect-model').inputValue());
 const set=async(p,key,value)=>{
  await p.locator('#inspect-model').press(key);
  await p.waitForFunction(value=>document.getElementById('inspection-percent').textContent===`${value}%`,value);
 };
 const check=(ok,message)=>{if(!ok)throw Error(message);result.checks.push(message);};
 const layout=async p=>p.evaluate(()=>{
  const bounds=id=>document.getElementById(id).getBoundingClientRect();
  const two=bounds('view-2d'),capture=bounds('capture-frame'),slider=bounds('inspection-control'),shell=bounds('shell-settings');
  const centered=(a,b)=>Math.abs(a.y+a.height/2-b.y-b.height/2)<2;
  return two.right<=capture.left&&centered(two,capture)&&slider.right<=shell.left&&centered(slider,shell);
 });
 try {
  await page.goto(new URL('?lang=zh-CN',base).href);await page.setViewportSize({width:1280,height:900});await page.waitForFunction(()=>!!window.espTester);await configure(page,'m5stack-s007');
  const slider=page.locator('#inspect-model');await slider.waitFor({state:'visible'});const canvas=page.locator('#scene-mount canvas').first();await page.waitForTimeout(100);
  check(await layout(page),'capture follows 3D/2D and the slider sits immediately left of the shell controls');
  const before=await canvas.screenshot();const runtimeBefore=await page.evaluate(async()=>{const s=await espTester.observe();return {status:s.status,firmware:s.firmware,scene:s.scene};});
  await set(page,'ArrowRight',1);
  check(await state(page)===1,'keyboard adjusts the exploded view by one percent');
  await set(page,'End',100);await page.waitForTimeout(100);
  const expanded=await canvas.screenshot({path:'output/playwright/inspection-workbench-expanded.png'});
  check(!before.equals(expanded),'actual rendered geometry changes at full separation');
  const orbitRect=await canvas.boundingBox();await page.mouse.move(orbitRect.x+10,orbitRect.y+10);await page.mouse.down();await page.mouse.move(orbitRect.x+10+Math.PI*50,orbitRect.y+10,{steps:8});await page.mouse.up();await page.waitForTimeout(150);check(!expanded.equals(await canvas.screenshot({path:'output/playwright/inspection-workbench-orbit.png'})),'public pointer orbit changes the separated model view');
  await page.evaluate(async()=>{const {scene}=await espTester.observe();await espTester.updateSceneMetadata({view:{...scene.view,appearance:{shell:{color:'#8456cf',opacity:.4,finish:'plastic'}}}});});
  check(await state(page)===100,'appearance update preserves separation');
  const shared=await page.evaluate(()=>espTester.share({mode:'embed',autostart:false}));
  const current=await page.evaluate(async()=>{const s=await espTester.observe();return {status:s.status,firmware:s.firmware,scene:s.scene};});check(current.status===runtimeBefore.status&&JSON.stringify(current.firmware)===JSON.stringify(runtimeBefore.firmware),'inspection leaves firmware lifecycle untouched');check(!JSON.stringify(current.scene).includes('inspection'),'inspection absent from scene/share data');
  await page.locator('#reset-view').click();await page.waitForTimeout(100);check(await state(page)===0,'reset view restores assembly');
  await page.evaluate(async()=>{const {scene}=await espTester.observe();const view={...scene.view};delete view.appearance;await espTester.updateSceneMetadata({view});});await page.waitForTimeout(100);
  check(before.equals(await canvas.screenshot({path:'output/playwright/inspection-workbench-restored.png'})),'reset restores original rendered pixels without drift');
  for(let i=0;i<3;i++){await set(page,'End',100);await set(page,'Home',0);}
  check(await state(page)===0,'repeated slider movement returns assembled');
  await set(page,'End',100);await page.locator('#view-2d').click();check(await slider.isHidden(),'2D hides the slider');await page.locator('#view-3d').click();check(await state(page)===0,'return from 2D is assembled');
  await set(page,'End',100);await configure(page,'m5stack-k130');check(await slider.isHidden(),'product without declarations has no slider');await configure(page,'m5stack-s007');check(await state(page)===0,'product replacement clears separation');
  await set(page,'End',100);await page.evaluate(async()=>{const {scene}=await espTester.observe();scene.accessories=[{id:'inspection-accessory',catalogId:'m5stack-gps-u032',parameters:{},wiring:{mode:'visual'}}];await espTester.configure(scene);});await ready(page);check(await slider.isDisabled()&&await state(page)===0,'external accessory restores assembly and disables separation');await page.locator('#clear-accessories').click();await ready(page);check(await slider.isEnabled()&&await state(page)===0,'removing accessories enables the slider');
  mini=await context.newPage();mini.on('pageerror',error=>errors.push(error.message));await mini.setViewportSize({width:390,height:844});await mini.goto(typeof shared==='string'?shared:shared.url);await ready(mini);await mini.locator('#inspect-model').waitFor({state:'visible'});check(await state(mini)===0,'shared embed opens assembled');
  const appearance=await mini.evaluate(async()=>(await espTester.observe()).scene.view.appearance);check(appearance?.shell?.color==='#8456cf','embed restores shared appearance');
  check(await layout(mini),'mobile embed keeps capture and exploded view adjacent to their controls');
  await set(mini,'End',100);check(await state(mini)===100,'embedded public slider separates model');await mini.screenshot({path:'output/playwright/inspection-embed-mobile.png',fullPage:true});
  check(await mini.evaluate(()=>document.documentElement.scrollWidth===innerWidth),'embed 390px has no horizontal overflow');await mini.locator('#reset-view').click();check(await state(mini)===0,'embed reset restores assembly');
  result.pageErrors=errors;check(errors.length===0,'no browser page errors');return result;
 }finally{try{await mini?.close();}finally{await context.unroute(pattern);page.off('pageerror',onError);}}
}
