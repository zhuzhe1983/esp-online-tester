// Exercise the real workbench and embed bridge in an isolated browser profile.
// No firmware is loaded and no application/physical-hardware success is inferred.
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {resolve,join} from 'node:path';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';

const [base,output,...selections]=process.argv.slice(2);
if(!base||!output||!selections.length)throw Error('Usage: node check-waveshare-platform-browser.mjs <platform URL> <output dir> <selection.json...>');
const {chromium}=await import(process.env.PLAYWRIGHT_MODULE||'playwright');
const root=new URL('../',import.meta.url),catalog=JSON.parse(await readFile(new URL('web/platform/catalog/products.json',root))).items;
const productIds=[...new Set((await Promise.all(selections.map(async p=>JSON.parse(await readFile(p,'utf8')).productIds))).flat())];
const products=productIds.map(id=>{const p=catalog.find(p=>p.id===id);assert(p,id);return p;});
await mkdir(output,{recursive:true});
const browser=await chromium.launch({headless:true,executablePath:process.env.CHROMIUM_PATH||'/usr/bin/chromium',args:['--no-sandbox','--enable-unsafe-swiftshader','--use-angle=swiftshader']});
const report={schemaVersion:1,startedAt:new Date().toISOString(),browser:await browser.version(),scope:'Actual workbench WebGL, 2D/3D switching, shared-scene restoration and origin/session-bound iframe RPC; no firmware boot or physical validation.',results:[],pageErrors:[]};
const seenModels=new Set();
try{
  for(const product of products){
    const context=await browser.newContext({viewport:{width:1440,height:1000}}),page=await context.newPage();
    const errors=[];context.on('page',p=>p.on('pageerror',e=>errors.push(e.message)));page.on('pageerror',e=>errors.push(e.message));
    const row={productId:product.id,board:product.runtime.board,model:product.model,checks:[]};
    try{
      await page.goto(new URL('index.html',base).href);
      await page.waitForFunction(()=>!!window.espTester);
      const configured=await page.evaluate(async id=>{
        const {loadCatalog,createDefaultScene}=await import('./catalog.js');
        await espTester.configure(createDefaultScene(await loadCatalog(),id));
        const state=await espTester.observe();return {productId:state.product.id,status:state.status,scene:state.scene};
      },product.id);
      assert.equal(configured.productId,product.id);assert.equal(configured.scene.firmware,null);row.checks.push('public configure preserves exact product without loading firmware');
      if(product.model){
        await page.waitForFunction(()=>['ready','error'].includes(document.querySelector('#scene-mount')?.dataset.modelStatus),null,{timeout:120000});
        assert.equal(await page.locator('#scene-mount').getAttribute('data-model-status'),'ready');
        const canvas=page.locator('#scene-mount canvas[data-engine]');
        assert(await canvas.isVisible());row.checks.push('production WebGL model ready');
        const key=JSON.stringify(product.model);
        if(!seenModels.has(key)){
          seenModels.add(key);await page.locator('#reset-view').click();await page.mouse.move(0,0);
          const filename=product.id+'-studio.png';await canvas.screenshot({path:join(output,filename)});
          const bytes=await readFile(join(output,filename));row.screenshot={file:filename,bytes:bytes.length,sha256:createHash('sha256').update(bytes).digest('hex')};
        }
        if(product.display){
          await page.locator('#view-2d').click();await page.waitForFunction(()=>document.querySelector('#view-2d')?.getAttribute('aria-pressed')==='true');
          await page.locator('#view-3d').click();await page.waitForFunction(()=>document.querySelector('#view-3d')?.getAttribute('aria-pressed')==='true');
          row.checks.push('2D/3D switches round-trip');
        }
      }
      const shared=await page.evaluate(()=>espTester.share({mode:'embed',autostart:false,autoplay:false}));
      const host=await context.newPage(),hostUrl=new URL('qa-host.html',base).href;
      await host.route(hostUrl,route=>route.fulfill({contentType:'text/html',body:'<!doctype html><html><body style="margin:0"><iframe id="player" style="width:1200px;height:850px;border:0" title="ESP test player"></iframe></body></html>'}));
      await host.goto(hostUrl);await host.locator('#player').evaluate((f,url)=>{f.src=url;},shared);
      await host.frameLocator('#player').locator('#stage-title').waitFor();
      const frame=host.frames().find(f=>f.url()===shared);assert(frame,'embed iframe loaded');
      await frame.waitForFunction(()=>!!window.espTester&&document.querySelector('#scene-mount')?.dataset.modelStatus!=='loading',null,{timeout:120000});
      const rpc=await host.evaluate(async()=>{
        const frame=document.querySelector('#player'),origin=new URL(frame.src).origin,session='waveshare_browser_qa';
        async function request(type,requestId,method,args=[]){return new Promise((resolve,reject)=>{
          const timer=setTimeout(()=>{window.removeEventListener('message',receive);reject(Error('embed RPC timeout'));},15000);
          function receive(event){const d=event.data;if(event.source!==frame.contentWindow||event.origin!==origin||d?.channel!=='esp-workbench'||d.version!=='1'||d.session!==session||d.requestId!==requestId)return;clearTimeout(timer);window.removeEventListener('message',receive);d.error?reject(Error(JSON.stringify(d.error))):resolve(d.result);}
          window.addEventListener('message',receive);frame.contentWindow.postMessage({channel:'esp-workbench',version:'1',type,session,requestId,method,args},origin);
        });}
        const hello=await request('hello','hello'),state=await request('call','observe','observe');
        return {version:hello.version,productId:state.product.id,scene:state.scene,status:state.status,lastError:state.lastError};
      });
      assert.equal(rpc.productId,product.id);assert.equal(rpc.scene.productId,product.id);assert.equal(rpc.scene.firmware,null);assert.equal(rpc.scene.view.mode,'embed');assert(!rpc.lastError,JSON.stringify(rpc.lastError));
      if(product.model)assert.equal(await frame.locator('#scene-mount').getAttribute('data-model-status'),'ready');
      row.checks.push('share restores exact scene in real embed iframe','origin/session-bound hello and observe RPC');row.embed={apiVersion:rpc.version,status:rpc.status};
      assert.equal(errors.length,0,errors.join('; '));row.status='passed';
    }catch(error){row.status='failed';row.error=error.stack||String(error);}
    finally{row.pageErrors=errors;report.pageErrors.push(...errors.map(message=>({productId:product.id,message})));report.results.push(row);await context.close();await writeFile(join(output,'report.json'),JSON.stringify(report,null,2)+'\n');console.log(product.id+': '+row.status);}
  }
}finally{report.completedAt=new Date().toISOString();await writeFile(join(output,'report.json'),JSON.stringify(report,null,2)+'\n');await browser.close();}
if(report.results.some(r=>r.status!=='passed'))process.exitCode=1;
