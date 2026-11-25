#!/usr/bin/env python3
"""Frozen-binary short-call comparison; no guest instrumentation or pacing changes."""
import argparse, hashlib, json, os, platform, re, signal, statistics, subprocess, time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = ASSETS = FW = BASE_CLI = NEW_BIN = None
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()

def jobs():
    out = {}
    for name, firmware, cpi, seconds, script, pairs in [
        ('wordle-accurate', 'wordle-merged', 1, 2, None, 5),
        ('wordle-fast-crane', 'wordle-merged', 16, 10, 'crane-cpi16.script', 5),
        ('userdemo-fast-menu', 'userdemo', 16, 10, 'userdemo-space.script', 3),
        ('wordle-aa', 'wordle-merged', 1, 2, None, 3),
    ]:
        out[name] = dict(kind='probe', pairs=pairs, binaries={'base':BASE_CLI/'probe-control', 'new':BASE_CLI/'probe-control' if name.endswith('-aa') else NEW_BIN/'probe-short-call'},
            args=[str(ASSETS/'esp32s3_rev0_rom.elf'), str(ASSETS/(firmware+'.bin')), str(cpi), str(seconds), '@PREFIX@', str(HERE/script) if script else '-', '30000'])
    for name, exe, rom, stem, board, flash, psram, seconds in [
        ('s3-hello', 'esp32sim', 'esp32s3_rev0_rom.elf', 'hello', 'none', 8, 2, 30),
        ('c3-hello', 'esp32sim-c3', 'esp32c3_rev3_rom.elf', 'c3-hello', None, 4, None, 30),
        ('c6-hello', 'esp32sim-c6', 'esp32c6_rev0_rom.elf', 'c6-hello', None, 4, None, 30),
        ('pocket-tank', 'esp32sim', 'esp32s3_rev0_rom.elf', 'pocket-tank', 'waveshare-amoled18-v2', 16, 8, 8),
    ]:
        args=['--boot', 'rom', '--rom', str(FW/rom), '--flash-mb', str(flash)]
        if board: args += ['--board', board]
        if psram is not None: args += ['--psram-mb', str(psram)]
        app = stem+'.bin' if stem=='pocket-tank' else stem.replace('hello', 'hello_world')+'.bin'
        args += ['--bootloader', str(FW/'public'/(stem+'-bootloader.bin')), '--ptable', str(FW/'public'/(stem+'-ptable.bin')), '--app', str(FW/'public'/app)]
        if name=='pocket-tank': args += ['--flash-at', '0x290000='+str(FW/'local/model_q4.bin'), '--tft-png', '@PREFIX@.png']
        args += ['--max-seconds', str(seconds), '--no-dump']
        out[name]=dict(kind='cli', pairs=3, binaries={'base':BASE_CLI/exe, 'new':NEW_BIN/(exe+'-short-call')},args=args)
    return out

def timed(job, side, prefix, remaining):
    env=os.environ.copy()
    for k in list(env):
        if k.startswith('ESP32SIM_') or k=='EOT_PROBE_PHASE_CYCLES': env.pop(k,None)
    env.update(ESP32SIM_VQ='1', ESP32SIM_BB='1')
    command=[str(job['binaries'][side])]+[x.replace('@PREFIX@',str(prefix)) for x in job['args']]
    child=None
    def alarm(*_): raise TimeoutError('finite CPU window exhausted')
    old=signal.signal(signal.SIGALRM,alarm)
    start=time.perf_counter()
    with prefix.with_suffix('.stdout').open('wb') as stdout, prefix.with_suffix('.stderr').open('wb') as stderr:
        try:
            child=subprocess.Popen(command,cwd=ROOT,env=env,stdout=stdout,stderr=stderr,start_new_session=True)
            signal.setitimer(signal.ITIMER_REAL, min(remaining,90))
            _,status,usage=os.wait4(child.pid,0)
            child.returncode=os.waitstatus_to_exitcode(status)
        finally:
            signal.setitimer(signal.ITIMER_REAL,0);signal.signal(signal.SIGALRM,old)
            if child is not None and child.returncode is None:
                os.killpg(child.pid,signal.SIGKILL);_,status,_=os.wait4(child.pid,0);child.returncode=os.waitstatus_to_exitcode(status)
    err=prefix.with_suffix('.stderr').read_text()
    assert child.returncode==0, (command,child.returncode,err[-2000:])
    fact={}
    if job['kind']=='probe':
        line=next(s for s in err.splitlines() if s.startswith('[probe] '))
        fact=dict(re.findall(r'(\w+)=([^ ]+)',line));code_bytes=fact.pop('bytes')
        fact['state_sha256']=hashlib.sha256(('\n'.join(err.splitlines()[err.splitlines().index(line)+1:])).encode()).hexdigest()
        fact['edges']=re.findall(r'^\[edge\].*$',err,re.M)
        fact['console_sha256']=sha(prefix.with_suffix('.console'))
    else:
        line=next(s for s in err.splitlines() if 'stop: ' in s)
        # Printed host throughput is not guest state. Keep all architectural stop fields.
        fact['stop']=re.sub(r' in [0-9.]+s wall = [0-9.]+ Minsn/s','',line)
        m=re.search(r'(?:core0 (\d+) \+ core1 (\d+)|(\d+)) insns',line)
        assert m,(line,command)
        fact['insns']=[int(m.group(1)),int(m.group(2))] if m.group(1) else [int(m.group(3))]
        fact['diagnostics']=[s for s in err.splitlines() if s.startswith(('  core','[emu] i2s frames out:','[emu] crypto:','[emu] chip reset'))]
        fact['console_sha256']=sha(prefix.with_suffix('.stdout'));code_bytes=None
    fact['png_sha256']=sha(prefix.with_suffix('.png')) if prefix.with_suffix('.png').exists() else None
    return dict(side=side,exit=child.returncode,user_s=usage.ru_utime,system_s=usage.ru_stime,wall_s=time.perf_counter()-start,
        peak_rss_bytes=usage.ru_maxrss*(1 if platform.system()=='Darwin' else 1024),minor_faults=usage.ru_minflt,major_faults=usage.ru_majflt,
        facts=fact,code_bytes=code_bytes,command=command)

