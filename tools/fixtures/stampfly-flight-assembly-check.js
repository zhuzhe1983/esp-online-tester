// Playwright wrapper run-code on a caller-supplied platform URL. This is a
// renderer-structure check with a synthetic pose, not guest execution evidence.
async page => page.evaluate(async()=>{
 const root=new URL('./',location.href),THREE=await import(new URL('vendor/three.module.min.js',root));
 const response=await fetch(new URL('scene-view.js',root)),original=await response.text();
 const anchor='return {setProduct, setAccessories';
 if(!original.includes(anchor))throw Error('Scene harness anchor changed');
 const source=original.replaceAll("from './",`from '${root.href}`).replace(anchor,'return {__assemblyTest:{group,flightGroup,orbitGroup},setProduct, setAccessories');
 const blob=URL.createObjectURL(new Blob([source],{type:'text/javascript'}));
 const {createSceneView}=await import(blob),catalog=await(await fetch(new URL('catalog/products.json',root))).json(),results=[];
 try {
  for(const id of ['m5stack-k138','m5stack-k138-v11']){
   const mount=document.createElement('div');mount.style='position:fixed;left:-2000px;width:640px;height:420px';document.body.append(mount);
   let finish;const ready=new Promise(r=>finish=r),view=createSceneView(mount,{onModelReady:finish}),product=catalog.items.find(p=>p.id===id);
   try {
    view.setProduct(product);await ready;if(mount.dataset.modelStatus!=='ready')throw Error('Model not ready');
    const {group,flightGroup,orbitGroup}=view.__assemblyTest;orbitGroup.updateMatrixWorld(true);
    const meshes=[];group.traverse(p=>{if(p.isMesh&&p.geometry){meshes.push({p,geometry:p.geometry,local:p.matrix.clone(),parent:p.parent,position:p.getWorldPosition(new THREE.Vector3()).applyMatrix4(orbitGroup.matrixWorld.clone().invert())});}});
    const pose={enabled:true,model:'stampfly-rigid-body-v1',worldFrame:'NED',bodyFrame:'FRD',positionM:[.4,.7,-.6],quaternionXyzw:[0,0,0,1],guestTime:2,epoch:1};
    view.updateFlight(pose);orbitGroup.updateMatrixWorld(true);let maxError=0;
    for(const s of meshes){if(s.p.geometry!==s.geometry||!s.p.matrix.equals(s.local)||s.p.parent!==s.parent)throw Error(`Changed local assembly: ${s.p.name}`);let p=s.p;while(p&&p!==flightGroup)p=p.parent;if(p!==flightGroup)throw Error(`Part outside flight root: ${s.p.name}`);const actual=s.p.getWorldPosition(new THREE.Vector3()).applyMatrix4(orbitGroup.matrixWorld.clone().invert()),expected=s.position.clone().add(new THREE.Vector3(700,400,600));maxError=Math.max(maxError,actual.distanceTo(expected));}
    if(maxError>1e-8)throw Error('Assembly translation mismatch');
    const optical=meshes.filter(({p})=>/optical|harness/i.test(p.name)).map(({p})=>p.name);
    view.updateFlight(null);orbitGroup.updateMatrixWorld(true);
    if(flightGroup.position.length()!==0||flightGroup.quaternion.w!==1)throw Error('Disable did not restore assembly');
    results.push({productId:id,meshCount:meshes.length,opticalMeshCount:optical.length,harnessWireCount:optical.filter(n=>/^optical-harness-wire-\d$/.test(n)).length,maxPositionErrorMm:maxError,allGeometryReferencesAndLocalMatricesUnchanged:true,allMeshesDescendFromFlightGroup:true,disableRestored:true});
   } finally {view.dispose();mount.remove();}
  }
  return{scope:'Actual final renderer and models, synthetic translation only; separate from real guest browser flight evidence',results};
 } finally {URL.revokeObjectURL(blob);}
})
