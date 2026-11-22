#!/usr/bin/env python3
"""Verify retained Seeed assets and the visible synthetic framebuffer plane.

This is geometry/data validation, not a device firmware or mechanical-fit test.
"""
from pathlib import Path
import hashlib,json,math,datetime,argparse,re
import numpy as np
ROOT=Path(__file__).resolve().parents[4]
ASSETS=ROOT/'web/platform/assets'

def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def rotation(v):
 x,y,z=v
 return np.array([[1,0,0],[0,math.cos(x),-math.sin(x)],[0,math.sin(x),math.cos(x)]])@np.array([[math.cos(y),0,math.sin(y)],[0,1,0],[-math.sin(y),0,math.cos(y)]])@np.array([[math.cos(z),-math.sin(z),0],[math.sin(z),math.cos(z),0],[0,0,1]])

def source_file(p,source):
 owner=source.get('sharedAssetProductId',p.name);name=source['file']
 assert isinstance(owner,str) and re.fullmatch(r'seeed-[a-z0-9]+(?:-[a-z0-9]+)*',owner)
 assert not source.get('sharedAssetProductId') or owner!=p.name
 assert isinstance(name,str) and Path(name).name==name and '\\' not in name and name not in ('.','..')
 target=ASSETS/owner/name
 assert target.parent.resolve().parent==ASSETS.resolve() and target.resolve().parent==target.parent.resolve()
 return target

