// Run with playwright_cli.sh run-code --filename after opening the caller-supplied platform URL.
// Requires output/playwright; screenshots use real CAD and explicitly synthetic RGB inputs.
async page => {
 const result=[],base=new URL('./review-lane-02.html',page.url());
 for(const id of ['m5stack-c008-b','m5stack-c008-b-v11']){
  const target=new URL(base);target.searchParams.set('id',id);await page.goto(target.href);await page.waitForFunction(()=>document.body.dataset.ready==='true');
  const item=await page.evaluate(async()=>{
   const THREE=await import('./vendor/three.module.min.js'),{applyRgbLedValues}=await import('./rgb-led-view.js');
   const levels=[0,1,5,19,255],hues=[[1,0,0],[0,1,0],[0,0,1],[1,.5,.25],[1,1,1]],values=hues.flatMap(h=>levels.map(v=>h.map(c=>Math.round(c*v))));
   const {group}=laneReview.current,meshes=group.userData.rgbLedMeshes;applyRgbLedValues(meshes,values);laneReview.show('front',{span:30,target:[0,0,0],position:[0,0,250]});
   const gl=laneReview.renderer.getContext();const samples=meshes.map((m,i)=>{const center=new THREE.Vector3();m.getWorldPosition(center);const pixel=new Uint8Array(4),x=Math.round((center.x/30+.5)*800),y=Math.round((center.y/30+.5)*800);gl.readPixels(x,y,1,1,gl.RGBA,gl.UNSIGNED_BYTE,pixel);return {index:i,raw:m.userData.rgb,linearEmissive:m.material.emissive.toArray(),pixel:Array.from(pixel)};});
   const m=meshes[2],at=m.getWorldPosition(new THREE.Vector3()),ray=new THREE.Raycaster(new THREE.Vector3(at.x,at.y,250),new THREE.Vector3(0,0,-1)),hits=ray.intersectObject(group,true).slice(0,5).map(h=>({name:h.object.name,z:h.point.z}));return {id:laneReview.current.id,samples,nearestLedHits:hits,rendererColorSpace:laneReview.renderer.outputColorSpace};
  });await page.locator('#render canvas').screenshot({path:'output/playwright/rgb-led-'+id+'.png'});if(item.samples[2].pixel[0]<120||item.samples[4].pixel[0]>=250||item.samples[2].pixel[0]>=item.samples[3].pixel[0]||item.samples[3].pixel[0]>=item.samples[4].pixel[0])throw Error('LED display visibility/order/headroom regression');result.push(item);
 }
 return {kind:'synthetic-display-calibration-not-firmware-validation',results:result};
}
