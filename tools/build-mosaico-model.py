#!/usr/bin/env python3
"""Rebuild Mosaico's official enclosure and separately labeled derived electronics.

Read the fixed-hash 3MF with --source, or explicitly fetch it with --allow-network.
The manufacturer assembly transforms are retained; printing-plate placement is
ignored. No nonuniform scaling, mesh simplification or missing triangles.
"""
import argparse, hashlib, io, json, math, struct, urllib.request, zipfile
import xml.etree.ElementTree as ET
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
ASSET=ROOT/'web/platform/assets/espressif-esp-mosaico'
NS={'m':'http://schemas.microsoft.com/3dmanufacturing/core/2015/02','p':'http://schemas.microsoft.com/3dmanufacturing/production/2015/06'}
PRESENTATION=[[ -1,0,0,0],[0,-1,0,0],[0,0,1,-5.550025463104248],[0,0,0,1]]
def source_bytes(path,allow_network):
 receipt=next(s for s in json.loads((ASSET/'source-manifest.json').read_text())['sources'] if s['id']=='official-enclosure-3mf')
 if path is not None:data=path.read_bytes()
 elif allow_network:
  with urllib.request.urlopen(receipt['url'],timeout=45) as response:
   if not response.url.startswith('https://'):raise ValueError('CAD source redirected outside HTTPS')
   data=response.read(receipt['bytes']+1)
 else:raise ValueError('Supply --source ESP-MOSAICO.3mf or explicitly opt in with --allow-network')
 if len(data)!=receipt['bytes'] or hashlib.sha256(data).hexdigest()!=receipt['sha256']:raise ValueError('Official 3MF size/SHA-256 mismatch; refusing changed source')
 return data
def matrix(text):
 a=[float(x) for x in text.split()]
 if len(a)!=12 or not all(math.isfinite(x) for x in a):raise ValueError('Invalid 3MF transform')
 return [[a[0],a[3],a[6],a[9]],[a[1],a[4],a[7],a[10]],[a[2],a[5],a[8],a[11]],[0,0,0,1]]
def transform(mat,p):return [sum(mat[i][j]*p[j] for j in range(3))+mat[i][3] for i in range(3)]
def bounds(points):return [[min(p[i] for p in points) for i in range(3)],[max(p[i] for p in points) for i in range(3)]]
def manufacturer_meshes(data):
 archive=zipfile.ZipFile(io.BytesIO(data));model=ET.fromstring(archive.read('3D/3dmodel.model'));settings=ET.fromstring(archive.read('Metadata/model_settings.config'))
 if model.get('unit')!='millimeter':raise ValueError('Unexpected CAD unit')
 license=next(m.text for m in model.findall('m:metadata',NS) if m.get('name')=='License')
 if license!='MakerWorld Exclusive License':raise ValueError('Unexpected source license')
 assembly={a.get('object_id'):a for a in settings.findall('.//assemble_item')}
 extruders={o.get('id'):int(next(m.get('value') for m in o.findall('metadata') if m.get('key')=='extruder')) for o in settings.findall('object')}
 colors=json.loads(archive.read('Metadata/project_settings.config'))['filament_colour'];result=[]
 for obj in model.findall('m:resources/m:object',NS):
  oid=int(obj.get('id'));component=obj.find('m:components/m:component',NS);path=component.get('{'+NS['p']+'}path').lstrip('/');mesh_id=component.get('objectid');raw=archive.read(path)
  src=next(o for o in ET.fromstring(raw).findall('m:resources/m:object',NS) if o.get('id')==mesh_id)
  vertices=[[float(v.get(k)) for k in ['x','y','z']] for v in src.findall('m:mesh/m:vertices/m:vertex',NS)]
  faces=[[int(t.get(k)) for k in ['v1','v2','v3']] for t in src.findall('m:mesh/m:triangles/m:triangle',NS)]
  cm=matrix(component.get('transform'));am=matrix(assembly[str(oid)].get('transform'))
  assembled=[transform(am,transform(cm,v)) for v in vertices];placed=[transform(PRESENTATION,v) for v in assembled]
  result.append(dict(objectId=oid,meshId=mesh_id,path=path,sha256=hashlib.sha256(raw).hexdigest(),componentMatrixRows=cm,assemblyMatrixRows=am,sourceBoundsMm=bounds(vertices),assemblyBoundsMm=bounds(assembled),boundsMm=bounds(placed),vertices=placed,faces=faces,color=colors[extruders[str(oid)]-1],sourceVertexCount=len(vertices),sourceTriangleCount=len(faces)))
 if {r['objectId']:len(r['faces']) for r in result}!={2:19916,4:2976,6:840}:raise ValueError('Unexpected source mesh inventory')
 return result
