import * as THREE from './vendor/three.module.min.js';

// A declared viewing arrangement, not a reconstruction of hidden assembly.
// Validate every group before changing any transform. All offsets use model axes.
export function createModelInspection(group, definitions, sources = []) {
  if (definitions === undefined || Array.isArray(definitions) && definitions.length === 0) return null;
  if (!Array.isArray(definitions) || definitions.length > 32) throw Error('Invalid model inspection groups');
  const ids = new Set(), claimed = new Set(), moves = [], hides = [];
  const companions = new Set();
  group.traverse(part => { for (const companion of part.userData.inspectionCompanions || []) companions.add(companion); });
  const knownSources = new Set(sources.map(source => source.id));
  const label = value => typeof value === 'string' && value.length > 0 && value.length <= 128;
  for (const definition of definitions) {
    const {id, parts, hideParts = [], offsetMm, sourceIds, notes} = definition || {};
    if (!label(id) || ids.has(id) || !Array.isArray(parts) || !parts.length || parts.length > 128 ||
      !Array.isArray(hideParts) || hideParts.length > 128 ||
      !Array.isArray(offsetMm) || offsetMm.length !== 3 || offsetMm.some(v => !Number.isFinite(v) || Math.abs(v) > 5000) || !offsetMm.some(v => v !== 0) ||
      !Array.isArray(sourceIds) || !sourceIds.length || sourceIds.length > 32 || !sourceIds.every(value => label(value) && knownSources.has(value)) ||
      typeof notes !== 'string' || !notes.trim() || notes.length > 2000) throw Error('Invalid model inspection group');
    ids.add(id);
    const offset = new THREE.Vector3(...offsetMm);
    for (const [names,hidden] of [[parts,false],[hideParts,true]]) for (const name of names) {
      const matches = group.children.filter(child => child.name === name);
      if (!label(name) || matches.length !== 1 || claimed.has(matches[0])) throw Error('Invalid model inspection part ownership');
      const part = matches[0];
      if (companions.has(part)) throw Error('Invalid model inspection companion ownership');
      // Photo twins come from the loader's actual projection, never a name guess.
      const members = new Set([part]);
      part.traverse(source => {
        for (const companion of source.userData.inspectionCompanions || []) {
          let ancestor = companion; while (ancestor && ancestor !== part) ancestor = ancestor.parent;
          if (!ancestor) members.add(companion); // Descendants already follow their declared assembly.
        }
      });
      for (const object of members) {
        if (object.parent !== group || claimed.has(object)) throw Error('Invalid model inspection part ownership');
        claimed.add(object);
        if(hidden)hides.push({object,visible:object.visible});
        else moves.push({object, position: object.position.clone(), offset});
      }
    }
  }
  let expanded = false;
  return {
    get expanded() { return expanded; },
    setExpanded(value) {
      if (typeof value !== 'boolean') throw Error('Model inspection state must be boolean');
      for (const {object, position, offset} of moves) object.position.copy(position).addScaledVector(offset, value ? 1 : 0);
      for (const {object, visible} of hides) object.visible=value?false:visible;
      expanded = value; group.updateMatrixWorld(true); return expanded;
    },
  };
}
