#!/usr/bin/env python3
"""Rebuild the four LILYGO CAD previews from retained, hash-pinned manufacturer files.
Usage: python tools/build-lilygo-models.py [product-id ...] --output /new/directory
Dependencies: tools/requirements-model.txt. No network access or firmware downloads.
"""
import argparse,hashlib,json,sys
from pathlib import Path
import numpy as np
from PIL import Image,ImageDraw,ImageFont
import trimesh
ROOT=Path(__file__).resolve().parents[1]
def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def write(path,value):path.parent.mkdir(parents=True,exist_ok=True);path.write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n')
def load_step(path):
 from OCP.STEPControl import STEPControl_Reader
 from OCP.IFSelect import IFSelect_RetDone
 from OCP.TopExp import TopExp_Explorer
 from OCP.TopAbs import TopAbs_SOLID,TopAbs_SHELL,TopAbs_FACE,TopAbs_REVERSED
 from OCP.TopoDS import TopoDS
 from OCP.BRepMesh import BRepMesh_IncrementalMesh
 from OCP.BRep import BRep_Tool
 from OCP.TopLoc import TopLoc_Location
 reader=STEPControl_Reader();assert reader.ReadFile(str(path))==IFSelect_RetDone;reader.TransferRoots();root=reader.OneShape();shapes=[];parts=[]
 for kind,avoid in [(TopAbs_SOLID,None),(TopAbs_SHELL,TopAbs_SOLID),(TopAbs_FACE,TopAbs_SHELL)]:
  exp=TopExp_Explorer(root,kind)if avoid is None else TopExp_Explorer(root,kind,avoid)
  while exp.More():shapes.append((exp.Current(),kind));exp.Next()
 for shape,kind in shapes:
  BRepMesh_IncrementalMesh(shape,.05,False,.2,False).Perform();vertices=[];faces=[];exp=TopExp_Explorer(shape,TopAbs_FACE);face_shapes=[]
  if kind==TopAbs_FACE:face_shapes=[shape]
  else:
   while exp.More():face_shapes.append(exp.Current());exp.Next()
  for item in face_shapes:
   face=TopoDS.Face_s(item);loc=TopLoc_Location();tri=BRep_Tool.Triangulation_s(face,loc)
   if not tri:continue
   offset=len(vertices)
   for i in range(1,tri.NbNodes()+1):
    p=tri.Node(i).Transformed(loc.Transformation());vertices.append([p.X(),p.Y(),p.Z()])
   for i in range(1,tri.NbTriangles()+1):
    f=[v+offset-1 for v in tri.Triangle(i).Get()]
    if face.Orientation()==TopAbs_REVERSED:f.reverse()
    faces.append(f)
  if not faces:parts.append(None);continue
  mesh=trimesh.Trimesh(vertices=vertices,faces=faces,process=True);mesh.update_faces(mesh.nondegenerate_faces(height=1e-10));mesh.remove_unreferenced_vertices();parts.append(mesh)
 return parts

