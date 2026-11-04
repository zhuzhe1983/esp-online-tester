import * as THREE from './vendor/three.module.min.js';

// Explicit millimetre outlines, not inferred PCB layouts. Edge cut-outs belong
// in the outer ring; holes must be enclosed, disjoint through-holes.
const epsilon=1e-8;
const cross=(a,b,c)=>(b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0]);
const onSegment=(a,b,p)=>Math.abs(cross(a,b,p))<epsilon&&p.every((v,i)=>v>=Math.min(a[i],b[i])-epsilon&&v<=Math.max(a[i],b[i])+epsilon);
function intersects(a,b,c,d) {
  const abC=cross(a,b,c),abD=cross(a,b,d),cdA=cross(c,d,a),cdB=cross(c,d,b);
  return (abC*abD<0&&cdA*cdB<0)||onSegment(a,b,c)||onSegment(a,b,d)||onSegment(c,d,a)||onSegment(c,d,b);
}
function inside(point,ring) {
  let result=false;
  for(let i=0,j=ring.length-1;i<ring.length;j=i++) {
    const a=ring[j],b=ring[i];if(onSegment(a,b,point))return false;
    if((a[1]>point[1])!==(b[1]>point[1])&&point[0]<(b[0]-a[0])*(point[1]-a[1])/(b[1]-a[1])+a[0])result=!result;
  }
  return result;
}
function validateRing(ring) {
  if(!Array.isArray(ring)||ring.length<3||ring.length>1024||ring.some(p=>!Array.isArray(p)||p.length!==2||p.some(v=>!Number.isFinite(v)||Math.abs(v)>5000)))throw Error('Invalid plate outline');
  let area=0;
  for(let i=0;i<ring.length;i++) {
    const a=ring[i],b=ring[(i+1)%ring.length];
    if(Math.hypot(a[0]-b[0],a[1]-b[1])<epsilon)throw Error('Invalid plate outline');
    area+=a[0]*b[1]-b[0]*a[1];
    for(let j=i+1;j<ring.length;j++)if(j!==i+1&&!(i===0&&j===ring.length-1)&&intersects(a,b,ring[j],ring[(j+1)%ring.length]))throw Error('Invalid plate outline');
  }
  if(Math.abs(area)<epsilon)throw Error('Invalid plate outline');
}
function ringsCross(a,b) {return a.some((p,i)=>b.some((q,j)=>intersects(p,a[(i+1)%a.length],q,b[(j+1)%b.length])));}
// Called after contour validation; reject even a supported-corner rectangle
// when it crosses a concave edge or completely encloses a through-hole.
export function plateSupportsRectangle({outlineMm,holesMm=[]},center,size) {
  const [x,y]=center,[w,h]=size,ring=[[x-w/2,y-h/2],[x+w/2,y-h/2],[x+w/2,y+h/2],[x-w/2,y+h/2]];
  return ring.every(p=>inside(p,outlineMm))&&!ringsCross(ring,outlineMm)&&!holesMm.some(hole=>ringsCross(ring,hole)||inside(hole[0],ring)||inside(ring[0],hole));
}
export function createPlateGeometry({outlineMm,holesMm=[],depthMm}={}) {
  if(!Number.isFinite(depthMm)||depthMm<=0||depthMm>5000)throw Error('Invalid plate depth');
  validateRing(outlineMm);
  if(!Array.isArray(holesMm)||holesMm.length>64||holesMm.reduce((n,h)=>n+(Array.isArray(h)?h.length:4097),outlineMm.length)>4096)throw Error('Invalid plate holes');
  for(const [i,hole] of holesMm.entries()) {
    validateRing(hole);
    if(!inside(hole[0],outlineMm)||ringsCross(hole,outlineMm)||holesMm.slice(0,i).some(other=>ringsCross(hole,other)||inside(hole[0],other)||inside(other[0],hole)))throw Error('Invalid plate holes');
  }
  const points=(ring,clockwise)=>{const result=ring.map(p=>new THREE.Vector2(...p));if(THREE.ShapeUtils.isClockWise(result)!==clockwise)result.reverse();return result;};
  const shape=new THREE.Shape(points(outlineMm,true));
  shape.holes=holesMm.map(ring=>new THREE.Path(points(ring,false)));
  return new THREE.ExtrudeGeometry(shape,{depth:depthMm,bevelEnabled:false,steps:1,curveSegments:1}).translate(0,0,-depthMm/2);
}
