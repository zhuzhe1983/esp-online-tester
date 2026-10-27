#!/usr/bin/env python3
"""Reproduce the U003 idle CPU gate; use independently cargo-build --release --bins trees."""
import argparse, datetime, hashlib, json, os, pathlib, platform, re, statistics, subprocess
p=argparse.ArgumentParser();p.add_argument('--base-bins',required=True);p.add_argument('--candidate-bins',required=True);p.add_argument('--rom-dir',required=True);p.add_argument('--receipt',required=True);p.add_argument('--resume',action='store_true');a=p.parse_args()
sha=lambda b:hashlib.sha256(b).hexdigest()
# The same four source-pinned baseline workloads used for U027; no firmware sweep.
previous=json.loads(pathlib.Path('docs/platform/evidence/m5stack-unit-button-u027/runtime/cpu.json').read_text())
bins={'hello':'esp32sim','c3-hello':'esp32sim-c3','c6-hello':'esp32sim-c6','pocket-tank':'esp32sim'}
report={'checkedAt':datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ'),'base':'9d072179','candidate':'U003 working tree identified by source hashes','method':'One excluded warmup pair then five measured alternating base/candidate pairs per workload; 30 guest seconds; /usr/bin/time -p user. Task-owned browsers and compilers stopped. Other host load uncontrolled. No speed claim.','host':{'os':platform.system(),'architecture':platform.machine(),'rust':subprocess.check_output(['rustc','--version'],text=True).strip()},'sourceSha256':{f:sha(pathlib.Path(f).read_bytes()) for f in ['esp-periph/src/rmt.rs','esp-soc/src/board.rs','esp-soc/src/machine/web.rs','esp32s3/src/board/stopwatch.rs']},'workloads':[]}
if a.resume:
 previous_report=json.loads(pathlib.Path(a.receipt).read_text());assert previous_report['sourceSha256']==report['sourceSha256'];report=previous_report
for old in previous['workloads']:
 if any(w['name']==old['name'] and w.get('workAndOutputEqual') for w in report['workloads']):continue
 report['workloads']=[w for w in report['workloads']if w['name']!=old['name']]
 name=old['name'];args=[x.replace('$ROM_DIR',a.rom_dir) for x in old['args']];paths={key:str(pathlib.Path(folder)/bins[name]) for key,folder in [('base',a.base_bins),('candidate',a.candidate_bins)]}
 w={'name':name,'args':old['args'],'inputs':{},'binarySha256':{key:sha(pathlib.Path(value).read_bytes())for key,value in paths.items()},'samples':[]};report['workloads'].append(w)
 for i,value in enumerate(args):
  if i and args[i-1] in ['--rom','--bootloader','--ptable','--app','--flash-at']:
   file=value.split('=',1)[-1];w['inputs'][args[i-1][2:]]={'file':old['args'][i],'sha256':sha(pathlib.Path(file).read_bytes())}
 for round in range(6):
  for key in (['base','candidate']if round%2==0 else ['candidate','base']):
   load=os.getloadavg();run=subprocess.run(['/usr/bin/time','-p',paths[key],*args],stdout=subprocess.PIPE,stderr=subprocess.PIPE,check=True);err=run.stderr.decode();m=re.search(r'(?:core0 )?([0-9]+)(?: \+ core1 ([0-9]+))? insns',err);assert m,err[-300:]
   row={'round':round,'warmup':round==0,'build':key,'userSeconds':float(re.search(r'^user ([0-9.]+)$',err,re.M)[1]),'instructions':[int(m[1]),int(m[2]or 0)],'guestOutputSha256':sha(run.stdout),'loadBefore':load};w['samples'].append(row)
   pathlib.Path(a.receipt).write_text(json.dumps(report,indent=2)+'\n');print(name,round,key,row['userSeconds'],flush=True)
 good=[r for r in w['samples']if not r['warmup']];assert len({(tuple(r['instructions']),r['guestOutputSha256'])for r in good})==1,'Guest work/output changed'
 w['medianUserSeconds']={key:statistics.median(r['userSeconds']for r in good if r['build']==key)for key in paths};w['changePercent']=100*(w['medianUserSeconds']['candidate']/w['medianUserSeconds']['base']-1);w['slowerPairs']=sum(next(r['userSeconds']for r in good if r['round']==n and r['build']=='candidate')>next(r['userSeconds']for r in good if r['round']==n and r['build']=='base')for n in range(1,6));w['workAndOutputEqual']=True
 pathlib.Path(a.receipt).write_text(json.dumps(report,indent=2)+'\n')
report['status']='measured';pathlib.Path(a.receipt).write_text(json.dumps(report,indent=2)+'\n')
