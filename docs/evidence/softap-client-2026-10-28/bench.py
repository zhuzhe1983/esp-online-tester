#!/usr/bin/env python3
"""Serial user-CPU qualification of disabled SoftAP: caller-supplied binaries and firmware roots."""
import argparse, hashlib, json, os, platform, re, resource, statistics, subprocess, time
from pathlib import Path
p=argparse.ArgumentParser()
for name in ('baseline','candidate','firmware','roms','model','output'): p.add_argument('--'+name,required=True,type=Path)
p.add_argument('--rounds',type=int,default=3)
p.add_argument('--resume',action='store_true')
a=p.parse_args(); a.output.mkdir(parents=True,exist_ok=True)
sha=lambda b:hashlib.sha256(b).hexdigest()
env={k:v for k,v in os.environ.items() if not k.startswith(('ESP32SIM_','ESP_EMU_'))}
sources={'baseline':'ea69be70','candidate':'1f0de1b06cf3780820a38904f4b33eb1f08183c8'}
prior=json.loads((a.output/'runs.json').read_text()) if a.resume else {}
rows=prior.get('rows',[]); cases=prior.get('cases',{})
for name,binary,board,chip,flash,psram,seconds in [('s3-hello','esp32sim','none','esp32s3',8,2,3000),('c3-hello','esp32sim-c3',None,'esp32c3',4,0,30),('c6-hello','esp32sim-c6',None,'esp32c6',4,0,30),('pocket-tank','esp32sim','waveshare-amoled18-v2','esp32s3',16,8,30)]:
    if len([x for x in rows if x['workload']==name])==2*(a.rounds+1): continue
    prefix='hello' if name=='s3-hello' else name
    rev='3' if chip=='esp32c3' else '0'
    inputs={'rom':a.roms/f'{chip}_rev{rev}_rom.elf','bootloader':a.firmware/f'{prefix}-bootloader.bin','ptable':a.firmware/f'{prefix}-ptable.bin','app':a.firmware/(f'{prefix}_world.bin' if prefix.endswith('hello') else 'pocket-tank.bin')}
    args=['--boot','rom','--no-dump','--console','uart0','--flash-mb',str(flash),'--max-seconds',str(seconds)]
    if board: args+=['--board',board]
    if psram: args+=['--psram-mb',str(psram)]
    for key,path in inputs.items(): args+=['--'+key,str(path)]
    if name=='pocket-tank': inputs['model']=a.model;args+=['--flash-at','0x290000='+str(a.model)]
    clean=list(args)
    for key,path in inputs.items(): clean=[x.replace(str(path),'$'+key.upper()) for x in clean]
    cases[name]={'binary':binary,'args':clean,'inputs':{k:{'file':v.name,'sha256':sha(v.read_bytes())} for k,v in inputs.items()},'artifacts':{arm:sha((getattr(a,arm)/binary).read_bytes()) for arm in sources}}
    for pair in range(a.rounds+1):
        for arm in (['baseline','candidate'] if pair%2 else ['candidate','baseline']):
            before=resource.getrusage(resource.RUSAGE_CHILDREN);load=os.getloadavg();t=time.monotonic()
            r=subprocess.run([str(getattr(a,arm)/binary),*args],capture_output=True,env=env,timeout=300)
            after=resource.getrusage(resource.RUSAGE_CHILDREN);wall=time.monotonic()-t
            stderr=r.stderr.decode(errors='replace')
            stop=re.search(r'\[emu\] stop: (\w+) — (.*?) insns in ',stderr)
            if r.returncode or not stop or stop[1]!='Halted': raise RuntimeError(stderr[-2000:])
            work=re.findall(r'(?:core\d+ )?(\d+)',stop[2]);console=sha(r.stdout)
            row={'workload':name,'pair':pair,'warmup':pair==0,'arm':arm,'userSeconds':round(after.ru_utime-before.ru_utime,6),'systemSeconds':round(after.ru_stime-before.ru_stime,6),'wallSeconds':round(wall,6),'loadBefore':load,'instructions':[int(x) for x in work],'consoleSha256':console}
            prior=next((x for x in rows if x['workload']==name),row)
            assert row['instructions']==prior['instructions'],(name,'instruction mismatch')
            assert console==prior['consoleSha256'],(name,'console mismatch')
            rows.append(row); print(json.dumps(row),flush=True)
            (a.output/'runs.json').write_text(json.dumps({'sources':sources,'cases':cases,'rows':rows},indent=2)+'\n')
summary={}
for name in cases:
    med={arm:statistics.median(x['userSeconds'] for x in rows if x['workload']==name and x['arm']==arm and not x['warmup']) for arm in sources}
    values={arm:[x['userSeconds'] for x in rows if x['workload']==name and x['arm']==arm and not x['warmup']] for arm in sources}
    slower=sum(values['candidate'][i]>values['baseline'][i] for i in range(a.rounds))
    summary[name]={'medianUserSeconds':med,'changePercent':100*(med['candidate']/med['baseline']-1),'rangeUserSeconds':{arm:[min(v),max(v)] for arm,v in values.items()},'candidateSlowerPairs':slower,'pairs':a.rounds,'identicalWorkAndConsole':True}
receipt={'sources':sources,'host':{'architecture':platform.machine(),'system':platform.system(),'osVersion':platform.mac_ver()[0]},'measure':'resource.getrusage(RUSAGE_CHILDREN) user CPU delta; serial subprocesses; alternating order; first pair warmup','cases':cases,'rows':rows,'summary':summary}
(a.output/'summary.json').write_text(json.dumps(receipt,indent=2)+'\n'); print(json.dumps(summary,indent=2),flush=True)
