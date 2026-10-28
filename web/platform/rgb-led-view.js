import * as THREE from './vendor/three.module.min.js';

// A small, depth-tested diffusion fringe follows the emitting face. It is an
// optical presentation, not a hover highlight or a calibrated physical light.
export function addRgbLedGlow(mesh) {
  mesh.geometry.computeBoundingBox();
  const bounds=mesh.geometry.boundingBox,size=bounds.getSize(new THREE.Vector3()),center=bounds.getCenter(new THREE.Vector3());
  const pixels=new Uint8Array(32*32*4);
  for(let y=0;y<32;y++)for(let x=0;x<32;x++){const at=(y*32+x)*4,r=Math.hypot((x-15.5)/15.5,(y-15.5)/15.5),core=Math.round(255*(.12+.88*Math.exp(-4*r*r)));pixels.set([core,core,core,Math.round(255*Math.max(0,1-r)**2)],at);}
  const map=new THREE.DataTexture(pixels,32,32);map.needsUpdate=true;map.minFilter=map.magFilter=THREE.LinearFilter;
  // The same neutral kernel softens the emitting face. Otherwise a uniformly
  // emissive CAD square still reads as painted plastic, even at high exposure.
  mesh.material.emissiveMap=map;mesh.material.needsUpdate=true;
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
    // PWM is linear intensity. A bounded display exposure makes a dim physical
    // emitter visible on an ordinary monitor without clipping the full-bright
    // diffuser. Scale all channels together: no hue shift or guest-data change.
    // This is an optical presentation curve, not measured LED photometry.
    const peak=Math.max(...rgb),exposure=peak ? .62*(peak/255)**.20 : 0;
    const color = new THREE.Color().setRGB(...rgb.map(value => peak ? exposure*value/peak : 0), THREE.LinearSRGBColorSpace);
    // Emission dominates; a small diffuse tint avoids scene lights driving the
    // bright lens beyond the display range. Off restores the original diffuser.
    if(peak)mesh.material.color.copy(color).multiplyScalar(.02);
    else mesh.material.color.set(mesh.userData.unlitColor ?? 0xc8d0cc);
    mesh.material.emissive?.copy(color); mesh.material.emissiveIntensity = 1;
    const glow=mesh.userData.ledGlow;if(glow){glow.visible=peak>0;glow.material.color.copy(color).multiplyScalar(exposure?1/exposure:0);glow.material.opacity=.09*Math.sqrt(exposure/.62);}
    mesh.userData.rgb = rgb;
  }
}
