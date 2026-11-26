// Partition one interleaved position/normal triangle at its photo's XY footprint.
// Every new vertex linearly interpolates all six source attributes. The outside
// fragments remain ordinary CAD so an image never stretches beyond its domain.
export function partitionTextureTriangle(values, bounds) {
  const [xmin,ymin,xmax,ymax]=bounds;
  let polygon=Array.from({length:3},(_,i)=>Array.from(values.slice(i*6,i*6+6)));
  const outside=[];
  const triangulate=(vertices,target)=>{for(let i=1;i+1<vertices.length;i++)target.push(...vertices[0],...vertices[i],...vertices[i+1]);};
  for(const [axis,edge,sign]of [[0,xmin,1],[0,xmax,-1],[1,ymin,1],[1,ymax,-1]]) {
    const inside=[],rejected=[];
    for(let i=0;i<polygon.length;i++) {
      const a=polygon[i],b=polygon[(i+1)%polygon.length],aInside=sign*(a[axis]-edge)>=0,bInside=sign*(b[axis]-edge)>=0;
      (aInside?inside:rejected).push(a);
      if(aInside!==bInside) {
        const t=(edge-a[axis])/(b[axis]-a[axis]),vertex=a.map((v,j)=>v+(b[j]-v)*t);
        vertex[axis]=edge;inside.push(vertex);rejected.push(vertex);
      }
    }
    triangulate(rejected,outside);polygon=inside;
    if(polygon.length<3)break;
  }
  const inside=[];triangulate(polygon,inside);return {inside,outside};
}
