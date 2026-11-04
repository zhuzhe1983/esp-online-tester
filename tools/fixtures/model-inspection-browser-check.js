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
 const state=async p=>p.locator('#inspect-model').getAttribute('aria-pressed');
 const check=(ok,message)=>{if(!ok)throw Error(message);result.checks.push(message);};
 try {
  await page.goto(new URL('?lang=zh-CN',base).href);await page.setViewportSize({width:1280,height:900});await page.waitForFunction(()=>!!window.espTester);await configure(page,'m5stack-s007');
  const button=page.locator('#inspect-model');await button.waitFor({state:'visible'});const canvas=page.locator('#scene-mount canvas').first();await page.waitForTimeout(100);
  const before=await canvas.screenshot();const runtimeBefore=await page.evaluate(async()=>{const s=await espTester.observe();return {status:s.status,firmware:s.firmware,scene:s.scene};});
  await button.click();await page.waitForTimeout(100);check(await state(page)==='true','workbench button separates declared model');const expanded=await canvas.screenshot({path:'output/playwright/inspection-workbench-expanded.png'});check(!before.equals(expanded),'actual rendered geometry changes');
  const orbitRect=await canvas.boundingBox();await page.mouse.move(orbitRect.x+10,orbitRect.y+10);await page.mouse.down();await page.mouse.move(orbitRect.x+10+Math.PI*50,orbitRect.y+10,{steps:8});await page.mouse.up();await page.waitForTimeout(150);check(!expanded.equals(await canvas.screenshot({path:'output/playwright/inspection-workbench-orbit.png'})),'public pointer orbit changes the separated model view');
  await page.evaluate(async()=>{const {scene}=await espTester.observe();await espTester.updateSceneMetadata({view:{...scene.view,appearance:{shell:{color:'#8456cf',opacity:.4,finish:'plastic'}}}});});
  check(await state(page)==='true','appearance update preserves inspection state');
  const shared=await page.evaluate(()=>espTester.share({mode:'embed',autostart:false}));
  const current=await page.evaluate(async()=>{const s=await espTester.observe();return {status:s.status,firmware:s.firmware,scene:s.scene};});check(current.status===runtimeBefore.status&&JSON.stringify(current.firmware)===JSON.stringify(runtimeBefore.firmware),'inspection leaves firmware lifecycle untouched');check(!JSON.stringify(current.scene).includes('inspection'),'inspection absent from scene/share data');
  await page.locator('#reset-view').click();await page.waitForTimeout(100);check(await state(page)==='false','reset view restores assembly');
  await page.evaluate(async()=>{const {scene}=await espTester.observe();const view={...scene.view};delete view.appearance;await espTester.updateSceneMetadata({view});});await page.waitForTimeout(100);
  check(before.equals(await canvas.screenshot({path:'output/playwright/inspection-workbench-restored.png'})),'reset restores original rendered pixels without drift');
  for(let i=0;i<3;i++){await button.click();await button.click();}check(await state(page)==='false','repeated public toggles return assembled');
  await button.click();await page.locator('#view-2d').click();check(await button.isHidden(),'2D hides inspection');await page.locator('#view-3d').click();check(await state(page)==='false','return from 2D is assembled');
  await button.click();await configure(page,'m5stack-k134');check(await button.isHidden(),'product without declarations has no control');await configure(page,'m5stack-s007');check(await state(page)==='false','product replacement clears inspection');
  await button.click();await page.evaluate(async()=>{const {scene}=await espTester.observe();scene.accessories=[{id:'inspection-accessory',catalogId:'m5stack-gps-u032',parameters:{},wiring:{mode:'visual'}}];await espTester.configure(scene);});await ready(page);check(await button.isDisabled()&&await state(page)==='false','external accessory restores assembly and disables inspection');await page.locator('#clear-accessories').click();await ready(page);check(await button.isEnabled()&&await state(page)==='false','removing accessories enables an assembled model');
  mini=await context.newPage();mini.on('pageerror',error=>errors.push(error.message));await mini.setViewportSize({width:390,height:844});await mini.goto(typeof shared==='string'?shared:shared.url);await ready(mini);const mb=mini.locator('#inspect-model');await mb.waitFor({state:'visible'});check(await state(mini)==='false','shared embed opens assembled');
  const appearance=await mini.evaluate(async()=>(await espTester.observe()).scene.view.appearance);check(appearance?.shell?.color==='#8456cf','embed restores shared appearance');await mb.click();check(await state(mini)==='true','embedded public button separates model');await mini.screenshot({path:'output/playwright/inspection-embed-mobile.png',fullPage:true});
  check(await mini.evaluate(()=>document.documentElement.scrollWidth===innerWidth),'embed 390px has no horizontal overflow');await mini.locator('#reset-view').click();check(await state(mini)==='false','embed reset restores assembly');
  result.pageErrors=errors;check(errors.length===0,'no browser page errors');return result;
 }finally{try{await mini?.close();}finally{await context.unroute(pattern);page.off('pageerror',onError);}}
}
