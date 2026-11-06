#!/usr/bin/env python3
"""Seat the two K141 left-side bus inserts in the unchanged manufacturer cut-outs.

Default is read-only measurement; --apply updates the declarative model and plan.
This does not infer hidden connector dimensions or add electrical port support.
"""
import argparse,copy,hashlib,json
from pathlib import Path
import numpy as np
import trimesh
ROOT=Path(__file__).resolve().parents[1]
ASSETS=ROOT/'web/platform/assets/m5stack-k141'
GEOMETRY_SHA='8eada24face950933c7ee7d2d649a1bcc2caf48c74402dbde4e583ad48565e31'
def sha(b):return hashlib.sha256(b).hexdigest()
def dump(path,value):path.write_text(json.dumps(value,ensure_ascii=False,indent=2)+'\n')
def measure(meta,binary):
 sections={}
 for name in ['shell-front','shell-back']:
  item=next(p for p in meta['parts']if p['name']==name);v=np.frombuffer(binary,dtype='<f4',offset=item['byteOffset'],count=item['vertexCount']*6).reshape(-1,6)[:,:3];mesh=trimesh.Trimesh(v,np.arange(len(v)).reshape(-1,3));sections[name]=mesh.section(plane_origin=[-35.999,0,0],plane_normal=[1,0,0]).discrete
 front=sections['shell-front'];back=sections['shell-back'];upper=next(p for p in front if p[:,2].min()>2 and p[:,1].min()>3 and p[:,1].max()<21);outer=max(front,key=lambda p:np.ptp(p[:,1]));rear=max(back,key=lambda p:np.ptp(p[:,1]));lower=next(p for p in back if 8<p[:,1].min()<10 and 14<p[:,1].max()<16)
 y485=float((upper[:,1].min()+upper[:,1].max())/2);floor=float(upper[:,2].min());ceiling=float(np.median(upper[(upper[:,2]>9.7)&(upper[:,2]<10.1),2]));tabtop=float(upper[:,2].max())
 can=outer[(outer[:,1]>4)&(outer[:,1]<20)&(outer[:,2]>-6)&(outer[:,2]<-.9)];can_ceiling=float(np.median(can[(can[:,2]>-1.3)&(can[:,2]<-1.2),2]));can_y=[float(can[:,1].min()),float(can[:,1].max())];seam=float(rear[:,2].max());switch_top=float(outer[(outer[:,1]>8)&(outer[:,1]<16)&(outer[:,2]<1),2].max())
 return {'sectionXmm':-35.999,'pwr485':{'yBoundsMm':[float(upper[:,1].min()),float(upper[:,1].max())],'mainZBoundsMm':[floor,ceiling],'centerYZmm':[round(y485,4),round((floor+ceiling)/2,4)],'keySlotsTopZmm':tabtop},'pwrCan':{'yBoundsMm':can_y,'mainZBoundsMm':[seam,can_ceiling],'centerYZmm':[round(sum(can_y)/2,4),round((seam+can_ceiling)/2,4)]},'upperSwitch':{'centerYZmm':[round(sum(can_y)/2,4),round((switch_top+can_ceiling)/2,4)]},'lowerSwitch':{'centerYZmm':[round(float((lower[:,1].min()+lower[:,1].max())/2),4),round(float((lower[:,2].min()+lower[:,2].max())/2),4)]}}
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--apply',action='store_true');args=parser.parse_args();plan_path=ASSETS/'model-plan.json';model_path=ASSETS/'model.json';plan=json.loads(plan_path.read_text());meta=json.loads(model_path.read_text());binary=(ASSETS/'geometry.bin').read_bytes();assert sha(binary)==GEOMETRY_SHA,'Native manufacturer geometry changed; re-audit the cut-outs'
 cad=measure(meta,binary);by_id={p['id']:p for p in plan['derivedParts']};before={p['id']:copy.deepcopy(p['geometry'])for p in plan['derivedParts']if p['id'].startswith(('pwr-485','pwr-can','bus-switch'))};changes=[]
 groups=[('pwr-485',[p for p in by_id if p.startswith('pwr-485')],cad['pwr485']['centerYZmm']),('pwr-can',[p for p in by_id if p.startswith('pwr-can')],cad['pwrCan']['centerYZmm'])]
 for i,name in enumerate(['upperSwitch','lowerSwitch']):groups.append((f'bus-switch-{i}',[f'bus-switch-{i}',f'bus-switch-slider-{i}'],cad[name]['centerYZmm']))
 for base,ids,yz in groups:
  center=by_id[base]['geometry']['centerMm'];target=[round(yz[0],2),round(yz[1],2)];delta=[0,target[0]-center[1],target[1]-center[2]]
  for id in ids:
   p=by_id[id];p['geometry']['centerMm']=[round(v+d,8)for v,d in zip(p['geometry']['centerMm'],delta)]
   if 'Left insert alignment' not in p['notes']:p['notes']+=' Left insert alignment: the unchanged native shell cut-out sets Y/Z; all contacts, collars and rear walls follow the same insert translation. X mouth/depth and hidden-dimension estimates are retained.'
  changes.append({'assembly':base,'deltaMm':delta,'targetCenterYZmm':target,'members':ids})
 # Key tabs fit their independent native notches; their bottom touches the newly
 # seated orange housing, rather than floating above it after a rigid Z move.
 housing=by_id['pwr-485']['geometry'];top=housing['centerMm'][2]+max(p[1]for p in housing['outlineMm'])
 for id,y in [('pwr-485-key-0',15.9),('pwr-485-key-1',8.1)]:
  key=by_id[id]['geometry'];key['centerMm'][1]=y;key['centerMm'][2]=round(top+key['sizeMm'][2]/2,8)
 marker='Left insert correction seats PWR485/PWR-CAN and both bus switches at native cut-out Y=12mm; carrier bodies and contacts move together. No native mesh, paper, screen, front Grove anchor or external envelope changes.'
 if marker not in plan['assemblyNotes']:plan['assemblyNotes']+=' '+marker
 meta['derivedParts']=copy.deepcopy(plan['derivedParts']);meta['sourceTransform']['note']=plan['assemblyNotes']
 report={'productId':'m5stack-k141','originalModelSha256':sha(model_path.read_bytes()),'geometrySha256':sha(binary),'nativeCutouts':cad,'assemblies':changes,'before':before,'after':{id:by_id[id]['geometry']for id in before},'unchanged':['manufacturer triangle positions/normals','side print and screen','insert X mouth planes/depths','Grove port-a/port-c connection anchors'],'notes':'Native cut-outs locate the visible inserts. Concealed dimensions remain photo estimates; PWR485/PWR-CAN are visual-only and have no runtime cable anchor. Mouth frames are measured from the actual rendered geometry in the companion browser test.'}
 if args.apply:dump(plan_path,plan);dump(model_path,meta)
 print(json.dumps(report,ensure_ascii=False,indent=2))
if __name__=='__main__':main()
