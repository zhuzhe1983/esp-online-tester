import * as THREE from './vendor/three.module.min.js';
const base = new URL('./assets/m5stack-papermono/', import.meta.url);
const material = (color, extra = {}) => new THREE.MeshStandardMaterial({color, roughness: .7, ...extra});
function cube(w,h,d,mat,x=0,y=0,z=0) { const mesh=new THREE.Mesh(new THREE.BoxGeometry(w,h,d),mat);mesh.position.set(x,y,z);return mesh; }
function capsule(w,h) { const s=new THREE.Shape(),r=h/2,l=(w-h)/2;s.moveTo(-l,-r);s.lineTo(l,-r);s.absarc(l,0,r,-Math.PI/2,Math.PI/2);s.lineTo(-l,r);s.absarc(-l,0,r,Math.PI/2,Math.PI*1.5);return s; }
export function createPapermonoModel(screenTexture) {
  const group=new THREE.Group();group.name='m5stack-papermono';const parts={},buttons=[],connectors={},aborter=new AbortController();let disposed=false,labelTexture,pcbTexture;
  // Official drawing: 59 x 99 mm front cover, R2 corners, seated within the CAD rim.
  const coverShape=new THREE.Shape(),w=29.5,h=49.5,r=2;coverShape.moveTo(-w+r,-h);coverShape.lineTo(w-r,-h);coverShape.quadraticCurveTo(w,-h,w,-h+r);coverShape.lineTo(w,h-r);coverShape.quadraticCurveTo(w,h,w-r,h);coverShape.lineTo(-w+r,h);coverShape.quadraticCurveTo(-w,h,-w,h-r);coverShape.lineTo(-w,-h+r);coverShape.quadraticCurveTo(-w,-h,-w+r,-h);
  const cover=new THREE.Mesh(new THREE.ShapeGeometry(coverShape),material(0x888d8e,{roughness:.55}));cover.name='front-cover';cover.position.z=3.8;group.add(cover);parts.glass=cover;
  const screen=new THREE.Mesh(new THREE.PlaneGeometry(50.84,86.4),new THREE.MeshBasicMaterial({map:screenTexture,polygonOffset:true,polygonOffsetFactor:-2,polygonOffsetUnits:-4}));screen.name='live-display';screen.position.set(0,2.41,3.82);screen.userData.screen=true;group.add(screen);parts.screen=screen;
  // The source drawing gives the window; the original CAD retains its actual front bevel.
  const pcb=new THREE.Group();pcb.name='pcb-main';pcb.userData={kind:'pcb-placeholder',label:'UNKNOWN',estimated:{sizeMm:[56,94,1],centerMm:[0,0,-.5]}};pcb.position.z=-.5;
  pcb.add(cube(56,94,1,material(0x859188)));parts['pcb-main']=pcb;group.add(pcb);
  if(typeof document!=='undefined') {
    const canvas=document.createElement('canvas');canvas.width=512;canvas.height=128;const c=canvas.getContext('2d');c.fillStyle='#f2f2e9';c.font='bold 70px monospace';c.textAlign='center';c.textBaseline='middle';c.fillText('UNKNOWN',256,64);pcbTexture=new THREE.CanvasTexture(canvas);pcbTexture.colorSpace=THREE.SRGBColorSpace;
    for(const back of [false,true]) {const label=new THREE.Mesh(new THREE.PlaneGeometry(42,10.5),new THREE.MeshBasicMaterial({map:pcbTexture,transparent:true,depthWrite:false}));label.name=`pcb-main-label-${back?'back':'front'}`;label.position.z=back?-.51:.51;if(back)label.rotation.y=Math.PI;pcb.add(label);}
  }
  const usb=new THREE.Group();usb.name='usb-c-insert';usb.position.set(-20.2,-50.52,.4);usb.rotation.x=Math.PI/2;
  const rim=capsule(8.75,3),inner=capsule(8,2.35);rim.holes.push(new THREE.Path(inner.getPoints(32)));
  usb.add(new THREE.Mesh(new THREE.ShapeGeometry(rim,32),material(0xa3a9ac,{metalness:.8,roughness:.3})),cube(7.8,2.2,.2,material(0x080a0a),0,0,-.12),cube(6.4,.55,.2,material(0x24292a),0,0,.04));group.add(usb);parts.usb=usb;
  const anchor=new THREE.Object3D();anchor.name='connector:usb-c:mouth';anchor.userData={kind:'connector-anchor',type:'USB-C',visualOnly:true};usb.add(anchor);connectors.usb={type:'USB-C',anchor,normal:new THREE.Vector3(0,0,1)};
  const sd=cube(.35,10.8,1.7,material(0x111415),-30.38,-34.2,-.6);sd.name='microsd-visible-slot';group.add(sd);parts.sd=sd;
  const led=cube(.25,8.4,1.7,material(0xc0c3bd),-30.42,9.5,-.3);led.name='rgb-led-diffuser';group.add(led);
  const ready=(async()=>{
    const r=await fetch(new URL('model.json',base),{signal:aborter.signal});if(!r.ok)throw Error(`PaperMono model HTTP ${r.status}`);const metadata=await r.json();
    const b=await fetch(new URL(metadata.binary,base),{signal:aborter.signal});if(!b.ok)throw Error(`PaperMono geometry HTTP ${b.status}`);const binary=await b.arrayBuffer();if(disposed)return;
    for(const part of metadata.parts) {
      const geometry=new THREE.BufferGeometry(),ib=new THREE.InterleavedBuffer(new Float32Array(binary,part.byteOffset,part.vertexCount*6),6);geometry.setAttribute('position',new THREE.InterleavedBufferAttribute(ib,3,0));geometry.setAttribute('normal',new THREE.InterleavedBufferAttribute(ib,3,3));geometry.computeBoundingSphere();
      const mesh=new THREE.Mesh(geometry,material(part.color));mesh.name=part.name;
      if(part.name==='shell'){mesh.userData.inputOccluder=true;mesh.userData.originalAppearance={shell:{color:part.color,opacity:1,finish:'plastic'}};}
      if(['button-a','button-b'].includes(part.name)){mesh.userData.control=part.name;buttons.push(mesh);}
      if(part.name==='power-button')mesh.userData.unsupported='PaperMono 尚无板级运行模型，电源键仅展示外观。';parts[part.name]=mesh;group.add(mesh);
    }
    const loaded=await new THREE.TextureLoader().loadAsync(new URL('rear-label.webp',base).href);if(disposed){loaded.dispose();return;}labelTexture=loaded;loaded.colorSpace=THREE.SRGBColorSpace;loaded.anisotropy=4;
    const label=new THREE.Mesh(new THREE.PlaneGeometry(58.9,99),new THREE.MeshBasicMaterial({map:loaded}));label.name='rear-product-label';label.position.z=-4.015;label.rotation.y=Math.PI;group.add(label);group.userData.provenance=metadata;return metadata;
  })();
  return {group,screen,buttons,parts,connectors,ready,dispose(){disposed=true;aborter.abort();labelTexture?.dispose();pcbTexture?.dispose();}};
}
