// The renderer observes board feedback; no host command may overwrite these values.
export function validServoSnapshot(value) {
  return typeof value?.supply==='boolean' && Array.isArray(value.axes) && value.axes.length===2 &&
    ['pan','tilt'].every(id=>value.axes.filter(axis=>axis?.id===id).length===1) &&
    value.axes.every(axis=>Number.isFinite(axis.positionDegrees) && Number.isFinite(axis.targetDegrees) &&
      typeof axis.enabled==='boolean' && typeof axis.moving==='boolean');
}
