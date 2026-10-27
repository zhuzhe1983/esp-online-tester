#!/usr/bin/env python3
"""Alternating native CPU regression; caller supplies both built bin directories."""
import argparse,datetime,hashlib,json,os,platform,re,statistics,subprocess
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--baseline',required=True);p.add_argument('--candidate',required=True);p.add_argument('--base-commit',required=True);p.add_argument('--output',required=True);p.add_argument('--pairs',type=int,default=3);a=p.parse_args()
assert a.pairs>=3
fw=Path('web/wasm/fw');cases=[]
for name,chip,flash,psram,board in [('hello','s3',8,2,'none'),('c3-hello','c3',4,0,'none'),('c6-hello','c6',4,0,'none'),('pocket-tank','s3',16,8,'waveshare-amoled18-v2')]:
 prefix={'hello':'hello','pocket-tank':'pocket-tank'}.get(name,name);app={'hello':'hello_world','c3-hello':'c3-hello_world','c6-hello':'c6-hello_world'}.get(name,name)
 rom={'s3':'esp32s3_rev0_rom.elf','c3':'esp32c3_rev3_rom.elf','c6':'esp32c6_rev0_rom.elf'}[chip]
 args=['--boot','rom','--board',board,'--flash-mb',str(flash),'--rom',str(fw/rom),'--bootloader',str(fw/'public'/f'{prefix}-bootloader.bin'),'--ptable',str(fw/'public'/f'{prefix}-ptable.bin'),'--app',str(fw/'public'/f'{app}.bin'),'--max-seconds','30','--no-dump']
 if chip=='s3':args+=['--psram-mb',str(psram)]
 if name=='pocket-tank':args+=['--flash-at',f'0x290000={fw}/local/model_q4.bin']
 cases.append(dict(name=name,binary='esp32sim' if chip=='s3' else f'esp32sim-{chip}',arguments=args))
report=dict(baseCommit=a.base_commit,checkedAt=datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ'),method=f'{a.pairs} alternating baseline/candidate pairs; /usr/bin/time -p user CPU; 30 guest seconds; no task-owned concurrent builds/probes, unrelated host load uncontrolled; no speedup claim',host=dict(system=platform.system(),architecture=platform.machine(),logicalCpus=os.cpu_count()),workloads=[])
for w in cases:
 samples=[]
 for pair in range(a.pairs):
  for arm in (['baseline','candidate'] if pair%2==0 else ['candidate','baseline']):
   load=os.getloadavg();r=subprocess.run(['/usr/bin/time','-p',str(Path(getattr(a,arm))/w['binary']),*w['arguments']],capture_output=True,check=True)
   err=r.stderr.decode();stop=re.search(r'\[emu\] stop: (.*?) — (.*?) insns in .*?; emulated ([0-9.]+)s \((\d+) cycles\); (\d+) exceptions, (\d+) interrupts',err)
   assert stop,err[-1000:]
   sample=dict(variant=arm,pair=pair+1,loadAverageBefore=load,loadAverageAfter=os.getloadavg(),userSeconds=float(re.search(r'^user ([0-9.]+)$',err,re.M)[1]),stop=stop[1],instructions=stop[2],guestSeconds=float(stop[3]),cycles=int(stop[4]),exceptions=int(stop[5]),interrupts=int(stop[6]),stdoutSha256=hashlib.sha256(r.stdout).hexdigest());samples.append(sample);print(w['name'],arm,pair+1,sample['userSeconds'],flush=True)
 w['samples']=samples;w['medianUserSeconds']={arm:statistics.median(s['userSeconds'] for s in samples if s['variant']==arm) for arm in ['baseline','candidate']};w['candidateChangePercent']=100*(w['medianUserSeconds']['candidate']/w['medianUserSeconds']['baseline']-1)
 for key in ['instructions','cycles','stdoutSha256','exceptions','interrupts']:w['identical_'+key]=len({s[key] for s in samples})==1;assert w['identical_'+key],(w['name'],key)
 paths=[v for v in w['arguments'] if v.startswith('web/')]+([str(fw/'local/model_q4.bin')] if w['name']=='pocket-tank' else [])
 w['inputSha256']={s:hashlib.sha256(Path(s).read_bytes()).hexdigest() for s in paths};report['workloads'].append(w);Path(a.output).write_text(json.dumps(report,indent=2)+'\n')
