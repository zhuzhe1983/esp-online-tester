#!/usr/bin/env python3
"""Independent lane01 extra source/model delivery; never rewrites lane03 submission."""
import argparse,importlib.util,json,shutil
from pathlib import Path
spec=importlib.util.spec_from_file_location('source_onboarding',Path(__file__).with_name('onboard-lane-01.py'));o=importlib.util.module_from_spec(spec);spec.loader.exec_module(o)
o.TEMP=Path('output/source/lane01-extra');PRIORITIES=[13,20,26,32,38,44,50,56,62,68,74,80]
def main():
 p=argparse.ArgumentParser();p.add_argument('command',choices=['prepare','build','verify','publish']);p.add_argument('--ids',nargs='*');a=p.parse_args();items=[x for x in json.loads((o.RUN/'queue.json').read_text())['products']if x['priority']in PRIORITIES]
 for item in items:
  id='m5stack-'+item['directory'].split('_')[0].lower()
  if a.ids and id not in a.ids:continue
  if a.command=='prepare':
   old=Path('output/source/lane03')/id
   if old.exists() and not(o.TEMP/id).exists():shutil.copytree(old,o.TEMP/id)
   o.prepare(item)
  elif a.command=='build':
   prepared=json.loads((o.TEMP/id/'prepared.json').read_text());assets=Path('web/platform/assets')/id;plan=json.loads((assets/'model-plan.json').read_text());o.build_model(prepared,plan,assets);o.build_knowledge(prepared,plan);print(id+' built',flush=True)
  elif a.command=='verify':o.verify_product(id)
  else:
   o.publish_product(id,o.RUN/'submissions/lane-01-extra.json',PRIORITIES)
   path=Path('docs/platform/models')/(id+'.md');path.write_text(path.read_text().replace('onboard-lane-01.py','onboard-lane-01-extra.py'))
   submission=o.RUN/'submissions/lane-01-extra.json';data=json.loads(submission.read_text());data['lane']='01-extra';o.write(submission,data)
if __name__=='__main__':main()
