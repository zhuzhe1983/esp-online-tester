// Run with the tracked Playwright CLI on review-lane-02.html?id=m5stack-k159-b.
// The uniform gray test texture checks screen placement; it is not guest firmware output.
async page=>{
  await page.setViewportSize({width:1800,height:1100});await page.route('**/*',route=>route.continue());await page.reload();await page.waitForFunction(()=>document.body.dataset.ready==='true');
  const screenshots=[],views=['front','back','ports','side'];
  for(const [name,shell] of [['original',null],['gold',{color:'#d7ab52',opacity:1,finish:'metal'}],['purple',{color:'#8456cf',opacity:.36,finish:'plastic'}]]){
    await page.evaluate(async shell=>{await laneReview.load('m5stack-k159-b');const t=laneReview.current.textures.get('blank');t.image.data.set([218,220,211,255]);t.needsUpdate=true;if(shell)laneReview.appearance({shell});},shell);
    for(const view of views){await page.evaluate(v=>laneReview.show(v),view);const path=`output/playwright/paperdiy-final-${name}-${view}.png`;await page.locator('#render canvas').screenshot({path});screenshots.push(path);}
    for(const [view,position] of [['front-oblique',[-160,60,250]],['back-oblique',[-160,60,-250]],['near-side',[240,-20,30]]]){
      await page.evaluate(({view,position})=>laneReview.show(view,{position,up:[0,1,0],span:145}),{view,position});
      const path=`output/playwright/paperdiy-final-${name}-${view}.png`;await page.locator('#render canvas').screenshot({path});screenshots.push(path);
    }
  }
  // The PCB is reachable through the same public inspection control as the assembled viewer.
  for(const [name,shell] of [['original',null],['gold',{color:'#d7ab52',opacity:1,finish:'metal'}],['purple',{color:'#8456cf',opacity:.36,finish:'plastic'}]]){
    await page.evaluate(async shell=>{await laneReview.load('m5stack-k159-b');if(shell)laneReview.appearance({shell});laneReview.current.group.userData.inspection.setExpanded(true);},shell);
    for(const [view,position,target,span] of [['components',[-72,-33,-250],[-72,-33,-12],72],['reverse',[-72,-33,250],[-72,-33,-12],72],['oblique',[-130,-5,-180],[-72,-33,-12],84],['edge',[-72,-180,-35],[-72,-33,-12],80]]){
      await page.evaluate(({view,position,target,span})=>laneReview.show(view,{position,target,span,up:[0,1,0]}),{view,position,target,span});
      const path=`output/playwright/paperdiy-pcb-${name}-${view}.png`;await page.locator('#render canvas').screenshot({path});screenshots.push(path);
    }
  }
  await page.evaluate(async()=>{await laneReview.load('m5stack-k159-b');laneReview.current.group.userData.inspection.setExpanded(true);laneReview.show('inspection',{position:[-120,70,-250],target:[0,0,0],up:[0,1,0],span:245});});
  const path='output/playwright/paperdiy-final-inspection.png';await page.locator('#render canvas').screenshot({path});screenshots.push(path);
  const inspection=await page.evaluate(()=>({expanded:laneReview.current.group.userData.inspection.expanded,unknown:laneReview.current.model.parts['pcb-main'].children.filter(p=>p.name.includes('unknown')).map(p=>({name:p.name,visible:p.visible}))}));
  if(!inspection.expanded||inspection.unknown.length!==1||!inspection.unknown[0].visible)throw Error('Missing inspectable UNKNOWN board');
  await page.evaluate(()=>{laneReview.current.group.userData.inspection.setExpanded(false);laneReview.show('back');});
  const metadata=await page.evaluate(()=>({artifacts:laneReview.current.meta.artifacts,screen:laneReview.current.meta.presentation.screen,views:laneReview.current.meta.reviewViews,buttons:laneReview.current.model.buttons.map(p=>p.userData.control)}));
  if(metadata.buttons.slice().sort().join(',')!=='button-a,button-b')throw Error('Button mappings changed');
  return {productId:'m5stack-k159-b',scope:'Production model geometry and presentation only; no new guest firmware or physical hardware validation',metadata,inspection,screenshots};
}
