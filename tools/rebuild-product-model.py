#!/usr/bin/env python3
"""Rebuild committed model plans in scratch, without private queues or source caches.

`plan` uses stdlib only and never downloads. `hydrate` is offline by default.
`build` hydrates inputs and imports the pinned CAD exporter only when needed.
"""
import argparse,datetime,hashlib,importlib.util,json,re,sys
from pathlib import Path
from urllib.parse import quote,urlsplit,urlunsplit
from urllib.request import Request,HTTPRedirectHandler,build_opener

ROOT=Path(__file__).resolve().parents[1]
MAX_BYTES=64*1024*1024

def digest(data):return hashlib.sha256(data).hexdigest()
def utc():return datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')
def write(path,data):
 if path.is_symlink():raise ValueError('Refusing symlink output')
 path.parent.mkdir(parents=True,exist_ok=True);path.write_text(json.dumps(data,ensure_ascii=False,indent=2)+'\n')
def filename(name):
 if not isinstance(name,str) or not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9._-]*',name):raise ValueError(f'Unsafe source/asset filename: {name!r}')
 return name

def load_product(root,identity):
 if not re.fullmatch(r'[a-z0-9][a-z0-9-]*',identity):raise ValueError('Invalid product ID')
 assets=root/'web/platform/assets'/identity;plan_path=assets/'model-plan.json';metadata_path=assets/'model.json'
 if not plan_path.is_file() or not metadata_path.is_file():raise ValueError('No committed generic model-plan.json/model.json for this ID; metadata-only and custom legacy renderers need their own documented generator')
 plan=json.loads(plan_path.read_text());metadata=json.loads(metadata_path.read_text());audit_path=root/'docs/platform/evidence'/identity/'source-audit.json';audit=json.loads(audit_path.read_text())if audit_path.exists()else{}
 sources=metadata.get('provenance',{}).get('sources',audit.get('sources',[]));by_id={s['id']:s for s in sources}
 if len(by_id)!=len(sources):raise ValueError('Duplicate source IDs')
 for old in audit.get('sources',[]):
  if old['id']in by_id and old.get('sha256') and by_id[old['id']].get('sha256')!=old['sha256']:raise ValueError(f"Source receipt disagreement: {old['id']}")
 required=list(dict.fromkeys([plan['sourceId'],*[p.get('sourceId',plan['sourceId'])for p in plan['parts']],*[p['sourceId']for p in plan.get('photos',[])],*[p['sourceId']for p in plan.get('textureSources',[])],'cad-license']))
 for sid in required:
  if sid not in by_id:raise ValueError(f'Missing pinned source receipt: {sid}')
  validate_source(by_id[sid])
 return {'id':identity,'root':root,'assets':assets,'plan':plan,'metadata':metadata,'audit':audit,'sources':sources,'byId':by_id,'required':required}

def validate_source(source):
 if source.get('status')!='fetched' or not re.fullmatch(r'[a-f0-9]{64}',source.get('sha256','')):raise ValueError(f"Source is not hash-pinned: {source['id']}")
 if not isinstance(source.get('bytes'),int) or not 0<source['bytes']<=MAX_BYTES:raise ValueError(f"Source exceeds64MiB or has no byte count: {source['id']}")
 filename(source['archiveName'])

def local_candidates(product,source):
 result=[]
 for name,expected in product['metadata'].get('artifacts',{}).items():
  filename(name)
  if expected==source['sha256']:
   path=product['assets']/name
   if path.is_symlink() or not path.resolve().is_relative_to(product['assets'].resolve()):raise ValueError('Source artifact escapes product asset directory')
   if path.is_file():result.append(path)
 return result

def checked_bytes(path,source):
 if path.stat().st_size!=source['bytes']:raise ValueError(f"Source byte count changed: {source['id']}")
 data=path.read_bytes()
 if digest(data)!=source['sha256']:raise ValueError(f"Source SHA-256 changed: {source['id']}")
 return data

def secure_url(url):
 parts=urlsplit(url)
 if parts.scheme!='https' or not parts.hostname or parts.username or parts.password:raise ValueError('Only credential-free HTTPS source URLs are allowed')
 return urlunsplit((parts.scheme,parts.netloc,quote(parts.path,safe='/%:@+,-._~'),quote(parts.query,safe='=&%:+,;@/?-._~'),''))
