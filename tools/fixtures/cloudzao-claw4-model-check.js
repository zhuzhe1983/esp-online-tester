// Run with the repository Playwright wrapper on a task-owned local platform page.
// Uses the real renderer. It never runs firmware, requests serial or downloads BINs.
async page => {
  const assert=(condition,message)=>{if(!condition)throw Error(message);};
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  await page.goto(new URL('./review-lane-01.html?id=cloudzao-claw4',page.url()).href);
  await page.waitForFunction(()=>document.body.dataset.ready==='true',null,{timeout:30000});
  const receipt=await page.evaluate(()=>{
    const review=window.laneReview,model=review.current.model,group=model.group;
    if(!group.userData.inspection)throw Error(group.userData.inspectionError||'Missing inspection');
    const base=group.children.map(part=>[part.name,part.position.toArray()]);
    group.userData.inspection.setExpanded(true);group.userData.inspection.setExpanded(false);
    const restored=JSON.stringify(base)===JSON.stringify(group.children.map(part=>[part.name,part.position.toArray()]));
    const fixed=['front-cover','rear-cover','camera-lens','power-key','rear-logo'];
    const before=fixed.map(id=>model.parts[id].material.color?.getHexString());
    review.appearance({shell:{color:'#d8ad44',opacity:1,finish:'metal'}});
    review.appearance({shell:{color:'#8a5bd6',opacity:.25,finish:'plastic'}});
    const unchanged=JSON.stringify(before)===JSON.stringify(fixed.map(id=>model.parts[id].material.color?.getHexString()));
    review.appearance({shell:{color:'#c8ccce',opacity:1,finish:'metal'}});
    review.show('oblique');
    return {restored,fixedMaterialsPreserved:unchanged,unknown:group.getObjectByName('pcb-main')?.userData.label,screenSizeMm:[model.screen.geometry.parameters.width,model.screen.geometry.parameters.height],geometrySource:review.current.meta.provenance.kind,firmwareExecuted:false};
  });
  assert(receipt.restored,'Inspection did not restore assembly');
  assert(receipt.fixedMaterialsPreserved,'Shell preset changed print/display/camera/button material');
  assert(receipt.unknown==='UNKNOWN','Unknown PCB label is missing');
  assert(errors.length===0,errors.join('\n'));
  return {...receipt,pageErrors:errors,visualReviewStillRequired:['front','back','side','oblique','gold','purple','expanded UNKNOWN']};
};
