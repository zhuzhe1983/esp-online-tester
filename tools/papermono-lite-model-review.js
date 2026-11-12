// Pass this function to the tracked Playwright CLI `run-code` command.
// The page must be web/platform/review-lane-02.html?id=m5stack-c153-lite.
// Tests the production manufacturer loader and RGB renderer; no firmware is run.
async (page) => {
  await page.setViewportSize({width:1800,height:1100});
  await page.reload();
  await page.waitForFunction(()=>document.body.dataset.ready==='true');
  const screenshots=[],views=['front','back','side'];
  for(const [name,shell] of [['original',null],['gold',{color:'#d6b24a',opacity:1,finish:'metal'}],['purple',{color:'#8054b3',opacity:.35,finish:'plastic'}]]) {
    await page.evaluate(async shell=>{await laneReview.load('m5stack-c153-lite');if(shell)laneReview.appearance({shell});},shell);
    for(const view of views){await page.evaluate(v=>laneReview.show(v),view);const path=`output/playwright/lite-final-${name}-${view}.png`;await page.locator('#render canvas').screenshot({path});screenshots.push(path);}
  }
  await page.evaluate(async()=>laneReview.load('m5stack-c153-lite'));
  for(const [name,position,up] of [['bottom',[0,-250,0],[0,0,1]],['top',[0,250,0],[0,0,1]],['right',[250,0,0],[0,0,1]],['front-oblique',[-160,60,250],[0,1,0]],['back-oblique',[-160,60,-250],[0,1,0]],['near-side',[-250,10,40],[0,1,0]]]) {
    await page.evaluate(({name,position,up})=>laneReview.show(name,{position,up,span:name==='near-side'?70:127,target:name==='near-side'?[0,10,0]:[0,0,0]}),{name,position,up});
    const path=`output/playwright/lite-final-${name}.png`;await page.locator('#render canvas').screenshot({path});screenshots.push(path);
  }
  const leds=[];
  for(const [name,rgb] of [['red',[255,0,0]],['green',[0,255,0]],['cyan',[0,255,255]],['off',[0,0,0]]]) {
    const result=await page.evaluate(async rgb=>{const {applyRgbLedValues}=await import('./rgb-led-view.js');applyRgbLedValues(laneReview.current.group.userData.rgbLedMeshes,[rgb]);laneReview.show('side');const m=laneReview.current.model.parts['rgb-led-diffuser'];return {rgb:m.userData.rgb,emissive:m.material.emissive.toArray(),emitting:m.material.emissive.toArray().some(v=>v>0)};},rgb);
    if(JSON.stringify(result.rgb)!==JSON.stringify(rgb)||result.emitting!==(name!=='off'))throw Error('RGB output mapping failed');
    const path=`output/playwright/lite-final-led-${name}.png`;await page.locator('#render canvas').screenshot({path});screenshots.push(path);leds.push({name,...result});
  }
  const metadata=await page.evaluate(()=>({artifactHashes:laneReview.current.meta.artifacts,screen:laneReview.current.meta.presentation.screen,buttons:laneReview.current.model.buttons.map(m=>m.userData.control),ledMeshes:laneReview.current.group.userData.rgbLedMeshes.map(m=>m.name)}));
  if(metadata.buttons.join(',')!=='button-a,button-b')throw Error('Missing side-button input mapping');
  return {productId:'m5stack-c153-lite',scope:'Production geometry/material/output routing only; no new firmware or hardware test',metadata,leds,screenshots};
}
