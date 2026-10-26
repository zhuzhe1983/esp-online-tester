"""Partition the unchanged manufacturer keyboard mesh into 56 real key caps.

Keeps every original triangle exactly once, including the carrier remainder.
No boxes, point hotspots, per-key scaling or inferred grid geometry is created.
"""
import hashlib
import importlib.util
import json
import shutil
from pathlib import Path

import numpy as np
import trimesh

spec=importlib.util.spec_from_file_location('onboard','tools/onboard-lane-01.py')
onboard=importlib.util.module_from_spec(spec);spec.loader.exec_module(onboard)
identity='m5stack-k132-adv';assets=Path('web/platform/assets')/identity
evidence=Path('docs/platform/evidence')/identity;folder=onboard.TEMP/identity
plan=json.loads((assets/'model-plan.json').read_text())
prepared=json.loads((folder/'prepared.json').read_text())
source=next(s for s in prepared['sources']if s['id']==plan['sourceId'])
assert onboard.sha((folder/source['archiveName']).read_bytes())==source['sha256']
mesh=onboard.load_parts(folder/source['archiveName'])[2]
matrix=np.array(plan['matrixRows']);vertices=mesh.vertices@matrix[:3,:3].T+matrix[:3,3]
triangles=vertices[mesh.faces]
# CAD has a separated cap top at Z7.007..7.2mm and a lower web at <=6.238mm.
# Selecting any triangle reaching6.5mm retains each full exterior cap wall down
# to its original4.7mm boundary. The disconnected lower web remains carrier.
selected=triangles[:,:,2].max(1)>6.5
edges=mesh.face_adjacency[np.all(selected[mesh.face_adjacency],axis=1)]
groups=trimesh.graph.connected_components(edges,nodes=np.flatnonzero(selected),min_len=1)
assert len(groups)==56 and all(len(g)==250 for g in groups)
caps=[]
for group in groups:
    values=triangles[group].reshape(-1,3);bounds=np.array([values.min(0),values.max(0)])
    caps.append({'faces':np.sort(group),'center':bounds.mean(0),'bounds':bounds})
rows=sorted(set(round(float(p['center'][1]),4)for p in caps),reverse=True)
assert len(rows)==4
ordered=[]
for row,y in enumerate(rows):
    items=sorted([p for p in caps if abs(p['center'][1]-y)<.001],key=lambda p:p['center'][0])
    assert len(items)==14
    for column,item in enumerate(items):ordered.append({**item,'row':row,'column':column})

def ranges(indices):
    indices=sorted(map(int,indices));out=[]
    for index in indices:
        if out and index==out[-1][1]:out[-1][1]+=1
        else:out.append([index,index+1])
    return out

labels=[['` / ~ / Esc (Fn)','1','2','3','4','5','6','7','8','9','0','- / _','= / +','Backspace / Del'],['Tab','Q','W','E','R','T','Y','U','I','O','P','[ / {','] / }','\\ / |'],['Fn','Aa / Shift','A','S','D','F','G','H','J','K','L','; / : / Up (Fn)',"' / \"",'Enter / OK'],['Ctrl','Opt / Meta','Alt','Z','X','C','V','B','N','M',', / < / Left (Fn)','. / > / Down (Fn)','/ / ? / Right (Fn)','Space']]
codes=[['Backquote',*[f'Digit{i}'for i in [1,2,3,4,5,6,7,8,9,0]],'Minus','Equal','Backspace'],['Tab',*['Key'+c for c in 'QWERTYUIOP'],'BracketLeft','BracketRight','Backslash'],['AltRight','ShiftLeft',*['Key'+c for c in 'ASDFGHJKL'],'Semicolon','Quote','Enter'],['ControlLeft','MetaLeft','AltLeft',*['Key'+c for c in 'ZXCVBNM'],'Comma','Period','Slash','Space']]
assert all(len(row)==14 for row in labels+codes)
before=(assets/'geometry.bin').read_bytes()
multiset=lambda binary:hashlib.sha256(b''.join(sorted(binary[i:i+72]for i in range(0,len(binary),72)))).hexdigest()
before_hash=multiset(before)
for filename in ['model.json','model-plan.json']:
    path=evidence/('before-key-split-'+filename)
    if not path.exists():shutil.copyfile(assets/filename,path)
