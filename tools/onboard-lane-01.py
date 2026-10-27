#!/usr/bin/env python3
"""Lane 01 source-backed onboarding. 'prepare' downloads exact sources; 'build' uses reviewed product plans.
Dependency versions: tools/requirements-model.txt. Never generates substitute box products.
"""
import argparse,base64,datetime,hashlib,json,re,shutil,struct,subprocess
from pathlib import Path
from urllib.parse import urlparse,quote
from html.parser import HTMLParser
from concurrent.futures import ThreadPoolExecutor
import numpy as np,trimesh
from PIL import Image,ImageDraw
RUN=Path('.butler/runs/20261027-010438-m5-hardware-83-esp32-esp-product');TEMP=Path('output/source/lane01');REV='e29c139cbc7c80b5ccc5557e5ab8025a7a19a109'
PRIORITIES=[1,5,8,11,14,18,21,24,27,30,33,36,39,42,45,48,51,54,57,60,63,66,69,72,75,78,81]
def now():return datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')
def sha(b):return hashlib.sha256(b).hexdigest()
def write(path,data):path.parent.mkdir(parents=True,exist_ok=True);path.write_text(json.dumps(data,ensure_ascii=False,indent=2)+'\n')
class Article(HTMLParser):
 def __init__(self):super().__init__();self.active=0;self.heading='';self.h=False;self.links=[];self.images=[];self.link=None;self.tables=[];self.table=None;self.row=None;self.cell=None
 def handle_starttag(self,t,a):
  d=dict(a)
  if t=='div' and 'markdown' in d.get('class','').split():self.active=1;return
  if not self.active:return
  if t=='div':self.active+=1
  if t in ['h1','h2','h3','h4']:self.h=True;self.heading=''
  if t=='a':self.link={'heading':self.heading,'url':d.get('href',''),'title':''}
  if t=='img':self.images.append({'heading':self.heading,'url':d.get('src',''),'title':d.get('alt','')})
  if t=='table':self.table={'heading':self.heading,'rows':[]}
  if t=='tr':self.row=[]
  if t in ['td','th']:self.cell=''
  if t=='br' and self.cell is not None:self.cell+='; '
 def handle_endtag(self,t):
  if not self.active:return
  if t=='div':self.active-=1
  if t in ['h1','h2','h3','h4']:self.h=False
  if t=='a' and self.link:self.links.append(self.link);self.link=None
  if t in ['td','th'] and self.row is not None and self.cell is not None:self.row.append(re.sub(r'\s+',' ',self.cell).strip());self.cell=None
  if t=='tr' and self.table is not None and self.row is not None:self.table['rows'].append(self.row);self.row=None
  if t=='table' and self.table:self.tables.append(self.table);self.table=None
 def handle_data(self,s):
  if self.h:self.heading+=s
  if self.link:self.link['title']+=s
  if self.cell is not None:self.cell+=s

def download(job):
 folder,source=job;f=folder/source['archiveName'];receiptfile=f.with_suffix(f.suffix+'.receipt.json')
 if receiptfile.exists() and f.exists():
  receipt=json.loads(receiptfile.read_text())
  if receipt.get('sha256')==sha(f.read_bytes()):return receipt
 r=subprocess.run(['curl','--fail','--location','--silent','--show-error','--max-time','50','--max-filesize','33554432','--output',str(f),'--write-out','%{url_effective}\n%{content_type}',quote(source['url'],safe=':/%?=&#+@;,-_~') ],text=True,capture_output=True)
 if r.returncode:result={**source,'status':'failed','attemptedAt':now(),'error':r.stderr.strip()[:400]};result.pop('archiveName',None)
 else:
  resolved,media=r.stdout.splitlines();b=f.read_bytes();result={**source,'status':'fetched','resolvedUrl':resolved,'fetchedAt':now(),'bytes':len(b),'mediaType':media,'sha256':sha(b)}
 write(receiptfile,result);return result

# KiCad container format: https://docs.kicad.org/doxygen/classEMBEDDED__FILES.html
# Independent MurmurHash3 x64/128 computation (Austin Appleby public-domain
# algorithm); legacy mode models KiCad's documented pre-fix tail padding.
mask=(1<<64)-1
rot=lambda n,b:((n<<b)|(n>>(64-b)))&mask

def kicad_murmur_checksum(data,legacy=False):
 h1=h2=0xABBA2345;c1=0x87c37b91114253d5;c2=0x4cf5ad432745937f;tail=len(data)%16;size=len(data)-tail
 for a,b in struct.iter_unpack('<QQ',data[:size]):
  h1^=rot(a*c1&mask,31)*c2&mask;h1=((rot(h1,27)+h2)*5+0x52dce729)&mask
  h2^=rot(b*c2&mask,33)*c1&mask;h2=((rot(h2,31)+h1)*5+0x38495ab5)&mask
 end=data[size:];length=len(data)
 if legacy and tail:
  padded=tail+4-tail%4;end=end.ljust(padded,b'\0');length=size+padded
  if padded==16:end=b''
 if len(end)>8:h2^=rot(int.from_bytes(end[8:],'little')*c2&mask,33)*c1&mask
 if end:h1^=rot(int.from_bytes(end[:8],'little')*c1&mask,31)*c2&mask
 def final(n):
  n=((n^(n>>33))*0xff51afd7ed558ccd)&mask;n=((n^(n>>33))*0xc4ceb9fe1a85ec53)&mask;return n^(n>>33)
 h1^=length;h2^=length;h1=(h1+h2)&mask;h2=(h2+h1)&mask;h1=final(h1);h2=final(h2);h1=(h1+h2)&mask;h2=(h2+h1)&mask
 return f'{h1:016X}{h2:016X}'

