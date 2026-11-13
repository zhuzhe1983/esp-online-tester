import * as THREE from './vendor/three.module.min.js';

// Fit the visible assembly in projection, including CAD whose origin is not its
// visual centre. Hidden input helpers and hover contours do not move the camera.
export function frameAssembly(camera, root, {zoom=1, fill=.86}={}) {
  root.updateWorldMatrix(true,true);
  const points=[],bounds=new THREE.Box3();
  root.traverseVisible(object=>{
    if(!object.isMesh||!object.geometry||object.userData.pickIgnore||object.userData.framingIgnore)return;
    if(![].concat(object.material||[]).some(material=>material.visible))return;
    object.geometry.computeBoundingBox();const box=object.geometry.boundingBox;if(!box||box.isEmpty())return;
    for(const x of [box.min.x,box.max.x])for(const y of [box.min.y,box.max.y])for(const z of [box.min.z,box.max.z]){
      const point=new THREE.Vector3(x,y,z).applyMatrix4(object.matrixWorld);points.push(point);bounds.expandByPoint(point);
    }
  });
  camera.clearViewOffset();
  if(!points.length){camera.position.set(0,0,158);camera.lookAt(0,0,0);camera.updateProjectionMatrix();return null;}
  const center=bounds.getCenter(new THREE.Vector3()),tan=Math.tan(THREE.MathUtils.degToRad(camera.fov/2));
  let distance=20;
  for(const point of points)distance=Math.max(distance,point.z-center.z+Math.max(Math.abs(point.y-center.y)/tan,Math.abs(point.x-center.x)/(tan*camera.aspect))/fill);
  distance=Math.max(bounds.max.z-center.z+camera.near*2,distance*zoom);
  camera.position.set(center.x,center.y,center.z+distance);camera.lookAt(center);
  camera.far=Math.max(3000,distance+bounds.getSize(new THREE.Vector3()).length()+1000);
  camera.updateProjectionMatrix();camera.updateMatrixWorld();
  const projected=new THREE.Box2();for(const point of points){point.project(camera);projected.expandByPoint(new THREE.Vector2(point.x,point.y));}
  const offset=projected.getCenter(new THREE.Vector2());
  // Perspective makes near/far faces unequal. Centre their projected outline,
  // rather than applying a product-specific left/right correction.
  camera.setViewOffset(2*camera.aspect,2,offset.x*camera.aspect,-offset.y,2*camera.aspect,2);
  return {center:offset.toArray(),size:projected.getSize(new THREE.Vector2()).toArray()};
}
