#!/usr/bin/env python3
"""Synchronize generated LilyGO live/debug surfaces with verified board profiles.

Discovers base, additional, RISC-V, classic and future LilyGO profile JSONs that
contain productId and boardId. Model plans and the catalog patch are updated;
run build-lilygo-expansion-models.py afterwards to emit assets. No source CAD
coordinates, transforms, triangles or CAD provenance are modified.
"""
import argparse,copy,json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];ASSETS=ROOT/'web/platform/assets'
parser=argparse.ArgumentParser(description='Update only live debug surfaces and semantic controls from verified LilyGO runtime profiles; preserve source CAD geometry and provenance.')
parser.add_argument('--profile-dir',type=Path,default=ROOT/'docs/platform/research')
args=parser.parse_args()
patchfile=ROOT/'docs/platform/research/lilygo-model-catalog-patches-2026-11-22.json';patches=json.loads(patchfile.read_text());patchby={r['id']:r for r in patches['items']};profiles={};sources={};blockers={}
for file in sorted(args.profile_dir.glob('lilygo-*profiles*.json')):
 data=json.loads(file.read_text())
 if not isinstance(data.get('items'),list):continue
 for item in data['items']:
  if not isinstance(item.get('productId'),str)or not item.get('boardId'):continue
  previous=profiles.get(item['productId'])
  if previous and previous.get('boardId')!=item['boardId']:raise ValueError('Conflicting board revisions for '+item['productId'])
  profiles[item['productId']]=item
 sources.update({s['id']:s for s in data.get('sources',[])})
 for item in data.get('deferred',[]):blockers[item['productId']]=item
for file in sorted(args.profile_dir.glob('lilygo-*-blockers-*.json')):
 data=json.loads(file.read_text());sources.update({s['id']:s for s in data.get('sources',[])})
 for item in data.get('items',[]):blockers[item['productId']]=item
if not profiles:raise ValueError('No supported productId/boardId runtime profiles found')
runtime_count=len(profiles)
for id,item in blockers.items():
 if id in profiles:continue
 profile=copy.deepcopy(item);profile.setdefault('controls',[]);profile.setdefault('sourceIds',[])
 if profile.get('displayEvidence',{}).get('present') is False:profile['physicalDisplay']=False
 display=profile.get('physicalDisplay')if isinstance(profile.get('physicalDisplay'),dict)else profile.get('displayEvidence')
 if display and display.get('width')and display.get('height'):profile['display']={**display,'implemented':False}
 profiles[id]=profile
