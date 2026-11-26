#!/usr/bin/env python3
"""Rebuild exact-SKU rear paper textures, keeping every existing native triangle unchanged.

Canonical label-only reproduction: --output NEW_SCRATCH SKU. No source downloads.
The existing base exporters also retain the explicit label definitions in model plans.
"""
import argparse,copy,hashlib,json,shutil
from pathlib import Path
from m5_texture_projection import projected_texture
ROOT=Path(__file__).resolve().parents[1]
IDS=['m5stack-k001-v26','m5stack-k002','m5stack-k010-v11','m5stack-k001-v27','m5stack-k001','m5stack-k001-b','m5stack-k128-lite']
def sha(data):return hashlib.sha256(data).hexdigest()
def canonical(value):return sha(json.dumps(value,sort_keys=True,separators=(',',':')).encode())
def write(path,value):path.write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n')
def build(identity,destination):
    folder=ROOT/'web/platform/assets'/identity;plan=json.loads((folder/'model-plan.json').read_text());model=json.loads((folder/'model.json').read_text());spec=plan['rearLabel'];out=destination/identity
    if out.resolve()==folder.resolve()or out.exists():raise ValueError('Use a new independent output directory')
    if identity not in IDS or model['id']!=identity:raise ValueError('Exact SKU mismatch')
    native=copy.deepcopy(model['parts'])
    for update in spec.get('nativeAppearanceUpdates',[]):
        if set(update['fields'])-{'color','material'}:raise ValueError('Only explicit native appearance corrections are allowed')
        part=next(p for p in native if p['name']==update['name'])
        for key,change in update['fields'].items():
            if part.get(key) not in [change['before'],change['after']]:raise ValueError('Unexpected native appearance input')
            part[key]=copy.deepcopy(change['before'])
    if sha((folder/model['binary']).read_bytes())!=spec['originalGeometrySha256']or canonical(native)!=spec['originalNativePartsSha256']or canonical(model['sourceTransform'])!=spec['originalTransformSha256']:raise ValueError('Original geometry/transforms changed')
    model['parts']=native
    for update in spec.get('nativeAppearanceUpdates',[]):
        part=next(p for p in model['parts']if p['name']==update['name'])
        for key,change in update['fields'].items():part[key]=copy.deepcopy(change['after'])
    out.mkdir(parents=True)
    for p in folder.iterdir():
        if p.is_file():shutil.copyfile(p,out/p.name)
    texture=spec['texture'];textures=[texture]+spec.get('additionalTextures',[])
    records=[projected_texture(folder/t['sourceFile'],t,out/t['file'])for t in textures];label=spec['part']
    added=spec.get('additionalParts',[])+[label];ids=[p['id']for p in added]
    model['derivedParts']=[p for p in model.get('derivedParts',[])if p['id']not in ids]+copy.deepcopy(added)
    files=[t['file']for t in textures]
    model['textures']=[t for t in model.get('textures',[])if t['file']not in files]+records
    groups=model['presentation'].setdefault('inspectionGroups',[])
    if spec.get('inspectionGroupDefinition')and not any(g['id']==spec['inspectionGroup']for g in groups):groups.append(copy.deepcopy(spec['inspectionGroupDefinition']))
    for group in groups:
        group['parts']=[p for p in group['parts']if p not in ids]
        if group['id']==spec['inspectionGroup']:group['parts'].extend(ids)
    for file in files:model['artifacts'][file]=sha((out/file).read_bytes())
    model['limitations']=[s for s in model.get('limitations',[])if 'Back label and PCB remain incomplete'not in s]
    for note in spec['limitations']:
        if note not in model['limitations']:model['limitations'].append(note)
    write(out/'model.json',model)
    assert sha((out/model['binary']).read_bytes())==spec['originalGeometrySha256']
    receipt={'productId':identity,'modelSha256':sha((out/'model.json').read_bytes()),'labelSha256':model['artifacts'][texture['file']],'sourcePhotoSha256':texture['sourceSha256'],'originalGeometrySha256':spec['originalGeometrySha256'],'nativeTrianglesAndTransformsUnchanged':True,'sourceRegistration':'explicit photo homography and alpha polygons; estimated, no independent optical calibration','paperDisplayGapMm':spec['paperDisplayGapMm'],'scope':spec.get('scope','Rear paper labels only. No shell/PCB/port geometry or runtime changes.')}
    write(out/'rear-label-rebuild.json',receipt);return receipt
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('ids',nargs='*');p.add_argument('--output',required=True,type=Path);a=p.parse_args()
    for identity in a.ids or IDS:print(json.dumps(build(identity,a.output),ensure_ascii=False))
