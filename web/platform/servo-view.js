// Declarative joint geometry; pose comes only from board servo feedback.
import * as THREE from './vendor/three.module.min.js';
const triplet = value => Array.isArray(value) && value.length === 3 && value.every(Number.isFinite);

export function createServoJoints(group, definitions = []) {
  if (!Array.isArray(definitions) || definitions.length > 16) throw Error('Invalid servo joints');
  const joints = [], ids = new Set(), claimed = new Set(), originalParts = [...group.children];
  for (const definition of definitions) {
    if (!definition || typeof definition !== 'object') throw Error('Invalid servo joint');
    const {id, parent, axis, pivotMm, parts, neutralDegrees = 0, direction = 1} = definition;
    if (typeof id !== 'string' || !/^[a-z][a-z0-9-]{0,47}$/.test(id) || ids.has(id) || !triplet(axis) || (!Number.isFinite(Math.hypot(...axis)) || Math.hypot(...axis) < .000001) || !triplet(pivotMm) || pivotMm.some(v => Math.abs(v) > 5000) || !Number.isFinite(neutralDegrees) || ![-1,1].includes(direction) || !Array.isArray(parts) || !parts.length) throw Error('Invalid servo joint');
    const parentJoint = parent === undefined ? null : joints.find(joint => joint.userData.servoId === parent);
    if (parent !== undefined && !parentJoint) throw Error('Servo parent must precede child');
    const meshes = parts.map(name => {
      const matches = originalParts.filter(child => child.name === name), mesh = matches[0];
      if (typeof name !== 'string' || claimed.has(name) || matches.length !== 1) throw Error('Invalid servo part membership');
      claimed.add(name); return mesh;
    });
    ids.add(id);
    const pivot = new THREE.Group(); pivot.name = `servo:${id}`;
    pivot.userData = {servoId:id, axis:new THREE.Vector3(...axis).divideScalar(Math.hypot(...axis)), pivotMm:[...pivotMm], neutralDegrees, direction, positionDegrees:neutralDegrees};
    pivot.position.set(...pivotMm);
    if (parentJoint) pivot.position.sub(new THREE.Vector3(...parentJoint.userData.pivotMm));
    (parentJoint || group).add(pivot);
    // All declared parts begin in assembly coordinates. Keep those coordinates
    // exactly at neutral; no bounding-box recentering or geometry deformation.
    for (const mesh of meshes) { mesh.position.sub(new THREE.Vector3(...pivotMm)); pivot.add(mesh); }
    joints.push(pivot);
  }
  group.userData.servoJoints = joints;
  return joints;
}

export function applyServoValues(joints, values) {
  for (const joint of joints) {
    const data = joint.userData, state = (Array.isArray(values) ? values : []).find(value => value?.id === data.servoId);
    // Missing feedback resets the visual pose on firmware/device changes.
    // A disabled servo retains its measured position; targets never animate it.
    const position = Number.isFinite(state?.positionDegrees) ? state.positionDegrees : data.neutralDegrees;
    const angle = (position % 360 - data.neutralDegrees % 360) * Math.PI / 180 * data.direction;
    joint.quaternion.setFromAxisAngle(data.axis, angle);
    data.positionDegrees = position;
  }
}