def build(id,output):
 src=ROOT/'web/platform/assets'/id;plan=json.loads((src/'model-plan.json').read_text());out=output/id;out.mkdir(parents=True,exist_ok=True);assert out.resolve()!=src.resolve(),'Use a separate output directory.'
 cache={};buffers=[];records=[];checks=[];offset=0
 for source in plan['sources']:
  p=src/source['file'];assert p.is_file()and digest(p)==source['sha256'],f'Source hash mismatch: {p.name}'
 for group in plan['parts']:
  file=group['file']
  if file not in cache:cache[file]=load_step(src/file)
  transform=np.array(group.get('matrixRows',plan['matrixRows']),dtype=float);r=transform[:3,:3];assert np.allclose(r.T@r,np.eye(3),atol=1e-7)and np.isclose(np.linalg.det(r),1),'Rigid transforms only'
  arrays=[]
  for i in group['components']:
   mesh=cache[file][i];assert mesh is not None,f'Empty CAD component {i}'
   vertices=mesh.vertices@r.T+transform[:3,3];normals=mesh.face_normals@r.T
   # Keep original triangle connectivity; per-face normals retain sharp CAD edges.
   arrays.append(np.concatenate([vertices[mesh.faces].reshape(-1,3),np.repeat(normals,3,axis=0)],axis=1))
  a=np.concatenate(arrays);f=a.astype('<f4');error=float(np.abs(f[:,:3]-a[:,:3]).max());checks.append({'part':group['name'],'maxExportErrorMm':error,'toleranceMm':.05,'pass':error<=.05})
  data=f.tobytes();buffers.append(data);record={k:v for k,v in group.items()if k not in ['file','components','matrixRows']};record.update(sourceFile=file,sourceComponents=group['components'],byteOffset=offset,vertexCount=len(f),boundsMm=[f[:,:3].min(0).tolist(),f[:,:3].max(0).tolist()]);records.append(record);offset+=len(data)
 binary=b''.join(buffers);(out/'geometry.bin').write_bytes(binary)
 for texture in plan.get('textureSources',[]):
  im=Image.open(src/texture['sourceFile']).convert('RGB');im.crop(texture['cropXYXY']).save(out/texture['file'])
 for label in plan.get('generatedLabels',[]):
  im=Image.new('RGBA',(512,96),(0,0,0,0));draw=ImageDraw.Draw(im);draw.text((256,48),label['text'],fill='white',font=ImageFont.load_default(size=70),anchor='mm');im.save(out/label['file'])
 texture_hashes={t['file']:digest(out/t['file'])for t in plan.get('textureSources',[])+plan.get('generatedLabels',[])}
 meta={'schemaVersion':'1','id':id,'unit':'mm','binary':'geometry.bin','strideFloats':6,'parts':records,'sourceTransform':{'matrixRows':plan['matrixRows'],'scale':1,'note':plan['assemblyNotes']},'dimensions':plan['dimensions'],'presentation':plan['presentation'],'derivedParts':plan.get('derivedParts',[]),'referencePhotos':plan['referencePhotos'],'textures':plan.get('textureSources',[]),'reviewViews':plan.get('reviewViews',{}),'provenance':{'kind':'manufacturer-cad','sources':plan['sources'],'tessellation':'OpenCascade 7.8.1.1; 0.05 mm absolute deflection, 0.2 rad angle; solids plus orphan shells and faces; native millimetres. Empty, untessellatable topology is listed as omitted.','license':plan['license']},'omittedComponents':plan.get('omittedComponents',[]),'limitations':plan['limitations'],'artifacts':{'geometry.bin':digest(out/'geometry.bin'),**texture_hashes,**{s['file']:s['sha256']for s in plan['sources']}}}
 write(out/'model.json',meta)
 bounds=np.array([p['boundsMm']for p in records]);extents=bounds[:,1].max(0)-bounds[:,0].min(0)
 write(out/'build-check.json',{'productId':id,'modelSha256':digest(out/'model.json'),'geometrySha256':digest(out/'geometry.bin'),'exportChecks':checks,'exportedBoundsMm':[bounds[:,0].min(0).tolist(),bounds[:,1].max(0).tolist()],'exportedExtentsMm':extents.tolist(),'independentDimensions':plan.get('dimensionChecks',[]),'status':'partial','limitations':plan['limitations']})
 print(json.dumps({'id':id,'parts':len(records),'bytes':len(binary),'extentsMm':extents.tolist(),'sourceFidelityPass':all(c['pass']for c in checks)}))

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('ids',nargs='*');p.add_argument('--output',type=Path,required=True);args=p.parse_args()
 ids=args.ids or ['lilygo-ttgo-t-display','lilygo-t-display-s3','lilygo-t-deck','lilygo-t-embed-cc1101']
 for id in ids:
  if id not in ['lilygo-ttgo-t-display','lilygo-t-display-s3','lilygo-t-deck','lilygo-t-embed-cc1101']:raise ValueError('Unsupported product ID')
  build(id,args.output)
if __name__=='__main__':main()