def verify(p,review_dirs=None):
 model=json.loads((p/'model.json').read_text());plan=json.loads((p/'model-plan.json').read_text());build=json.loads((p/'build-check.json').read_text())
 names=[part['name'] for part in model['parts']]+[part['id'] for part in model.get('derivedParts',[])]
 assert len(names)==len(set(names)),(p.name,'duplicate part names')
 source_ids=[source['id'] for source in plan['sources']]
 assert len(source_ids)==len(set(source_ids)),(p.name,'duplicate source IDs')
 for group in model['presentation'].get('inspectionGroups',[]):
  assert 0<len(group['parts'])<=128 and len(group['offsetMm'])==3 and any(group['offsetMm']),(p.name,group['id'],'invalid movement group')
  assert all(math.isfinite(v) and abs(v)<=5000 for v in group['offsetMm'])
  assert all(name in names+['live-display'] for name in group['parts']),(p.name,group['id'])
  assert group.get('sourceIds') and all(s in source_ids for s in group['sourceIds'])
 for source in plan['sources']:
  if source.get('file'):
   source_file(p,source)
 for part in model['parts']+model.get('derivedParts',[]):
  assert all(source_id in source_ids for source_id in part.get('sourceIds',[])),(p.name,part)
 a=np.fromfile(p/'geometry.bin',dtype='<f4').reshape(-1,6)
 assert np.isfinite(a).all()
 assert len(a)%3==0
 assert build['modelSha256']==sha(p/'model.json')
 assert build['geometrySha256']==sha(p/'geometry.bin')
 assert np.allclose(np.linalg.norm(a[:,3:],axis=1),1,atol=.0001)
 matrix=np.array(plan['matrixRows']);R=matrix[:3,:3]
 assert np.allclose(R.T@R,np.eye(3)) and np.isclose(np.linalg.det(R),1)
 cursor=0
 for part in model['parts']:
  assert part['byteOffset']==cursor
  values=a[cursor//24:cursor//24+part['vertexCount']]
  assert np.allclose([values[:,:3].min(0),values[:,:3].max(0)],part['boundsMm'])
  cursor+=part['vertexCount']*24
 assert cursor==(p/'geometry.bin').stat().st_size
 for name,digest in model['artifacts'].items():assert sha(p/name)==digest,(p.name,name)
 for src in plan['sources']:
  if src.get('file'):assert sha(source_file(p,src))==src.get('retained',src)['sha256'],(p.name,src['id'],'source hash')
 for name,digest in model.get('sharedArtifacts',{}).items():
  owner,file=name.split('/');assert sha(source_file(p,{'sharedAssetProductId':owner,'file':file}))==digest,(p.name,name)
 # Orthographic rays toward +Z at inner framebuffer samples; displayed plane
 # is opaque and must not be hidden behind source front geometry.
 triangles=a[:,:3].reshape(-1,3,3).astype(float)
 x0,y0,z0=triangles[:,0].T;x1,y1,z1=triangles[:,1].T;x2,y2,z2=triangles[:,2].T
 det=(y1-y2)*(x0-x2)+(x2-x1)*(y0-y2);valid=abs(det)>1e-10;safe=np.where(valid,det,1)
 samples=[];screen=model['presentation'].get('screen')
 if screen:
  screen=model['presentation']['screen'];screenR=rotation(screen.get('rotation',[0,0,0]));w,h=screen['sizeMm'];samples=[]
  for u in np.linspace(-.46,.46,5):
   for v in np.linspace(-.46,.46,5):
    if screen.get('shape')=='circle' and u*u+v*v>.46**2:continue
    point=np.array([u*w,v*h,0])@screenR.T+screen['positionMm'];x,y,z=point
    b0=((y1-y2)*(x-x2)+(x2-x1)*(y-y2))/safe;b1=((y2-y0)*(x-x2)+(x0-x2)*(y-y2))/safe;b2=1-b0-b1
    hit=valid&(b0>=-1e-7)&(b1>=-1e-7)&(b2>=-1e-7);depth=b0*z0+b1*z1+b2*z2
    front=float(depth[hit].max()) if hit.any() else None
    samples.append(dict(uv=[float(u),float(v)],planeZMm=float(z),sourceFrontZMm=front,occluded=bool(front is not None and front>z+.001)))
 assert not any(s['occluded'] for s in samples),(p.name,[s for s in samples if s['occluded']])
 review=next((d/(p.name+'-review.png') for d in (review_dirs or [Path(__file__).parent]) if (d/(p.name+'-review.png')).exists()),None)
 assert review is not None,(p.name,'missing software review')
 render=review.with_name(p.name+'-render.json')
 if render.exists():
  render_data=json.loads(render.read_text());assert render_data['modelSha256']==sha(p/'model.json') and render_data['reviewSha256']==sha(review)
  assert len(render_data['softwareViews'])==3 and not any(v['clipped'] for v in render_data['softwareViews'])
 return dict(productId=p.name,method=plan['method'],parts=len(model['parts']),triangles=len(a)//3,geometryBytes=cursor,modelSha256=sha(p/'model.json'),geometrySha256=sha(p/'geometry.bin'),allSourceAndArtifactHashes='pass',rigidTransform='pass',geometryContract='pass',framebufferMode='present' if screen else 'absent-no-integrated-display',framebufferFrontSamples=samples,renderReviewFile=str(review.resolve().relative_to(ROOT)),renderReviewSha256=sha(review),limitations=dict(webglBrowser='separate integration check',firmware='not tested by this script',photoOverlay='not calibrated',hardwareFit='not tested'))

def verify_catalog(path,ids=None):
 catalog=json.loads(path.read_text());products={p['id']:p for p in catalog['items']};reports=[]
 for p in sorted(ASSETS.glob('seeed-*/model.json')):
  if ids and p.parent.name not in ids:continue
  model=json.loads(p.read_text());product=products[model['id']]
  assert product['model']['type']=='manufacturer-cad'
  assert product['model']['url'].removeprefix('./')=='assets/'+model['id']+'/model.json'
  controls={c['id']:c for c in product.get('controls',[])}
  bound=[part['controlId'] for part in model['parts'] if part.get('controlId')]
  bound += [button['id'] for button in model['presentation'].get('buttons',[])]
  assert all(key in controls and controls[key]['type']=='button' for key in bound),(model['id'],bound)
  screen=model['presentation'].get('screen');aspect_error=None
  if screen:
   assert product.get('display'),(model['id'],'screen without product display')
   ratio=screen['sizeMm'][0]/screen['sizeMm'][1];expected=product['display']['width']/product['display']['height'];aspect_error=ratio/expected-1
   assert abs(aspect_error)<.005,(model['id'],'screen aspect mismatch',aspect_error)
  else:assert product.get('display') is None,(model['id'],'product declares a missing screen')
  reports.append(dict(productId=model['id'],modelUrl=product['model']['url'],loaderContract='pass',bound3dButtonIds=bound,unboundCatalogButtonIds=[key for key,c in controls.items() if c['type']=='button' and key not in bound],screenPixelAspectRelativeError=aspect_error,displayMode='present' if screen else 'absent-no-integrated-display',modelMethod=model['provenance']['kind'],sourceFileCount=sum(bool(s.get('file')) for s in model['provenance']['sources'])))
 return dict(catalogSha256=sha(path),scope='Static metadata cross-check only; not a browser or firmware test',products=reports)

if __name__=='__main__':
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('ids',nargs='*');parser.add_argument('--catalog',type=Path);parser.add_argument('--output',type=Path,default=Path(__file__).parent/'checks.json');parser.add_argument('--review-dir',type=Path,action='append');args=parser.parse_args()
 directories=(args.review_dir or [])+[Path(__file__).parent]
 paths=[p.parent for p in sorted(ASSETS.glob('seeed-*/model-plan.json')) if not args.ids or p.parent.name in args.ids]
 if args.ids:assert len(paths)==len(set(args.ids)),'Missing requested model'
 reports=[verify(p,directories) for p in paths]
 out=dict(checkedAt=datetime.datetime.now(datetime.timezone.utc).isoformat().replace('+00:00','Z'),sourceOfChecks='Retained local/shared source files, native exports and synthetic display planes; no inherited product acceptance',devices=reports)
 if args.catalog:out['catalogIntegration']=verify_catalog(args.catalog,args.ids)
 args.output.parent.mkdir(parents=True,exist_ok=True);args.output.write_text(json.dumps(out,ensure_ascii=False,indent=2)+'\n')
 print(json.dumps(dict(devices=len(reports),geometryBytes=sum(r['geometryBytes'] for r in reports),status='pass')))
