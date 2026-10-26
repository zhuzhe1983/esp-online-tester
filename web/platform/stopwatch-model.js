// Manufacturer CAD in millimetres; see assets/m5stack-stopwatch/model.json for provenance.
import * as THREE from './vendor/three.module.min.js';
const base = new URL('./assets/m5stack-stopwatch/', import.meta.url);
const material = (color, options = {}) => new THREE.MeshStandardMaterial({color, roughness: .65, ...options});
function cube(w,h,d,mat,x=0,y=0,z=0) { const m=new THREE.Mesh(new THREE.BoxGeometry(w,h,d),mat); m.position.set(x,y,z); return m; }
function disc(radius,mat,x,y,z,back=false,segments=128) { const m=new THREE.Mesh(new THREE.CircleGeometry(radius,segments),mat); m.position.set(x,y,z); if(back)m.rotation.y=Math.PI; return m; }
function capsule(w,h) {
  const r=h/2,l=(w-h)/2,s=new THREE.Shape(); s.moveTo(-l,-r); s.lineTo(l,-r); s.absarc(l,0,r,-Math.PI/2,Math.PI/2,false); s.lineTo(-l,r); s.absarc(-l,0,r,Math.PI/2,Math.PI*1.5,false); return s;
}
export function createStopwatchModel(screenTexture) {
  const group=new THREE.Group(); group.name='m5stack-stopwatch';
  const buttons=[], aborter=new AbortController(); let disposed=false, labelTexture;
  const glass=new THREE.Mesh(new THREE.CylinderGeometry(24.54,24.54,.5,128),material(0x080a0c,{roughness:.18,metalness:.15}));
  glass.rotation.x=Math.PI/2;glass.position.z=7.5;glass.name='cover-glass';group.add(glass);
  const screen=disc(22.225,new THREE.MeshBasicMaterial({map:screenTexture,polygonOffset:true,polygonOffsetFactor:-1,polygonOffsetUnits:-2}),0,0,7.78); screen.name='live-display'; screen.userData.screen=true; group.add(screen);
  // Opaque internals behind through-holes; this does not claim an electronics model.
  const opaqueInternals = new THREE.Mesh(new THREE.CylinderGeometry(23.5,23.5,12,128),new THREE.MeshBasicMaterial({color:0x050505}));
  opaqueInternals.rotation.x=Math.PI/2; opaqueInternals.name='interior-occlusion-only'; group.add(opaqueInternals);
  // Connector inserts are source-informed visual additions, not claimed component CAD.
  const port=new THREE.Group(); port.name='usb-c-insert'; port.rotation.x=Math.PI/2; port.position.set(0,-25.05,.65);
  const rim=capsule(8.9,3.2), inner=capsule(8.05,2.42); rim.holes.push(new THREE.Path(inner.getPoints(40)));
  port.add(new THREE.Mesh(new THREE.ShapeGeometry(rim,40),material(0xbac0c5,{metalness:.8,roughness:.3})));
  port.add(cube(7.7,2.1,.2,material(0x070808),0,0,-.15),cube(6.5,.55,.35,material(0x30343a),0,0,.02)); group.add(port);
  const grove=new THREE.Group(); grove.name='hy2-4p-insert'; grove.position.set(17.55,-17.55,.55);
  grove.setRotationFromMatrix(new THREE.Matrix4().makeBasis(new THREE.Vector3(Math.SQRT1_2,Math.SQRT1_2,0),new THREE.Vector3(0,0,1),new THREE.Vector3(Math.SQRT1_2,-Math.SQRT1_2,0)));
  const white=material(0xe7e6df),dark=material(0x121619),gold=material(0xb5a174,{metalness:.7});
  grove.add(cube(8,4.2,.6,dark,0,0,-.5),cube(8.3,.65,.9,white,0,2,0),cube(.65,4.3,1.2,white,-4,0,0),cube(.65,4.3,1.2,white,4,0,0),cube(2.2,.6,1,white,-2.8,-2,0),cube(2.2,.6,1,white,2.8,-2,0));
  for(let i=0;i<4;i++)grove.add(cube(.45,1,.7,gold,-3+i*2,.3,-.1)); group.add(grove);
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
      if(['button-a','button-b'].includes(part.name)){mesh.userData.control=part.name;buttons.push(mesh);} group.add(mesh);
    }
    const loaded=await new THREE.TextureLoader().loadAsync(new URL('rear-label.webp',base).href);
    if(disposed){loaded.dispose();return;} labelTexture=loaded; labelTexture.colorSpace=THREE.SRGBColorSpace; labelTexture.anisotropy=4;
    const label=new THREE.Mesh(new THREE.PlaneGeometry(52,52),new THREE.MeshStandardMaterial({map:labelTexture,transparent:true,alphaTest:.5,roughness:.8,metalness:0,depthWrite:true}));
    label.name='rear-label-v1.0.1'; label.position.z=-7.79; label.rotation.y=Math.PI; group.add(label); group.userData.provenance=metadata;
    return metadata;
  })();
  return {group,screen,buttons,ready,dispose(){disposed=true;aborter.abort();labelTexture?.dispose();}};
}