plan['parts']=[p for p in plan['parts']if not p.get('controlId','').startswith('key-r')]
carrier=next(p for p in plan['parts']if p['name']=='keyboard-carrier')
carrier['sourceFaceRanges']=ranges(np.flatnonzero(~selected));carrier['role']='internal'
controls=[c for c in plan['catalog']['controls']if not c['id'].startswith('key-r')]
coverage=np.zeros(len(mesh.faces),dtype=np.int32);coverage[np.flatnonzero(~selected)]+=1
receipt=[]
for item in ordered:
    row,column=item['row'],item['column'];control_id=f'key-r{row}-c{column}'
    coverage[item['faces']]+=1
    plan['parts'].append({'sourceComponent':2,'name':control_id,'role':'button','color':carrier['color'],'controlId':control_id,'sourceFaceRanges':ranges(item['faces'])})
    controls.append({'id':control_id,'name':labels[row][column],'type':'matrix-key','row':row,'column':column,'defaultKeyCode':codes[row][column]})
    receipt.append({'id':control_id,'row':row,'column':column,'faceCount':len(item['faces']),'boundsMm':item['bounds'].tolist(),'centerMm':item['center'].tolist()})
assert np.all(coverage==1)
url='https://raw.githubusercontent.com/m5stack/M5Cardputer-UserDemo/b549eac0a3c65bc108186c276b8fac0a214aaa4e/main/hal/keyboard/keyboard.cpp'
original=next(s for s in json.loads(Path('output/source/cardputer-adv-runtime/sources.json').read_text())if s['url']==url)
code_source={**original,'id':'official-keyboard-map','title':'Official CardputerADV keyboard mapping at b549eac0','kind':'software-source','status':'fetched','archiveName':'official-keyboard-map.cpp','fetchedAt':original['fetchedAt'].split('.')[0]+'Z','notes':'Reused actual source receipt; source commit is not asserted to be the reproducible build of the CDN v0.3 image.'}
shutil.copyfile('output/source/cardputer-adv-runtime/factory/main/hal/keyboard/keyboard.cpp',folder/code_source['archiveName']);assert onboard.sha((folder/code_source['archiveName']).read_bytes())==code_source['sha256']
prepared['sources']=[s for s in prepared['sources']if s['id']!=code_source['id']]+[code_source]
plan['extraSources']=[s for s in plan.get('extraSources',[])if s['id']!=code_source['id']]+[{k:v for k,v in code_source.items()if k in ['id','url','title','kind','archiveName']}]
plan['catalog']['controls']=controls
plan['catalog']['inputDevices'][0]['geometryModeled']=True
plan['facts']=[f for f in plan['facts']if f['id']!='keyboard-mesh-partition']+[{'id':'keyboard-mesh-partition','label':'56-key exterior geometry and host mapping','value':{'rows':4,'columns':14,'sourceComponent':2,'facesPerCap':250,'carrierFaces':int((~selected).sum()),'sourceFacesTotal':len(mesh.faces),'fnDefaultHostCode':'AltRight'},'basis':'cad-measurement','sourceIds':[plan['sourceId'],'product-photo-d3ec52b5a4','official-keyboard-map'],'method':'Exterior cap triangles reach viewerZ>6.5mm;56 disconnected groups are sorted top-to-bottom and left-to-right using their measured bounds. Full cap triangles and the untouched carrier partition cover the original39168faces once. Host codes match primary printed keys and official modifiers; AltRight for Fn is an explicit platform convention, not a manufacturer HID code.'}]
onboard.write(folder/'prepared.json',prepared);onboard.write(assets/'model-plan.json',plan)
onboard.build_model(prepared,plan,assets);onboard.build_knowledge(prepared,plan)
after_hash=multiset((assets/'geometry.bin').read_bytes());assert after_hash==before_hash,'Triangle positions or normals changed during partition'
report={'schemaVersion':'1','id':identity,'checkedAt':onboard.now(),'sourceSha256':source['sha256'],'sourceComponent':2,'sourceFaces':len(mesh.faces),'capCount':56,'capFaces':14000,'carrierFaces':25168,'allSourceFacesCoveredExactlyOnce':True,'beforeAfterTriangleMultisetSha256':after_hash,'modelSha256':onboard.sha((assets/'model.json').read_bytes()),'selection':'Any source triangle reaching viewerZ>6.5mm, connected by original face adjacency; no triangle clipping or new geometry. Rows use measured cap bounds, not an assumed grid.','keys':receipt,'limitations':['Visual cap grouping does not establish firmware compatibility or switch travel.','Fn defaults to host AltRight by platform convention; Fn-layer arrows/Esc require the actual Fn chord.']}
onboard.write(evidence/'keyboard-partition.json',report)
onboard.write(onboard.RUN/'submissions/cardputer-adv-controls.json',{'schemaVersion':'1','productId':identity,'modelSha256':report['modelSha256'],'controls':controls,'inputDevices':plan['catalog']['inputDevices'],'sourceIds':['official-keyboard-map','product-photo-d3ec52b5a4'],'note':'Merge controls/model only; preserve runtime support set by the board implementation.'})
print(json.dumps({k:report[k]for k in ['capCount','capFaces','carrierFaces','allSourceFacesCoveredExactlyOnce','beforeAfterTriangleMultisetSha256','modelSha256']}))
