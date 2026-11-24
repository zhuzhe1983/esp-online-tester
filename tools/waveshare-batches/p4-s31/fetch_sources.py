#!/usr/bin/env python3
"""Fetch exact official inputs with URL/time/size/hash receipts; never mirror firmware into Git."""
import argparse,datetime,hashlib,json,os,pathlib,tempfile,urllib.request
CACHE=pathlib.Path(os.environ.get('WAVESHARE_SOURCE_CACHE',str(pathlib.Path(tempfile.gettempdir())/'waveshare-source-cache')))
CACHE.mkdir(parents=True,exist_ok=True)
def utc():return datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00','Z')
def fetch(url,key):
 p=CACHE/key;p.parent.mkdir(parents=True,exist_ok=True);receipt=p.with_name(p.name+'.receipt.json')
 if p.is_file() and receipt.is_file():
  r=json.loads(receipt.read_text());assert hashlib.sha256(p.read_bytes()).hexdigest()==r['sha256'];return p,r
 requested=utc()
 try:
  with urllib.request.urlopen(urllib.request.Request(url,headers={'User-Agent':'Mozilla/5.0'}),timeout=60) as r:
   b=r.read();m={'url':url,'finalUrl':r.url,'requestedAt':requested,'fetchedAt':utc(),'status':'fetched','httpStatus':r.status,'mediaType':r.headers.get('Content-Type','application/octet-stream'),'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest()}
  p.write_bytes(b);receipt.write_text(json.dumps(m,indent=2)+'\n');return p,m
 except Exception as e:
  m={'url':url,'attemptedAt':utc(),'status':'failed','error':str(e)};receipt.write_text(json.dumps(m,indent=2)+'\n');raise
if __name__=='__main__':
 parser=argparse.ArgumentParser(description=__doc__)
 parser.add_argument('--cache',type=pathlib.Path,required=True)
 parser.add_argument('repositories',nargs='+')
 args=parser.parse_args();CACHE=args.cache;CACHE.mkdir(parents=True,exist_ok=True)
 for name in args.repositories:
  p,r=fetch('https://api.github.com/repos/waveshareteam/'+name+'/git/trees/main?recursive=1',name+'/tree.json')
  d=json.loads(p.read_text());print(name,d['sha'],len(d['tree']))