def extract_kicad_steps(container,folder):
 import zstandard
 folder=Path(folder);raw=(folder/container['archiveName']).read_bytes();assert sha(raw)==container['sha256']
 pattern=r'\(file\s+\(name "([^"]+\.(?:step|stp))"\)\s+\(type [^)]+\)\s+\(data\s+\|(.*?)\|\s*\)\s+\(checksum "([0-9A-Fa-f]+)"\)'
 result=[]
 for name,data,checksum in re.findall(pattern,raw.decode(),re.S|re.I):
  assert Path(name).name==name and len(name)<240
  compressed=base64.b64decode(re.sub(r'\s+','',data),validate=True);size=zstandard.frame_content_size(compressed);assert 0<size<=64*1024*1024,'Embedded STEP exceeds64MiB'
  payload=zstandard.ZstdDecompressor().decompress(compressed,max_output_size=64*1024*1024);assert len(payload)==size and payload.startswith(b'ISO-10303-21;')
  actual=kicad_murmur_checksum(payload);algorithm='MurmurHash3-x64-128'
  if actual!=checksum.upper():actual=kicad_murmur_checksum(payload,True);algorithm+='-KiCad-V1-tail-padding'
  assert actual==checksum.upper(),'Embedded KiCad checksum mismatch'
  url=container['url']+'#embedded='+quote(name);sid='manufacturer-cad-'+sha(url.encode())[:10];filename=sid+'.step';(folder/filename).write_bytes(payload)
  source={'id':sid,'title':'Embedded manufacturer STEP: '+name,'url':url,'resolvedUrl':url,'kind':'manufacturer-cad','status':'fetched','fetchedAt':container['fetchedAt'],'mediaType':'application/step','archiveName':filename,'bytes':len(payload),'sha256':sha(payload),'discoveredFrom':container['id'],'acquisition':{'method':'base64+zstd embedded payload; not a separate HTTP request','containerId':container['id'],'containerSha256':container['sha256'],'embeddedName':name,'checksumAlgorithm':algorithm,'declaredChecksum':checksum,'verifiedChecksum':actual,'decoder':'zstandard0.25.0'},'notes':'Fetch time is the actual enclosing official file acquisition. URL fragment identifies the embedded record; payload bytes/hash are distinct from the retained container receipt.'};result.append(source)
 return result

def read_step_styles(path):
 """Native STEP names/sRGB display colors, in load_parts' stable topology order.
 Colors are manufacturer CAD display hints, not measured optical properties.
 """
 from OCP.STEPCAFControl import STEPCAFControl_Reader
 from OCP.TDocStd import TDocStd_Document
 from OCP.TCollection import TCollection_ExtendedString
 from OCP.XCAFDoc import XCAFDoc_DocumentTool,XCAFDoc_ColorSurf,XCAFDoc_ColorGen
 from OCP.TDF import TDF_Label
 from OCP.TDataStd import TDataStd_Name
 from OCP.Quantity import Quantity_Color,Quantity_TOC_sRGB
 from OCP.TopExp import TopExp_Explorer
 from OCP.TopAbs import TopAbs_SOLID,TopAbs_SHELL,TopAbs_FACE
 from OCP.IFSelect import IFSelect_RetDone
 from OCP.BRepBndLib import BRepBndLib
 from OCP.Bnd import Bnd_Box
 doc=TDocStd_Document(TCollection_ExtendedString('XmlXCAF'));reader=STEPCAFControl_Reader();reader.SetColorMode(True);reader.SetNameMode(True)
 assert reader.ReadFile(str(path))==IFSelect_RetDone and reader.Transfer(doc),'XCAF STEP transfer failed'
 st=XCAFDoc_DocumentTool.ShapeTool_s(doc.Main());ct=XCAFDoc_DocumentTool.ColorTool_s(doc.Main());root=reader.Reader().OneShape();result=[]
 for kind,avoid,label in [(TopAbs_SOLID,None,'solid'),(TopAbs_SHELL,TopAbs_SOLID,'open-shell'),(TopAbs_FACE,TopAbs_SHELL,'standalone-face')]:
  explorer=TopExp_Explorer(root,kind) if avoid is None else TopExp_Explorer(root,kind,avoid)
  while explorer.More():
   shape=explorer.Current();color=Quantity_Color();found=ct.GetInstanceColor(shape,XCAFDoc_ColorSurf,color)or ct.GetColor(shape,XCAFDoc_ColorSurf,color)or ct.GetColor(shape,XCAFDoc_ColorGen,color);source_label=TDF_Label();name=TDataStd_Name();has_name=st.Search(shape,source_label,True,True,True)and source_label.FindAttribute(TDataStd_Name.GetID_s(),name)
   box=Bnd_Box();BRepBndLib.AddOptimal_s(shape,box,False,False);bounds=box.Get()
   rgb=list(color.Values(Quantity_TOC_sRGB))if found else None
   result.append({'index':len(result),'stepTopology':label,'sourceName':name.Get().ToExtString()if has_name else None,'sourceColorSrgb':rgb,'color':'#'+''.join(f'{round(max(0,min(1,c))*255):02x}'for c in rgb)if rgb else None,'exactBoundsMm':[list(bounds[:3]),list(bounds[3:])]});explorer.Next()
 return result

