#!/usr/bin/env python3
"""Check actual production-loader meshes, render views, then discard redundant dumps."""
import argparse,hashlib,json,shutil,subprocess
from pathlib import Path
MODELS=['waveshare-esp32-s3-touch-lcd-5','waveshare-esp32-s3-lcd-5','waveshare-esp32-s3-touch-lcd-5b','waveshare-esp32-s3-lcd-5b','waveshare-esp32-s3-touch-lcd-4-3b','waveshare-esp32-s3-touch-lcd-4','waveshare-esp32-s3-lcd-4']
def main():
 a=argparse.ArgumentParser(description=__doc__);a.add_argument('--output',required=True,type=Path);a.add_argument('--models',nargs='+',choices=MODELS,default=MODELS);a.add_argument('--expanded',action='store_true');args=a.parse_args();rows=[]
 for model in args.models:
  out=args.output/model;mesh=out/'temporary-mesh';out.mkdir(parents=True,exist_ok=True)
  commands=[['node','tools/check-waveshare-model-loading.mjs',model,'--output',str(out/'loader.json'),'--dump',str(mesh)],['python','tools/review-waveshare-models.py','--meshes',str(mesh),'--output',str(out)]]
  if args.expanded:commands[0].append('--expanded')
  for i,cmd in enumerate(commands):
   with(out/f'check-{i}.log').open('w')as log:r=subprocess.run(cmd,stdout=log,stderr=subprocess.STDOUT)
   rows.append(dict(model=model,stage=i,exit=r.returncode,command=cmd));(args.output/'status.json').write_text(json.dumps(rows,indent=2)+'\n');print(model,i,r.returncode,flush=True)
   if r.returncode:raise SystemExit(r.returncode)
  hashes=[dict(path=str(p.relative_to(mesh)),bytes=p.stat().st_size,sha256=hashlib.file_digest(p.open('rb'),'sha256').hexdigest())for p in mesh.rglob('*')if p.is_file()];(out/'transient-mesh-hashes.json').write_text(json.dumps(hashes,indent=2)+'\n');shutil.rmtree(mesh)
if __name__=='__main__':main()
