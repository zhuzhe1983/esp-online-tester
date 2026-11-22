#!/usr/bin/env node
// Real Chromium, local Three.js model loader and public Worker/WASM API.
// Usage: NODE_PATH=<playwright package root> node tools/check-lilygo-expansion-browser.mjs --url http://localhost:8765/platform/ --output output/lilygo-review
import {createRequire} from 'node:module';
import {mkdir,writeFile,readFile} from 'node:fs/promises';
import {resolve} from 'node:path';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
import {lilygoC5Probe} from './fixtures/lilygo-c5-probe.mjs';
const require=createRequire(import.meta.url),{chromium}=require('playwright');
const arg=name=>process.argv[process.argv.indexOf(name)+1];
assert(process.argv.includes('--url'),'Supply the workbench --url.');
const output=resolve(process.argv.includes('--output')?arg('--output'):'output/lilygo-review');
await mkdir(output,{recursive:true});
const browser=await chromium.launch({headless:true,executablePath:process.env.CHROMIUM_PATH||'/usr/bin/chromium',args:['--no-sandbox','--use-angle=swiftshader','--enable-unsafe-swiftshader']});
const page=await browser.newPage({viewport:{width:1440,height:1050}}),errors=[],servedWasm=[];
page.on('pageerror',error=>errors.push(error.message));
page.on('response',response=>{if(new URL(response.url()).pathname.endsWith('/esp32sim.wasm'))servedWasm.push(response.body().then(bytes=>({sha256:createHash('sha256').update(bytes).digest('hex'),bytes:bytes.length})).catch(error=>({error:error.message})));});
const receipt={schemaVersion:'1',checkedAt:new Date().toISOString(),browser:await browser.version(),backend:'Chromium / local Three.js / real Worker and WASM',checks:[],limitations:['Visual load and protocol diagnostics do not verify physical model fit or factory firmware compatibility.','The C5 and S2 probes are original SRAM diagnostic code, not manufacturer images.']};
try{
  await page.goto(arg('--url'));
  await page.waitForFunction(async()=>window.espTester&&!document.querySelector('#manufacturer-select')?.disabled&&(await espTester.observe()).scene);
  await page.locator('#manufacturer-select').selectOption('LILYGO');
  await page.waitForFunction(async()=>!document.querySelector('#product-select').disabled&&(await espTester.observe()).product?.manufacturer==='LILYGO');
  await page.locator('#product-family-select').selectOption('');
  await page.waitForFunction(()=>{const options=[...document.querySelector('#product-select').options];return options.length===127&&options.every(option=>option.value.startsWith('lilygo-'));});
  const order=await page.evaluate(async()=>{
    const items=(await (await fetch('./catalog/products.json')).json()).items;
    return [...document.querySelector('#product-select').options].map(option=>{const p=items.find(p=>p.id===option.value);return {id:p.id,date:p.chronology?.date||null};});
  });
  assert.equal(order.length,127);let previous='9999-12-31';
  for(const item of order){const date=item.date||'';assert(date<=previous,`${item.id} breaks newest-first ordering`);previous=date;}
  assert.equal(await page.locator('#product-select optgroup').count(),0,'Family grouping must not break release chronology');
  receipt.chronology=order;receipt.checks.push('127 exact products displayed newest-first across families');
  receipt.models=await page.evaluate(async()=>{
    const {createManufacturerModel}=await import('./manufacturer-model.js');
    const THREE=await import('./vendor/three.module.min.js');
    const products=(await (await fetch('./catalog/products.json')).json()).items.filter(p=>p.manufacturer==='LILYGO');
    const results=[],renderer=new THREE.WebGLRenderer({antialias:false,preserveDrawingBuffer:true});
    renderer.setSize(256,256);renderer.setClearColor(0xeef2f7);
    const scene=new THREE.Scene(),light=new THREE.DirectionalLight(0xffffff,2);
    light.position.set(1,2,3);scene.add(light,new THREE.AmbientLight(0xffffff,2));
    const gl=renderer.getContext(),pixels=new Uint8Array(256*256*4);
    try{for(const product of products){
      const model=createManufacturerModel(product,null);await model.ready;
      try{
        let meshes=0;model.group.traverse(object=>{if(object.isMesh)meshes++;});
        if(!meshes)throw Error(product.id+' has no renderable meshes');
        scene.add(model.group);model.group.updateMatrixWorld(true);
        const bounds=new THREE.Box3().setFromObject(model.group),center=bounds.getCenter(new THREE.Vector3()),size=bounds.getSize(new THREE.Vector3());
        const span=Math.max(size.x,size.y,size.z)*1.6,camera=new THREE.OrthographicCamera(-span/2,span/2,span/2,-span/2,.01,span*10);
        camera.position.copy(center).add(new THREE.Vector3(span*.45,span*.3,span*3));camera.lookAt(center);
        renderer.render(scene,camera);gl.readPixels(0,0,256,256,gl.RGBA,gl.UNSIGNED_BYTE,pixels);
        const error=gl.getError();if(error!==gl.NO_ERROR)throw Error(product.id+' WebGL error '+error);
        let visiblePixels=0;for(let i=4;i<pixels.length;i+=4)if(pixels[i]!==pixels[0]||pixels[i+1]!==pixels[1]||pixels[i+2]!==pixels[2])visiblePixels++;
        if(!visiblePixels||!renderer.info.render.triangles)throw Error(product.id+' rendered an empty image');
        results.push({id:product.id,meshes,screen:!!model.screen,mode:product.model.mode||product.model.type,renderedTriangles:renderer.info.render.triangles,visiblePixels,webglError:error});
      }finally{
        scene.remove(model.group);model.dispose();model.group.traverse(object=>{object.geometry?.dispose();for(const material of [].concat(object.material||[]))material.dispose();});renderer.renderLists.dispose();
      }
    }}finally{renderer.dispose();renderer.forceContextLoss();}
    return results;
  });
  assert.equal(receipt.models.length,127);receipt.checks.push('127 manufacturer models fetched, constructed and rendered through WebGL with nonempty pixel output');
  const selected=['lilygo-t7-c5','lilygo-t-dongle-c5','lilygo-t-display-s3-pro-external','lilygo-t-display-p4','lilygo-t-deck-pro','lilygo-t-watch-ultra'];
  receipt.views=[];
  for(const id of selected){
    await page.evaluate(async id=>{const{loadCatalog,createDefaultScene}=await import('./catalog.js');const scene=createDefaultScene(await loadCatalog(),id);scene.firmware=null;scene.view.deviceView='3d';await espTester.configure(scene);},id);
    await page.waitForFunction(()=>document.querySelector('#scene-mount').dataset.modelStatus==='ready');
    await page.locator('#reset-view').click();
    await page.screenshot({path:resolve(output,id+'.png')});
    const detail=await page.evaluate(()=>({origin:document.querySelector('#model-origin').textContent,specs:document.querySelector('#product-specs').textContent}));
    receipt.views.push({id,...detail});
  }
  const probe=lilygoC5Probe();receipt.firmware={sha256:createHash('sha256').update(probe).digest('hex'),bytes:probe.length,kind:'original-rv32i-sram-diagnostic'};
  await page.evaluate(async bytes=>{
    const{loadCatalog,createDefaultScene}=await import('./catalog.js');const scene=createDefaultScene(await loadCatalog(),'lilygo-t-dongle-c5');scene.firmware=null;scene.view.deviceView='3d';await espTester.configure(scene);
    await espTester.loadFirmware({file:new File([new Uint8Array(bytes)],'lilygo-c5-sram.bin')});await espTester.start();
  },[...probe]);
  await page.evaluate(()=>espTester.waitFor({port:'uart0',serialIncludes:'FRAME COMPLETE\n',timeoutMs:30000}));
  const waitColor=async (color,width=80,height=160)=>{
    await page.waitForFunction(async({color,width,height})=>{try{const f=await espTester.capture({format:'rgba'});return f.width===width&&f.height===height&&f.pixels.every((v,i)=>v===[...color,255][i%4]);}catch{return false;}},{color,width,height});
  };
  await waitColor([255,0,0]);
  await page.screenshot({path:resolve(output,'lilygo-t-dongle-c5-guest-red.png')});
  for(const [pressed,color,marker] of [[true,[0,255,0],'BOOT DOWN\n'],[false,[255,0,0],'BOOT UP\n']]){
    await page.evaluate(async({pressed,marker})=>{const ready=espTester.waitFor({port:'uart0',serialIncludes:marker,fromNow:true,timeoutMs:30000});await espTester.input({type:'button',id:'boot',pressed});await ready;},{pressed,marker});
    await waitColor(color);
    if(pressed)await page.screenshot({path:resolve(output,'lilygo-t-dongle-c5-guest-green.png')});
  }
  await page.evaluate(()=>espTester.pause());
  receipt.checks.push('Real RV32I GPIO software SPI changes all 80x160 pixels after actual BOOT press and release');
  const s2Probe=await readFile(new URL('../esp32s2/tests/fixtures/dongle-sram-diagnostic.bin',import.meta.url));
  receipt.s2Firmware={sha256:createHash('sha256').update(s2Probe).digest('hex'),bytes:s2Probe.length,kind:'original-xtensa-s2-sram-diagnostic'};
  await page.evaluate(async bytes=>{
    const{loadCatalog,createDefaultScene}=await import('./catalog.js');const scene=createDefaultScene(await loadCatalog(),'lilygo-t-dongle-esp32-s2-1-14-inch-lcd-board');scene.firmware=null;await espTester.configure(scene);
    await espTester.loadFirmware({file:new File([new Uint8Array(bytes)],'lilygo-s2-sram.bin')});await espTester.start();
  },[...s2Probe]);
  await page.evaluate(()=>espTester.waitFor({port:'uart0',serialIncludes:'FRAME RED\n',timeoutMs:30000}));
  await waitColor([255,0,0],135,240);
  for(const [pressed,color,marker] of [[true,[0,255,0],'FRAME GREEN\n'],[false,[255,0,0],'FRAME RED\n']]){
    await page.evaluate(async({pressed,marker})=>{const ready=espTester.waitFor({port:'uart0',serialIncludes:marker,fromNow:true,timeoutMs:30000});await espTester.input({type:'button',id:'boot',pressed});await ready;},{pressed,marker});
    await waitColor(color,135,240);
    if(pressed)await page.screenshot({path:resolve(output,'lilygo-t-dongle-s2-guest-green.png')});
  }
  await page.evaluate(()=>espTester.pause());
  receipt.checks.push('Independent S2 Xtensa guest changes all 135x240 pixels through its GPIO software SPI after BOOT input');
  const wasmArtifacts=await Promise.all(servedWasm);
  assert(wasmArtifacts.length>0&&wasmArtifacts.every(item=>item.sha256),'Capture the WASM bytes actually fetched by the browser');
  assert.equal(new Set(wasmArtifacts.map(item=>item.sha256)).size,1,'One consistent served WASM artifact per check');
  receipt.wasmSha256=wasmArtifacts[0].sha256;receipt.wasmBytes=wasmArtifacts[0].bytes;
  receipt.pageErrors=errors;assert.deepEqual(errors,[]);receipt.status='pass';
  await writeFile(resolve(output,'browser-check.json'),JSON.stringify(receipt,null,2)+'\n');
  console.log(JSON.stringify({status:receipt.status,modelCount:receipt.models.length,checks:receipt.checks,pageErrors:errors}));
}catch(error){
  receipt.status='fail';receipt.error=error.message;receipt.pageErrors=errors;
  await writeFile(resolve(output,'browser-check.json'),JSON.stringify(receipt,null,2)+'\n');
  await page.screenshot({path:resolve(output,'failure.png'),timeout:10000}).catch(()=>{});
  throw error;
}finally{await page.evaluate(()=>window.espTester?.dispose()).catch(()=>{});await browser.close();}
