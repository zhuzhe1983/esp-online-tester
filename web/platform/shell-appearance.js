import * as THREE from './vendor/three.module.min.js';
import {SHELL_DEFAULT, isShellFinish} from './appearance.js';

export const DEFAULT_STOPWATCH_APPEARANCE = Object.freeze({shell: SHELL_DEFAULT});
const record = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const ownedMaterials = new WeakMap();
let brushEnvironment = null;
let brushEnvironmentUsers = 0;
// Visual approximations, not measured optical properties of the manufacturer's shell.
// Color/opacity stay independent of finish. The old plastic and metal BRDFs are retained.
const finishes = {
  plastic: {roughness: .66, metalness: 0},
  'glossy-plastic': {roughness: .14, metalness: 0, clearcoat: .7, clearcoatRoughness: .08},
  'frosted-plastic': {roughness: .56, metalness: 0, transmission: .52, thickness: .65, ior: 1.46, specularIntensity: .85},
  'clear-resin': {roughness: .055, metalness: 0, transmission: .92, thickness: 1.2, ior: 1.49, clearcoat: .95, clearcoatRoughness: .04},
  rubber: {roughness: .94, metalness: 0, specularIntensity: .32, ior: 1.42},
  metal: {roughness: .3, metalness: .78},
  'brushed-metal': {roughness: .39, metalness: .86, anisotropy: .49},
  'iridescent-titanium': {roughness: .29, metalness: .76, iridescence: 1, iridescenceIOR: 1.38, iridescenceThicknessRange: [180, 620], clearcoat: .28, clearcoatRoughness: .22},
  ceramic: {roughness: .23, metalness: 0, clearcoat: .62, clearcoatRoughness: .15, ior: 1.52},
};
const physicalDefaults = {anisotropy: 0, clearcoat: 0, clearcoatRoughness: 0, transmission: 0, thickness: 0, ior: 1.5, specularIntensity: 1, iridescence: 0, iridescenceIOR: 1.3, iridescenceThicknessRange: [100, 400]};

