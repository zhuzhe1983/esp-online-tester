#!/usr/bin/env python3
"""Rebuild source-pinned K007-V26/K049-C/U017-PCBA/U038/U082-F previews.

No network. Manufacturer STL components retain one rigid assembly transform.
Photo-derived geometry is explicitly estimated and never substitutes a runtime.
"""
import argparse,hashlib,importlib.util,json,shutil
from pathlib import Path
import numpy as np
import trimesh
from PIL import Image
import cv2
ROOT=Path(__file__).resolve().parents[1]
ASSETS=ROOT/'web/platform/assets'
IDS=['m5stack-k007-v26','m5stack-k049-c','m5stack-u017-pcba','m5stack-u038','m5stack-u082-f']
spec=importlib.util.spec_from_file_location('preview_primitives',ROOT/'tools/build-lilygo-expansion-models.py')
primitives=importlib.util.module_from_spec(spec);spec.loader.exec_module(primitives)
def digest(b):return hashlib.sha256(b).hexdigest()
def write(p,x):p.write_text(json.dumps(x,ensure_ascii=False,indent=2)+'\n')
def build(identity,destination):
    source=ASSETS/identity;plan=json.loads((source/'model-plan.json').read_text());out=destination/identity;out.mkdir(parents=True,exist_ok=True)
    if plan.get('controlSurfaceRecipe'):
        spec=importlib.util.spec_from_file_location('control_surface_rebuild',ROOT/'tools/rebuild-m5-control-models.py');builder=importlib.util.module_from_spec(spec);spec.loader.exec_module(builder)
        result=builder.build(identity,ROOT,ROOT,destination);model=json.loads((out/'model.json').read_text())
        return {'productId':identity,'modelSha256':result['modelSha256'],'geometrySha256':result['geometrySha256'],'parts':len(model['parts']),'triangles':sum(x['vertexCount']//3 for x in model['parts']),'scope':plan['representationScope'],'hardwareValidated':False,'firmwareExecuted':False}
    for receipt in plan['provenance']['sources']:
        filename=receipt.get('archiveName')
        if filename in plan['retainedFiles']:
            data=(source/filename).read_bytes()
            if digest(data)!=receipt['sha256']or len(data)!=receipt['bytes']:raise ValueError('Pinned source mismatch: '+filename)
    parts=[];buffers=[];offset=0
    def add(record,values):
        nonlocal offset
        v=np.asarray(values,dtype='<f4');assert np.isfinite(v).all()and len(v)%3==0
        item={**record,'byteOffset':offset,'vertexCount':len(v),'boundsMm':[v[:,:3].min(0).astype(float).tolist(),v[:,:3].max(0).astype(float).tolist()]};parts.append(item);buffers.append(v.tobytes());offset+=v.nbytes
    if plan.get('cad'):
        c=plan['cad'];mesh=trimesh.load(source/c['file']);components=mesh.split(only_watertight=False);matrix=np.asarray(c['matrixRows']);rot=matrix[:3,:3]
        assert np.allclose(rot.T@rot,np.eye(3))and np.isclose(np.linalg.det(rot),1)
        for part in c['parts']:
            m=components[part['sourceComponent']];vertices=m.vertices@rot.T+matrix[:3,3]
            normals=m.face_normals@rot.T;values=np.concatenate([vertices[m.faces].reshape(-1,3),np.repeat(normals,3,axis=0)],axis=1)
            add({**part,'basis':'manufacturer-cad','sourceIds':[c['sourceId']]},values)
    for part in plan.get('parts',[]):
        record={k:v for k,v in part.items()if k not in ['kind','sizeMm','radiusMm','openingMm','innerRadiusMm','centerMm','rotation','outlineMm']}
        add(record,primitives.primitive(part))
    artifacts={}
    for filename in plan['retainedFiles']:
        data=(source/filename).read_bytes();artifacts[filename]=digest(data)
        if source.resolve()!=out.resolve():shutil.copyfile(source/filename,out/filename)
    for texture in plan.get('photoDerivations',[]):
        image=np.asarray(Image.open(source/texture['sourceFile']).convert('RGB'))
        w,h=texture['outputPixels'];quad=np.asarray(texture['sourceQuadPixels'],dtype=np.float32)
        matrix=cv2.getPerspectiveTransform(quad,np.asarray([[0,0],[w,0],[w,h],[0,h]],dtype=np.float32))
        rgb=cv2.warpPerspective(image,matrix,(w,h),flags=cv2.INTER_CUBIC)
        alpha=np.zeros(image.shape[:2],dtype=np.uint8);cv2.fillPoly(alpha,[np.asarray(texture['alphaPolygonPixels'],dtype=np.int32)],255)
        for polygon in texture.get('excludedPolygonsPixels',[]):cv2.fillPoly(alpha,[np.asarray(polygon,dtype=np.int32)],0)
        opacity=cv2.warpPerspective(alpha,matrix,(w,h),flags=cv2.INTER_NEAREST)
        if texture.get('alphaOutputPolygonPixels'):
            final_mask=np.zeros((h,w),dtype=np.uint8);cv2.fillPoly(final_mask,[np.asarray(texture['alphaOutputPolygonPixels'],dtype=np.int32)],255);opacity=cv2.bitwise_and(opacity,final_mask)
        for circle in texture.get('excludedOutputCirclesPixels',[]):cv2.circle(opacity,tuple(circle['center']),circle['radius'],0,-1)
        for polygon in texture.get('excludedOutputPolygonsPixels',[]):cv2.fillPoly(opacity,[np.asarray(polygon,dtype=np.int32)],0)
        rgba=np.dstack([rgb,opacity]);Image.fromarray(rgba).save(out/texture['file'],optimize=True)
        artifacts[texture['file']]=digest((out/texture['file']).read_bytes())
    geometry=b''.join(buffers);(out/'geometry.bin').write_bytes(geometry);artifacts['geometry.bin']=digest(geometry)
    model={k:plan[k]for k in ['id','dimensions','presentation','derivedParts','referencePhotos','provenance','limitations','reviewViews','photoDerivations','omittedComponents']if k in plan}
    model.update(schemaVersion='1',unit='mm',strideFloats=6,binary='geometry.bin',parts=parts,artifacts=artifacts)
    if plan.get('cad'):model['sourceTransform']={'matrixRows':plan['cad']['matrixRows'],'scale':1,'note':'Native manufacturer assembly components; one rigid transform, millimetres, no fitting or scaling.'}
    write(out/'model.json',model)
    return {'productId':identity,'modelSha256':digest((out/'model.json').read_bytes()),'geometrySha256':digest(geometry),'parts':len(parts),'triangles':sum(x['vertexCount']//3 for x in parts),'scope':plan['representationScope'],'hardwareValidated':False,'firmwareExecuted':False}
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,required=True);p.add_argument('ids',nargs='*');a=p.parse_args();print(json.dumps([build(i,a.output)for i in a.ids or IDS],ensure_ascii=False,indent=2))
