#!/usr/bin/env python3
"""Rebuild LILYGO previews from per-product, source-pinned model plans.

Usage: python tools/build-lilygo-expansion-models.py --output /tmp/lilygo-models [product-id ...]
No network access. Requires numpy; CAD plans additionally use the existing
build-lilygo-models.py exporter and tools/requirements-model.txt dependencies.
Debug layout sizes never become product measurements or runtime support claims.
"""
import argparse
import gzip
import hashlib
import importlib.util
import io
import json
import math
from pathlib import Path
import shutil
import tempfile
import numpy as np

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / 'web/platform/assets'

def digest(data):
    return hashlib.sha256(data).hexdigest()

def write(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')

def rounded_outline(width, height, radius, segments=10):
    radius = min(radius, width / 2, height / 2)
    points = []
    for cx, cy, start in [(width/2-radius,height/2-radius,0),(-width/2+radius,height/2-radius,90),(-width/2+radius,-height/2+radius,180),(width/2-radius,-height/2+radius,270)]:
        for i in range(segments + 1):
            a = math.radians(start + i*90/segments)
            points.append([cx+radius*math.cos(a),cy+radius*math.sin(a)])
    return np.array(points)

def triangles(points):
    vertices = np.asarray(points, dtype=float).reshape(-1,3,3)
    normals = np.cross(vertices[:,1]-vertices[:,0], vertices[:,2]-vertices[:,0])
    length = np.linalg.norm(normals, axis=1)
    keep = length > 1e-10
    vertices, normals, length = vertices[keep], normals[keep], length[keep]
    return np.concatenate([vertices.reshape(-1,3),np.repeat(normals/length[:,None],3,axis=0)],axis=1)

def extrusion(outline, depth, hole=None):
    out = np.array(outline); n = len(out); faces = []
    rings = [out] if hole is None else [out,np.array(hole)]
    for index, ring in enumerate(rings):
        for i in range(n):
            a, b = ring[i], ring[(i+1)%n]
            face = [[*a,-depth/2],[*b,-depth/2],[*b,depth/2],[*a,depth/2]]
            if index: face.reverse()
            faces.extend([face[:3],[face[0],face[2],face[3]]])
    for z,reverse in [(depth/2,False),(-depth/2,True)]:
        for i in range(n):
            j=(i+1)%n
            fs=[[[0,0,z],[*out[i],z],[*out[j],z]]] if hole is None else [[[ *hole[i],z],[*out[i],z],[*out[j],z]],[[*hole[i],z],[*out[j],z],[*hole[j],z]]]
            for face in fs: faces.append(face[::-1] if reverse else face)
    return triangles(faces)

def primitive(spec):
    width,height,depth=spec['sizeMm']; kind=spec.get('kind','box')
    if min(width,height,depth)<=0: raise ValueError('Non-positive geometry size')
    radius=spec.get('radiusMm',min(width,height)*.08 if kind=='rounded-box' else 0)
    if kind=='polygon':
        outline=np.array(spec['outlineMm'],dtype=float)
    elif kind=='cylinder':
        outline=np.array([[width/2*math.cos(a),height/2*math.sin(a)] for a in np.linspace(0,math.tau,64,endpoint=False)])
    elif radius: outline=rounded_outline(width,height,radius)
    else: outline=np.array([[width/2,height/2],[-width/2,height/2],[-width/2,-height/2],[width/2,-height/2]])
    hole=None
    if kind=='ring':
        inner=spec['openingMm'];outline=rounded_outline(width,height,radius,10)
        hole=rounded_outline(inner[0],inner[1],spec.get('innerRadiusMm',max(.01,radius-(width-inner[0])/2)),10)
    values=extrusion(outline,depth,hole)
    if spec.get('rotation'):
        x,y,z=spec['rotation'];cx,sx,cy,sy,cz,sz=math.cos(x),math.sin(x),math.cos(y),math.sin(y),math.cos(z),math.sin(z)
        r=np.array([[cz,-sz,0],[sz,cz,0],[0,0,1]])@np.array([[cy,0,sy],[0,1,0],[-sy,0,cy]])@np.array([[1,0,0],[0,cx,-sx],[0,sx,cx]])
        values[:,:3]=values[:,:3]@r.T;values[:,3:]=values[:,3:]@r.T
    values[:,:3]+=np.array(spec.get('centerMm',[0,0,0]))
    return values

def cad_values(folder, specification):
    source=folder/specification['file']; archived=source.read_bytes()
    if digest(archived)!=specification['archiveSha256']: raise ValueError('CAD archive hash mismatch: '+source.name)
    raw=gzip.decompress(archived) if source.suffix=='.gz' else archived
    if digest(raw)!=specification['sha256']: raise ValueError('CAD source hash mismatch: '+source.name)
    if specification.get('extractProductSuffix'):
        extraction_spec=importlib.util.spec_from_file_location('retained_step_product_extractor',ROOT/'tools/extract-step-product.py')
        extractor=importlib.util.module_from_spec(extraction_spec);extraction_spec.loader.exec_module(extractor)
        raw,extraction=extractor.extract(raw,specification['extractProductSuffix'])
        if extraction['sha256']!=specification['extraction']['sha256']:raise ValueError('CAD product extraction hash mismatch')
    module_spec=importlib.util.spec_from_file_location('retained_lilygo_cad_exporter',ROOT/'tools/build-lilygo-models.py')
    module=importlib.util.module_from_spec(module_spec);module_spec.loader.exec_module(module)
    if specification.get('format')=='stl':
        import trimesh
        meshes=[trimesh.load(io.BytesIO(raw),file_type='stl',force='mesh',process=False)]
    else:
        with tempfile.TemporaryDirectory(prefix='lilygo-cad-') as temp:
            path=Path(temp)/'source.step';path.write_bytes(raw);meshes=module.load_step(path)
    matrix=np.array(specification['matrixRows'],dtype=float);r=matrix[:3,:3]
    if not np.allclose(r.T@r,np.eye(3),atol=1e-8) or not np.isclose(np.linalg.det(r),1):raise ValueError('Only rigid CAD transforms are allowed')
    groups=specification.get('groups') or [{'name':f'cad-part-{i:03d}','components':[i],'role':'component','color':'#9aa3a5'} for i,m in enumerate(meshes) if m is not None]
    for group in groups:
        arrays=[]
        for i in group['components']:
            mesh=meshes[i]
            if mesh is None:continue
            p=mesh.vertices@r.T+matrix[:3,3];normals=mesh.face_normals@r.T
            arrays.append(np.concatenate([p[mesh.faces].reshape(-1,3),np.repeat(normals,3,axis=0)],axis=1))
        if arrays:yield {k:v for k,v in group.items() if k!='components'}|{'basis':'manufacturer-cad','sourceComponents':group['components'],'sourceFile':specification['file']},np.concatenate(arrays)

def build(product_id, output):
    folder=ASSETS/product_id;plan=json.loads((folder/'model-plan.json').read_text());out=output/product_id;out.mkdir(parents=True,exist_ok=True)
    if plan['id']!=product_id:raise ValueError('Plan identity mismatch')
    records=[];buffers=[];offset=0;errors=[]
    items=list(cad_values(folder,plan['cad'])) if plan.get('cad') else []
    for additional in plan.get('additionalCad',[]):items.extend(cad_values(folder,additional))
    items.extend(({k:v for k,v in part.items() if k not in ['geometry','kind','sizeMm','radiusMm','openingMm','innerRadiusMm','centerMm','rotation','outlineMm']},primitive(part))for part in plan.get('parts',[]))
    if len(items)>512:raise ValueError('Too many parts for the browser contract')
    for record,values in items:
        if not np.isfinite(values).all():raise ValueError('Non-finite geometry')
        f=values.astype('<f4');error=float(np.abs(f[:,:3]-values[:,:3]).max());errors.append(error)
        binary=f.tobytes();buffers.append(binary);record.update(byteOffset=offset,vertexCount=len(f),boundsMm=[f[:,:3].min(0).tolist(),f[:,:3].max(0).tolist()]);records.append(record);offset+=len(binary)
    if offset>64*1024*1024:raise ValueError('Model exceeds browser geometry budget')
    binary=b''.join(buffers)
    compressed=plan.get('binaryCompression')=='gzip'
    binary_name='geometry.bin.gz' if compressed else 'geometry.bin'
    stored=gzip.compress(binary,mtime=0) if compressed else binary
    (out/binary_name).write_bytes(stored)
    stale=out/('geometry.bin' if compressed else 'geometry.bin.gz')
    if stale.exists():stale.unlink()
    artifacts={binary_name:digest(stored)}
    for filename in plan.get('retainedFiles',[]):
        data=(folder/filename).read_bytes();artifacts[filename]=digest(data)
        if out.resolve()!=folder.resolve():shutil.copyfile(folder/filename,out/filename)
    model={k:plan[k]for k in ['id','dimensions','presentation','derivedParts','referencePhotos','provenance','limitations','reviewViews','debugPreview','photoDerivation','dimensionChecks','representationScope','manualPhotoCalibration','omittedComponents']if k in plan}
    if plan.get('cad'):
        transform_note='STL does not encode units; coordinates interpreted as mm using cited dimension checks, with no rescaling.'if plan['cad'].get('format')=='stl'else'Selected product local coordinates; no assembly placement inferred; rigid viewing transform only.'if plan['cad'].get('extraction')else'Rigid transform only; supplied assembly placements and native millimetres retained.'
        model['sourceTransform']={'matrixRows':plan['cad']['matrixRows'],'scale':1,'sourceUnit':plan['cad'].get('sourceUnit','mm'),'note':transform_note}
        if plan['cad'].get('extraction'):model['cadExtraction']=plan['cad']['extraction']
        if plan.get('additionalCad'):model['sourceTransform']['additionalSources']=[{'file':source['file'],'matrixRows':source['matrixRows'],'sourceUnit':source.get('sourceUnit','mm')}for source in plan['additionalCad']]
    model.update(schemaVersion='1',unit='mm',strideFloats=6,binary=binary_name,parts=records,artifacts=artifacts)
    if compressed:model['binaryCompression']='gzip'
    write(out/'model.json',model)
    bounds=np.array([r['boundsMm']for r in records]);receipt={'productId':product_id,'modelSha256':digest((out/'model.json').read_bytes()),'geometrySha256':digest(binary),'storedGeometrySha256':digest(stored),'storedGeometryBytes':len(stored),'expandedGeometryBytes':len(binary),'binaryCompression':'gzip'if compressed else'none','mode':plan['mode'],'partCount':len(records),'triangleCount':sum(r['vertexCount']//3 for r in records),'maxFloat32ExportErrorMm':max(errors,default=0),'geometryBoundsMm':[bounds[:,0].min(0).tolist(),bounds[:,1].max(0).tolist()]if len(bounds)else None,'exportPassed':max(errors,default=0)<=.05,'physicalValidation':'Not implied by export; see per-plan evidence and limitations.','runtimeValidation':False}
    write(out/'build-check.json',receipt)
    return receipt

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=Path,required=True);parser.add_argument('ids',nargs='*');args=parser.parse_args()
    ids=args.ids or [p.parent.name for p in sorted(ASSETS.glob('lilygo-*/model-plan.json')) if json.loads(p.read_text()).get('generator')=='build-lilygo-expansion-models.py']
    ids.sort(key=lambda i:json.loads((ASSETS/i/'model-plan.json').read_text()).get('priorityRank',10000))
    for product_id in ids:
        result=build(product_id,args.output);print(json.dumps(result,ensure_ascii=False),flush=True)
if __name__=='__main__':main()
