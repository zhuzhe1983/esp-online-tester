#!/usr/bin/env python3
"""Rebuild reviewed interaction metadata only; keep native buffers and visible fields."""
import argparse, copy, hashlib, json
from pathlib import Path
p=argparse.ArgumentParser()
for key in ['source-root','plan-root','out','ids']:p.add_argument('--'+key,required=True)
a=p.parse_args();root=Path(a.source_root);plans=Path(a.plan_root);out=Path(a.out);rows=[]
if out.resolve().is_relative_to(root.resolve()) or out.resolve().is_relative_to(plans.resolve()):raise ValueError('Output must be outside both source and plan checkouts')
def sha(b):return hashlib.sha256(b).hexdigest()
for identity in a.ids.split(','):
    if not identity.startswith('m5stack-') or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789-' for c in identity):raise ValueError('Unsafe product ID')
    folder=Path('web/platform/assets')/identity;old_bytes=(root/folder/'model.json').read_bytes();model=json.loads(old_bytes);plan_path=plans/folder/'model-plan.json';plan_bytes=(plan_path if plan_path.exists() else root/folder/'model-plan.json').read_bytes();plan=json.loads(plan_bytes)
    native=plan.get('cad',{}).get('parts',plan.get('parts',[]));native={part['name']:part for part in native}
    for part in model['parts']:
        source=native.get(part['name'])
        if source is not None:
            if 'controlId' in source:part['controlId']=source['controlId']
            else:part.pop('controlId',None)
    derived={part['id']:part for part in plan.get('derivedParts',[])}
    for part in model.get('derivedParts',[]):
        source=derived.get(part['id'])
        if source is not None:
            for key in ['controlId','encoderId','encoderPress','notes']:
                if key in source:part[key]=copy.deepcopy(source[key])
                else:part.pop(key,None)
    if 'buttons' in plan.get('presentation',{}):model['presentation']['buttons']=copy.deepcopy(plan['presentation']['buttons'])
    else:model['presentation'].pop('buttons',None)
    destination=out/folder;destination.mkdir(parents=True,exist_ok=True);data=(json.dumps(model,ensure_ascii=False,indent=2)+'\n').encode();(destination/'model.json').write_bytes(data)
    rows.append(dict(productId=identity,sourceModelSha256=sha(old_bytes),planSha256=sha(plan_bytes),modelSha256=sha(data),scope='Interaction metadata rebuild from reviewed plan; retained native buffers, textures and visible fields are unchanged. No CAD re-tessellation.'))
out.mkdir(parents=True,exist_ok=True);(out/'rebuild-receipt.json').write_text(json.dumps(rows,indent=2)+'\n');print(json.dumps(rows))