export function normalizeShellAppearance(value, defaults = DEFAULT_STOPWATCH_APPEARANCE) {
  if (value != null && (!record(value) || Object.keys(value).some(key => key !== 'shell'))) throw new Error('外观仅支持 shell 配置。');
  const shell = value?.shell === undefined ? {} : value.shell, original = defaults.shell;
  if (!record(shell) || Object.keys(shell).some(key => !['color', 'opacity', 'finish'].includes(key))) throw new Error('外壳仅支持颜色、不透明度和材质。');
  const result = Object.fromEntries(['color', 'opacity', 'finish'].map(key => [key, shell[key] === undefined ? original[key] : shell[key]]));
  if (typeof result.color !== 'string' || !/^#[0-9a-f]{6}$/i.test(result.color)) throw new Error('外壳颜色须使用 #RRGGBB。');
  if (!Number.isFinite(result.opacity) || result.opacity < 0 || result.opacity > 1) throw new Error('外壳不透明度须在 0 到 1 之间。');
  if (!isShellFinish(result.finish)) throw new Error('请选择有效的外壳材质。');
  return {shell: {...result, color: result.color.toLowerCase()}};
}

function sharedByAnotherMesh(shell, material) {
  let root = shell, shared = false;
  while (root.parent) root = root.parent;
  root.traverse(object => { if (object !== shell && [].concat(object.material || []).includes(material)) shared = true; });
  return shared;
}

function createBrushEnvironment() {
  const width = 64, height = 32, pixels = new Uint8Array(width * height * 4);
  for (let y = 0; y < height; y++) {
    const latitude = ((y + 0.5) / height - 0.5) * Math.PI;
    const vertical = Math.sin(latitude), radial = Math.cos(latitude);
    for (let x = 0; x < width; x++) {
      const azimuth = ((x + 0.5) / width - 0.5) * 2 * Math.PI;
      const horizontal = radial * Math.cos(azimuth), depth = radial * Math.sin(azimuth);
      const key = 0.62 * Math.max(0, -0.31 * horizontal + 0.54 * vertical + 0.78 * depth) ** 12;
      const rim = 0.28 * Math.max(0, 0.70 * horizontal - 0.17 * vertical - 0.69 * depth) ** 8;
      const luminance = Math.round(255 * (0.055 + 0.06 * Math.max(0, vertical) + key + rim));
      pixels.set([luminance, luminance, luminance, 255], (y * width + x) * 4);
    }
  }
  const texture = new THREE.DataTexture(pixels, width, height);
  texture.mapping = THREE.EquirectangularReflectionMapping;
  texture.colorSpace = THREE.LinearSRGBColorSpace;
  texture.minFilter = texture.magFilter = THREE.LinearFilter;
  texture.needsUpdate = true;
  return texture;
}

function setBrushEnvironment(state, enabled) {
  const {material} = state;
  if (enabled && !material.envMap) {
    // Supply a soft reflection studio only for this finish, without changing scene lighting.
    brushEnvironment ||= createBrushEnvironment();
    brushEnvironmentUsers++;
    state.usesBrushEnvironment = true;
    material.envMap = brushEnvironment;
    material.envMapIntensity = 0.58;
  } else if (!enabled && state.usesBrushEnvironment) {
    state.usesBrushEnvironment = false;
    material.envMap = null;
    material.envMapIntensity = state.sourceEnvMapIntensity;
    brushEnvironmentUsers--;
    if (brushEnvironmentUsers === 0) {
      brushEnvironment.dispose();
      brushEnvironment = null;
    }
  }
}

function addBrushShader(shader) {
  shader.fragmentShader = `
    float shellBrushHash(vec2 cell) {
      vec3 p = fract(vec3(cell.xyx) * vec3(0.1031, 0.11369, 0.13787));
      p += dot(p, p.yzx + 33.33);
      return fract((p.x + p.y) * p.z);
    }
    float shellBrushNoise(vec2 coordinate) {
      vec2 footprint = fwidth(coordinate);
      float detail = 1.0 - smoothstep(0.50, 1.35, max(footprint.x, footprint.y));
      vec2 cell = floor(coordinate);
      vec2 blend = smoothstep(0.0, 1.0, fract(coordinate));
      float a = mix(shellBrushHash(cell), shellBrushHash(cell + vec2(1.0, 0.0)), blend.x);
      float b = mix(shellBrushHash(cell + vec2(0.0, 1.0)), shellBrushHash(cell + vec2(1.0, 1.0)), blend.x);
      return (mix(a, b, blend.y) - 0.5) * detail;
    }
  ` + shader.fragmentShader;
  shader.fragmentShader = shader.fragmentShader.replace('#include <lights_physical_fragment>', `
    vec3 brushDirection = normalize(vShellBrushDirection);
    vec3 brushTangent = brushDirection - normal * dot(brushDirection, normal);
    float brushLength = length(brushTangent);
    float brushAlignment = smoothstep(0.06, 0.42, brushLength);
    vec3 grain = vShellGrainPosition;
    float brushVariation = brushAlignment * (
      0.64 * shellBrushNoise(vec2(grain.x * 0.30, grain.y * 4.5 + grain.z * 3.8)) +
      0.36 * shellBrushNoise(vec2(grain.x * 0.94, grain.y * 16.1 - grain.z * 13.7)));
    roughnessFactor = clamp(roughnessFactor + 0.11 * brushVariation, 0.0525, 1.0);
    diffuseColor.rgb *= 1.0 + 0.040 * brushVariation;
    #include <lights_physical_fragment>
    #ifdef USE_ANISOTROPY
      // The anisotropic highlight follows the projected machining axis on CAD parts.
      vec3 fallback = abs(normal.z) < 0.8 ? vec3(0.0, 0.0, 1.0) : vec3(0.0, 1.0, 0.0);
      material.anisotropyT = brushLength > 0.0001
        ? brushTangent / brushLength : normalize(cross(normal, fallback));
      material.anisotropyB = cross(normal, material.anisotropyT);
      material.anisotropy *= brushAlignment;
      material.alphaT = mix(pow2(material.roughness), 1.0, pow2(material.anisotropy));
    #endif`);
}

function shellMaterial(shell) {
  const previous = ownedMaterials.get(shell);
  if (previous?.material === shell.material) return previous;
  previous?.material.dispose();
  const source = shell.material;
  if (!source.isMeshStandardMaterial) throw new Error('外壳需要标准 PBR 材质。');
  const material = new THREE.MeshPhysicalMaterial();
  // Copy only the standard surface properties; PhysicalMaterial.copy requires a physical source.
  THREE.MeshStandardMaterial.prototype.copy.call(material, source);
  material.defines = {STANDARD: '', PHYSICAL: ''};
  const sourceCompile = source.onBeforeCompile, sourceCacheKey = source.customProgramCacheKey();
  const state = {material, brushed: false, titanium: false, usesBrushEnvironment: false, sourceEnvMapIntensity: source.envMapIntensity};
  material.onBeforeCompile = function(shader, renderer) {
    sourceCompile.call(this, shader, renderer);
    if (!state.brushed && !state.titanium) return;
    // CAD meshes can use their local coordinates even when they have no UVs.
    const varyings = 'varying vec3 vShellGrainPosition;\n' +
      (state.brushed ? 'varying vec3 vShellBrushDirection;\n' : '');
    const brushDirection = state.brushed ? '\nvShellBrushDirection = mat3(modelViewMatrix) * vec3(1.0, 0.0, 0.0);' : '';
    shader.vertexShader = varyings + shader.vertexShader.replace('#include <begin_vertex>',
      '#include <begin_vertex>\nvShellGrainPosition = position;' + brushDirection);
    shader.fragmentShader = varyings + shader.fragmentShader;
    if (state.brushed) addBrushShader(shader);
    if (state.titanium) {
      shader.fragmentShader = shader.fragmentShader.replace('#include <lights_physical_fragment>', `
      // Display-only oxide tint, plus the physical angular thin-film response below.
      float titaniumPhase = vShellGrainPosition.y * 0.055 + vShellGrainPosition.x * 0.026;
      vec3 titaniumTint = 0.46 + 0.34 * cos(titaniumPhase + vec3(0.0, 2.1, 4.2));
      diffuseColor.rgb *= titaniumTint;
      #include <lights_physical_fragment>
      #ifdef USE_IRIDESCENCE
        float titaniumBand = 0.5 + 0.5 * sin(vShellGrainPosition.y * 0.055 + vShellGrainPosition.x * 0.026);
        material.iridescenceThickness = mix(180.0, 620.0, titaniumBand);
      #endif`);
      shader.fragmentShader = shader.fragmentShader.replace('#include <roughnessmap_fragment>', `#include <roughnessmap_fragment>
      float shellGrainPhase = vShellGrainPosition.y * 8.0;
      float shellGrainWidth = fwidth(shellGrainPhase);
      float shellGrain = sin(shellGrainPhase + sin(vShellGrainPosition.x * 0.45) * 0.4) / (1.0 + shellGrainWidth * shellGrainWidth);
      roughnessFactor = clamp(roughnessFactor * (1.0 + 0.18 * shellGrain), 0.04, 1.0);`);
    }
  };
  material.customProgramCacheKey = () => `${sourceCacheKey}|shell-grain:${state.brushed ? 1 : 0}|titanium:${state.titanium ? 1 : 0}`;
  const release = () => {
    if (ownedMaterials.get(shell) === state) ownedMaterials.delete(shell);
    material.removeEventListener('dispose', release);
    setBrushEnvironment(state, false);
    material.onBeforeCompile = () => {};
    material.customProgramCacheKey = () => '';
  };
  material.addEventListener('dispose', release);
  shell.material = material; ownedMaterials.set(shell, state);
  // Other surfaces may share the input material. Leave those properties and resources untouched.
  if (!sharedByAnotherMesh(shell, source)) source.dispose();
  return state;
}

export function applyShellAppearance(shell, value, {occluder} = {}) {
  if (!shell?.material) return;
  const appearance = normalizeShellAppearance(value).shell, state = shellMaterial(shell), material = state.material;
  Object.assign(material, physicalDefaults, finishes[appearance.finish]);
  if (appearance.finish === 'plastic' && appearance.opacity < 1) material.roughness = .32;
  state.brushed = appearance.finish === 'brushed-metal'; state.titanium = appearance.finish === 'iridescent-titanium';
  setBrushEnvironment(state, state.brushed);
  material.color.set(appearance.color); material.opacity = appearance.opacity;
  const seeThrough = appearance.opacity < 1 || material.transmission > 0;
  material.transparent = appearance.opacity < 1; material.depthWrite = !seeThrough;
  material.side = seeThrough ? THREE.DoubleSide : THREE.FrontSide;
  material.needsUpdate = true;
  // Opacity is a viewing preference, not removal of the physical shell for input picking.
  shell.userData.inputOccluder = true;
  if (occluder) occluder.visible = !seeThrough;
}
