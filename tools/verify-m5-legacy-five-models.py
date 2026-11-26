#!/usr/bin/env python3
"""Offline source fidelity, regeneration and alpha checks for five legacy SKUs."""
import argparse,hashlib,importlib.util,itertools,json,tempfile
from pathlib import Path
import numpy as np
from PIL import Image
from scipy.spatial import cKDTree
ROOT=Path(__file__).resolve().parents[1]
ASSETS=ROOT/'web/platform/assets'
def digest(b):return hashlib.sha256(b).hexdigest()
def verify():
    spec=importlib.util.spec_from_file_location('m5_legacy_builder',ROOT/'tools/build-m5-legacy-five-models.py');builder=importlib.util.module_from_spec(spec);spec.loader.exec_module(builder)
    rebuilt=[]
    with tempfile.TemporaryDirectory(prefix='m5-legacy-rebuild-')as temp:
        for identity in builder.IDS:
            builder.build(identity,Path(temp));source=ASSETS/identity;output=Path(temp)/identity
            metadata=json.loads((source/'model.json').read_text());assert(output/'model.json').read_bytes()==(source/'model.json').read_bytes()
            for filename in metadata['artifacts']:assert(output/filename).read_bytes()==(source/filename).read_bytes()
            rebuilt.append({'id':identity,'metadataByteIdentical':True,'allArtifactsByteIdentical':True,'modelSha256':digest((source/'model.json').read_bytes())})
    fidelity=[];perms=np.array(list(itertools.permutations(range(3))))
    for identity in ['m5stack-u038','m5stack-u082-f']:
        folder=ASSETS/identity;plan=json.loads((folder/'model-plan.json').read_text());metadata=json.loads((folder/'model.json').read_text());raw=(folder/plan['cad']['file']).read_bytes()
        faces=np.frombuffer(raw,dtype=np.dtype([('normal','<f4',(3,)),('v','<f4',(3,3)),('attribute','<u2')]),offset=84)
        matrix=np.array(plan['cad']['matrixRows']);native=faces['v']@matrix[:3,:3].T+matrix[:3,3];tree=cKDTree(native.mean(1));buffer=np.fromfile(folder/'geometry.bin',dtype='<f4').reshape(-1,6);count=0;worst=0
        for part in metadata['parts']:
            if part.get('basis')!='manufacturer-cad':continue
            start=part['byteOffset']//24;triangles=buffer[start:start+part['vertexCount'],:3].reshape(-1,3,3).astype(float)
            _,indices=tree.query(triangles.mean(1),k=4);candidates=native[indices][:,:,perms,:]
            error=np.abs(triangles[:,None,None,:,:]-candidates).max(axis=(-1,-2)).min(axis=(1,2));assert error.max()<.00005
            worst=max(worst,float(error.max()));count+=len(triangles)
        fidelity.append({'id':identity,'rawSTLSha256':digest(raw),'caseTrianglesComparedToOriginalRawSTL':count,'everyCaseTriangleIsOriginal':True,'maxCoordinateErrorMm':worst,'toleranceMm':.00005})
    paper=np.asarray(Image.open(ASSETS/'m5stack-k049-c/rear-paper.png'));pcb=np.asarray(Image.open(ASSETS/'m5stack-u017-pcba/pcb-front.png'))
    assert paper[300,100,3]==0 and paper[300,180,3]==255 and paper[1050,580,3]==0 and paper[:20,:,3].max()==0
    assert pcb[797,49,3]==0 and pcb[480,180,3]==0 and pcb[100,250,3]==0
    return {'rebuild':rebuilt,'rawCADSourceFidelity':fidelity,'textureAlpha':{'paperPlasticNotchExcluded':True,'paperPrintRetained':True,'paperPortsExcluded':True,'pcbHoleExcluded':True,'separateCameraAndFFCNotPrintedTwice':True},'scope':'Actual offline regeneration, independent rawSTL triangle comparison and photo-alpha pixel assertions. No GPU/photo1:1, hardware or firmware execution acceptance.'}
if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=Path);args=parser.parse_args();result=verify();text=json.dumps(result,ensure_ascii=False,indent=2)+'\n'
    if args.output:args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(text)
    print(text)
