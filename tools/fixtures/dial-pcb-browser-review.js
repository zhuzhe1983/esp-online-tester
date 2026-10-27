async (page) => {
  const report=[];
  for(const id of ['m5stack-k130','m5stack-k130-v11']) {
    await page.evaluate(async id=>{await window.laneReview.load(id);},id);
    const data=await page.evaluate(async()=>{
      const r=window.laneReview,pcb=r.current.model.parts['pcb-main'];
      const canvas=document.createElement('canvas');canvas.width=canvas.height=800;const ctx=canvas.getContext('2d',{willReadFrequently:true});let changed=0,views=0,outside=0,worst={pixels:0};
      for(const elevation of [-60,0,60])for(let angle=0;angle<360;angle+=30){
        const a=angle*Math.PI/180,e=elevation*Math.PI/180,position=[250*Math.cos(e)*Math.cos(a),250*Math.cos(e)*Math.sin(a),250*Math.sin(e)];
        r.show('inspection',{position,up:Math.abs(elevation)<85?[0,0,1]:[0,1,0],span:66});ctx.drawImage(r.renderer.domElement,0,0);const before=ctx.getImageData(0,0,800,800).data;
        pcb.visible=false;r.show('inspection',{position,up:[0,0,1],span:66});ctx.drawImage(r.renderer.domElement,0,0);const after=ctx.getImageData(0,0,800,800).data;
        let pixels=0;for(let i=0;i<before.length;i+=4)if(before[i]!==after[i]||before[i+1]!==after[i+1]||before[i+2]!==after[i+2])pixels++; changed+=pixels;if(pixels>worst.pixels)worst={pixels,position};
        for(let y=0;y<800;y++) {let left=800,right=-1;for(let x=0;x<800;x++){const i=(y*800+x)*4;if(Math.min(after[i],after[i+1],after[i+2])<245){left=Math.min(left,x);right=x;}}for(let x=0;x<800;x++){const i=(y*800+x)*4;if((x<left-1||x>right+1)&&(Math.abs(before[i]-after[i])>3||Math.abs(before[i+1]-after[i+1])>3||Math.abs(before[i+2]-after[i+2])>3))outside++;}}
        pcb.visible=true;views++;
      }
      r.show("inspection",{position:worst.position,up:[0,0,1],span:66});return {id:r.current.id,views,worst,outsideOpaqueRowEnvelopePixels:outside,opaquePixelDifferencesWithPcbHidden:changed,labels:pcb.children.filter(x=>x.isMesh&&x.geometry.type==='PlaneGeometry').length,shape:pcb.children[0].geometry.type,center:pcb.position.toArray(),shellCount:r.current.group.userData.shellMeshes.length,control:r.current.model.buttons.map(b=>b.userData.control),encoder:r.current.model.encoders?.map(b=>b.userData.encoder)};
    });
    if(data.outsideOpaqueRowEnvelopePixels!==0||data.labels!==2||data.shape!=="CylinderGeometry")throw Error(JSON.stringify(data));
    report.push(data);await page.locator("#render canvas").screenshot({path:`output/playwright/${id}-pcb-opening.png`});
    for(const view of ['front','back','side']) {
      await page.evaluate(view=>window.laneReview.show(view,view==='side'?{position:[250,200,20],up:[0,0,1],span:66}:{span:66}),view);
      await page.locator('#render canvas').screenshot({path:`output/playwright/${id}-pcb-${view}.png`});
    }
    await page.evaluate(()=>window.laneReview.appearance({shell:{color:'#d4af37',opacity:1,finish:'metal'}}));
    await page.locator('#render canvas').screenshot({path:`output/playwright/${id}-pcb-gold.png`});
    await page.evaluate(()=>{window.laneReview.appearance({shell:{color:'#8456cf',opacity:.25,finish:'plastic'}});window.laneReview.show('back',{position:[160,100,-250],up:[0,1,0],span:66});});
    await page.locator('#render canvas').screenshot({path:`output/playwright/${id}-pcb-transparent.png`});
    await page.evaluate(()=>{for(const item of window.laneReview.current.group.children)if(item.name!=='pcb-main')item.visible=false;window.laneReview.show('back',{span:42});});
    await page.locator('#render canvas').screenshot({path:`output/playwright/${id}-pcb-isolated.png`});
  }
  return report;
}
