#!/usr/bin/env python3
"""Integrate explicitly reviewed product submissions; preserve verified runtime overrides."""
import argparse, datetime, hashlib, json, pathlib, re
p=argparse.ArgumentParser();p.add_argument('submission',type=pathlib.Path);p.add_argument('--review',type=pathlib.Path);p.add_argument('--queue',type=pathlib.Path,required=True);a=p.parse_args()
read=lambda p:json.loads(pathlib.Path(p).read_text())
def write(p,data):pathlib.Path(p).write_text(json.dumps(data,ensure_ascii=False,indent=2)+'\n')
s=read(a.submission);review=read(a.review) if a.review else s;metadata_only=set(review.get('metadataOnlyReviewedIds',[]));ids=set(review.get('reviewedIds',[]))|metadata_only;assert ids,'An explicit reviewed IDs list is required'
expected={item['id']:item.get('modelSha256') for item in review.get('items',[]) if 'id' in item}
products=read('web/platform/catalog/products.json');firmwares=read('web/platform/catalog/firmwares.json');index=read('web/platform/knowledge/index.json')
byid={x['id']:x for x in products['items']};fw={x['id']:x for x in firmwares['items']};entries={x['id']:x for x in index['entries']}
for item in s['products']:
 if item['id'] not in ids:continue
 pid=item['id'];knowledge=read('web/platform/knowledge/'+pid+'.json')
 if pid in metadata_only:assert not item.get('model') and item['runtime']['board'] is None,pid+' metadata-only entry must not imply a model/runtime'
 else:
  modelpath=pathlib.Path('web/platform')/item['model']['url'];model=read(modelpath)
  if expected.get(pid):assert hashlib.sha256(modelpath.read_bytes()).hexdigest()==expected[pid],pid+' changed after review'
  for name,sha in model.get('artifacts',{}).items():assert hashlib.sha256((modelpath.parent/name).read_bytes()).hexdigest()==sha,(pid,name)
 old=byid.get(pid)
 if old and old.get('runtime',{}).get('board'):
  item['runtime']=old['runtime'];item['limitations']=old['limitations'];item['defaultFirmwareId']=old.get('defaultFirmwareId')
  if 'inputDevices' in old:item['inputDevices']=old['inputDevices']
 byid[pid]=item;entries[pid]={'id':pid,'kind':'product','catalogId':pid,'name':item['name'],'url':'./'+pid+'.json','updatedAt':knowledge['updatedAt']}
for item in s.get('firmwares',s.get('firmwareReferences',[])):
 if not set(item.get('productIds',[]))&ids:continue
 if fw.get(item['id'],{}).get('source') and not item.get('source'):continue
 fw[item['id']]=item
for pid in ids:
 item=byid[pid];candidates=[f for f in fw.values() if pid in f.get('productIds',[]) and f.get('role')=='factory']
 if item.get('defaultFirmwareId') not in fw and candidates:item['defaultFirmwareId']=candidates[0]['id']
# One official factory entry per identical source reference; prefer a runnable pinned entry.
for pid in ids:
 candidates=[f for f in fw.values() if pid in f.get('productIds',[]) and f.get('role')=='factory']
 if len(candidates)>1:
  chosen=next((f for f in candidates if f.get('source')),next((f for f in candidates if f['id'].endswith('-factory')),candidates[0]))
  byid[pid]['defaultFirmwareId']=chosen['id']
  for f in candidates:
   if f is not chosen and len(f['productIds'])==1 and not f.get('source'):fw.pop(f['id'])
for item in byid.values():
 if 'firmwareReferenceIds' in item:item['firmwareReferenceIds']=[f['id'] for f in fw.values() if item['id'] in f.get('productIds',[])]
rank={}
for item in read(a.queue)['products']:
 sku=item['directory'].split('_')[0];pid='m5stack-'+re.sub('[^a-z0-9-]+','-',sku.lower());rank[pid]=item['priority']
rank['m5stack-papermono']=rank.get('m5stack-c153',999);rank['m5stack-stopwatch']=rank.get('m5stack-c152',999)
products['items']=sorted(byid.values(),key=lambda x:rank.get(x['id'],999));firmwares['items']=list(fw.values());index['entries']=list(entries.values());index['updatedAt']=datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00','Z')
for path,obj in [('web/platform/catalog/products.json',products),('web/platform/catalog/firmwares.json',firmwares),('web/platform/knowledge/index.json',index)]:write(path,obj)
print(json.dumps({'integratedReviewed':len(ids),'products':len(products['items']),'firmwares':len(firmwares['items'])}))
