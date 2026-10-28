import * as THREE from './vendor/three.module.min.js';

// A small, depth-tested diffusion fringe follows the emitting face. It is an
// optical presentation, not a hover highlight or a calibrated physical light.
export function addRgbLedGlow(mesh) {
  mesh.geometry.computeBoundingBox();
  const bounds=mesh.geometry.boundingBox,size=bounds.getSize(new THREE.Vector3()),center=bounds.getCenter(new THREE.Vector3());
  const pixels=new Uint8Array(32*32*4);
  for(let y=0;y<32;y++)for(let x=0;x<32;x++){const at=(y*32+x)*4,r=Math.hypot((x-15.5)/15.5,(y-15.5)/15.5);pixels.set([255,255,255,Math.round(255*Math.max(0,1-r)**2)],at);}
  const map=new THREE.DataTexture(pixels,32,32);map.needsUpdate=true;map.minFilter=map.magFilter=THREE.LinearFilter;
  const glow=new THREE.Mesh(new THREE.PlaneGeometry(size.x*2.6,size.y*2.6),new THREE.MeshBasicMaterial({map,color:0,transparent:true,opacity:0,depthWrite:false,blending:THREE.AdditiveBlending,toneMapped:false}));
  glow.name='led-diffusion';glow.position.set(center.x,center.y,bounds.max.z+.006);glow.raycast=()=>{};glow.visible=false;
  mesh.add(glow);mesh.userData.ledGlow=glow;return map;
}

// Firmware emits 8-bit RGB intensities. Keep the lens independent of shell
// recolouring, and show a neutral diffuser when all three channels are off.
export function applyRgbLedValues(meshes, values) {
  for (const [index, mesh] of meshes.entries()) {
    if (!mesh?.material) continue;
    const input = values?.[index] || [0,0,0];
    const rgb = [0,1,2].map(channel => Number.isFinite(input[channel]) ? Math.round(Math.max(0,Math.min(255,input[channel]))) : 0);
    // LED bytes describe PWM intensity, not gamma-encoded image pixels. Decoding
    // them as sRGB crushed low settings (e.g. 19/255) into an almost black face.
    const color = new THREE.Color().setRGB(...rgb.map(value => value/255), THREE.LinearSRGBColorSpace);
    mesh.material.color.set(rgb.some(Boolean) ? color : mesh.userData.unlitColor ?? 0xc8d0cc);
    mesh.material.emissive?.copy(color); mesh.material.emissiveIntensity = .8;
    const glow=mesh.userData.ledGlow;if(glow){const peak=Math.max(...rgb)/255;glow.visible=peak>0;glow.material.color.copy(color).multiplyScalar(peak?1/peak:0);glow.material.opacity=.35*Math.sqrt(peak);}
    mesh.userData.rgb = rgb;
  }
}
