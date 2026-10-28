#!/usr/bin/env python3
"""Run unchanged classic core with locally built public P0 fixtures; no stubs.
Usage: capture.py --inputs DIR --emulator FILE --rom FILE --output DIR
DIR contains fw-A2 and fw-A30 from recipe.sh. Run from repository root.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

p = argparse.ArgumentParser()
p.add_argument('--inputs', type=Path, required=True)
p.add_argument('--emulator', type=Path, required=True)
p.add_argument('--rom', type=Path, required=True)
p.add_argument('--output', type=Path, required=True)
a = p.parse_args()
a.output.mkdir(parents=True, exist_ok=True)
results = {}
for label in ['A2', 'A30', 'baseline']:
    fw = a.inputs / ('fw-' + label) / 'p0_probe.ino'
    files = [str(fw) + ext for ext in ['.bootloader.bin', '.partitions.bin', '.bin']]
    if label == 'baseline':
        files = ['web/wasm/fw/public/' + f + '.bin' for f in
                 ['classic-hello-bootloader', 'classic-hello-ptable', 'classic-hello_world']]
    cmd = [str(a.emulator.resolve()), '--chip', 'esp32', '--boot', 'rom', '--rom',
           str(a.rom.resolve()), '--board', 'esp32dev', '--flash-mb', '4',
           '--max-insns', '150000000', '--no-reboot', '--no-dump']
    for flag, file in zip(['--bootloader', '--ptable', '--app'], files):
        cmd += [flag, file]
    if label != 'baseline':
        cmd += ['--elf', str(fw) + '.elf', '--debug', 'mmio']
        for prefix in ['psram', 'esp_spiram', 'esp_psram', 'cache_sram_mmu']:
            cmd += ['--trace-fn', prefix]
    raw = a.output / (label + '.raw')
    with raw.open('wb') as err:
        run = subprocess.run(['nice', '-n', '10'] + cmd, stdout=subprocess.PIPE,
                             stderr=err, timeout=180, check=True)
    serial = run.stdout
    (a.output / (label + '.serial')).write_bytes(serial)
    text = raw.read_text()
    trace = []
    for line in text.splitlines():
        # Preserve chronological SPI0/1 and DPORT control accesses, plus linked fn entries.
        m = re.search(r'\((0x[0-9a-f]+)\)', line)
        if line.startswith('[fn]') or (m and
            (0x3ff42000 <= int(m[1], 16) < 0x3ff44000 or
             0x3ff00000 <= int(m[1], 16) < 0x3ff00080)):
            trace.append(line)
    # Retain only the init/probe window, not unrelated flash traffic.
    start = next((i for i, t in enumerate(trace) if ' c0 psramInit(' in t), 0)
    end = max((i for i, t in enumerate(trace) if ' c0 psram_cmd_end(' in t), default=-1)
    trace = trace[start:end + 26] if end >= start else []
    (a.output / (label + '.trace')).write_text('\n'.join(trace) + '\n')
    match = re.search(r'\[emu\] stop: .*', text)
    assert match is not None, 'missing bounded stop report'
    stop = match[0]
    # Wall time/rate is not a benchmark or correctness pin.
    stop = re.sub(r' in .*?; emulated', '; emulated', stop)
    results[label] = {'serial_bytes': len(serial), 'serial_sha256': hashlib.sha256(serial).hexdigest(),
                      'stop': stop, 'trace_lines': len(trace),
                      'inputs': {Path(f).name: hashlib.sha256(Path(f).read_bytes()).hexdigest() for f in files}}
    if label != 'baseline':
        assert b'P0 found=0 heap=0 free=0' in serial
        assert b'P0 result=no-psram' in serial
        assert b'PSRAM ID read error: 0x00164020' in serial
        assert not any(' c0 cache_sram_mmu' in t for t in trace)
        assert '[wr]' in '\n'.join(trace)
    else:
        assert b'Hello' in serial
    raw.unlink()
(a.output / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
print(json.dumps(results, indent=2))
