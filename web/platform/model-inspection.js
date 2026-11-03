import * as THREE from './vendor/three.module.min.js';

// A declared viewing arrangement, not a reconstruction of hidden assembly.
// Validate every group before changing any transform. All offsets use model axes.
export function createModelInspection(group, definitions, sources = []) {
  if (definitions === undefined || Array.isArray(definitions) && definitions.length === 0) return null;
  if (!Array.isArray(definitions) || definitions.length > 32) throw Error('Invalid model inspection groups');
  const ids = new Set(), claimed = new Set(), moves = [];
  const knownSources = new Set(sources.map(source => source.id));
  const label = value => typeof value === 'string' && value.length > 0 && value.length <= 128;
  for (const definition of definitions) {
    const {id, parts, offsetMm, sourceIds, notes} = definition || {};
    if (!label(id) || ids.has(id) || !Array.isArray(parts) || !parts.length || parts.length > 128 ||
      !Array.isArray(offsetMm) || offsetMm.length !== 3 || offsetMm.some(v => !Number.isFinite(v) || Math.abs(v) > 5000) || !offsetMm.some(v => v !== 0) ||
      !Array.isArray(sourceIds) || !sourceIds.length || sourceIds.length > 32 || !sourceIds.every(value => label(value) && knownSources.has(value)) ||
      typeof notes !== 'string' || !notes.trim() || notes.length > 2000) throw Error('Invalid model inspection group');
    ids.add(id);
    const offset = new THREE.Vector3(...offsetMm);
    for (const name of parts) {
      const matches = group.children.filter(child => child.name === name);
      if (!label(name) || matches.length !== 1 || claimed.has(matches[0])) throw Error('Invalid model inspection part ownership');
      const part = matches[0];
      // Photo twins come from the loader's actual projection, never a name guess.
      for (const object of [part, ...(part.userData.inspectionCompanions || [])]) {
        if (object.parent !== group || claimed.has(object)) throw Error('Invalid model inspection part ownership');
        claimed.add(object); moves.push({object, position: object.position.clone(), offset});
      }
    }
  }
  let expanded = false;
  return {
    get expanded() { return expanded; },
    setExpanded(value) {
      if (typeof value !== 'boolean') throw Error('Model inspection state must be boolean');
      for (const {object, position, offset} of moves) object.position.copy(position).addScaledVector(offset, value ? 1 : 0);
      expanded = value; group.updateMatrixWorld(true); return expanded;
    },
  };
}
