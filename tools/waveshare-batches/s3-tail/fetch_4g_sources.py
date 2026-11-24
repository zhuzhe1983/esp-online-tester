"""Fetch caller-supplied manufacturer URLs; retain raw receipts and ZIP inventories."""
from pathlib import Path
import argparse,concurrent.futures,datetime,hashlib,json,urllib.request,urllib.parse,zipfile
ap=argparse.ArgumentParser();ap.add_argument('--urls',type=Path,required=True);ap.add_argument('--output',type=Path,required=True);a=ap.parse_args();a.output.mkdir(parents=True,exist_ok=True)
def sha(b):return hashlib.sha256(b).hexdigest()
def fetch(u):
 tail=Path(urllib.parse.urlsplit(u).path);name=(tail.parent.name+'-'+tail.name+'.html')if'docs.waveshare.com'in u else tail.name;p=a.output/name;r=p.with_name(p.name+'.receipt.json')
 if p.exists()and r.exists():
  old=json.loads(r.read_text())
  if sha(p.read_bytes())==old.get('sha256'):return old
 now=datetime.datetime.now(datetime.timezone.utc).isoformat();tmp=p.with_name(p.name+'.part')
 try:
  req=urllib.request.Request(u,headers={'User-Agent':'Mozilla/5.0'})
  with urllib.request.urlopen(req,timeout=60)as z, tmp.open('wb')as f:
   h=hashlib.sha256();n=0
   while True:
    b=z.read(1<<20)
    if not b:break
    n+=len(b)
    if n>300<<20:raise ValueError('300 MiB bounded download exceeded')
    h.update(b);f.write(b)
   row={'url':u,'resolvedUrl':z.geturl(),'requestedAt':now,'fetchedAt':datetime.datetime.now(datetime.timezone.utc).isoformat(),'mediaType':z.headers.get('Content-Type'),'httpStatus':z.status,'bytes':n,'sha256':h.hexdigest(),'file':p.name}
  tmp.replace(p);r.write_text(json.dumps(row,indent=2)+'\n');print(name,n,flush=True);return row
 except Exception as e:
  tmp.unlink(missing_ok=True);row={'url':u,'attemptedAt':now,'error':str(e)};print(name,str(e),flush=True);return row
urls=json.loads(a.urls.read_text())
with concurrent.futures.ThreadPoolExecutor(max_workers=3)as pool:results=list(pool.map(fetch,urls))
(a.output/'download-results.json').write_text(json.dumps(results,indent=2)+'\n')
for r in results:
 if'error'in r or not r['file'].lower().endswith('.zip'):continue
 p=a.output/r['file']
 with zipfile.ZipFile(p)as z:
  members=[{'path':x.filename,'bytes':x.file_size,'crc32':x.CRC}for x in z.infolist()];(a.output/(p.stem+'-inventory.json')).write_text(json.dumps({'container':r,'captureKind':'derived-ZIP-directory','members':members},indent=2)+'\n');selected=[]
  for x in z.infolist():
   q=Path(x.filename)
   if x.is_dir()or q.is_absolute()or'..'in q.parts:continue
   if q.suffix.lower()not in['.pdf','.stp','.step','.ino','.c','.cpp','.h','.md','.txt','.csv','.json','.bin','.py','.defaults']or x.file_size>120<<20:continue
   if any(v in x.filename.lower()for v in['node_modules/','.git/','/tools/','/libs/','/libraries/','/managed_components/'])and q.suffix.lower()not in['.bin','.stp','.step']:continue
   b=z.read(x);out=a.output/'members'/p.stem/q;out.parent.mkdir(parents=True,exist_ok=True);out.write_bytes(b);receipt={**r,'bytes':len(b),'sha256':sha(b),'archiveMember':x.filename,'archiveFormat':'zip','containerSha256':r['sha256'],'containerBytes':r['bytes'],'containerFile':p.name};out.with_name(out.name+'.receipt.json').write_text(json.dumps(receipt,indent=2)+'\n');selected.append(str(q))
  print(p.name,len(members),'members,',len(selected),'selected',flush=True)
