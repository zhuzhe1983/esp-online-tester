#!/usr/bin/env python3
"""Offline, source-pinned exterior previews for U007/U082/U109-X/U133.

Only visible photograph-constrained parts are approximated. Unmeasured PCB
faces remain UNKNOWN. This exporter never downloads or executes firmware.
"""
import argparse
import copy
import gzip
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import numpy as np
from PIL import Image, ImageDraw
ROOT = Path(__file__).resolve().parents[1]
IDS = ['m5stack-u007','m5stack-u082','m5stack-u109-x','m5stack-u133']
def digest(b):return hashlib.sha256(b).hexdigest()
def write(p,d):p.write_text(json.dumps(d,ensure_ascii=False,indent=2)+'\n')
def build(product_id,output):
    folder=ROOT/'web/platform/assets'/product_id
    plan=json.loads((folder/'model-plan.json').read_text())
    if product_id!=plan['id']:raise ValueError('Product identity changed')
    out=output/product_id
    if out.resolve()==folder.resolve():raise ValueError('Rebuild into a separate directory')
    out.mkdir(parents=True,exist_ok=True)
    for name,expected in plan['retainedHashes'].items():
        if Path(name).name!=name or digest((folder/name).read_bytes())!=expected:raise ValueError('Retained source changed: '+name)
        shutil.copyfile(folder/name,out/name)
    spec=importlib.util.spec_from_file_location('legacy_camera_primitives',ROOT/'tools/build-lilygo-expansion-models.py')
    primitives=importlib.util.module_from_spec(spec);spec.loader.exec_module(primitives)
    parts=[];arrays=[];offset=0
    for part in plan['parts']:
        values=primitives.primitive(part).astype('<f4')
        record={k:copy.deepcopy(v)for k,v in part.items()if k not in ['kind','sizeMm','radiusMm','centerMm','rotation','outlineMm']}
        record.update(byteOffset=offset,vertexCount=len(values),boundsMm=[values[:,:3].min(0).tolist(),values[:,:3].max(0).tolist()]);parts.append(record);arrays.append(values.tobytes());offset+=values.nbytes
    for t in plan.get('textures',[]):
        image=Image.open(folder/t['sourceFile']).convert('RGBA');factor=4
        alpha=Image.new('L',(image.width*factor,image.height*factor),0);draw=ImageDraw.Draw(alpha)
        for poly in t.get('includePolygons',[]):draw.polygon([(round(x*factor),round(y*factor))for x,y in poly],fill=255)
        for rect in t.get('includeRects',[]):draw.rectangle([round(x*factor)for x in rect],fill=255)
        for rect in t.get('excludeRects',[]):draw.rectangle([round(x*factor)for x in rect],fill=0)
        for x,y,r in t.get('excludeCircles',[]):draw.ellipse([round(v*factor)for v in [x-r,y-r,x+r,y+r]],fill=0)
        image.putalpha(alpha.resize(image.size,Image.Resampling.LANCZOS));image.crop(t['cropXYXY']).save(out/t['file'],optimize=True)
    raw=b''.join(arrays)
    (out/'geometry.bin.gz').write_bytes(gzip.compress(raw,mtime=0))
    model=dict(schemaVersion='1',id=product_id,unit='mm',binary='geometry.bin.gz',binaryCompression='gzip',strideFloats=6,parts=parts,
               dimensions=plan['dimensions'],presentation=plan['presentation'],derivedParts=plan['derivedParts'],derivedSurfaces=[],referencePhotos=plan['referencePhotos'],textures=plan.get('textures',[]),provenance=plan['provenance'],limitations=plan['limitations'])
    model['artifacts']={p.name:digest(p.read_bytes())for p in sorted(out.iterdir())if p.is_file()}
    write(out/'model.json',model)
    bounds=None
    if raw:
        v=np.frombuffer(raw,dtype='<f4').reshape(-1,6)
        if not np.isfinite(v).all():raise ValueError('Non-finite preview geometry')
        bounds=[v[:,:3].min(0).tolist(),v[:,:3].max(0).tolist()]
    receipt=dict(productId=product_id,modelSha256=digest((out/'model.json').read_bytes()),geometrySha256=digest(raw),nativeApproximationBoundsMm=bounds,
                 scope='Only exported visible approximation meshes; declarative PCB/shell/socket parts excluded. No independent dimension, optical, calibration or firmware pass is implied.')
    write(out/'rebuild-receipt.json',receipt)
    return receipt
if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,required=True);parser.add_argument('product_ids',nargs='*');args=parser.parse_args();args.output.mkdir(parents=True,exist_ok=True)
    for product_id in args.product_ids or IDS:
        if product_id not in IDS:raise ValueError('Unsupported product')
        print(json.dumps(build(product_id,args.output)))
