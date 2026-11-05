// Model-only check: the color pattern is diagnostic input, not guest execution.
async page=>{
 const assert=(value,message)=>{if(!value)throw Error(message);};
 await page.setViewportSize({width:1140,height:720});
 await page.waitForFunction(()=>document.body.dataset.ready==='true');
 const geometry=await page.evaluate(()=>review.check());
 assert(geometry.screen.shape==='CircleGeometry'&&geometry.screen.usesSuppliedTexture,'Runtime texture must cover a circular inner display');
 assert(geometry.bezel.visible===geometry.bezel.samples,'The outer cover-glass bezel remains visible');
 assert(geometry.edges.every(row=>row.visible===row.samples),'Display edge must remain visible in all review views');
 assert(geometry.buttons['button-blue'].control==='button-a'&&geometry.buttons['button-green'].control==='button-b','Colored source buttons map to A/B');
 assert(Object.values(geometry.buttons).every(row=>row.visibleSamples>0),'Both source button meshes have exposed hit surfaces');
 assert(!geometry.powerEmitsInput&&geometry.connectorNames.length===0,'No power GPIO or absent connector is introduced');
 const box=await page.locator('#view').boundingBox();
 const click=async(x,y)=>page.mouse.click(box.x+x,box.y+y);
 await click(147,144);await click(392,144);
 const buttons=await page.evaluate(()=>review.inputs.splice(0));
 assert(JSON.stringify(buttons)===JSON.stringify([{type:'button',id:'button-a',pressed:true},{type:'button',id:'button-a',pressed:false},{type:'button',id:'button-b',pressed:true},{type:'button',id:'button-b',pressed:false}]),'3D pointer clicks must emit paired A/B press and release');
 await page.mouse.move(box.x+270,box.y+270);await page.mouse.down();await page.mouse.move(box.x+290,box.y+285);await page.mouse.up();
 const touch=await page.evaluate(()=>review.inputs.splice(0));
 assert(touch.length>=3&&touch.every(event=>event.type==='touch')&&touch[0].down===true&&touch.at(-1).down===false,'3D touch emits down, move, release');
 assert(Math.abs(touch[0].x-234)<=2&&Math.abs(touch[0].y-233)<=2,'Circle center maps to native framebuffer center');
 await click(450,450);assert((await page.evaluate(()=>review.inputs.splice(0))).length===0,'Outside-circle clicks emit no touch');
 await page.screenshot({path:'output/playwright/triviapod-screen-front.png'});
 await page.evaluate(()=>{review.show('oblique');review.view.setOrientation([-.0993346654,-.1976768117,-.0198338381,.9751703272],{notify:false});});
 await page.screenshot({path:'output/playwright/triviapod-screen-oblique.png'});
 await page.evaluate(()=>{review.view.setMode('2d');review.inputs.length=0;});
 await click(42,42);assert((await page.evaluate(()=>review.inputs.splice(0))).length===0,'2D corners outside the circle emit no touch');
 await click(270,270);const flat=await page.evaluate(()=>review.inputs.splice(0));assert(flat.length===2&&flat[0].down&&flat[1].down===false,'2D circular display sends paired touches');
 await page.evaluate(()=>{review.view.setMode('3d');review.view.setProduct(review.product,{previewOnly:true});});
 await page.waitForFunction(()=>document.querySelector('#view').dataset.modelStatus==='ready');
 await click(270,270);assert((await page.evaluate(()=>review.inputs.splice(0))).length===0,'Visual-only previews emit no runtime input');
 return {geometry,inputs:{buttons,touch,flat,previewOnlyBlocked:true,outsideCircleBlocked:true},scope:'Model geometry and input delivery only; diagnostic pattern, no firmware execution or hardware validation.'};
}