class HttpsRedirect(HTTPRedirectHandler):
 def redirect_request(self,req,fp,code,msg,headers,newurl):return super().redirect_request(req,fp,code,msg,headers,secure_url(newurl))

def fetch_exact(source,destination):
 request=Request(secure_url(source['url']),headers={'User-Agent':'esp-online-tester-model-reproducer/1'})
 with build_opener(HttpsRedirect()).open(request,timeout=45) as response:
  declared=response.headers.get('Content-Length')
  if declared and int(declared)>source['bytes']:raise ValueError(f"Source length changed: {source['id']}")
  chunks=[];total=0
  while True:
   chunk=response.read(min(1024*1024,source['bytes']+1-total))
   if not chunk:break
   total+=len(chunk)
   if total>source['bytes']:raise ValueError(f"Source exceeds pinned byte count: {source['id']}")
   chunks.append(chunk)
  data=b''.join(chunks);resolved=secure_url(response.geturl())
 if len(data)!=source['bytes'] or digest(data)!=source['sha256']:raise ValueError(f"Remote source changed; refusing replacement: {source['id']}")
 destination.write_bytes(data);return resolved

def exporter(root):
 spec=importlib.util.spec_from_file_location('product_model_exporter',root/'tools/onboard-lane-01.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module

def scratch_path(root,path):
 path=Path(path).resolve()
 if path==root or path==Path(path.anchor):raise ValueError('Scratch must be a dedicated directory')
 for name in ['web','docs','tools','.git','.butler']:
  if path.is_relative_to((root/name).resolve()):raise ValueError('Scratch cannot overwrite committed product data, tools, evidence or task state')
 path.mkdir(parents=True,exist_ok=True);return path

def hydrate(product,scratch,allow_network=False):
 scratch=scratch_path(product['root'],scratch);folder=scratch/'sources'/product['id']
 for child in [folder,scratch/'hydration']:
  if not child.resolve().is_relative_to(scratch):raise ValueError('Scratch child escapes scratch directory')
 folder.mkdir(parents=True,exist_ok=True);records=[];active=set();done=set()
 def materialize(sid):
  if sid in done:return
  if sid in active:raise ValueError('Cyclic embedded source dependency')
  active.add(sid);source=product['byId'][sid];validate_source(source);path=folder/source['archiveName'];record={'sourceId':sid,'sha256':source['sha256'],'bytes':source['bytes'],'originalFetchedAt':source.get('fetchedAt'),'acquiredAt':utc()}
  if path.is_symlink():raise ValueError('Refusing symlink source cache')
  if path.exists():checked_bytes(path,source);record['method']='verified-scratch-cache'
  else:
   candidates=local_candidates(product,source)
   if candidates:
    data=checked_bytes(candidates[0],source);path.write_bytes(data);record.update(method='committed-asset',catalogFile=str(candidates[0].relative_to(product['root'])))
   elif source.get('acquisition',{}).get('containerId'):
    container_id=source['acquisition']['containerId']
    if container_id not in product['byId']:raise ValueError('Missing pinned embedded container receipt')
    if source['acquisition'].get('containerSha256')!=product['byId'][container_id]['sha256']:raise ValueError('Embedded container hash receipt disagrees')
    materialize(container_id);module=exporter(product['root']);derived=module.extract_kicad_steps(product['byId'][container_id],folder);match=next((s for s in derived if s['id']==sid),None)
    if not match or match['sha256']!=source['sha256']:raise ValueError(f'Embedded payload changed: {sid}')
    checked_bytes(path,source);record.update(method='verified-kicad-embedded-payload',containerId=container_id)
   elif allow_network:
    record.update(method='https-exact-hash',resolvedUrl=fetch_exact(source,path))
   else:raise ValueError(f"Missing offline source {sid}; expected {source['sha256']}. Re-run with --allow-network to fetch its pinned public URL: {source['url']}")
  active.remove(sid);done.add(sid);records.append(record)
 for sid in product['required']:materialize(sid)
 prepared={'id':product['id'],'sources':product['sources']};write(folder/'prepared.json',prepared);write(folder/'components.json',product['audit'].get('components',[]));receipt={'schemaVersion':'1','productId':product['id'],'acquiredAt':utc(),'sources':records,'originalReceiptPolicy':'Original source receipts are unchanged; acquisition times above describe this reconstruction only. No original evidence is rewritten.'};write(scratch/'hydration'/f"{product['id']}.json",receipt);return folder,receipt

def compare_build(expected,actual,output):
 mismatches=[];verified=[]
 for name,actual_hash in actual['artifacts'].items():
  if digest((output/name).read_bytes())!=actual_hash or expected['artifacts'].get(name)!=actual_hash:mismatches.append('artifact:'+name)
  else:verified.append(name)
 for field in ['unit','binary','strideFloats','parts','sourceTransform','omittedComponents','dimensions','presentation','derivedParts','derivedSurfaces','referencePhotos','textures','reviewViews']:
  if expected.get(field)!=actual.get(field):mismatches.append('contract:'+field)
 return {'passed':not mismatches,'verifiedArtifacts':verified,'mismatches':mismatches,'unusedOriginalArtifacts':sorted(set(expected['artifacts'])-set(actual['artifacts'])),'modelJsonByteIdentical':digest(json.dumps(expected,ensure_ascii=False,indent=2).encode()+b'\n')==digest((output/'model.json').read_bytes()),'scope':'Exact regenerated geometry, source/reference files, textures and model contract. Metadata byte order and historical unused files are reported separately. No fresh browser, hardware or firmware acceptance.'}

def build(product,scratch,allow_network=False):
 scratch=scratch_path(product['root'],scratch);output=scratch/'rebuilt'/product['id']
 for child in [output,scratch/'evidence'/product['id'],scratch/'comparison']:
  if not child.resolve().is_relative_to(scratch):raise ValueError('Scratch child escapes scratch directory')
 if (scratch/'evidence'/product['id']).exists():raise ValueError('Evidence output already exists; choose a fresh --scratch directory')
 if output.exists():raise ValueError('Rebuild output already exists; choose a fresh --scratch directory to preserve previous results')
 folder,_=hydrate(product,scratch,allow_network);module=exporter(product['root']);module.TEMP=scratch/'sources';module.REV=product['metadata']['provenance']['commit'];prepared=json.loads((folder/'prepared.json').read_text());actual,_=module.build_model(prepared,product['plan'],output,evidence_root=scratch/'evidence',license_path=folder/product['byId']['cad-license']['archiveName']);comparison=compare_build(product['metadata'],actual,output);write(scratch/'comparison'/f"{product['id']}.json",comparison)
 if not comparison['passed']:raise ValueError('Rebuild differs from committed artifacts: '+', '.join(comparison['mismatches']))
 return comparison

def main(argv=None):
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('command',choices=['plan','hydrate','build']);p.add_argument('product_id');p.add_argument('--scratch',help='Dedicated scratch directory; required for hydrate/build');p.add_argument('--allow-network',action='store_true',help='Fetch missing pinned public sources only; changed bytes/hash always fail');a=p.parse_args(argv);product=load_product(ROOT,a.product_id)
 if a.command=='plan':
  print(json.dumps({'productId':a.product_id,'sourceCommit':product['metadata']['provenance']['commit'],'requiredSources':[{'id':sid,'sha256':product['byId'][sid]['sha256'],'bytes':product['byId'][sid]['bytes'],'localCandidates':[str(path.relative_to(ROOT))for path in local_candidates(product,product['byId'][sid])],'embeddedContainer':product['byId'][sid].get('acquisition',{}).get('containerId')}for sid in product['required']],'network':'disabled by default; plan does not read large artifacts or download anything'},indent=2));return
 if not a.scratch:p.error('--scratch is required for hydrate/build')
 result=hydrate(product,a.scratch,a.allow_network)[1]if a.command=='hydrate'else build(product,a.scratch,a.allow_network);print(json.dumps({'productId':a.product_id,'command':a.command,'passed':result.get('passed',True),'verifiedCount':len(result.get('verifiedArtifacts',result.get('sources',[])))},indent=2))
if __name__=='__main__':
 try:main()
 except (ValueError,OSError,KeyError,AssertionError)as error:print('ERROR: '+str(error),file=sys.stderr);raise SystemExit(1)
