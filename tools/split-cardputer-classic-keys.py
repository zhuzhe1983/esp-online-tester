"""Split K132/K132-V11 native keys without changing triangles, normals or labels.

Source component11 supplies56 disconnected exterior caps above viewerZ6.5mm.
The remainder stays intact. Source metadata receipts stay exact and versioned;
source copies remain temporary. Board runtime/catalog integration is separate.
"""
import argparse,copy,hashlib,importlib.util,json,re,shutil
from pathlib import Path
import numpy as np,trimesh
spec=importlib.util.spec_from_file_location('onboard','tools/onboard-lane-01.py');o=importlib.util.module_from_spec(spec);spec.loader.exec_module(o)
ORIGINS={'m5stack-k132':'lane02','m5stack-k132-v11':'lane02-extra'}
o.TEMP=Path('output/source/cardputer-key-partition')
labels=[['` / ~ / Esc','1','2','3','4','5','6','7','8','9','0','- / _','= / +','Del / Backspace'],['Tab','Q','W','E','R','T','Y','U','I','O','P','[ / {','] / }','\\ / |'],['Fn','Aa / Shift','A','S','D','F','G','H','J','K','L','; / : / Up',"' / \"",'Enter / OK'],['Ctrl','Opt','Alt','Z','X','C','V','B','N','M',', / < / Left','. / > / Down','/ / ? / Right','Space']]
codes=[['Backquote',*[f'Digit{i}'for i in [1,2,3,4,5,6,7,8,9,0]],'Minus','Equal','Backspace'],['Tab',*['Key'+c for c in 'QWERTYUIOP'],'BracketLeft','BracketRight','Backslash'],['AltRight','ShiftLeft',*['Key'+c for c in 'ASDFGHJKL'],'Semicolon','Quote','Enter'],['ControlLeft','MetaLeft','AltLeft',*['Key'+c for c in 'ZXCVBNM'],'Comma','Period','Slash','Space']]
primary=['`',*list('1234567890'),'-','=','del','tab',*list('qwertyuiop'),'[',']','\\','fn','shift',*list('asdfghjkl'),';',"'",'enter','ctrl','opt','alt',*list('zxcvbnm'),',','.','/','space']
assert len(primary)==56 and all(len(r)==14 for r in labels+codes)
commits={'current':'a34d7ebcd508fb903a2b1123be8c8b54abd55d07','binary-base':'f0b37ae5ebe709925fa54cca643418d96210f216'}
all_sources=json.loads(Path('output/source/cardputer-runtime/sources.json').read_text());mapping_sources=[]
for label,commit in commits.items():
 root=Path('output/source/cardputer-runtime')/('factory'if label=='current'else'binary-base')/'main/hal/keyboard';header=(root/'keyboard.h').read_text();block=header.split('_key_value_map[4][14] =',1)[1].split('};',1)[0];actual=[json.loads('"'+x+'"')for x in re.findall(r'^\s*\{+\s*"((?:\\.|[^"\\])*)"',block,re.M)];assert actual==primary,f'{label}: official physical-key table differs'
 for filename in ['keyboard.h','keyboard.cpp','keymap.h']:
  url=f'https://raw.githubusercontent.com/m5stack/M5Cardputer-UserDemo/{commit}/main/hal/keyboard/{filename}';s=next(s for s in all_sources if s['url']==url);data=(root/filename).read_bytes();assert len(data)==s['bytes'] and o.sha(data)==s['sha256'];mapping_sources.append(({**s,'id':f'official-classic-{label}-{filename.replace(".","-")}','title':f'Original Cardputer {filename} at {commit[:8]}','kind':'software-source','status':'fetched','archiveName':f'official-{label}-{filename}','fetchedAt':s['fetchedAt'].split('.')[0]+'Z','notes':'Physical table and scan evidence only. Binary reports f0b37ae-dirty; neither source tree is asserted to reproduce that modified binary. Fn/HID behavior changed between these versions.'},root/filename))
def ranges(indices):
 out=[]
 for index in sorted(map(int,indices)):
  if out and index==out[-1][1]:out[-1][1]+=1
  else:out.append([index,index+1])
 return out
def triangle_multiset(data):
 assert len(data)%72==0
 return hashlib.sha256(b''.join(sorted(data[i:i+72]for i in range(0,len(data),72)))).hexdigest()