def load_parts(path):
 """Source STL components or native STEP solids. STEP tessellates in mm at .05mm/.2rad, no scaling."""
 path=Path(path)
 if path.suffix.lower() not in ['.step','.stp']:return trimesh.load(path).split(only_watertight=False)
 from OCP.STEPControl import STEPControl_Reader
 from OCP.IFSelect import IFSelect_RetDone
 from OCP.TopExp import TopExp_Explorer
 from OCP.TopAbs import TopAbs_SOLID,TopAbs_SHELL,TopAbs_FACE,TopAbs_REVERSED
 from OCP.TopoDS import TopoDS
 from OCP.BRepMesh import BRepMesh_IncrementalMesh
 from OCP.BRep import BRep_Tool
 from OCP.TopLoc import TopLoc_Location
 reader=STEPControl_Reader();assert reader.ReadFile(str(path))==IFSelect_RetDone,'STEP read failed';reader.TransferRoots();root=reader.OneShape();shapes=[];parts=[]
 # Keep existing solid indices stable. Open manufacturer surfaces such as USB
 # quilts are appended only when not owned by an already-exported solid/shell.
 for kind,avoid,label in [(TopAbs_SOLID,None,'solid'),(TopAbs_SHELL,TopAbs_SOLID,'open-shell'),(TopAbs_FACE,TopAbs_SHELL,'standalone-face')]:
  explorer=TopExp_Explorer(root,kind) if avoid is None else TopExp_Explorer(root,kind,avoid)
  while explorer.More():shapes.append((explorer.Current(),label));explorer.Next()
 for solid,label in shapes:
  BRepMesh_IncrementalMesh(solid,.05,False,.2,False).Perform();vertices=[];faces=[];items=TopExp_Explorer(solid,TopAbs_FACE)
  if label=='standalone-face':face_shapes=[solid]
  else:
   face_shapes=[]
   while items.More():face_shapes.append(items.Current());items.Next()
  for face_shape in face_shapes:
   face=TopoDS.Face_s(face_shape);location=TopLoc_Location();tri=BRep_Tool.Triangulation_s(face,location)
   if tri:
    start=len(vertices)
    for i in range(1,tri.NbNodes()+1):
     point=tri.Node(i).Transformed(location.Transformation());vertices.append([point.X(),point.Y(),point.Z()])
    for i in range(1,tri.NbTriangles()+1):
     face_indices=[x+start-1 for x in tri.Triangle(i).Get()]
     if face.Orientation()==TopAbs_REVERSED:face_indices.reverse()
     faces.append(face_indices)
  mesh=trimesh.Trimesh(vertices=vertices,faces=faces,process=True);keep=mesh.nondegenerate_faces(height=1e-10);removed=int((~keep).sum());mesh.update_faces(keep);mesh.remove_unreferenced_vertices();mesh.metadata.update(degenerateTessellationFacesRemoved=removed,stepTopology=label);parts.append(mesh)
 assert parts,'STEP contains no tessellatable solids';return parts

