// Declarative manufacturer CAD loader. Source transforms and measurements live in model.json.
// Catalog entries never load executable product scripts or remote texture URLs.
import * as THREE from './vendor/three.module.min.js';
import {createGroveSocket,verifiedGrove} from './grove-geometry.js';
import {createPcbPlaceholderGeometry} from './pcb-placeholder.js';
const safeAsset = (name, root) => {
  if (typeof name !== 'string' || !/^[a-zA-Z0-9][a-zA-Z0-9._/-]*$/.test(name) || name.split('/').includes('..')) throw Error('Invalid local model asset');
  return new URL(name,root);
};
const vector = (v, fallback=[0,0,0]) => Array.isArray(v)&&v.length===3&&v.every(Number.isFinite)?v:fallback;
function geometry(values) {const result=new THREE.BufferGeometry(),buffer=new THREE.InterleavedBuffer(new Float32Array(values),6);result.setAttribute('position',new THREE.InterleavedBufferAttribute(buffer,3,0));result.setAttribute('normal',new THREE.InterleavedBufferAttribute(buffer,3,3));result.computeBoundingSphere();return result;}
function flatLabel(text) {
  if(typeof document==='undefined')return null;
  const canvas=document.createElement('canvas');canvas.width=512;canvas.height=128;const context=canvas.getContext('2d');context.fillStyle='#f2f2e9';context.font='bold 68px monospace';context.textAlign='center';context.textBaseline='middle';context.fillText(text,256,64);const texture=new THREE.CanvasTexture(canvas);texture.colorSpace=THREE.SRGBColorSpace;return texture;
}
function visualConnector(type,housingColor) {
  const group=new THREE.Group(),standard=(color,metalness=0)=>new THREE.MeshStandardMaterial({color,metalness,roughness:.55});
  const add=(size,color,position=[0,0,0],metalness=0)=>{const mesh=new THREE.Mesh(new THREE.BoxGeometry(...size),standard(color,metalness));mesh.position.set(...position);group.add(mesh);};
  if(type==='USB-C') {
    const capsule=(w,h)=>{const shape=new THREE.Shape(),r=h/2,l=(w-h)/2;shape.moveTo(-l,-r);shape.lineTo(l,-r);shape.absarc(l,0,r,-Math.PI/2,Math.PI/2);shape.lineTo(-l,r);shape.absarc(-l,0,r,Math.PI/2,Math.PI*1.5);return shape;};
    const rim=capsule(8.9,3.2);rim.holes.push(new THREE.Path(capsule(8.05,2.42).getPoints(32)));group.add(new THREE.Mesh(new THREE.ShapeGeometry(rim,32),standard(0x9ea7ad,.8)));add([7.8,2.2,.2],0x111616,[0,0,-.12]);add([6.4,.55,.3],0x303737,[0,0,.05]);
  } else return createGroveSocket({color:housingColor});

  return group;
}
export function createManufacturerModel(product, screenTexture) {
  const group=new THREE.Group();group.name=product.id;const parts={},buttons=[],connectors={},shells=[],textures=[],aborter=new AbortController();let disposed=false,screen=null;
  const modelUrl=new URL(product.model.url,import.meta.url),root=new URL('./',modelUrl),platform=new URL('./',import.meta.url);
  if(modelUrl.origin!==platform.origin||!modelUrl.pathname.startsWith(new URL('./assets/',platform).pathname))throw Error('Manufacturer models must be local platform assets');
  const ready=(async()=>{
    const response=await fetch(modelUrl,{signal:aborter.signal,cache:'no-store'});if(!response.ok)throw Error(`Model metadata HTTP ${response.status}`);const metadata=await response.json();
    const asset=name=>{const url=safeAsset(name,root),hash=metadata.artifacts?.[name];if(typeof hash==='string'&&/^[a-f0-9]{64}$/.test(hash))url.searchParams.set('sha256',hash);return url;};
    if(metadata.unit!=='mm'||metadata.strideFloats!==6||!Array.isArray(metadata.parts)||metadata.parts.length>512)throw Error('Invalid manufacturer model geometry contract');
    const file=await fetch(asset(metadata.binary),{signal:aborter.signal});if(!file.ok)throw Error(`Model geometry HTTP ${file.status}`);const binary=await file.arrayBuffer();if(disposed)return;
    if(binary.byteLength>64*1024*1024)throw Error('Model geometry exceeds 64 MiB');
    for(const part of metadata.parts) {
      if(!Number.isInteger(part.byteOffset)||part.byteOffset<0||part.byteOffset%4||!Number.isInteger(part.vertexCount)||part.vertexCount<3||part.vertexCount%3||part.byteOffset+part.vertexCount*24>binary.byteLength)throw Error('Invalid model part bounds');
      const values=new Float32Array(binary,part.byteOffset,part.vertexCount*6);if(!values.every(Number.isFinite))throw Error('Invalid model coordinate');
      const standard=new THREE.MeshStandardMaterial({color:part.color || '#737b79',roughness:.7,metalness:0});
      let regular=values,painted=null,uv=null,texture,decal=false;
      if(part.texture) {
        const projection=part.texture,[xmin,ymin,xmax,ymax]=projection.boundsMm || [];decal=projection.mode==='decal';
        if(projection.zPlanesMm && (!Array.isArray(projection.zPlanesMm)||projection.zPlanesMm.length>32||!projection.zPlanesMm.every(Number.isFinite)))throw Error('Invalid CAD texture planes');
        if(projection.zToleranceMm!==undefined&&(!Number.isFinite(projection.zToleranceMm)||projection.zToleranceMm<0||projection.zToleranceMm>1))throw Error('Invalid CAD texture plane tolerance');
        const lowZ=projection.minZMm ?? -Infinity,highZ=projection.maxZMm ?? Infinity;if(typeof lowZ!=='number'||typeof highZ!=='number'||Number.isNaN(lowZ)||Number.isNaN(highZ)||lowZ>highZ)throw Error('Invalid CAD texture depth range');
        if(projection.projection!=='xy'||![xmin,ymin,xmax,ymax].every(Number.isFinite)||xmax<=xmin||ymax<=ymin||!['front','back'].includes(projection.side))throw Error('Invalid CAD texture projection');
        const loaded=await new THREE.TextureLoader().loadAsync(asset(projection.file).href);if(disposed){loaded.dispose();standard.dispose();return;}texture=loaded;textures.push(loaded);texture.colorSpace=THREE.SRGBColorSpace;texture.anisotropy=4;
        const a=[],b=[],coordinates=[],sign=projection.side==='back'?-1:1,threshold=projection.minNormalZ ?? .85;
        for(let at=0;at<values.length;at+=18) {
          const planeMatch=!projection.zPlanesMm || projection.zPlanesMm.some(plane=>[values[at+2],values[at+8],values[at+14]].every(z=>Math.abs(z-plane)<=(projection.zToleranceMm??.005)));
          const selected=planeMatch && sign*(values[at+5]+values[at+11]+values[at+17])/3>=threshold && [values[at+2],values[at+8],values[at+14]].every(z=>z>=lowZ&&z<=highZ);
          (selected?b:a).push(...values.subarray(at,at+18));
          if(selected)for(let k=0;k<3;k++){const u=(values[at+k*6]-xmin)/(xmax-xmin);coordinates.push(sign===1?u:1-u,(values[at+k*6+1]-ymin)/(ymax-ymin));}
        }
        regular=decal?values:a;painted=b;uv=coordinates;
      }
      const mesh=new THREE.Mesh(geometry(regular),standard);mesh.name=part.name;mesh.userData.inputOccluder=true;group.add(mesh);parts[part.name]=mesh;
      if(part.role==='shell'||part.name==='shell'||(!part.role&&/case|shell|housing/i.test(part.name)))shells.push(mesh);
      if(Number.isInteger(part.ledIndex)&&part.ledIndex>=0&&part.ledIndex<256){mesh.userData.ledIndex=part.ledIndex;mesh.userData.unlitColor=part.color || '#cdd0c8';(group.userData.rgbLedMeshes ||= [])[part.ledIndex]=mesh;}
      if(part.encoderId){mesh.userData.encoder=part.encoderId;}
      if(part.controlId){mesh.userData.control=part.controlId;buttons.push(mesh);}
      // Paper is opaque where the alpha mask exists: write depth so a later
      // transparent shell cannot tint it. Cut-out pixels preserve real holes.
      if(painted?.length){const geom=geometry(painted);geom.setAttribute('uv',new THREE.Float32BufferAttribute(uv,2));const face=new THREE.Mesh(geom,new THREE.MeshBasicMaterial({map:texture,transparent:false,depthWrite:true,alphaTest:decal?.5:0,polygonOffset:true,polygonOffsetFactor:-2,polygonOffsetUnits:-4}));face.name=`${part.name}-photo-surface`;face.userData.inputOccluder=true;group.add(face);}
    }
    if(!shells.length&&metadata.parts.length&&metadata.parts.every(part=>!part.role)) shells.push(parts[metadata.parts[0].name]);
    parts.shell=shells[0];group.userData.shellMeshes=shells;
    for(const part of metadata.derivedParts || []) {
      const size=vector(part.geometry?.sizeMm || part.dimensionsMm,[20,20,1]),center=vector(part.geometry?.centerMm || part.centerMm);
      if(part.kind==='lathe') {
        // Radius/height profile in mm, revolved around local +Y; explicit product rotation follows.
        const points=part.geometry?.profileMm,surface=part.material || {};
        if(!Array.isArray(points)||points.length<2||points.length>128||points.some((p,i)=>!Array.isArray(p)||p.length!==2||!p.every(Number.isFinite)||p[0]<0||p[0]>5000||Math.abs(p[1])>5000||(i&&p[1]<points[i-1][1]))||!points.some(p=>p[0]>0)||points.at(-1)[1]<=points[0][1])throw Error('Invalid lathe profile');
        if(Object.keys(surface).some(key=>!['metalness','roughness'].includes(key))||Object.values(surface).some(v=>!Number.isFinite(v)||v<0||v>1))throw Error('Invalid derived material');
        const mesh=new THREE.Mesh(new THREE.LatheGeometry(points.map(p=>new THREE.Vector2(...p)),64),new THREE.MeshStandardMaterial({color:part.color || '#666d69',roughness:surface.roughness??.7,metalness:surface.metalness??0}));mesh.name=part.id;mesh.position.set(...center);mesh.rotation.set(...vector(part.geometry?.rotation));mesh.userData.inputOccluder=true;parts[part.id]=mesh;group.add(mesh);continue;
      }
      if(['box','cylinder','torus'].includes(part.kind)) {const g=part.geometry || {},surface=part.material || {};if(Object.keys(surface).some(key=>!['metalness','roughness'].includes(key))||Object.values(surface).some(v=>!Number.isFinite(v)||v<0||v>1))throw Error('Invalid derived material');const dimensions=part.kind==='box'?size:part.kind==='cylinder'?[g.radiusMm,g.depthMm]:[g.radiusMm,g.tubeMm];if(!dimensions.every(v=>Number.isFinite(v)&&v>0&&v<=5000))throw Error('Invalid derived part dimensions');const shape=part.kind==='torus'?new THREE.TorusGeometry(g.radiusMm,g.tubeMm,16,64):part.kind==='cylinder'?new THREE.CylinderGeometry(g.radiusMm,g.radiusMm,g.depthMm,48).rotateX(Math.PI/2):new THREE.BoxGeometry(...size);const mesh=new THREE.Mesh(shape,new THREE.MeshStandardMaterial({color:part.color || '#666d69',roughness:surface.roughness??.7,metalness:surface.metalness??0}));mesh.name=part.id;mesh.position.set(...center);mesh.rotation.set(...vector(part.geometry?.rotation));mesh.userData.inputOccluder=true;parts[part.id]=mesh;group.add(mesh);continue;}
      if(part.kind==='label') {
        if(!Array.isArray(part.geometry?.sizeMm)||part.geometry.sizeMm.length!==2||!part.geometry.sizeMm.every(v=>Number.isFinite(v)&&v>0&&v<=5000))throw Error('Invalid label dimensions');
        const map=await new THREE.TextureLoader().loadAsync(asset(part.texture).href);if(disposed){map.dispose();return;}textures.push(map);map.colorSpace=THREE.SRGBColorSpace;map.anisotropy=4;
        const face=new THREE.Mesh(new THREE.PlaneGeometry(...part.geometry.sizeMm),new THREE.MeshBasicMaterial({map,transparent:false,depthWrite:true,alphaTest:.5,polygonOffset:true,polygonOffsetFactor:-2,polygonOffsetUnits:-4}));face.name=part.id;face.position.set(...center);face.rotation.set(...vector(part.geometry.rotation));face.userData.inputOccluder=true;parts[part.id]=face;group.add(face);continue;
      }
      if(part.kind==='connector-visual'&&['USB-C','HY2.0-4P'].includes(part.connectorType)) {
        if(part.housingColor!==undefined&&!/^#[a-fA-F0-9]{6}$/.test(part.housingColor))throw Error('Invalid connector housing color');
        const socket=visualConnector(part.connectorType,part.housingColor);socket.name=part.id;socket.position.set(...center);socket.rotation.set(...vector(part.geometry?.rotation));socket.userData={kind:'connector-visual',basis:part.basis,sourceIds:part.sourceIds};parts[part.id]=socket;group.add(socket);
        const anchor=new THREE.Object3D();anchor.name=`connector:${part.id}:mouth`;anchor.userData={kind:'connector-anchor',type:part.connectorType,visualOnly:true};socket.add(anchor);const connector={type:part.connectorType,anchor,normal:new THREE.Vector3(0,0,1),allowConnections:part.allowConnections,pinOrder:part.pinOrder,sourceIds:part.sourceIds};if(part.connectorType==='USB-C'||verifiedGrove(connector))connectors[part.id]=connector;continue;
      }
      if(part.kind!=='pcb-placeholder')continue;
      const pcb=new THREE.Group();pcb.name=part.id || 'pcb-main';pcb.position.set(...center);pcb.rotation.set(...vector(part.geometry?.rotation));pcb.userData={kind:'pcb-placeholder',label:'UNKNOWN',estimated:true};
      pcb.add(new THREE.Mesh(createPcbPlaceholderGeometry(size,part.geometry?.shape),new THREE.MeshStandardMaterial({color:0x859188,roughness:.85})));
      const label=flatLabel('UNKNOWN');if(label){textures.push(label);const width=Math.min(size[0]*.82,42);for(const back of [false,true]){const mesh=new THREE.Mesh(new THREE.PlaneGeometry(width,width/4),new THREE.MeshBasicMaterial({map:label,transparent:true,depthWrite:false}));mesh.position.z=(back?-1:1)*(size[2]/2+.015);if(back)mesh.rotation.y=Math.PI;pcb.add(mesh);}}
      parts[pcb.name]=pcb;group.add(pcb);
    }
    for(const knob of metadata.presentation?.potentiometers || []) {
      if(typeof knob.id!=='string'||!Array.isArray(knob.parts)||!knob.parts.length||knob.parts.some(name=>!parts[name])||!Number.isFinite(knob.visualSweepRadians)||knob.visualSweepRadians<=0||knob.visualSweepRadians>2*Math.PI)throw Error('Invalid potentiometer presentation');
      const pivot=new THREE.Group();pivot.name=`potentiometer:${knob.id}`;pivot.position.set(...vector(knob.centerMm));pivot.userData.value=.5;pivot.userData.visualSweepRadians=knob.visualSweepRadians;group.add(pivot);
      for(const name of knob.parts){const mesh=parts[name];mesh.position.sub(pivot.position);pivot.add(mesh);mesh.userData.potentiometer={id:knob.id,pivot};}
      (group.userData.potentiometers ||= []).push(pivot);
    }
    // Photo-aligned cover keys are separate from the display's native touch
    // range. Invisible hit meshes leave printed art/materials unchanged.
    for(const region of metadata.presentation?.buttons || []) {
      if(!product.controls?.some(c=>c.id===region.id && c.type==='button'))throw Error('Unknown cover button');
      const size=region.sizeMm,pos=region.positionMm;
      if(!Array.isArray(size)||size.length!==2||!size.every(v=>Number.isFinite(v)&&v>0&&v<=5000)||!Array.isArray(pos)||pos.length!==3||!pos.every(Number.isFinite)||!['circle','capsule','rectangle'].includes(region.shape))throw Error('Invalid cover button region');
      let geometry;
      if(region.shape==='circle')geometry=new THREE.CircleGeometry(Math.min(...size)/2,48);
      else if(region.shape==='capsule'){const [w,h]=size,r=Math.min(w,h)/2,l=w/2-r,shape=new THREE.Shape();if(w<h)throw Error('Cover capsule width must cover its height');shape.moveTo(-l,-r);shape.lineTo(l,-r);shape.absarc(l,0,r,-Math.PI/2,Math.PI/2);shape.lineTo(-l,r);shape.absarc(-l,0,r,Math.PI/2,Math.PI*1.5);geometry=new THREE.ShapeGeometry(shape,24);}
      else geometry=new THREE.PlaneGeometry(...size);
      const mesh=new THREE.Mesh(geometry,new THREE.MeshBasicMaterial({transparent:true,opacity:0,depthWrite:false}));mesh.name=`cover-key:${region.id}`;mesh.position.set(...pos);mesh.rotation.set(...vector(region.rotation));mesh.userData={control:region.id,inputOccluder:true};parts[mesh.name]=mesh;buttons.push(mesh);group.add(mesh);
    }
    const display=metadata.presentation?.screen;
    if(display&&product.display){screen=new THREE.Mesh(display.shape==='circle'?new THREE.CircleGeometry(display.sizeMm[0]/2,96):new THREE.PlaneGeometry(...display.sizeMm),new THREE.MeshBasicMaterial({map:screenTexture,polygonOffset:true,polygonOffsetFactor:-2,polygonOffsetUnits:-4}));screen.position.set(...vector(display.positionMm));screen.rotation.set(...vector(display.rotation));screen.name='live-display';screen.userData.screen=true;parts.screen=screen;group.add(screen);}
    // Some devices (AtomS3) use the whole non-touch display cover as a physical button.
    if(screen && display.controlId && product.controls?.some(control=>control.id===display.controlId && control.type==='button')){screen.userData.control=display.controlId;buttons.push(screen);}
    group.userData.provenance=metadata;return metadata;
  })();
  return {group,get screen(){return screen;},buttons,parts,connectors,ready,dispose(){disposed=true;aborter.abort();for(const texture of textures)texture.dispose();}};
}