def split(id):
 assets=Path('web/platform/assets')/id;evidence=Path('docs/platform/evidence')/id;folder=o.TEMP/id;original=Path('output/source')/ORIGINS[id]/id
 if not folder.exists():shutil.copytree(original,folder)
 plan=json.loads((assets/'model-plan.json').read_text());prepared=json.loads((folder/'prepared.json').read_text());source=next(s for s in prepared['sources']if s['id']==plan['sourceId']);assert o.sha((folder/source['archiveName']).read_bytes())==source['sha256'];mesh=o.load_parts(folder/source['archiveName'])[11];matrix=np.asarray(plan['matrixRows']);vertices=mesh.vertices@matrix[:3,:3].T+matrix[:3,3];triangles=vertices[mesh.faces];selected=triangles[:,:,2].max(1)>6.5;edges=mesh.face_adjacency[np.all(selected[mesh.face_adjacency],axis=1)];groups=trimesh.graph.connected_components(edges,nodes=np.flatnonzero(selected),min_len=1);assert len(groups)==56 and all(len(g)==250 for g in groups)
 caps=[]
 for group in groups:
  points=triangles[group].reshape(-1,3);bounds=np.asarray([points.min(0),points.max(0)]);caps.append({'faces':np.sort(group),'center':bounds.mean(0),'bounds':bounds})
 rows=sorted(set(round(float(c['center'][1]),4)for c in caps),reverse=True);assert len(rows)==4
 before=(assets/'geometry.bin').read_bytes();before_hash=triangle_multiset(before);old_model=json.loads((assets/'model.json').read_text());before_textures={t['file']:o.sha((assets/t['file']).read_bytes())for t in old_model['textures']}
 for filename in ['model.json','model-plan.json']:
  path=evidence/('before-classic-key-split-'+filename)
  if not path.exists():shutil.copyfile(assets/filename,path)
 plan['parts']=[p for p in plan['parts']if not p.get('controlId','').startswith('key-r')];carrier=next(p for p in plan['parts']if p['name']=='keyboard-carrier');assert carrier['sourceComponent']==11
 carrier['sourceFaceRanges']=ranges(np.flatnonzero(~selected));carrier['role']='internal';controls=[c for c in plan['catalog']['controls']if not c['id'].startswith('key-r')];coverage=np.zeros(len(mesh.faces),dtype=np.int32);coverage[np.flatnonzero(~selected)]+=1;receipt=[]
 for row,y in enumerate(rows):
  items=sorted([c for c in caps if abs(c['center'][1]-y)<.001],key=lambda c:c['center'][0]);assert len(items)==14
  for col,item in enumerate(items):
   identity=f'key-r{row}-c{col}';coverage[item['faces']]+=1;plan['parts'].append({'sourceComponent':11,'name':identity,'role':'button','color':carrier['color'],'controlId':identity,'sourceFaceRanges':ranges(item['faces'])});controls.append({'id':identity,'name':labels[row][col],'type':'matrix-key','row':row,'column':col,'defaultKeyCode':codes[row][col]});receipt.append({'id':identity,'row':row,'column':col,'sourcePrimary':primary[row*14+col],'faceCount':len(item['faces']),'boundsMm':item['bounds'].tolist(),'centerMm':item['center'].tolist(),'scanSelector':3-row+(4 if col%2==0 else 0),'scanInputGpio':[13,15,3,4,5,6,7][col//2]})
 assert np.all(coverage==1)
 for s,path in mapping_sources:
  shutil.copyfile(path,folder/s['archiveName']);prepared['sources']=[p for p in prepared['sources']if p['id']!=s['id']]+[s]
 plan['catalog']['controls']=controls
 for device in plan['catalog'].get('inputDevices',[]):
  if device['id']=='keyboard':device['geometryModeled']=True
 o.write(folder/'prepared.json',prepared);o.write(assets/'model-plan.json',plan);o.build_model(prepared,plan,assets)
 after=(assets/'geometry.bin').read_bytes();assert triangle_multiset(after)==before_hash,'Triangle positions/normals changed during partition'
 assert all(o.sha((assets/f).read_bytes())==h for f,h in before_textures.items()),'Texture artifact changed'
 new_model=json.loads((assets/'model.json').read_text());assert old_model['derivedParts']==new_model['derivedParts'],'Existing connector/label definitions changed';assert old_model['derivedSurfaces']==new_model['derivedSurfaces'],'Existing image surfaces changed'
 photo=next(p['sourceId']for p in plan['photos']if p['view']=='front');report={'schemaVersion':'1','id':id,'checkedAt':o.now(),'sourceSha256':source['sha256'],'sourceComponent':11,'sourceFaces':len(mesh.faces),'capCount':56,'capFaces':int(selected.sum()),'carrierFaces':int((~selected).sum()),'allSourceFacesCoveredExactlyOnce':True,'beforeAfterTriangleMultisetSha256':before_hash,'modelSha256':o.sha((assets/'model.json').read_bytes()),'texturesUnchanged':before_textures,'derivedPortsLabelsUnchanged':True,'selection':'Original triangles whose maximum viewerZ exceeds6.5mm;56 source-adjacent components, sorted by measured cap centers top-to-bottom/left-to-right. No assumed hotspot grid, scaling or triangle clipping.','sourceIds':[photo,*(s['id']for s,_ in mapping_sources)],'mappingSources':[s for s,_ in mapping_sources],'sourceTablesAgreeOnAll56PrimaryKeys':True,'keys':receipt,'limitations':['Partial product model retained; key grouping alone is not firmware/hardware verification.','Fn hostAltRight and Opt hostMetaLeft are platform mapping choices, not vendor USB HID codes.','The dirty f0b37ae binary is not proven reproducible from either source commit; Fn/HID differences are outside this geometry/control contract.']};o.write(evidence/'keyboard-partition.json',report);submission={'schemaVersion':'1','productId':id,'modelSha256':report['modelSha256'],'controls':controls,'inputDevices':plan['catalog']['inputDevices'],'sourceIds':report['sourceIds'],'mappingSources':report['mappingSources'],'note':'Merge model/controls only; preserve board runtime and firmware from runtime integration. Fn/Opt host codes are explicit platform choices.'};o.write(o.RUN/'submissions'/(id+'-controls.json'),submission)
 print(json.dumps({k:report[k]for k in ['id','capCount','capFaces','carrierFaces','beforeAfterTriangleMultisetSha256','modelSha256']}),flush=True)
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--ids',nargs='*',choices=list(ORIGINS));a=p.parse_args()
 for id in a.ids or ORIGINS:split(id)
