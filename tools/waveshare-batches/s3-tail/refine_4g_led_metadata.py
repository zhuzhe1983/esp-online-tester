"""Correct the LED semantic key without changing already verified native geometry."""
from pathlib import Path
import json,hashlib
ROOT=Path(__file__).resolve().parents[3]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save(p,d):p.write_text(json.dumps(d,ensure_ascii=False,indent=2)+'\n')
for modem in ['a7670e','sim7670g']:
 d=ROOT/'web/platform/assets'/('waveshare-esp32-s3-'+modem+'-4g')
 pp=d/'model-plan.json';plan=json.loads(pp.read_text());mp=d/'model.json';model=json.loads(mp.read_text());geometry_before=sha(d/'geometry.bin.gz')
 for parts in [plan['parts'],model['parts']]:
  led=next(x for x in parts if x['name']=='rgb-led');assert led['sourceComponents']==[410]
  led.pop('rgbLedIndex',None);led['ledIndex']=0
 save(pp,plan);model['artifacts']['model-plan.json']=sha(pp);save(mp,model)
 b=d/'build-check.json';check=json.loads(b.read_text());check['modelSha256']=sha(mp)
 check['metadataRefinement']={'scope':'LED semantic key changed to production-loader ledIndex; no coordinates, source selection or geometry bytes changed.',
  'script':'tools/waveshare-batches/s3-tail/refine_4g_led_metadata.py','geometrySha256Unchanged':geometry_before}
 assert sha(d/'geometry.bin.gz')==geometry_before;save(b,check)
 print(model['id'],'native geometry unchanged; semantic index0')