def write_model(destination,source):
 destination.mkdir(parents=True,exist_ok=True)
 metadata={'schemaVersion':'1','id':'espressif-esp-mosaico','unit':'mm','binary':'geometry.bin','strideFloats':6,'parts':[],'derivedParts':[]}
 values=[]
 def tri(a,b,c):
  u=[b[i]-a[i] for i in range(3)];v=[c[i]-a[i] for i in range(3)];n=[u[1]*v[2]-u[2]*v[1],u[2]*v[0]-u[0]*v[2],u[0]*v[1]-u[1]*v[0]];l=math.sqrt(sum(x*x for x in n))
  if l<=1e-10:raise ValueError('Degenerate triangle would lose source geometry')
  for p in [a,b,c]:values.extend([*p,*[x/l for x in n]])
 def quad(a,b,c,d):tri(a,b,c);tri(a,c,d)
 def part(name,color,role,fn,basis='image-measurement',sourceIds=None,**extra):
  at=len(values);fn();pts=[values[i:i+3] for i in range(at,len(values),6)]
  metadata['parts'].append(dict(name=name,color=color,role=role,byteOffset=at*4,vertexCount=(len(values)-at)//6,boundsMm=bounds(pts),basis=basis,sourceIds=sourceIds or ['isometric','front','back'],**extra))
 def rect(w,h,r):
  out=[]
  for cx,cy,start in [(w/2-r,h/2-r,0),(-w/2+r,h/2-r,90),(-w/2+r,-h/2+r,180),(w/2-r,-h/2+r,270)]:
   for i in range(13):
    t=math.radians(start+i*90/12);out.append([cx+r*math.cos(t),cy+r*math.sin(t)])
  return out
 def plate(w,h,r,z0,z1):
  ps=rect(w,h,r)
  for a,b in zip(ps,ps[1:]+ps[:1]):
   tri([0,0,z1],[*a,z1],[*b,z1]);tri([0,0,z0],[*b,z0],[*a,z0]);quad([*a,z0],[*b,z0],[*b,z1],[*a,z1])
 meshes=manufacturer_meshes(source)
 for mesh in meshes:
  oid=mesh['objectId'];groups={'shell':[]} if oid==2 else {'button-boot':[],'button-power':[],'button-link':[]} if oid==4 else {'button-ai':[]}
  for index,face in enumerate(mesh['faces']):
   points=[mesh['vertices'][i] for i in face];x=sum(p[0] for p in points)/3
   name='shell' if oid==2 else ('button-boot' if x < -4.5 else 'button-power' if x > 4.5 else 'button-link') if oid==4 else 'button-ai'
   groups[name].append(index)
  mesh['partitions']={name:len(indices) for name,indices in groups.items()}
  for name,indices in groups.items():
   def emit():
    for index in indices:tri(*[mesh['vertices'][i] for i in mesh['faces'][index]])
   controls={'button-boot':'boot','button-ai':'ai'}
   part(name,mesh['color'],'shell' if oid==2 else 'button',emit,basis='manufacturer-cad',sourceIds=['official-enclosure-3mf','official-assembly-guide'],sourceObjectId=oid,sourceMeshId=mesh['meshId'],assemblyGroup='official-white-dual-button' if oid==4 else name,**({'controlId':controls[name]} if name in controls else {}))
 # Display dimensions are independently specified. Its Z placement and corner radius
 # remain assembly estimates; the enclosure CAD does not include a display.
 part('front-cover','#101315','glass',lambda:plate(43.3,43.3,2.2,5.55,6.25),basis='manufacturer-datasheet-and-estimated-placement',sourceIds=['display','front'],notes='43.3mm square and 0.7mm cover thickness from H0216F002AMT004-1; corner radius and seated Z are estimates.')
 def derived(id,kind,geo,color='#333a39',**extra):metadata['derivedParts'].append(dict(id=id,kind=kind,geometry=geo,color=color,basis='image-measurement',sourceIds=['isometric','front','back'],**extra))
 for id,z,size,color in [('pcb-main',2.1,40,'#53655d'),('pcb-base',-4.9,42.2,'#222726')]:derived(id,'pcb-placeholder',dict(sizeMm=[size,size,1],centerMm=[0,0,z],labelSizeMm=[28,7],labelSide='both' if id=='pcb-main' else 'front'),color,label='UNKNOWN',notes='Estimated board-space placeholder. No trace, component, thickness or board-edge CAD claimed. Base rear appearance is photo-derived, not a manufacturer back cover.')
 derived('usb-c','connector-visual',dict(centerMm=[0,-21.95,-1.8],rotation=[math.pi/2,0,0]),connectorType='USB-C',allowConnections=False,connectionStatus='visual-only-unverified',notes='Generic visual mouth seated inside the original CAD USB opening; connector type and insertion depth are not manufacturer CAD.')
 for x in [-17,17]:
  for y in [-17,17]:
   derived(f'rear-screw-{x}-{y}','cylinder',dict(centerMm=[x,y,-5.58],radiusMm=1.25,depthMm=.35),'#171d1e',material=dict(metalness=.6,roughness=.4),notes='Four visible dark screw heads; exact fastener standard and hidden shank unknown.')
 for i in range(7):derived(f'rear-pad-{i}','box',dict(sizeMm=[2.1,2.8,.08],centerMm=[(i-3)*2.54,-12,-5.44]),'#c5a65b',material=dict(metalness=.6,roughness=.45),notes='Seven pads visible in V1.2 back photo; exact pitch/size estimated, not connector fit evidence.')
 derived('rear-brand','label',dict(sizeMm=[31,7],centerMm=[0,13,-5.415],rotation=[0,math.pi,0]),'#c5cbc5',texture='rear-brand.svg',notes='Text transcribed from V1.2 back photo; typography approximation, QR code deliberately omitted.')
 derived('rear-pad-labels','label',dict(sizeMm=[23,2.8],centerMm=[0,-15,-5.415],rotation=[0,math.pi,0]),'#d8ddd4',texture='rear-pad-labels.svg',notes='Back-view labels; see official electrical pin table, not this display text, for wiring.')
 metadata['presentation']=dict(bodySizeMm=[44.8980027,44.8980027,11.10000038],screen=dict(sizeMm=[38.39,38.39],positionMm=[0,0,6.26],rotation=[0,0,0]),inspectionGroups=[dict(id='front',parts=['front-cover','live-display'],offsetMm=[0,0,38],sourceIds=['guide-v12','display'],notes='Display lifted for inspection; offset is a viewer convention.'),dict(id='case',parts=['shell','button-boot','button-power','button-link','button-ai'],offsetMm=[60,-55,0],sourceIds=['official-enclosure-3mf','official-assembly-guide'],notes='The three manufacturer parts retain their assembly relationship, including the one-piece white dual-button controller.'),dict(id='base',parts=['pcb-base','usb-c']+[x['id'] for x in metadata['derivedParts'] if x['id'].startswith(('rear-screw','rear-pad','rear-brand'))],offsetMm=[0,0,-38],sourceIds=['back','guide-v12'],notes='Photo-derived base-board appearance separated along thickness; estimated internals do not become verified CAD.')])
 metadata['dimensions']=dict(width=44.8980027,height=44.8980027,depth=11.10000038,unit='mm',basis='manufacturer-cad',sourceId='official-enclosure-3mf',scope='Main shell only; no back cover or electronics supplied. Whole product manufacturing dimensions remain unverified.')
 manifest=json.loads((ASSET/'source-manifest.json').read_text())
 metadata['sourceGeometry']=dict(sourceSha256=hashlib.sha256(source).hexdigest(),unit='millimeter',scale=1,presentationMatrixRows=PRESENTATION,assemblySource='Metadata/model_settings.config assemble_item (component transforms composed first)',printingPlateTransformsUsed=False,triangleCount=23732,omittedTriangles=0,parts=[{k:v for k,v in mesh.items() if k not in ['vertices','faces']} for mesh in meshes],whiteButtonPartition='Complete triangles grouped by platform centroid X: BOOT < -4.5mm, POWER > 4.5mm, center linkage otherwise; no clipping, duplicate or omitted triangles.')
 references=[s for s in json.loads((ROOT/'docs/platform/evidence/mosaico-sources/sources.json').read_text()) if s['id'] in ['guide-v12','front','back','isometric','core-front','base-back','display','schematic-core','schematic-base']]
 metadata['provenance']=dict(method='manufacturer enclosure CAD with separately identified derived display and UNKNOWN board placeholders',sources=references+manifest['sources'],generator='tools/build-mosaico-model.py',license='Official enclosure/button geometry: MakerWorld Exclusive License (unchanged). No open redistribution license is claimed. Independently generated display/placeholder geometry and text SVG use repository MIT; manufacturer references and trademarks retain their original rights.',licenseFile='LICENSE-Model.txt',sourceAuthor=manifest['sourceAuthor'],materialBasis='Original colors follow the 3MF print-profile filament_colour/extruder fields (#CBC6B8 shell/white dual key, #FF671F AI key), not calibrated production-plastic measurements.')
 metadata['limitations']=['Only the shell and two button components come from manufacturer CAD; its original license is retained.','No back cover is present in this 3MF. The visible rear surface is an estimated PCB-base placeholder with photo-derived text/pads/screw heads.','The source part has no explicit V1.2 mechanical revision; matching to V1.2 is limited to official assembly guide and visible layout. Whole-product 1:1 or manufacturing tolerance acceptance is not claimed.','Display XY and cover thickness come from the panel datasheet; cover corner radius, seated Z and all internal board dimensions are estimates.','Two UNKNOWN board placeholders preserve stable IDs; wiring, battery, speaker, connector insertion depth and hidden fastener geometry remain unverified.','POWER and the white center linkage have no GPIO hotspot. No reference photo or generated UI covers the guest framebuffer.']
 binary=struct.pack('<'+'f'*len(values),*values);(destination/'geometry.bin').write_bytes(binary);metadata['artifacts']={'geometry.bin':hashlib.sha256(binary).hexdigest()};
 for name,svg in {
  'rear-brand.svg':'<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="256"><text x="512" y="115" text-anchor="middle" fill="#dddeda" font-family="sans-serif" font-size="104" font-style="italic" font-weight="bold">ESP-MOSAICO</text><text x="512" y="198" text-anchor="middle" fill="#dddeda" font-family="sans-serif" font-size="45">Powered by Espressif</text></svg>',
  'rear-pad-labels.svg':'<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="128"><text x="512" y="78" text-anchor="middle" fill="#dddeda" font-family="monospace" font-size="48">GND BOOT RST RX TX 5V GND</text></svg>'
 }.items():
  (destination/name).write_text(svg+'\n');metadata['artifacts'][name]=hashlib.sha256((destination/name).read_bytes()).hexdigest()
 license_text='ESP-MOSAICO manufacturer enclosure and button geometry\nSource author: ESP-Mosaico / Espressif\nOriginal 3MF metadata License: MakerWorld Exclusive License\nSource: https://mosaico.ai/api/v1/files/attachments/1/9734120a8783.3mf\nOfficial hardware page: https://mosaico.ai/en/hardware/bzWKymkXPu95\nLicense reference: https://makerworld.com/en/exclusive-model-policy\n\nThe original license designation is retained for all manufacturer-derived triangles.\nThis conversion does not relicense them under MIT or claim an open redistribution grant.\nGenerated display/UNKNOWN placeholders and text SVG are separate repository-authored work.\n'
 (destination/'LICENSE-Model.txt').write_text(license_text);metadata['artifacts']['LICENSE-Model.txt']=hashlib.sha256(license_text.encode()).hexdigest()
 manifest_bytes=(ASSET/'source-manifest.json').read_bytes();(destination/'source-manifest.json').write_bytes(manifest_bytes);metadata['artifacts']['source-manifest.json']=hashlib.sha256(manifest_bytes).hexdigest()
 (destination/'model.json').write_text(json.dumps(metadata,ensure_ascii=False,indent=2)+'\n');print(json.dumps({'vertices':len(values)//6,'bytes':len(binary),'manufacturerTriangles':23732,'geometrySha256':metadata['artifacts']['geometry.bin'],'modelSha256':hashlib.sha256((destination/'model.json').read_bytes()).hexdigest()}))
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--output',type=Path,default=ASSET);p.add_argument('--source',type=Path);p.add_argument('--allow-network',action='store_true');args=p.parse_args();write_model(args.output,source_bytes(args.source,args.allow_network))
