#!/usr/bin/env python3
"""Check reviewed lane02 submissions, exact asset bytes and port constraints."""
import json,hashlib
from pathlib import Path
import numpy as np
from PIL import Image
from jsonschema import Draft202012Validator
from referencing import Registry,Resource
ROOT=Path(__file__).resolve().parents[1];SITE=ROOT/'web/platform';RUN=ROOT/'.butler/runs/20261027-010438-m5-hardware-83-esp32-esp-product/submissions'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
schemas={p.name:json.loads(p.read_text())for p in(SITE/'schemas').glob('*.json')};registry=Registry().with_resources((name,Resource.from_contents(s))for name,s in schemas.items());checks=[]
for lane in ['lane-02','lane-02-extra']:
 payload=json.loads((RUN/(lane+'.json')).read_text())
 for kind,plural in [('product','products'),('firmware','firmwares')]:
  validator=Draft202012Validator({'$ref':f'catalog.schema.json#/$defs/{kind}'},registry=registry)
  for item in payload[plural]:
   errors=list(validator.iter_errors(item));assert not errors,f"{item['id']}: {[e.message for e in errors]}"
 for p in payload['products']:
  id=p['id'];folder=SITE/'assets'/id;plan=json.loads((folder/'model-plan.json').read_text());m=json.loads((folder/'model.json').read_text());k=json.loads((SITE/'knowledge'/(id+'.json')).read_text());source_ids={s['id']for s in k['sources']}
  for name,digest in m['artifacts'].items():assert sha(folder/name)==digest,(id,name,'hash')
  geometry=np.fromfile(folder/m['binary'],dtype='<f4').reshape(-1,m['strideFloats']);assert np.isfinite(geometry).all(),(id,'nonfinite geometry')
  for part in m['parts']:
   assert part['byteOffset']%24==0 and part['vertexCount']>0 and part['byteOffset']//24+part['vertexCount']<=len(geometry),(id,part['name'],'range')
  for part in plan.get('derivedParts',[]):
   assert all(s in source_ids for s in part.get('sourceIds',[])),(id,part['id'],'source')
   if part.get('connectorType')=='HY2.0-4P':
    assert part.get('housingColor')and part.get('sourceIds'),(id,part['id'],'color/source')
    if part.get('allowConnections'):
     assert sorted(part.get('pinOrder',[]))==['gnd','signal1','signal2','vcc'],(id,'wire order')
     assert part.get('matingEvidence',{}).get('status')in['verified-photo','verified','reviewed-visual'],(id,'mating source')
   if part.get('kind')=='label':
    image=Image.open(folder/part['texture']);assert image.mode=='RGBA'and image.getchannel('A').getextrema()==(0,255),(id,'mask alpha')
  receipt=json.loads((ROOT/'docs/platform/evidence'/id/'verification.json').read_text());assert receipt['modelSha256']==sha(folder/'model.json'),(id,'stale model receipt');assert receipt['geometrySha256']==sha(folder/m['binary']),(id,'stale geometry receipt');assert receipt.get('browser',{}).get('status')=='reviewed-partial',(id,'missing browser review')
  assert id in payload['reviewedIds'],(id,'not reviewed')
  checks.append(id)
print('Validated',len(checks),'products: local schema, source IDs, geometry bounds/finite, hashes, alpha masks, explicit connector colors and verified wire order.')
