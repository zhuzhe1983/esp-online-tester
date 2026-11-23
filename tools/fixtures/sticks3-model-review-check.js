// Run on /platform/review-lane-01.html?id=m5stack-k150 using the Playwright CLI wrapper.
async page => {
  await page.reload();
  await page.waitForFunction(() => document.body.dataset.ready === 'true');
  return page.evaluate(async () => {
    const THREE = await import('./vendor/three.module.min.js');
    const {model,meta}=laneReview.current;
    if(meta.id!=='m5stack-k150') throw Error('Expected the StickS3 K150 review page');
    const {parts,group,connectors}=model;
    group.updateMatrixWorld(true);
    const objects=group.children.filter(object=>object!==parts['back-paper']);
    const ray=(start,direction,near=0,far=100)=>{
      const caster=new THREE.Raycaster(new THREE.Vector3(...start),new THREE.Vector3(...direction),near,far);
      const hit=caster.intersectObjects(objects,true)[0];
      return hit ? {part:hit.object.name,distanceMm:Number(hit.distance.toFixed(4))} : null;
    };
    const pockets=[];
    for(const z of [-6.065,-3.525])for(let column=0;column<8;column++) {
      const hit=ray([(column-3.5)*2.54,24.1,z],[0,-1,0]);
      if(hit?.part!=='hat2-pocket-floor'||Math.abs(hit.distanceMm-8.35)>.01) throw Error('Model blocks a Hat2 pocket before its recessed floor');
      pockets.push(hit.distanceMm);
    }
    const usb=ray([0,-24.2,3.2],[0,1,0]);
    const grove=ray([0,-24.2,-1.9],[0,1,0]);
    if(usb?.part!=='usb-c-rear'||usb.distanceMm<3||grove?.part!=='socket-interior'||grove.distanceMm<4||connectors.grove) throw Error('Unexpected lower-end geometry or Grove connection capability');
    const anchor=connectors['usb-c'].anchor;
    const mouth=anchor.getWorldPosition(new THREE.Vector3()).toArray();
    const normal=new THREE.Vector3(0,0,1).transformDirection(anchor.matrixWorld).toArray();
    if(normal[1]>-.9999) throw Error('Type-C mouth faces away from the lower end');

    const paper=parts['back-paper'];
    const src=paper.material.map.image;
    const [width,height]=[src.width,src.height];
    const canvas=document.createElement('canvas');
    canvas.width=width;
    canvas.height=height;
    const context=canvas.getContext('2d');
    context.drawImage(src,0,0);
    const pixels=context.getImageData(0,0,width,height).data;
    const {width:paperWidth,height:paperHeight}=paper.geometry.parameters;
    const paperCheck={sampled:0,recessSupported:0,seamOrMagnetEdge:0,unsupported:0,hiddenByLip:0};
    for(let y=8;y<height;y+=16)for(let x=8;x<width;x+=16) {
      if(pixels[4*(y*width+x)+3]<243) continue;
      paperCheck.sampled++;
      const point=new THREE.Vector3(((x+.5)/width-.5)*paperWidth,(.5-(y+.5)/height)*paperHeight,0).applyMatrix4(paper.matrixWorld);
      const hit=ray([point.x,point.y,point.z-1],[0,0,1],0,1.09);
      if(hit&&hit.distanceMm<.99)paperCheck.hiddenByLip++;
      else if(hit)paperCheck.recessSupported++;
      else if((point.y>=14 && point.y<=15.5)||Math.abs(Math.hypot(point.x,point.y)-5.93)<.15)paperCheck.seamOrMagnetEdge++;
      else paperCheck.unsupported++;
    }
    if(paperCheck.unsupported>0||paperCheck.hiddenByLip>1) throw Error(`Paper alignment is outside the traced support: ${JSON.stringify(paperCheck)}`);
    const physical=new THREE.Box3();
    for(const object of objects)if(!object.name.startsWith('cover-key:'))physical.expandByObject(object,true);
    const dimensions=physical.getSize(new THREE.Vector3()).toArray();
    const target=[24,48,15];
    if(dimensions.some((mm,index)=>Math.abs(mm-target[index])>.05)) throw Error('Unexpected assembled envelope');
    const matrixRequests=new Set(performance.getEntriesByType('resource').map(entry=>new URL(entry.name).searchParams.get('sha256')));
    if(!['geometry.bin','back-paper.png'].every(file=>matrixRequests.has(meta.artifacts[file]))) throw Error('Unversioned geometry or paper texture');

    const inspection=group.userData.inspection;
    const [front,board,rear]=meta.presentation.inspectionGroups;
    const layers=[front,board,rear];
    const expectedOffsets=[28,3,-23];
    const terminals=['pcb-main','usb-c','usb-c-insulating-seat','grove','grove-retaining-clip-left',
      'grove-retaining-clip-right','hat2-header','hat2-pocket-floor'];
    if(terminals.some(id=>!board.parts.includes(id))) throw Error('A board-mounted terminal left the PCB layer');
    if(!front.parts.includes('live-display')||!rear.parts.includes('back-paper')||!rear.parts.includes('cover-key:btnB'))
      throw Error('A display, sticker or case key left its housing layer');
    if(layers.some((layer,index)=>layer.offsetMm[2]!==expectedOffsets[index])) throw Error('Unexpected inspection layer spacing');
    const tracked=layers.flatMap(layer=>layer.parts);
    const objectsByName=new Map(group.children.map(object=>[object.name,object]));
    const assembled=Object.fromEntries(tracked.map(id=>[id,objectsByName.get(id).position.clone()]));
    const checkProgress=(amount,apply)=>{
      apply();
      for(const layer of layers) for(const id of layer.parts) {
        const expected=assembled[id].clone().addScaledVector(new THREE.Vector3(...layer.offsetMm),amount);
        if(expected.distanceTo(objectsByName.get(id).position)>1e-8) throw Error(`Unexpected inspection displacement on ${id}`);
      }
    };
    checkProgress(.5,()=>inspection.setProgress(.5));
    checkProgress(1,()=>inspection.setExpanded(true));
    const bounds=layers.map(layer=>layer.parts.reduce((box,id)=>box.expandByObject(objectsByName.get(id),true),new THREE.Box3()));
    const gaps=[bounds[0].min.z-bounds[1].max.z,bounds[1].min.z-bounds[2].max.z];
    if(gaps.some(gap=>gap<14)) throw Error('The PCB layer still crowds the shell');
    checkProgress(0,()=>inspection.setExpanded(false));
    return {dimensions,pockets,usb,grove,mouth,normal,paperCheck,paperPlane:paper.position.toArray(),paperPixels:[width,height],artifactHashes:meta.artifacts,inspection:{layers,gaps,reassembled:true}};
  });
}
