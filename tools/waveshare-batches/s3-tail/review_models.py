"""Serial production-loader review with bounded temporary mesh storage."""
from pathlib import Path
import argparse,json,hashlib,subprocess,sys,tempfile,time
ap=argparse.ArgumentParser();ap.add_argument('--evidence',type=Path,required=True);ap.add_argument('--scratch',type=Path,required=True);args=ap.parse_args();ROOT=Path(__file__).resolve().parents[3];E=args.evidence.resolve();E.mkdir(parents=True,exist_ok=True);D=args.scratch.resolve();D.mkdir(parents=True,exist_ok=True)
ids=['waveshare-esp32-s3-touch-lcd-2-8-v1','waveshare-esp32-s3-touch-lcd-2-8-v2','waveshare-esp32-s3-lcd-2-8','waveshare-esp32-s3-touch-lcd-1-69','waveshare-esp32-s3-touch-lcd-7','waveshare-esp32-s3-lcd-7','waveshare-esp32-s3-touch-lcd-7-external-antenna','waveshare-esp32-s3-touch-lcd-1-28-b','waveshare-esp32-s3-matrix'];aliases={ids[1]:ids[0],ids[2]:ids[0],ids[6]:ids[4]};results=[];receipts=[]
for pid in ids:
 p=ROOT/'web/platform/assets'/pid/'model.json'
 for _ in range(120):
  if p.exists():break
  time.sleep(5)
 assert p.exists(),pid
 out=E/(pid+'-loader.json');cmd=['node','--max-old-space-size=3072','tools/check-waveshare-model-loading.mjs','--output',str(out)]
 if pid not in aliases:cmd+=['--dump',str(D)]
 subprocess.run(cmd+[pid],cwd=ROOT,check=True,timeout=240);record=json.loads(out.read_text());results.extend(record['results']);row={'id':pid,'modelSha256':hashlib.sha256(p.read_bytes()).hexdigest(),'loaderResult':out.name}
 if pid in aliases:
  base=ROOT/'web/platform/assets'/aliases[pid]/'geometry.bin.gz';ours=p.parent/'geometry.bin.gz';assert ours.read_bytes()==base.read_bytes();row.update(geometryEquivalentTo=aliases[pid],visualReview=aliases[pid]+'-four-views.png',scope='Identical nativegeometry bytes andindependent production-loader validation. SKU/revision differences remain unverified as stated in product/model limitations.')
 else:
  mesh=D/(pid+'.json');row['meshDump']={'bytes':mesh.stat().st_size,'sha256':hashlib.sha256(mesh.read_bytes()).hexdigest()};subprocess.run([sys.executable,'tools/review-waveshare-models.py','--meshes',str(D),'--output',str(E)],cwd=ROOT,check=True,timeout=600);mesh.unlink();row['visualReview']=pid+'-four-views.png';row['scope']='Actual loader mesh geometry-only orthographic fourviews. NoWebGL, hardwarefit or originalfactory framebuffer claim.'
 receipts.append(row);(E/'model-review-receipts.json').write_text(json.dumps(receipts,indent=2)+'\n');out.unlink();print(pid,'reviewed',flush=True)
(E/'model-loader-check.json').write_text(json.dumps({'scope':'Actual production Three.js CPUgeometry/rays/sourcechecks; noWebGL orhardware acceptance. Exactgeometryaliases are recorded independently.','results':results},indent=2)+'\n')
