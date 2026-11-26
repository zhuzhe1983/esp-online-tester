#!/usr/bin/env python3
"""Compare semantic native parts after an independent export reorders triangles.

This does not replace byte/hash/contract checks. Position rounding is 0.00001 mm;
normal components are rounded separately to 0.00001 before canonical ordering.
"""
import argparse,gzip,hashlib,json
from pathlib import Path
import numpy as np

def sha(data):return hashlib.sha256(data).hexdigest()
def load(folder):
    meta=json.loads((folder/'model.json').read_text());data=(folder/meta['binary']).read_bytes()
    if meta.get('binaryCompression')=='gzip':data=gzip.decompress(data)
    return meta,np.frombuffer(data,dtype='<f4').reshape(-1,6)
def canonical(values,normals=False):
    columns=6 if normals else 3
    values=np.rint(values.reshape(-1,3,6)[:,:,:columns]/.00001).astype('<i8')
    values=np.asarray([triangle[np.lexsort(triangle.T[::-1])]for triangle in values]).reshape(-1,columns*3)
    return sha(values[np.lexsort(values.T[::-1])].tobytes())
def compare(original,rebuilt):
    a,x=load(original);b,y=load(rebuilt)
    if a['id']!=b['id']:raise ValueError('Exact product identity mismatch')
    rows=[]
    for p in a['parts']:
        q=next(v for v in b['parts']if v['name']==p['name'])
        left=x[p['byteOffset']//24:p['byteOffset']//24+p['vertexCount']]
        right=y[q['byteOffset']//24:q['byteOffset']//24+q['vertexCount']]
        rows.append({'part':p['name'],'triangles':len(left)//3,'positionsEqual':left.shape==right.shape and canonical(left)==canonical(right),'positionsAndNormalsEqual':left.shape==right.shape and canonical(left,True)==canonical(right,True)})
    return {'productId':a['id'],'originalModelSha256':sha((original/'model.json').read_bytes()),'rebuiltModelSha256':sha((rebuilt/'model.json').read_bytes()),'positionRoundingMm':.00001,'normalComponentRounding':.00001,'allNativePartsEqual':all(r['positionsAndNormalsEqual']for r in rows),'parts':rows,'scope':'Native semantic parts only. Three vertices within each triangle and triangle records are sorted after explicit rounding. A pass does not make differing binary bytes, derived parts, textures, metadata contracts or physical fit pass.'}
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('original',type=Path);p.add_argument('rebuilt',type=Path);p.add_argument('--output',type=Path);args=p.parse_args();result=compare(args.original,args.rebuilt);data=json.dumps(result,indent=2)+'\n'
    if args.output:args.output.write_text(data)
    print(json.dumps({k:v for k,v in result.items()if k!='parts'}));raise SystemExit(0 if result['allNativePartsEqual']else 1)
