#!/usr/bin/env python3
"""Alternate ROM-boot hello and pocket-tank controls on caller-supplied builds."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import statistics
import subprocess
import time

p = argparse.ArgumentParser()
p.add_argument('--baseline', required=True, type=Path)
p.add_argument('--candidate', required=True, type=Path)
p.add_argument('--firmware-dir', required=True, type=Path)
p.add_argument('--rom-dir', required=True, type=Path)
p.add_argument('--output', required=True, type=Path)
p.add_argument('--pairs', type=int, default=7)
p.add_argument('--initial-cpu-max', '--idle-cpu-max', dest='cpu_max', type=float, default=150)
a = p.parse_args()

def cpu():
    # Only an aggregate is retained: no process names, command lines or identities.
    return round(sum(float(x) for x in subprocess.check_output(['ps', '-A', '-o', '%cpu='], text=True).split()), 1)

gate = []
for _ in range(60):
    gate.append(cpu())
    if len(gate) >= 3 and max(gate[-3:]) <= a.cpu_max:
        break
    time.sleep(1)
else:
    a.output.write_text(json.dumps({'idleGatePassed': False, 'idleCpuMax': a.cpu_max, 'aggregateCpuSamples': gate}, indent=2) + '\n')
    raise SystemExit('No quiet CPU window; did not benchmark.')
print('Initial aggregate CPU threshold passed:', gate[-3:], flush=True)
records = []
inputs = {}
for label, chip, stem, rom, flash in [
    ('classic-hello', 'esp32', 'classic-hello', 'esp32_rev300_rom.elf', 4),
    ('hello', 'esp32s3', 'hello', 'esp32s3_rev0_rom.elf', 8),
    ('c3-hello', 'esp32c3', 'c3-hello', 'esp32c3_rev3_rom.elf', 4),
    ('c6-hello', 'esp32c6', 'c6-hello', 'esp32c6_rev0_rom.elf', 4),
    ('pocket-tank', 'esp32s3', 'pocket-tank', 'esp32s3_rev0_rom.elf', 16),
]:
    files = [a.rom_dir / rom] + [a.firmware_dir / 'public' / (stem + suffix) for suffix in ['-bootloader.bin', '-ptable.bin', '_world.bin']]
    if label == 'pocket-tank':
        files[3] = a.firmware_dir / 'public/pocket-tank.bin'
        files.append(a.firmware_dir / 'local/model_q4.bin')
    seconds = 3 if label == 'pocket-tank' else 30
    inputs[label] = {f.name: hashlib.sha256(f.read_bytes()).hexdigest() for f in files}
    cmd = ['--chip', chip, '--board', 'none', '--boot', 'rom', '--rom', str(files[0]), '--flash-mb', str(flash),
           '--bootloader', str(files[1]), '--ptable', str(files[2]), '--app', str(files[3]), '--max-seconds', str(seconds), '--no-dump']
    if chip == 'esp32s3':
        cmd += ['--psram-mb', '8' if label == 'pocket-tank' else '2']
    if label == 'pocket-tank':
        cmd += ['--board', 'waveshare-amoled18-v2', '--flash-at', '0x290000=' + str(files[4])]
    samples = []
    for pair in range(a.pairs + 1):  # one warmup pair, excluded
        for name in (['baseline', 'candidate'] if pair % 2 == 0 else ['candidate', 'baseline']):
            busy = cpu()
            r = subprocess.run(['/usr/bin/time', '-p', str(getattr(a, name)), *cmd], capture_output=True, check=True)
            stderr = r.stderr.decode()
            match = re.search(r'stop: .*? — (.*?) insns.*?emulated ([\d.]+)s \((\d+) cycles\)', stderr)
            if not match:
                raise RuntimeError('missing execution totals: ' + stderr[-1200:])
            row = {'build': name, 'pair': pair, 'warmup': pair == 0, 'userSeconds': float(re.search(r'^user ([\d.]+)$', stderr, re.M)[1]),
                   'aggregateCpuBefore': busy, 'loadAverageAfter': list(os.getloadavg()), 'instructions': match[1],
                   'guestSeconds': float(match[2]), 'cycles': int(match[3]), 'consoleSha256': hashlib.sha256(r.stdout).hexdigest()}
            samples.append(row)
    measured = [s for s in samples if not s['warmup']]
    for field in ['instructions', 'cycles', 'consoleSha256']:
        assert len({s[field] for s in samples}) == 1, (label, field)
    medians = {name: statistics.median(s['userSeconds'] for s in measured if s['build'] == name) for name in ['baseline', 'candidate']}
    records.append({'workload': label, 'seconds': seconds, 'pairs': a.pairs, 'mediansUserSeconds': medians,
                    'candidateChangePercent': round(100 * (medians['candidate'] / medians['baseline'] - 1), 2), 'samples': samples})
    print(label, medians, flush=True)
a.output.write_text(json.dumps({'initialLoadGatePassed': True, 'idleGatePassed': max(gate[-3:]) <= 150, 'aggregateCpuMax': a.cpu_max, 'aggregateCpuSamples': gate,
                               'inputsSha256': inputs, 'workloads': records}, indent=2) + '\n')
