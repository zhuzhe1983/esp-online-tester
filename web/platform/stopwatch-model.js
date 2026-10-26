// Manufacturer CAD in millimetres; see assets/m5stack-stopwatch/model.json for provenance.
import * as THREE from './vendor/three.module.min.js';
import {createGroveSocket} from './grove-geometry.js';
const base = new URL('./assets/m5stack-stopwatch/', import.meta.url);
const material = (color, options = {}) => new THREE.MeshStandardMaterial({color, roughness: .65, ...options});
function cube(w,h,d,mat,x=0,y=0,z=0) { const m=new THREE.Mesh(new THREE.BoxGeometry(w,h,d),mat); m.position.set(x,y,z); return m; }
function disc(radius,mat,x,y,z,back=false,segments=128) { const m=new THREE.Mesh(new THREE.CircleGeometry(radius,segments),mat); m.position.set(x,y,z); if(back)m.rotation.y=Math.PI; return m; }
function capsule(w,h) {
  const r=h/2,l=(w-h)/2,s=new THREE.Shape(); s.moveTo(-l,-r); s.lineTo(l,-r); s.absarc(l,0,r,-Math.PI/2,Math.PI/2,false); s.lineTo(-l,r); s.absarc(-l,0,r,Math.PI/2,Math.PI*1.5,false); return s;
}
export function createStopwatchModel(screenTexture) {
  const group=new THREE.Group(); group.name='m5stack-stopwatch';
  const buttons=[], parts={}, connectors={}, aborter=new AbortController(); let disposed=false, labelTexture, pcbLabelTexture;
  const glass=new THREE.Mesh(new THREE.CylinderGeometry(24.54,24.54,.5,128),material(0x080a0c,{roughness:.18,metalness:.15}));
  glass.rotation.x=Math.PI/2;glass.position.z=7.5;glass.name='cover-glass';group.add(glass);parts.glass=glass;
  const screen=disc(22.225,new THREE.MeshBasicMaterial({map:screenTexture,polygonOffset:true,polygonOffsetFactor:-1,polygonOffsetUnits:-2}),0,0,7.78); screen.name='live-display'; screen.userData.screen=true; group.add(screen);parts.screen=screen;
  // Opaque internals behind through-holes; this does not claim an electronics model.
  const opaqueInternals = new THREE.Mesh(new THREE.CylinderGeometry(23.5,23.5,12,128),new THREE.MeshBasicMaterial({color:0x050505}));
  opaqueInternals.rotation.x=Math.PI/2; opaqueInternals.name='interior-occlusion-only'; group.add(opaqueInternals);parts.occluder=opaqueInternals;
  // Explicitly requested placeholder: no PCB drawing, thickness or placement has been verified.
  const pcb=new THREE.Group();pcb.name='pcb-main';pcb.userData={kind:'pcb-placeholder',label:'UNKNOWN',estimated:{diameterMm:44,thicknessMm:1,centerMm:[0,0,0]},verifiedComponents:[]};
  const substrate=new THREE.Mesh(new THREE.CylinderGeometry(22,22,1,96),material(0x859188));substrate.rotation.x=Math.PI/2;substrate.name='pcb-main-substrate';pcb.add(substrate);
  if(typeof document!=='undefined') {
    const labelCanvas=document.createElement('canvas');labelCanvas.width=512;labelCanvas.height=128;const context=labelCanvas.getContext('2d');
    context.fillStyle='#f2f2e9';context.font='bold 72px monospace';context.textAlign='center';context.textBaseline='middle';context.fillText('UNKNOWN',256,64);
    pcbLabelTexture=new THREE.CanvasTexture(labelCanvas);pcbLabelTexture.colorSpace=THREE.SRGBColorSpace;
    for(const back of [false,true]) {const label=new THREE.Mesh(new THREE.PlaneGeometry(36,9),new THREE.MeshBasicMaterial({map:pcbLabelTexture,transparent:true,depthWrite:false}));label.name=back?'pcb-main-label-back':'pcb-main-label-front';label.position.z=back?-.515:.515;if(back)label.rotation.y=Math.PI;pcb.add(label);}
  }
  parts['pcb-main']=pcb;group.add(pcb);
  // Connector inserts are source-informed visual additions, not claimed component CAD.
  const port=new THREE.Group(); port.name='usb-c-insert'; port.rotation.x=Math.PI/2; port.position.set(0,-25.05,.65);
  const rim=capsule(8.9,3.2), inner=capsule(8.05,2.42); rim.holes.push(new THREE.Path(inner.getPoints(40)));
  port.add(new THREE.Mesh(new THREE.ShapeGeometry(rim,40),material(0xbac0c5,{metalness:.8,roughness:.3})));
  port.add(cube(7.7,2.1,.2,material(0x070808),0,0,-.15),cube(6.5,.55,.35,material(0x30343a),0,0,.02)); group.add(port);
  const usbAnchor=new THREE.Object3D();usbAnchor.name='connector:usb-c:mouth';usbAnchor.position.z=.1;usbAnchor.userData={kind:'connector-anchor',type:'USB-C',normal:[0,0,1],visualOnly:true};port.add(usbAnchor);
  connectors.usb={type:'USB-C',anchor:usbAnchor,normal:new THREE.Vector3(0,0,1)};parts.usb=port;
  const grove=createGroveSocket(); grove.name='hy2-4p-insert'; grove.position.set(17.55,-17.55,.55);
  grove.setRotationFromMatrix(new THREE.Matrix4().makeBasis(new THREE.Vector3(Math.SQRT1_2,Math.SQRT1_2,0),new THREE.Vector3(0,0,1),new THREE.Vector3(Math.SQRT1_2,-Math.SQRT1_2,0)));
  const dark=material(0x121619);group.add(grove);
  // Socket-frame +Z is the outward normal (+sqrt(1/2),-sqrt(1/2),0) in product coordinates.
  // Rim is z=0; the plug nose occupies the open socket cavity, its stop stays outside.
  const groveAnchor=new THREE.Object3D();groveAnchor.name='connector:grove:mouth';groveAnchor.position.z=0;groveAnchor.userData={kind:'connector-anchor',type:'HY2.0-4P',normal:[0,0,1],visualOnly:true};grove.add(groveAnchor);
  // In this socket's local +X tangent, the rear-label order is G11, G10, 5V, GND.
  connectors.grove={type:'HY2.0-4P',anchor:groveAnchor,normal:new THREE.Vector3(0,0,1),maxConnections:1,allowConnections:true,sourceIds:['photo-07'],pinOrder:['signal2','signal1','vcc','gnd']};parts.grove=grove;
  for(const [x,count] of [[16.8,6],[-16.8,7]]) {
    const header=new THREE.Group(); header.name=`rear-header-${count}p`;
    header.add(cube(2.65,count*2.54+.5,.8,dark,x,8.1-(count-1)*1.27,-6.9));
    for(let i=0;i<count;i++)header.add(cube(1.5,1.5,.1,material(0x020303),x,8.1-i*2.54,-7.35)); group.add(header);
  }
  for(const x of [-20,20]) {
    group.add(disc(1.7,material(0xbac1c7,{metalness:.65,roughness:.35}),x,0,-7.55,true));
    group.add(disc(.85,material(0x15181a),x,0,-7.57,true,6));
  }
  const ready=(async()=>{
    const response=await fetch(new URL('model.json',base),{signal:aborter.signal}); if(!response.ok)throw new Error(`StopWatch model: HTTP ${response.status}`);
    const metadata=await response.json();
    const binaryResponse=await fetch(new URL(metadata.binary,base),{signal:aborter.signal}); if(!binaryResponse.ok)throw new Error(`StopWatch geometry: HTTP ${binaryResponse.status}`);
    const binary=await binaryResponse.arrayBuffer(); if(disposed)return;
    for(const part of metadata.parts) {
      const geometry=new THREE.BufferGeometry();
      const interleaved=new THREE.InterleavedBuffer(new Float32Array(binary,part.byteOffset,part.vertexCount*6),6);
      geometry.setAttribute('position',new THREE.InterleavedBufferAttribute(interleaved,3,0)); geometry.setAttribute('normal',new THREE.InterleavedBufferAttribute(interleaved,3,3)); geometry.computeBoundingSphere();
      const mesh=new THREE.Mesh(geometry,material(part.color,{roughness:part.name==='shell'?.66:.4})); mesh.name=part.name;
      if(['button-a','button-b'].includes(part.name)){mesh.userData.control=part.name;buttons.push(mesh);}
      if(part.name==='power-button')mesh.userData.unsupported='当前没有实现 StopWatch 电源键控制；此红色按键仅展示外观，不会发送 GPIO 或伪造开关机。';
      if(part.name==='shell'){mesh.userData.inputOccluder=true;mesh.userData.originalAppearance={shell:{color:part.color,opacity:1,finish:'plastic'}};}
      parts[part.name]=mesh;group.add(mesh);
    }
    const loaded=await new THREE.TextureLoader().loadAsync(new URL('rear-label.webp',base).href);
    if(disposed){loaded.dispose();return;} labelTexture=loaded; labelTexture.colorSpace=THREE.SRGBColorSpace; labelTexture.anisotropy=4;
    const label=new THREE.Mesh(new THREE.PlaneGeometry(52,52),new THREE.MeshStandardMaterial({map:labelTexture,transparent:true,alphaTest:.5,roughness:.8,metalness:0,depthWrite:true}));
    label.name='rear-label-v1.0.1'; label.position.z=-7.79; label.rotation.y=Math.PI; group.add(label); group.userData.provenance=metadata;
    return metadata;
  })();
  return {group,screen,buttons,parts,connectors,ready,dispose(){disposed=true;aborter.abort();labelTexture?.dispose();pcbLabelTexture?.dispose();}};
}
