// Pass this function to the tracked Playwright wrapper run-code command on
// review-lane-01.html. It writes scoped evidence only. The caller must close its
// task-owned browser in finally; see docs/platform/models/unit-library.md.
async (page) => {
 const receipts=[];
 await page.setViewportSize({width:1700,height:1200});
 await page.evaluate(()=>{const s=document.createElement('style');s.textContent='#render,#render canvas{width:800px!important;height:800px!important}';document.head.append(s)});
 for(const key of ['button-u027','angle-u005','rgb-u003']) {
  const id='m5stack-unit-'+key,base='docs/platform/evidence/'+id+'/';
  await page.evaluate(id=>laneReview.load(id),id);
  const record=await page.evaluate(()=>({id:laneReview.current.id,artifacts:laneReview.current.meta.artifacts,dimensions:laneReview.current.meta.dimensions,derivedParts:laneReview.current.meta.derivedParts.filter(p=>p.kind==='connector-visual')}));
  for(const view of ['front','back','oblique','connector']) {
   await page.evaluate(view=>laneReview.show(view),view);
   await page.locator('#render canvas').screenshot({path:base+view+'.png'});
   if(view==='front'){await page.evaluate(()=>laneReview.mask());await page.locator('#render canvas').screenshot({path:base+'front-mask.png'});}
  }
  for(const preset of [{name:'gold',color:'#c7a84a',opacity:1,finish:'metal'},{name:'purple',color:'#875acb',opacity:.35,finish:'plastic'}]){
   await page.evaluate(p=>{laneReview.appearance({shell:{color:p.color,opacity:p.opacity,finish:p.finish}});laneReview.show('back',{position:[30,30,-100],span:46,target:[0,0,0]})},preset);
   await page.locator('#render canvas').screenshot({path:base+'back-'+preset.name+'.png'});
  }
  await page.evaluate(()=>{laneReview.show('oblique');for(const mesh of laneReview.current.group.userData.shellMeshes)mesh.visible=false;laneReview.show('oblique');});
  await page.locator('#render canvas').screenshot({path:base+'unknown-pcb.png'});
  await page.evaluate(id=>laneReview.load(id),id);
  record.cable=await page.evaluate(async()=>{
   const {createStopwatchModel}=await import('./stopwatch-model.js'),{createGroveCable}=await import('./accessory-view.js');
   const cur=laneReview.current,host=createStopwatchModel(cur.textures.get('screen'));host.group.position.set(-66,0,0);cur.group.add(host.group);await host.ready;const cable=createGroveCable(host.connectors.grove,cur.model.connectors.grove,cur.group);cur.group.add(cable.group);laneReview.show('connector');return {endpoints:cable.group.userData.endpoints,wires:cable.group.children.filter(x=>x.name.startsWith('wire:')).map(x=>x.userData)};
  });
  await page.locator('#render canvas').screenshot({path:base+'connected-inlet.png'});
  await page.evaluate(()=>laneReview.show('pair',{position:[25,-80,160],target:[-33,-8,0],span:138}));
  await page.locator('#render canvas').screenshot({path:base+'connected-pair.png'});
  receipts.push(record);
 }
 return receipts;
}
