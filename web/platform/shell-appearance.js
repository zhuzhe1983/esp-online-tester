import * as THREE from './vendor/three.module.min.js';
import {SHELL_DEFAULT, isShellFinish} from './appearance.js';

export const DEFAULT_STOPWATCH_APPEARANCE = Object.freeze({shell: SHELL_DEFAULT});
const record = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const ownedMaterials = new WeakMap();
// Visual approximations, not measured optical properties of the manufacturer's shell.
// Color/opacity stay independent of finish. The old plastic and metal BRDFs are retained.
const finishes = {
  plastic: {roughness: .66, metalness: 0},
  'glossy-plastic': {roughness: .14, metalness: 0, clearcoat: .7, clearcoatRoughness: .08},
  'frosted-plastic': {roughness: .56, metalness: 0, transmission: .52, thickness: .65, ior: 1.46, specularIntensity: .85},
  'clear-resin': {roughness: .055, metalness: 0, transmission: .92, thickness: 1.2, ior: 1.49, clearcoat: .95, clearcoatRoughness: .04},
  rubber: {roughness: .94, metalness: 0, specularIntensity: .32, ior: 1.42},
  metal: {roughness: .3, metalness: .78},
  'brushed-metal': {roughness: .43, metalness: .92},
  ceramic: {roughness: .23, metalness: 0, clearcoat: .62, clearcoatRoughness: .15, ior: 1.52},
};
const physicalDefaults = {clearcoat: 0, clearcoatRoughness: 0, transmission: 0, thickness: 0, ior: 1.5, specularIntensity: 1};

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
  const state = {material, brushed: false};
  material.onBeforeCompile = function(shader, renderer) {
    sourceCompile.call(this, shader, renderer);
    if (!state.brushed) return;
    // Object-space procedural grain needs neither UVs nor texture allocations on the CAD shell.
    shader.vertexShader = 'varying vec3 vShellGrainPosition;\n' + shader.vertexShader;
    shader.vertexShader = shader.vertexShader.replace('#include <begin_vertex>', '#include <begin_vertex>\nvShellGrainPosition = position;');
    shader.fragmentShader = 'varying vec3 vShellGrainPosition;\n' + shader.fragmentShader;
    shader.fragmentShader = shader.fragmentShader.replace('#include <roughnessmap_fragment>', `#include <roughnessmap_fragment>
      float shellGrainPhase = vShellGrainPosition.y * 8.0;
      float shellGrainWidth = fwidth(shellGrainPhase);
      float shellGrain = sin(shellGrainPhase + sin(vShellGrainPosition.x * 0.45) * 0.4) / (1.0 + shellGrainWidth * shellGrainWidth);
      roughnessFactor = clamp(roughnessFactor * (1.0 + 0.18 * shellGrain), 0.04, 1.0);`);
  };
  material.customProgramCacheKey = () => `${sourceCacheKey}|shell-grain:${state.brushed ? 1 : 0}`;
  const release = () => {
    if (ownedMaterials.get(shell) === state) ownedMaterials.delete(shell);
    material.removeEventListener('dispose', release);
    // Renderer disposal releases the generated shader program; there are no owned texture maps.
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
  state.brushed = appearance.finish === 'brushed-metal';
  material.color.set(appearance.color); material.opacity = appearance.opacity;
  const seeThrough = appearance.opacity < 1 || material.transmission > 0;
  material.transparent = appearance.opacity < 1; material.depthWrite = !seeThrough;
  material.side = seeThrough ? THREE.DoubleSide : THREE.FrontSide;
  material.needsUpdate = true;
  // Opacity is a viewing preference, not removal of the physical shell for input picking.
  shell.userData.inputOccluder = true;
  if (occluder) occluder.visible = !seeThrough;
}