for f in ASSETS.glob('lilygo-*/model-plan.json'):
 plan=json.loads(f.read_text());id=plan['id']
 if plan.get('generator')!='build-lilygo-expansion-models.py':continue
 cad_snapshot=json.dumps([plan.get('cad'),plan.get('additionalCad'),[s for s in plan['provenance']['sources']if s.get('kind')=='manufacturer-cad']],sort_keys=True)
 plan['provenance']['sources']=[s if s.get('kind')=='manufacturer-cad' else sources.get(s['id'],s)for s in plan['provenance']['sources']]
 profile=profiles.get(id)
 if not profile:f.write_text(json.dumps(plan,ensure_ascii=False,indent=2)+'\n');continue
 existing={s['id']for s in plan['provenance']['sources']};plan['provenance']['sources'].extend(sources[s].copy()for s in profile['sourceIds']if s in sources and s not in existing)
 patch=patchby[id];patch['sourceIds']=list(dict.fromkeys(patch['sourceIds']+profile['sourceIds']))
 display=profile.get('display')
 if not display and isinstance(profile.get('physicalDisplay'),dict):display={**profile['physicalDisplay'],'implemented':False}
 bw,bh,depth=[plan['dimensions'][k]for k in ['width','height','depth']]
 if profile.get('physicalDisplay') is False:
  # An explicit, source-backed no-screen correction overrides a mistaken
  # storefront/wiki hint. A missing runtime display alone does not mean no LCD.
  plan['presentation'].pop('screen',None);plan.pop('displayNativeOrientation',None);patch.pop('displayHint',None)
  plan['parts']=[p for p in plan['parts']if p['name']!='debug-screen-frame'];plan['derivedParts']=[p for p in plan.get('derivedParts',[])if p['id']!='debug-panel-label']
  plan['debugPreview']['physicalDisplay']=False;plan['debugPreview']['liveFramebuffer']=False
  plan['debugPreview']['displayPlacement']='no-physical-display'
  display=None
 if display:
  ratio=display['width']/display['height'];old=plan.get('presentation',{}).get('screen');wrong=not old or abs(old['sizeMm'][0]/old['sizeMm'][1]-ratio)>1e-7
  if wrong:
   sw=45*display['width']/max(display['width'],display['height']);sh=sw/ratio;sx=bw/2+8+sw/2
   plan['parts']=[p for p in plan['parts']if p['name']!='debug-screen-frame'];plan['derivedParts']=[p for p in plan.get('derivedParts',[])if p['id']!='debug-panel-label']
   plan['parts'].append({'name':'debug-screen-frame','role':'component','kind':'ring','sizeMm':[sw+3,sh+8,1.4],'openingMm':[sw,sh],'radiusMm':2,'innerRadiusMm':.2,'centerMm':[sx,0,0],'color':'#20343d','basis':'debug-layout'})
   plan['presentation']['screen']={'sizeMm':[sw,sh],'positionMm':[sx,1,.72],'rotation':[0,0,0],'notes':'Separate native-orientation debug panel, not an uncalibrated physical window.'};plan['derivedParts'].append({'id':'debug-panel-label','kind':'printed-marking','basis':'debug-layout','sourceIds':profile['sourceIds'],'lines':['LIVE DEBUG'],'color':'#bde5d8','geometry':{'sizeMm':[sw,3.2],'centerMm':[sx,-sh/2-2,.72]}});plan['presentation']['bodySizeMm']=[bw+sw+12,max(bh,sh+8),max(depth,1.4)]
  implemented=display.get('implemented') is not False
  patch['displayHint']={'width':display['width'],'height':display['height'],'sourceIds':profile['sourceIds'],'basis':'exact selected BSP/controller native orientation'if implemented else'verified physical panel; runtime frame output not implemented','physicalSizeMm':None,'implemented':implemented}
  plan['displayNativeOrientation']={'width':display['width'],'height':display['height'],'sourceIds':profile['sourceIds'],'implemented':implemented}
  plan['debugPreview']['liveFramebuffer']=implemented
  plan['debugPreview']['physicalDisplay']=True;plan['debugPreview']['displayPlacement']='separate-native-orientation-debug-panel'
  for part in plan.get('derivedParts',[]):
   if part['id']=='debug-panel-label':part['lines']=['LIVE DEBUG'if implemented else'DISPLAY PREVIEW']
  if implemented:
   if plan['presentation']['screen'].get('notes')=='Physical display preview only; the selected runtime has no implemented framebuffer transport.':
    plan['presentation']['screen']['notes']='Source-backed live framebuffer on a separate native-orientation debug panel; physical window placement is not implied.'
   plan['limitations']=[note for note in plan['limitations']if note!='DISPLAY PREVIEW 仅表示已核实的实体屏幕尺寸比例；此板运行时尚未实现显示总线和帧输出。']
  else:
   plan['presentation']['screen']['notes']='Physical display preview only; the selected runtime has no implemented framebuffer transport.'
   if not any('DISPLAY PREVIEW' in n for n in plan['limitations']):plan['limitations'].append('DISPLAY PREVIEW 仅表示已核实的实体屏幕尺寸比例；此板运行时尚未实现显示总线和帧输出。')
 # Explicit debugging controls provide semantic click surfaces without claiming
 # unverified physical button placement. Physical CAD/photographic matches win.
 plan['parts']=[p for p in plan['parts']if not p['name'].startswith('debug-control:')];plan['derivedParts']=[p for p in plan.get('derivedParts',[])if not p['id'].startswith('debug-control-label:')]
 mapped={p.get('controlId')for p in plan['parts']+plan.get('cad',{}).get('groups',[])}|{p.get('id')for p in plan['presentation'].get('buttons',[])}
 controls=[c for c in profile['controls']if c['type']in ['button','matrix-key']and c['id']not in mapped]
 if controls:
  matrix=[c for c in controls if c['type']=='matrix-key'];simple=[c for c in controls if c['type']=='button'];basey=-max(bh/2,plan['presentation'].get('screen',{}).get('sizeMm',[0,0])[1]/2)-8;centerx=plan['presentation'].get('screen',{}).get('positionMm',[0,0,0])[0];rows=[]
  for i,c in enumerate(simple):rows.append((c,centerx+(i-(len(simple)-1)/2)*13,basey,11,4,c.get('name',c['id']).split(' /')[0][:12]))
  for c in matrix:rows.append((c,centerx+(c['column']-4.5)*6,basey-8-c['row']*6,5.3,4.5,c['name']if len(c['name'])<=2 else 'R'+str(c['row']+1)+'C'+str(c['column']+1)))
  for c,x,y,w,h,label in rows:
   plan['parts'].append({'name':'debug-control:'+c['id'],'role':'component','kind':'rounded-box','sizeMm':[w,h,1.3],'centerMm':[x,y,0],'radiusMm':.5,'color':'#24434c','controlId':c['id'],'basis':'separate debug control; not a physical part','sourceIds':profile['sourceIds']});plan['derivedParts'].append({'id':'debug-control-label:'+c['id'],'kind':'printed-marking','basis':'debug-layout','sourceIds':profile['sourceIds'],'lines':[label],'color':'#dcf1e9','geometry':{'sizeMm':[w*.92,h*.8],'centerMm':[x,y,.67]}})
  maxy=max(abs(y)+h/2 for _,x,y,w,h,_ in rows);maxx=max(abs(x)+w/2 for _,x,y,w,h,_ in rows);oldsize=plan['presentation']['bodySizeMm'];plan['presentation']['bodySizeMm']=[max(oldsize[0],maxx*2),max(oldsize[1],maxy*2),max(depth,1.4)];plan['debugPreview']['controlPlacement']='Separate labeled debugging controls where physical locations are uncalibrated; official physical matches are preserved.'
  caption_y=basey+4;plan['derivedParts'].append({'id':'debug-control-label:heading','kind':'printed-marking','basis':'debug-layout','sourceIds':profile['sourceIds'],'lines':['DEBUG INPUT'],'color':'#49656d','geometry':{'sizeMm':[max(20,len(simple)*13),3],'centerMm':[centerx,caption_y,.67]}})
  span=max(plan['presentation']['bodySizeMm'][:2])*1.2
  for view in plan.get('reviewViews',{}).values():view['span']=span
  if not any('DEBUG INPUT' in n for n in plan['limitations']):plan['limitations'].append('标记 DEBUG INPUT 的按键是独立调试控件，不表示已核实的实物按键位置。')
 assert json.dumps([plan.get('cad'),plan.get('additionalCad'),[s for s in plan['provenance']['sources']if s.get('kind')=='manufacturer-cad']],sort_keys=True)==cad_snapshot,'Runtime hints must not change CAD geometry or provenance'
 f.write_text(json.dumps(plan,ensure_ascii=False,indent=2)+'\n')
patchfile.write_text(json.dumps(patches,ensure_ascii=False,indent=2)+'\n');print('Refreshed native display/control hints for',runtime_count,'runtime profiles and',len(profiles)-runtime_count,'model-only evidence records')
