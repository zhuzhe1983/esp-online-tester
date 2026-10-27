import argparse,datetime,hashlib,json,pathlib,re,statistics,subprocess
p=argparse.ArgumentParser();p.add_argument('--base',required=True);p.add_argument('--candidate',required=True);p.add_argument('--root',required=True);a=p.parse_args();root=pathlib.Path(a.root)
hash=lambda f:hashlib.sha256(pathlib.Path(f).read_bytes()).hexdigest()
r={'checkedAt':datetime.datetime.now(datetime.timezone.utc).isoformat(),'baselineCommit':'84e67571f0d1aecddf77d2432678fec81baabf72','method':'Five alternating pairs, main then candidate, identical public inputs; median /usr/bin/time -p user seconds. No browser or other build/guest during timing.','seconds':30,'binaries':{},'workloads':{}}
for manifest,binname in [('hello','esp32sim'),('c3-hello','esp32sim-c3'),('c6-hello','esp32sim-c6')]:
 cfg=json.loads((root/'web/wasm/fw'/f'{manifest}.json').read_text());args=['--board','none','--boot','rom','--max-seconds','30','--flash-mb',str(cfg['flash_mb']),'--no-dump']
 if manifest=='hello':args+=['--psram-mb',str(cfg['psram_mb'])]
 for kind,path in cfg['files'].items():args+=['--'+kind,str(root/'web/wasm/fw'/path)]
 samples={'main':[],'candidate':[]};correct=[]
 for n in range(5):
  for kind,folder in [('main',a.base),('candidate',a.candidate)]:
   binary=pathlib.Path(folder)/binname;r['binaries'][kind+'-'+binname]=hash(binary)
   out=subprocess.run(['/usr/bin/time','-p',str(binary)]+args,capture_output=True,check=True)
   stderr=out.stderr.decode();cpu=float(re.search(r'^user ([0-9.]+)$',stderr,re.M)[1]);stop=next(x for x in stderr.splitlines() if '[emu] stop:' in x).split(' in ')[0]
   samples[kind].append(cpu);correct.append((hashlib.sha256(out.stdout).hexdigest(),stop))
 assert len(set(correct))==1,(manifest,correct)
 med={k:statistics.median(v) for k,v in samples.items()};r['workloads'][manifest]={'userSeconds':samples,'medianUserSeconds':med,'candidateDeltaPercent':(med['candidate']/med['main']-1)*100,'stdoutSha256':correct[0][0],'instructions':correct[0][1],'inputs':{k:hash(root/'web/wasm/fw'/v) for k,v in cfg['files'].items()}}
print(json.dumps(r,indent=2))
