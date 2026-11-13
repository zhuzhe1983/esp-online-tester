import * as THREE from './vendor/three.module.min.js';
// Clip actual shell triangles in a small projector volume. Positions remain in the
// shell's local space, so holes stay open and decals follow exploded assemblies.
export function createStickerGeometry(mesh,item,extent){
  const geometry=mesh.geometry,position=geometry.getAttribute('position'),normal=geometry.getAttribute('normal'),index=geometry.index,surfaceUv=geometry.getAttribute('uv');
  const width=extent*item.size,height=width/item.aspect,depth=Math.max(.25,Math.min(width,height)*.3);
  const anchor=item.anchor,axis=new THREE.Vector3(...anchor.normal).normalize();
  const basis=new THREE.Quaternion().setFromUnitVectors(new THREE.Vector3(0,0,1),axis);
  basis.multiply(new THREE.Quaternion().setFromAxisAngle(new THREE.Vector3(0,0,1),THREE.MathUtils.degToRad(item.rotation)));
  const center=new THREE.Vector3(...anchor.point).add(new THREE.Vector3(item.x*extent,item.y*extent,0).applyQuaternion(basis));
  const projector=new THREE.Matrix4().compose(center,basis,new THREE.Vector3(1,1,1)),inverse=projector.clone().invert();
  const positions=[],normals=[],uvs=[],surfaceUvs=[],count=index?index.count:position.count;
  const interpolate=(a,b,t)=>({p:a.p.clone().lerp(b.p,t),n:a.n.clone().lerp(b.n,t).normalize(),uv:a.uv.clone().lerp(b.uv,t)});
  function clip(vertices,component,sign,limit){
    const out=[];for(let i=0;i<vertices.length;i++){const a=vertices[i],b=vertices[(i+1)%vertices.length],da=sign*a.p.getComponent(component)-limit,db=sign*b.p.getComponent(component)-limit;if(da<=0)out.push(a);if((da<=0)!==(db<=0))out.push(interpolate(a,b,da/(da-db)));}return out;
  }
  for(let i=0;i<count;i+=3){
    const ids=[0,1,2].map(j=>index?index.getX(i+j):i+j),p=ids.map(id=>new THREE.Vector3().fromBufferAttribute(position,id));
    const face=new THREE.Vector3().subVectors(p[1],p[0]).cross(new THREE.Vector3().subVectors(p[2],p[0])).normalize();
    if(face.dot(axis)<.15)continue; // Never paint the opposite side of a thin shell.
    let vertices=ids.map((id,j)=>({p:p[j].applyMatrix4(inverse),n:normal?new THREE.Vector3().fromBufferAttribute(normal,id):face.clone(),uv:surfaceUv?new THREE.Vector2().fromBufferAttribute(surfaceUv,id):new THREE.Vector2()}));
    for(const [component,limit]of [[0,width/2],[1,height/2],[2,depth]])for(const sign of [-1,1])vertices=clip(vertices,component,sign,limit);
    for(let j=1;j+1<vertices.length;j++)for(const v of [vertices[0],vertices[j],vertices[j+1]]){uvs.push(.5+v.p.x/width,.5+v.p.y/height);positions.push(...v.p.clone().applyMatrix4(projector));normals.push(...v.n);surfaceUvs.push(...v.uv);}
  }
  const result=new THREE.BufferGeometry();result.setAttribute('position',new THREE.Float32BufferAttribute(positions,3));result.setAttribute('normal',new THREE.Float32BufferAttribute(normals,3));result.setAttribute('uv',new THREE.Float32BufferAttribute(uvs,2));result.setAttribute('surfaceUv',new THREE.Float32BufferAttribute(surfaceUvs,2));return result;
}
