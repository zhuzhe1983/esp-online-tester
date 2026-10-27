import * as THREE from './vendor/three.module.min.js';

// Firmware emits 8-bit RGB intensities. Keep the lens independent of shell
// recolouring, and show a neutral diffuser when all three channels are off.
export function applyRgbLedValues(meshes, values) {
  for (const [index, mesh] of meshes.entries()) {
    if (!mesh?.material) continue;
    const input = values?.[index] || [0,0,0];
    const rgb = [0,1,2].map(channel => Number.isFinite(input[channel]) ? Math.round(Math.max(0,Math.min(255,input[channel]))) : 0);
    const color = new THREE.Color().setRGB(...rgb.map(value => value/255), THREE.SRGBColorSpace);
    mesh.material.color.set(rgb.some(Boolean) ? color : mesh.userData.unlitColor ?? 0xc8d0cc);
    mesh.material.emissive?.copy(color); mesh.material.emissiveIntensity = .8;
    mesh.userData.rgb = rgb;
  }
}
