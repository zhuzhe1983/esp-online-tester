#!/usr/bin/env python3
"""Compare fixed-work CPU cost; raw guest output is hashed, never retained here."""
import argparse, datetime, hashlib, json, os, pathlib, platform, re, resource, statistics, subprocess, time
ap = argparse.ArgumentParser(description=__doc__)
for name in ('base', 'new', 'base-rev', 'new-rev', 'rom-dir', 'output'):
    ap.add_argument('--' + name, required=True)
ap.add_argument('--repo', default='.')
ap.add_argument('--pairs', type=int, default=7)
ap.add_argument('--workloads', default='s3-hello,c3-hello,c6-hello,pocket-tank,classic-hello')
a = ap.parse_args()
root = pathlib.Path(a.repo).resolve()
roms = pathlib.Path(a.rom_dir).resolve()
sha = lambda b: hashlib.sha256(b).hexdigest()
prior = json.loads((root / 'docs/evidence/p4-cpu/09-lcd35-board.json').read_text())
commands = {k: v['command'] for k, v in prior['workloads'].items()}
commands['classic-hello'] = ['esp32sim', '--chip', 'esp32', '--board', 'esp32dev', '--boot', 'rom', '--rom', '$ESP32SIM_ROM_DIR/esp32_rev300_rom.elf', '--flash-mb', '16', '--bootloader', 'web/wasm/fw/public/classic-hello-bootloader.bin', '--ptable', 'web/wasm/fw/public/classic-hello-ptable.bin', '--app', 'web/wasm/fw/public/classic-hello_world.bin', '--max-seconds', '30', '--no-dump']
result = {'date_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(), 'base_revision': a.base_rev, 'new_revision': a.new_rev, 'pairs': a.pairs, 'method': 'resource.RUSAGE_CHILDREN user CPU deltas; one warmup per side; alternating order; same command and bytes', 'environment': {'system': platform.system(), 'release': platform.release(), 'architecture': platform.machine(), 'logical_cpus': os.cpu_count(), 'rustc': subprocess.check_output(['rustc', '--version'], text=True).strip()}, 'inputs_sha256': {}, 'binaries_sha256': {}, 'workloads': {}}
for name in a.workloads.split(','):
    command = commands[name]
    actual = [s.replace('$ESP32SIM_ROM_DIR', str(roms)) for s in command[1:]]
    for i, token in enumerate(actual):
        if i and actual[i-1] in ['--rom', '--bootloader', '--ptable', '--app', '--flash-at', '--script']:
            path = pathlib.Path(token.split('=',1)[-1])
            path = path if path.is_absolute() else root / path
            label = 'ROM/' + path.name if actual[i-1] == '--rom' else str(path.relative_to(root))
            result['inputs_sha256'][label] = sha(path.read_bytes())
    row = {'command': command, 'runs': []}
    result['workloads'][name] = row
    for pair in range(-1, a.pairs):
        order = ['base','new'] if pair % 2 == 0 else ['new','base']
        for side in order:
            binary = pathlib.Path(getattr(a, side)).resolve() / command[0]
            result['binaries_sha256'][side + '/' + command[0]] = sha(binary.read_bytes())
            load = os.getloadavg()
            before = resource.getrusage(resource.RUSAGE_CHILDREN).ru_utime
            started = time.monotonic()
            run = subprocess.run([str(binary)] + actual, cwd=root, capture_output=True, timeout=180)
            user = resource.getrusage(resource.RUSAGE_CHILDREN).ru_utime - before
            stderr = run.stderr.decode('utf-8', errors='replace')
            dual = re.search(r'core0 (\d+) \+ core1 (\d+) insns', stderr)
            single = re.search(r'— (\d+) insns', stderr)
            cycles = re.search(r'emulated [0-9.]+s \((\d+) cycles\)', stderr)
            if run.returncode or not (dual or single) or not cycles:
                raise RuntimeError(f'{name}/{side}: exit {run.returncode}, missing complete stop report')
            item = {'pair': pair, 'side': side, 'order': order, 'user_s': round(user,6), 'wall_s': round(time.monotonic()-started,6), 'insns': list(map(int,(dual or single).groups())), 'cycles': int(cycles[1]), 'console_sha256': sha(run.stdout), 'exit': run.returncode, 'load_before': load}
            row['runs'].append(item)
            pathlib.Path(a.output).write_text(json.dumps(result,indent=2)+'\n')
        print(name, 'warmup' if pair < 0 else pair + 1, flush=True)
    signatures = {(tuple(x['insns']),x['cycles'],x['console_sha256']) for x in row['runs']}
    row['exact_work_and_console'] = len(signatures) == 1
    if len(signatures) != 1:
        raise RuntimeError(f'{name}: instruction, cycle or console mismatch')
    measured = [x for x in row['runs'] if x['pair'] >= 0]
    med = {side:statistics.median(x['user_s'] for x in measured if x['side']==side) for side in ['base','new']}
    slower = sum(next(x['user_s'] for x in measured if x['pair']==i and x['side']=='new') > next(x['user_s'] for x in measured if x['pair']==i and x['side']=='base') for i in range(a.pairs))
    row['summary'] = {'median_user_s':med,'change_percent':round((med['new']/med['base']-1)*100,3),'new_slower_pairs':slower,'flagged':slower >= a.pairs-1}
    pathlib.Path(a.output).write_text(json.dumps(result,indent=2)+'\n')
    print(name,row['summary'],flush=True)