def main():
    global ROOT, ASSETS, FW, BASE_CLI, NEW_BIN
    ap=argparse.ArgumentParser();ap.add_argument('--repo',type=Path,required=True);ap.add_argument('--assets',type=Path,required=True);ap.add_argument('--base-bins',type=Path,required=True);ap.add_argument('--new-bins',type=Path,required=True);ap.add_argument('--mode',choices=['preflight','formal'],required=True);ap.add_argument('--out',type=Path,required=True);ap.add_argument('--limit-seconds',type=float,default=210)
    a=ap.parse_args();a.out=a.out.resolve();a.out.mkdir(parents=True,exist_ok=True)
    ROOT=a.repo.resolve();ASSETS=a.assets.resolve();FW=ROOT/'web/wasm/fw';BASE_CLI=a.base_bins.resolve();NEW_BIN=a.new_bins.resolve()
    start=time.monotonic();table=jobs()
    if a.mode=='preflight': table={k:v for k,v in table.items() if v['kind']=='cli'}
    result=dict(mode=a.mode,purpose='short-call-only frozen production binaries; preflight is contended and not evidence of speed',clock_hz=240000000,
        settings=dict(quantum=64,vq=1,bb=1,native_defer=False,frontiers=False,probe_run_budget=30000),platform=dict(os=platform.system(),release=platform.release(),machine=platform.machine(),cpus=os.cpu_count()),
        started=time.strftime('%Y-%m-%dT%H:%M:%S%z'),load_before=os.getloadavg(),workloads={})
    def save(): (a.out/'results.json').write_text(json.dumps(result,indent=2)+'\n')
    try:
        for name,job in table.items():
            rec=dict(kind=job['kind'],pairs=job['pairs'],binary_sha256={k:sha(v) for k,v in job['binaries'].items()},args=job['args'],runs=[])
            result['workloads'][name]=rec
            orders=[('preflight',['base','new'])] if a.mode=='preflight' else [('warmup',['new','base'])]+[(str(i),['base','new'] if i%2 else ['new','base']) for i in range(1,job['pairs']+1)]
            for pair,sides in orders:
                for side in sides:
                    remaining=a.limit_seconds-(time.monotonic()-start)
                    assert remaining>0,'finite window exhausted before next arm'
                    item=timed(job,side,a.out/(name+'-'+pair+'-'+side),remaining);item['pair']=pair
                    if rec['runs']: assert item['facts']==rec['runs'][0]['facts'],(name,pair,side,'architectural/output mismatch',item['facts'],rec['runs'][0]['facts'])
                    rec['runs'].append(item);save();print(name,pair,side,round(item['user_s'],6),flush=True)
            measured=[x for x in rec['runs'] if x['pair']!='warmup'];b=[x['user_s'] for x in measured if x['side']=='base'];n=[x['user_s'] for x in measured if x['side']=='new']
            rec['summary']=dict(base_median_s=statistics.median(b),new_median_s=statistics.median(n),change_pct=(statistics.median(n)/statistics.median(b)-1)*100,
                base_range_s=[min(b),max(b)],new_range_s=[min(n),max(n)],new_slower_pairs=sum(next(x['user_s'] for x in measured if x['pair']==p and x['side']=='new')>next(x['user_s'] for x in measured if x['pair']==p and x['side']=='base') for p,_ in orders if p!='warmup'),identical_work_and_output=True)
            save();print('SUMMARY',name,rec['summary'],flush=True)
        result['status']='passed'
    except BaseException as e:
        result['status']='failed';result['error']=repr(e);raise
    finally:
        result['finished']=time.strftime('%Y-%m-%dT%H:%M:%S%z');result['elapsed_s']=time.monotonic()-start;result['load_after']=os.getloadavg();save()

if __name__=='__main__': main()