def prepare(product):
 id='m5stack-'+product['directory'].split('_')[0].lower();folder=TEMP/id;folder.mkdir(parents=True,exist_ok=True)
 cached=next(x for x in json.loads(Path('output/source/m5-inventory-docs.json').read_text()) if x['directory']==product['directory']);html=Path('output/source/m5-inventory-docs')/(cached['temporaryKey']+'.html');raw=html.read_bytes();assert sha(raw)==cached['sha256'];article=Article();article.feed(raw.decode());shutil.copyfile(html,folder/'official-docs.html')
 doc={'id':'official-docs','title':cached['title'],'url':cached['url'],'resolvedUrl':cached['url'],'kind':'official-product-document','status':'fetched','fetchedAt':cached['fetchedAt'].split('.')[0]+'Z','sha256':sha(raw),'bytes':len(raw),'mediaType':'text/html','archiveName':'official-docs.html','notes':'Reused actual corpus receipt '+cached['fetchedAt']+'; no new fetch timestamp invented.'}
 sources=[doc];jobs=[];seen=set()
 def add(url,title,kind,fetch=False):
  if not url.startswith('https://') or url in seen:return
  seen.add(url);sid=kind+'-'+sha(url.encode())[:10];ext=Path(urlparse(url).path).suffix.lower() or '.bin';source={'id':sid,'title':title or kind,'url':url,'kind':kind,'status':'linked','discoveredFrom':'official-docs'}
  if fetch:source['archiveName']=sid+ext;jobs.append((folder,source))
  else:sources.append(source)
 for file in product['files']:
  if file['extension'] in ['stl','obj','step','stp']:add('https://raw.githubusercontent.com/m5stack/M5_Hardware/'+REV+'/'+quote(file['path']),file['path'],'manufacturer-cad',True)
 gallery_heading=next((x['heading']for x in article.images if x['title']=='Preview'),cached['title']);gallery=[x for x in article.images if x['title']=='Preview' and x['heading']==gallery_heading]
 for x in gallery[:18]:add(x['url'],cached['title']+' official photo','product-photo',True)
 for x in article.images:
  if x['heading']=='Model Size':add(x['url'],cached['title']+' dimension drawing','dimension-drawing',True)
 for x in article.links:
  if x['url'].startswith('https://github.com/m5stack/M5_Hardware/blob/') and Path(urlparse(x['url']).path).suffix.lower()=='.kicad_mod':
   tail=x['url'].split('/blob/',1)[1].split('/',1)[1];add('https://raw.githubusercontent.com/m5stack/M5_Hardware/'+REV+'/'+quote(tail),x['title'],'pcb-source',True)
  if x['heading']=='Structure' and x['url'].startswith('https://github.com/m5stack/M5_Hardware/blob/') and Path(urlparse(x['url']).path).suffix.lower() in ['.stl','.step','.stp']:
   tail=x['url'].split('/blob/',1)[1].split('/',1)[1];add('https://raw.githubusercontent.com/m5stack/M5_Hardware/'+REV+'/'+quote(tail),x['title'],'manufacturer-cad',True)
  if x['heading'] in ['Schematics','Datasheets','Model Size','Structure','ESP-IDF','Arduino'] or 'firmware' in x['title'].lower():
   kind='schematic' if x['heading']=='Schematics' else 'datasheet' if x['heading']=='Datasheets' else 'dimension-drawing' if x['heading']=='Model Size' else 'software-source' if x['heading'] in ['Arduino','ESP-IDF'] else 'manufacturer-source'
   add(x['url'],x['title'],kind,x['url'].lower().endswith('.pdf') and kind in ['schematic','datasheet','dimension-drawing'])
 plan_path=Path('web/platform/assets')/id/'model-plan.json'
 if plan_path.exists():
  for extra in json.loads(plan_path.read_text()).get('extraSources',[]):
   if extra['url'] not in seen:seen.add(extra['url']);jobs.append((folder,extra))
 sources+=list(ThreadPoolExecutor(8).map(download,jobs))
 for container in list(sources):
  if container.get('status')=='fetched' and container.get('archiveName','').endswith('.kicad_mod'):sources+=extract_kicad_steps(container,folder)
 data={'id':id,'queue':product,'sources':sources,'tables':article.tables,'links':article.links,'images':article.images};write(folder/'prepared.json',data)
 cad=[]
 for s in sources:
  if s['kind']=='manufacturer-cad' and s['status']=='fetched' and Path(s['archiveName']).suffix.lower() in ['.stl','.step','.stp']:
   parts=load_parts(folder/s['archiveName'])
   cad.append({'sourceId':s['id'],'components':[{'index':i,'triangles':len(p.faces),'boundsMm':p.bounds.tolist(),'extentsMm':p.extents.tolist(),'volumeMm3':float(p.volume) if np.isfinite(p.volume) else None,'stepTopology':p.metadata.get('stepTopology')} for i,p in enumerate(parts)]})
 write(folder/'components.json',cad)
 thumbs=[]
 for i,s in enumerate(s for s in sources if s['kind']=='product-photo' and s['status']=='fetched'):
  im=Image.open(folder/s['archiveName']).convert('RGB');im.thumbnail((240,240));tile=Image.new('RGB',(250,275),'white');tile.paste(im,((250-im.width)//2,25));ImageDraw.Draw(tile).text((5,5),str(i)+' '+Path(urlparse(s['url']).path).name[:30],fill='black');thumbs.append(tile)
 if thumbs:
  sheet=Image.new('RGB',(250*4,275*((len(thumbs)+3)//4)),'#eeeeee')
  for i,im in enumerate(thumbs):sheet.paste(im,((i%4)*250,(i//4)*275))
  sheet.save(folder/'contact-sheet.jpg')
 print(json.dumps({'id':id,'priority':product['priority'],'sources':len(sources),'fetched':sum(s['status']=='fetched' for s in sources),'cad':cad},ensure_ascii=False))

def build_model(prepared,plan,output,*,evidence_root=None,license_path=None):
 """Stable reusable API. prepared is prepare JSON; plan uses parts/sourceId/matrixRows/targets. output Path.
 Only selected manufacturer triangles are exported. Plans are reviewed evidence, never auto-fit geometry.
 """
 output=Path(output);output.mkdir(parents=True,exist_ok=True);folder=TEMP/prepared['id'];evidence_root=Path(evidence_root) if evidence_root is not None else Path('docs/platform/evidence');license_path=Path(license_path) if license_path is not None else Path('web/platform/assets/m5stack-gps-u032-v11/LICENSE-M5Stack.txt')
 if not any(s['id']=='cad-license' for s in prepared['sources']):
  license_source=next(s for s in json.loads(Path('web/platform/knowledge/m5stack-gps-u032-v11.json').read_text())['sources'] if s['id']=='cad-license');prepared['sources'].append(license_source);shutil.copyfile(license_path,folder/license_source['archiveName'])
 sources={s['id']:s for s in prepared['sources']};source=sources[plan['sourceId']];raw=(folder/source['archiveName']).read_bytes();assert sha(raw)==source['sha256'];parts=load_parts(folder/source['archiveName']);part_sources={source['id']:parts}
 matrix=np.array(plan['matrixRows'],dtype=float);r=matrix[:3,:3];assert np.allclose(r.T@r,np.eye(3),atol=1e-7) and np.isclose(np.linalg.det(r),1), 'Only rigid right-handed transforms permitted'
 records=[];buffers=[];offset=0;allv=[];checks=[]
 for item in plan['parts']:
  item_source=sources[item.get('sourceId',source['id'])]
  if item_source['id'] not in part_sources:
   item_path=folder/item_source['archiveName'];assert sha(item_path.read_bytes())==item_source['sha256'];part_sources[item_source['id']]=load_parts(item_path)
  part=part_sources[item_source['id']][item['sourceComponent']];local=np.array(item.get('matrixRows',np.eye(4)),dtype=float);lr=local[:3,:3];assert np.allclose(lr.T@lr,np.eye(3),atol=1e-7) and np.isclose(np.linalg.det(lr),1), 'Component transforms must be rigid';effective=matrix@local;pr=effective[:3,:3];v=part.vertices@pr.T+effective[:3,3];n=part.face_normals@pr.T;corners=[]
  face_indices=np.arange(len(part.faces))
  if 'sourceFaceRanges' in item:
   ranges=item['sourceFaceRanges'];assert ranges and all(len(pair)==2 and all(isinstance(q,int)for q in pair)and 0<=pair[0]<pair[1]<=len(part.faces)for pair in ranges),'Invalid source face range'
   face_indices=np.concatenate([np.arange(a,b)for a,b in ranges]);assert len(np.unique(face_indices))==len(face_indices),'Duplicate source face selection'
  for fi in face_indices:
   face=part.faces[fi]
   for vi in face:
    adjacent=part.vertex_faces[vi];adjacent=adjacent[adjacent>=0];adjacent=adjacent[(n[adjacent]@n[fi])>np.cos(np.deg2rad(38))];normal=(n[adjacent]*part.area_faces[adjacent,None]).sum(0);length=np.linalg.norm(normal);normal=normal/length if length>1e-12 else n[fi];assert np.isfinite(normal).all();corners.append([*v[vi],*normal])
  data=np.asarray(corners,dtype='<f4');expected=v[part.faces[face_indices]].reshape(-1,3);error=float(np.abs(data[:,:3]-expected).max());checks.append({'name':item['name']+' source-fidelity','errorMm':error,'toleranceMm':.05,'pass':error<=.05,'sourceId':item_source['id']});binary=data.tobytes();buffers.append(binary);allv.append(data[:,:3]);records.append({**item,'byteOffset':offset,'vertexCount':len(corners),'boundsMm':[expected.min(0).tolist(),expected.max(0).tolist()]});offset+=len(binary)
 for surface in plan.get('surfaces',[]):
  w,h=surface['sizeMm'];radius=surface['radiusMm'];cx,cy,z=surface['positionMm'];points=[]
  for x,y,start in [(w/2-radius,h/2-radius,0),(-w/2+radius,h/2-radius,90),(-w/2+radius,-h/2+radius,180),(w/2-radius,-h/2+radius,270)]:
   for angle in np.linspace(start,start+90,13):points.append([cx+x+radius*np.cos(np.deg2rad(angle)),cy+y+radius*np.sin(np.deg2rad(angle)),z])
  if surface.get('outlineMm'):
   points=[[x,y,z]for x,y in surface['outlineMm']]
   signed=sum(points[i][0]*points[(i+1)%len(points)][1]-points[(i+1)%len(points)][0]*points[i][1]for i in range(len(points)))
   if signed<0:points.reverse()
  corners=[];normal=[0,0,-1 if surface.get('side')=='back' else 1]
  for i in range(len(points)):
   triangle=[[cx,cy,z],points[i],points[(i+1)%len(points)]]
   if normal[2]<0:triangle.reverse()
   corners.extend([[*v,*normal] for v in triangle])
  data=np.array(corners,dtype='<f4');binary=data.tobytes();buffers.append(binary);allv.append(data[:,:3]);records.append({'name':surface['name'],'sourceComponent':None,'role':surface.get('role','glass'),**{key:surface[key] for key in ['texture','controlId','encoderId'] if surface.get(key)},'color':surface.get('color','#111719'),'byteOffset':offset,'vertexCount':len(data),'boundsMm':[data[:,:3].min(0).tolist(),data[:,:3].max(0).tolist()],'basis':'image-measurement','sourceIds':surface['sourceIds'],'notes':surface['notes']});offset+=len(binary)
 (output/'geometry.bin').write_bytes(b''.join(buffers));source_file='source'+Path(source['archiveName']).suffix.lower();shutil.copyfile(folder/source['archiveName'],output/source_file);
 for sid in part_sources:
  if sid!=source['id']:shutil.copyfile(folder/sources[sid]['archiveName'],output/('source-'+sources[sid]['archiveName']))
 shutil.copyfile(license_path,output/'LICENSE-M5Stack.txt')
 joined=np.concatenate(allv);extent=joined.max(0)-joined.min(0)
 for target in plan.get('targets',[]):
  measured=float(extent[target['axis']]) if 'parts' not in target else float(np.max([records[i]['boundsMm'][1][target['axis']] for i in target['parts']])-np.min([records[i]['boundsMm'][0][target['axis']] for i in target['parts']]))
  error=abs(measured-target['valueMm']);checks.append({**target,'measuredMm':measured,'errorMm':error,'pass':error<=target['toleranceMm']})
 photo_records=[]
 for photo in plan.get('photos',[]):
  s=sources[photo['sourceId']];assert s['status']=='fetched';assert sha((folder/s['archiveName']).read_bytes())==s['sha256'];filename='reference-'+s['archiveName'];shutil.copyfile(folder/s['archiveName'],output/filename)
  if photo.get('cropXYXY'):
   original=filename;filename='reference-crop-'+photo['view']+'.webp';Image.open(folder/s['archiveName']).convert('RGB').crop(photo['cropXYXY']).save(output/filename,lossless=True);photo={**photo,'originalFile':original,'cropNotes':'Fixed documented ROI excludes unrelated cable/coin/packaging; original source is retained.'}
  photo_records.append({**photo,'file':filename,'sourceSha256':s['sha256']})
 texture_records=[]
 for texture in plan.get('textureSources',[]):
  s=sources[texture['sourceId']];assert s['status']=='fetched';im=Image.open(folder/s['archiveName']).convert('RGB');assert sha((folder/s['archiveName']).read_bytes())==s['sha256']
  draw=ImageDraw.Draw(im)
  for blank in texture.get('blankSourcePixels',[]):draw.rectangle(blank['xyxy'],fill=blank.get('color','#111719'))
  if texture.get('alphaMask'):
   mask_spec=texture['alphaMask'];factor=4;alpha=Image.new('L',(im.width*factor,im.height*factor),0);painter=ImageDraw.Draw(alpha)
   for poly in mask_spec.get('includePolygons',[]):painter.polygon([(round(x*factor),round(y*factor))for x,y in poly],fill=255)
   for rect in mask_spec.get('includeRoundedRects',[]):painter.rounded_rectangle([round(v*factor)for v in rect['xyxy']],radius=rect.get('radiusPx',0)*factor,fill=255)
   for poly in mask_spec.get('excludePolygons',[]):painter.polygon([(round(x*factor),round(y*factor))for x,y in poly],fill=0)
   for rect in mask_spec.get('excludeRects',[]):painter.rectangle([round(v*factor)for v in rect],fill=0)
   for ellipse in mask_spec.get('excludeEllipses',[]):painter.ellipse([round(v*factor)for v in ellipse],fill=0)
   im=im.convert('RGBA');im.putalpha(alpha.resize(im.size,Image.Resampling.LANCZOS))
  if texture.get('perspectiveQuad'):
   import cv2
   quad=np.asarray(texture['perspectiveQuad'],dtype=np.float32);width,height=texture['rectifiedPixels'];assert quad.shape==(4,2) and np.isfinite(quad).all() and 1<=width<=4096 and 1<=height<=4096
   transform=cv2.getPerspectiveTransform(quad,np.asarray([[0,0],[width-1,0],[width-1,height-1],[0,height-1]],dtype=np.float32));im=Image.fromarray(cv2.warpPerspective(np.asarray(im.convert('RGBA')),transform,(width,height),flags=cv2.INTER_LANCZOS4,borderMode=cv2.BORDER_CONSTANT,borderValue=(0,0,0,0)))
   method='Four explicit original-image film corners rectified by OpenCV getPerspectiveTransform/warpPerspective (Lanczos4); original-coordinate alpha ROI and hardware exclusions applied before warp. Original pixels only; projection uncertainty recorded, not independent geometry verification.'
  else:
   im=im.crop(texture['cropXYXY']);method='Fixed axis-aligned crop; original-coordinate explicit alpha ROI at4x supersampling when alphaMask supplied; exclusions remove fasteners/holes/shell. No generated pixels or perspective warp.'
  im.save(output/texture['file'],lossless=True);texture_records.append({**texture,'sourceSha256':s['sha256'],'outputPixels':list(im.size),'method':method})
 meta={'schemaVersion':'1','id':prepared['id'],'unit':'mm','binary':'geometry.bin','strideFloats':6,'parts':records,'sourceTransform':{'matrixRows':plan['matrixRows'],'scale':1,'note':plan['assemblyNotes']},'omittedComponents':plan.get('omittedComponents',[]),'dimensions':plan.get('dimensions',{}),'presentation':plan.get('presentation',{}),'derivedParts':plan.get('derivedParts',[]),'derivedSurfaces':plan.get('surfaces',[]),'referencePhotos':photo_records,'textures':texture_records,'reviewViews':plan.get('reviewViews',{}),'provenance':{'repository':'https://github.com/m5stack/M5_Hardware','commit':REV,'sourceUrl':source['url'],'sourceSha256':source['sha256'],'tessellation':{'step':'OpenCascade 7.8.1.1; absolute deflection0.05mm, angle0.2rad, native solids plus orphan shells/faces; units converted by STEP reader to millimetres','stl':'Original triangles'},'sourceLicense':'MIT; Copyright (c) 2021 M5Stack; see LICENSE-M5Stack.txt','photoRights':'Official product photographs copyright M5Stack / named manufacturer. No separate open photo license verified; retained for product reference, not relicensed under project MIT.','sources':prepared['sources']},'limitations':plan['limitations'],'artifacts':{f.name:sha(f.read_bytes()) for f in output.iterdir() if f.name in ['source.stl','source.step','source.stp','geometry.bin','LICENSE-M5Stack.txt'] or f.name.startswith('source-') or f.name.startswith('reference-') or f.name in [t['file'] for t in texture_records]}}
 write(output/'model.json',meta);report={'schemaVersion':'1','checkedAt':now(),'productId':prepared['id'],'modelSha256':sha((output/'model.json').read_bytes()),'geometrySha256':sha((output/'geometry.bin').read_bytes()),'sourceSha256':source['sha256'],'dimensions':checks,'photos':[{'sourceId':p['sourceId'],'view':p['view'],'status':'pending-browser-comparison'} for p in photo_records],'status':'partial','limitations':plan['limitations']};write(evidence_root/prepared['id']/'verification.json',report);write(evidence_root/prepared['id']/'source-audit.json',{'sources':prepared['sources'],'components':json.loads((folder/'components.json').read_text()),'plan':plan});return meta,report


def build_knowledge(prepared,plan):
 id=prepared['id'];facts=[{'id':'soc','label':'Primary controller','value':prepared['queue']['primaryChip'],'basis':'official','sourceIds':['official-docs'],'group':'compute'}]
 for i,table in enumerate(prepared['tables']):
  if table['heading'] in ['Specifications','Specification']:
   for j,row in enumerate(table['rows'][1:]):
    if len(row)>=2:facts.append({'id':f'spec-{i}-{j}','label':row[0],'value':' | '.join(row[1:]),'basis':'official','sourceIds':['official-docs'],'group':'specification'})
  elif table['heading'] not in ['Product Comparison','Version Change','Easyloader']:
   facts.append({'id':f'table-{i}','label':table['heading'],'value':{'rows':table['rows']},'basis':'official','sourceIds':['official-docs'],'group':'pinmap'})
 facts+=plan.get('facts',[])
 pcb_note=('Manufacturer populated mechanical PCB CAD is retained, including sourced component shapes. Copper layers, electrical connectivity, exact BOM and manufacturing tolerances are not verified by mechanical CAD.' if plan.get('pcbModel')=='manufacturer-populated-cad' else 'No complete populated PCB layout or Gerber verified in the audited files. A manufacturer mechanical board outline, when present, does not establish every component or copper trace. Unknown internals remain a neutral UNKNOWN placeholder.')
 record={'schemaVersion':'1','id':id,'kind':'product','catalogId':id,'title':plan['title'],'summary':plan['summary'],'summarySourceIds':['official-docs'],'updatedAt':now(),'hardwareRevision':plan.get('hardwareRevision','unspecified'),'identity':{'manufacturer':'M5Stack','sku':prepared['queue']['skuAsDocumented'],'directory':prepared['queue']['directory']},'facts':facts,'revisions':plan.get('revisions',[]),'sources':prepared['sources'],'assets':[{'id':s['id'],'sourceId':s['id'],'role':s['kind'],'url':s['url'],**({'localUrl':f"./assets/{id}/source{Path(s['archiveName']).suffix.lower()}"} if s['id']==plan['sourceId'] else {})} for s in prepared['sources'] if s['kind'] in ['manufacturer-cad','product-photo','dimension-drawing','schematic','datasheet']],'unknowns':plan.get('unknowns',[])+[{'id':'user-hardware-revision','label':'User hardware revision','value':None,'basis':'unknown','sourceIds':['official-docs'],'notes':'Catalog identity does not infer the revision owned by the user.'},{'id':'pcb-layout','label':'PCB physical layout','value':None,'basis':'unknown','sourceIds':[plan['sourceId']],'notes':pcb_note}],'collectionNotes':['Specification and pin-map rows preserve factual parameters from the exact documented product, not sibling SKUs.','Raw full official page/PDF downloads stay temporary; images and manufacturer CAD retain distinct rights.','Model import does not establish CPU/board/peripheral firmware support.']}
 write(Path('web/platform/knowledge')/(id+'.json'),record);return record

def verify_product(id):
 import cv2
 assets=Path('web/platform/assets')/id;out=Path('docs/platform/evidence')/id;out.mkdir(parents=True,exist_ok=True);meta=json.loads((assets/'model.json').read_text());report=json.loads((out/'verification.json').read_text());assert report['modelSha256']==sha((assets/'model.json').read_bytes())
 def mask(im):
  m=(np.array(im.convert('RGB')).min(2)<245).astype('uint8');cs,_=cv2.findContours(m,cv2.RETR_EXTERNAL,cv2.CHAIN_APPROX_SIMPLE);out=np.zeros_like(m)
  if cs:cv2.drawContours(out,[max(cs,key=cv2.contourArea)],-1,1,-1)
  return out.astype(bool)
 results=[]
 for photo in meta['referencePhotos']:
  renderpath=Path('output/playwright')/(id+'-'+photo['view']+'.png')
  if not renderpath.exists():results.append({'view':photo['view'],'sourceId':photo['sourceId'],'status':'unmeasured'});continue
  ref=Image.open(assets/photo['file']).convert('RGB').resize((800,800),Image.Resampling.LANCZOS);render=Image.open(renderpath).convert('RGB');assert render.size==(800,800);maskpath=renderpath.with_name(renderpath.stem+'-mask.png');maskimage=Image.open(maskpath).convert('RGB') if maskpath.exists() else render;r,m=mask(ref),mask(maskimage);union=int((r|m).sum());iou=float((r&m).sum()/union) if union else 0.;quantified=photo['view'] in ['front','back'] and photo.get('silhouetteComparable',True)
  result={'view':photo['view'],'sourceId':photo['sourceId'],'sourceSha256':photo['sourceSha256'],'renderSha256':sha(renderpath.read_bytes()),'renderMaskSha256':sha(maskpath.read_bytes()) if maskpath.exists() else None,'silhouetteIoU':iou,'status':('pass' if iou>=.95 else 'fail') if quantified else 'qualitative-only','minimum':.95 if quantified else None,'camera':meta.get('reviewViews',{}).get(photo['view'],{}),'method':('Render mask uses unchanged geometry and per-material black while retaining texture alpha/alpha-test; transparent decal canvas never becomes an opaque silhouette. ' if maskpath.exists() else '')+'800x800; min RGB<245; largest external filled contour. Image-derived scale/center and rigid camera only; labels, controls, internal surfaces are not certified by silhouette.'};results.append(result)
  diff=np.full((800,800,3),255,dtype=np.uint8);diff[r&m]=[50,70,60];diff[r&~m]=[255,85,85];diff[m&~r]=[60,145,255];panel=Image.new('RGB',(1600,440),'white');ImageDraw.Draw(panel).text((10,12),id+' '+photo['view']+' | official / model / 50% overlay / silhouette difference | IoU '+f'{iou:.4f}'+' | '+result['status'],fill='black')
  for j,img in enumerate([ref,render,Image.blend(ref,render,.5),Image.fromarray(diff)]):panel.paste(img.resize((400,400)),(j*400,40))
  panel.save(out/(photo['view']+'-comparison.webp'),quality=90)
 report.update(checkedAt=now(),photos=results,reviewScriptSha256=sha(Path('web/platform/review-lane-01.js').read_bytes()),buildScriptSha256=sha(Path(__file__).read_bytes()),genericRendererSha256=sha(Path('web/platform/manufacturer-model.js').read_bytes()),cameraNotes='Camera scale/center calibrated to original official silhouette and independent body dimensions; no geometry rescaling.',status='partial');write(out/'verification.json',report)
 print(json.dumps({'id':id,'dimensionPass':all(x['pass'] for x in report['dimensions']),'photos':[(x['view'],x.get('silhouetteIoU'),x['status']) for x in results]}));return report

def publish_product(id,submission_path=None,priorities=None):
 assets=Path('web/platform/assets')/id;plan=json.loads((assets/'model-plan.json').read_text());knowledge=json.loads((Path('web/platform/knowledge')/(id+'.json')).read_text());prepared=json.loads((TEMP/id/'prepared.json').read_text());cfg=plan['catalog'];doc=next(s for s in knowledge['sources'] if s['id']=='official-docs');report=json.loads((Path('docs/platform/evidence')/id/'verification.json').read_text())
 assert report['modelSha256']==sha((assets/'model.json').read_bytes())
 runtime=cfg.get('runtime',{'board':None,'flashMb':cfg.get('flashMb'),'psramMb':cfg.get('psramMb'),'status':'unavailable'})
 runtime_note='已提供部分板级模型；仅几何预览不能证明固件兼容，实际范围见产品运行时记录。' if runtime.get('board') else '本产品尚无板级模拟器，模型和资料不能证明固件兼容。'
 firmware_note='官方固件可尝试运行，未逐一验证。' if runtime.get('board') else '官方固件入口仅参考，未验证版本/hash/镜像不填猜测值。'
 entry={'id':id,'name':plan['title'],'manufacturer':'M5Stack','sku':prepared['queue']['skuAsDocumented'],'hardwareRevision':plan.get('hardwareRevision','unspecified'),'chip':cfg['chip'],'module':prepared['queue']['primaryChip'],'summary':plan['summary'],'knowledgeUrl':f'./knowledge/{id}.json','runtime':runtime,'display':cfg.get('display'),'controls':cfg.get('controls',[]),'interfaces':cfg.get('interfaces',[]),'model':{'type':'manufacturer-cad','url':f'./assets/{id}/model.json','status':'partial'},'dimensions':{**plan['dimensions'],'basis':'official'},'limitations':[runtime_note,'尺寸和照片的通过、未通过、未测量分别保留；内部 UNKNOWN 不表示真实 PCB。']+plan['limitations'],'sources':[{**doc,'checkedAt':doc['fetchedAt'][:10]}]}
 if cfg.get('inputDevices'):entry['inputDevices']=cfg['inputDevices']
 ref=None
 if cfg.get('publication'):
  pub=cfg['publication'];ref={'id':id+'-official-reference','name':pub['title'],'role':pub.get('role','factory'),'publisher':'M5Stack','productIds':[id],'status':'reference','source':None,'publicationUrl':pub['url'],'version':None,'compatibility':'unavailable','limitations':['Official source/reference only. No pinned executable image, layout, checksum, browser CORS or board boot verified.'],'sources':[{'title':pub['title'],'url':pub['url'],'checkedAt':doc['fetchedAt'][:10],'status':'linked'}]};entry['firmwareReferenceIds']=[ref['id']]
  if ref['role']=='factory':entry['defaultFirmwareId']=ref['id']
 out=Path(submission_path) if submission_path else RUN/'submissions/lane-01.json';submission=json.loads(out.read_text()) if out.exists() else {'schemaVersion':'1','lane':'01','orderBasis':'queue priority (latest Products path change, not product launch date)','products':[],'knowledgeEntries':[],'firmwareReferences':[],'completedIds':[],'pendingIds':[]};submission['updatedAt']=now()
 for key,value in [('products',entry),('knowledgeEntries',{'id':id,'kind':'product','catalogId':id,'name':plan['title'],'url':'./'+id+'.json','updatedAt':knowledge['updatedAt']})]:submission[key]=[x for x in submission[key]if x['id']!=id]+[value]
 submission['firmwareReferences']=[x for x in submission['firmwareReferences']if id not in x['productIds']]+([ref]if ref else[]);submission['completedIds']=list(dict.fromkeys(submission['completedIds']+[id]));queue=[x for x in json.loads((RUN/'queue.json').read_text())['products']if x['priority']in (priorities or PRIORITIES)];order={'m5stack-'+x['directory'].split('_')[0].lower():x['priority']for x in queue}
 for key in ['products','knowledgeEntries']:submission[key].sort(key=lambda x:order[x['id']])
 submission['completedIds'].sort(key=order.get);submission['pendingIds']=[k for k in order if k not in submission['completedIds']];write(out,submission)
 dimrows='\n'.join('| '+c['name']+' | '+str(c.get('measuredMm',c.get('errorMm')))+' | '+str(c.get('valueMm',0))+' | '+('通过'if c['pass']else'未通过')+' |'for c in report['dimensions']);photolist='\n'.join('- '+x['view']+': '+x['status']+'; IoU '+str(x.get('silhouetteIoU','未测量')) for x in report['photos'])
 text=f'''# {plan['title']} 外观与知识入库

{plan['summary']}

来源：[官方产品文档]({doc['url']})，M5_Hardware 固定 `{REV}`。逐源 URL、UTC、SHA-256、下载状态见 [knowledge](../../../web/platform/knowledge/{id}.json) 与 [source audit](../evidence/{id}/source-audit.json)。旧抓取保留原始时间，不伪造刷新日期。

{plan['assemblyNotes']}

[model-plan.json](../../../web/platform/assets/{id}/model-plan.json) 记录组件选择、刚体变换、照片裁切、纹理投影与估算表面；厂家几何不缩放。STL 保留源三角形；STEP 使用 OpenCascade7.8.1.1 以0.05mm弦偏差/0.2rad角度细分。UNKNOWN 是中性占位，未编造 PCB 走线或未核实电子元件。

| 检查 | 实测或误差 mm | 目标 mm | 结果 |
| --- | ---: | ---: | --- |
{dimrows}

实际平台 `manufacturer-model.js` 的 headless 渲染与原照片对照在 [verification.json](../evidence/{id}/verification.json)：

{photolist}

正反视图只有照片确属近正投影且原始型号一致时才用0.95轮廓门槛。侧视/斜视保留为定性检查，不以轮廓掩盖标签、组件或装配差异。引用照片的纹理不是独立验证依据；未通过/缺失项不会改写成通过。

厂家 CAD 保留 MIT；官方照片及其裁切纹理版权归厂家，未验证独立开放图片许可，不套用项目 MIT。屏幕照片中的固件像素不作为模拟输出。runtime 为 {runtime['status']}。{runtime_note} {firmware_note}

重生成：

```sh
python tools/onboard-lane-01.py prepare --ids {id}
python tools/onboard-lane-01.py build --ids {id}
```

通过受跟踪的 headless 浏览器打开 `web/platform/review-lane-01.html?id={id}`；按 model reviewViews 保存800×800正反侧画布到 `output/playwright/{id}-front.png` 等，再运行：

```sh
python tools/onboard-lane-01.py verify --ids {id}
python tools/onboard-lane-01.py publish --ids {id}
```

当前限制：

'''+ '\n'.join('- '+x for x in plan['limitations'])+'\n';path=Path('docs/platform/models')/(id+'.md');path.parent.mkdir(parents=True,exist_ok=True);path.write_text(text);print(id+' published to lane submission')


def main():
 p=argparse.ArgumentParser();p.add_argument('command',choices=['prepare','build','verify','publish']);p.add_argument('--count',type=int,default=3);p.add_argument('--ids',nargs='*');a=p.parse_args();products=[x for x in json.loads((RUN/'queue.json').read_text())['products'] if x['priority'] in PRIORITIES]
 if a.ids:products=[x for x in products if 'm5stack-'+x['directory'].split('_')[0].lower() in a.ids]
 else:products=products[:a.count]
 for product in products:
  if a.command=='prepare':prepare(product)
  elif a.command=='verify':verify_product('m5stack-'+product['directory'].split('_')[0].lower())
  elif a.command=='publish':publish_product('m5stack-'+product['directory'].split('_')[0].lower())
  else:
   id='m5stack-'+product['directory'].split('_')[0].lower();prepared=json.loads((TEMP/id/'prepared.json').read_text());output=Path('web/platform/assets')/id;plan=json.loads((output/'model-plan.json').read_text());build_model(prepared,plan,output);build_knowledge(prepared,plan);print(id+' built')
if __name__=='__main__':main()
