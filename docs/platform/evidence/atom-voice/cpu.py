import subprocess,json,hashlib,re,statistics,datetime,argparse
from pathlib import Path
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
parser=argparse.ArgumentParser();parser.add_argument('--baseline-bin',type=Path,required=True);parser.add_argument('--output',type=Path,required=True);args_cli=parser.parse_args();root=Path.cwd();base=args_cli.baseline_bin.resolve();fw=root/'web/wasm/fw';out={'checkedAt':datetime.datetime.now(datetime.timezone.utc).isoformat(),'baselineCommit':'84e67571f0d1aecddf77d2432678fec81baabf72','method':'Five alternating main/candidate pairs; /usr/bin/time -p median user seconds; no other guest/build/browser during timing.','seconds':30,'binaries':{},'workloads':{}}
for name,binary in [('hello','esp32sim'),('c3-hello','esp32sim-c3'),('c6-hello','esp32sim-c6'),('classic-hello','esp32sim')]:
 cfg=({'flash_mb':4,'files':{'rom':'esp32_rev300_rom.elf','bootloader':'public/classic-hello-bootloader.bin','ptable':'public/classic-hello-ptable.bin','app':'public/classic-hello_world.bin'}} if name=='classic-hello' else json.loads((fw/(name+'.json')).read_text()));args=['--boot','rom','--flash-mb',str(cfg['flash_mb']),'--max-seconds','30','--no-dump']
 if name=='classic-hello':args+=['--chip','esp32','--board','esp32dev']
 if name=='hello':args+=['--psram-mb',str(cfg['psram_mb'])]
 for key,value in cfg['files'].items():args+=['--'+key,str(fw/value)]
 samples={'main':[],'candidate':[]};reference=None;reference_stop=None;reference_cycles=None
 for n in range(5):
  for label,path in [('main',base/binary),('candidate',root/'target/release'/binary)]:
   out['binaries'][label+'-'+binary]=sha(path);run=subprocess.run(['/usr/bin/time','-p',str(path),*args],capture_output=True,check=True);stderr=run.stderr.decode();sample=float(re.search(r'^user\s+([0-9.]+)$',stderr,re.M)[1]);line=next(x for x in stderr.splitlines() if '[emu] stop:' in x);stop=line.split(' in ')[0];cycles=re.findall(r'\d+ cycles',line);samples[label].append(sample)
   if reference is None:reference=run.stdout;reference_stop=stop;reference_cycles=cycles
   assert reference==run.stdout,(name,label,n,'UART mismatch');assert reference_stop==stop,(name,label,n,'instructions mismatch');assert reference_cycles==cycles,(name,label,n,'cycles mismatch')
 med={k:statistics.median(v) for k,v in samples.items()};out['workloads'][name]={'args':[s.replace(str(root)+'/','') for s in args],'userSeconds':samples,'medianUserSeconds':med,'candidateDeltaPercent':(med['candidate']/med['main']-1)*100,'stdoutSha256':hashlib.sha256(reference).hexdigest(),'instructions':reference_stop,'cycleLines':reference_cycles,'inputs':{k:sha(fw/v) for k,v in cfg['files'].items()}}
 print(name,out['workloads'][name]['candidateDeltaPercent'],flush=True)
out['interpretation']='Cross-revision production regression guard, not attribution or optimization. Hundredth-second quantization and host noise limit small differences. All UART and reported guest work match. Classic FRC2 changes require the additional classic hello control. Native classic quantum was already 64. No S3 core changes; no Pocket workload required.'
args_cli.output.write_text(json.dumps(out,indent=2)+'\n')
