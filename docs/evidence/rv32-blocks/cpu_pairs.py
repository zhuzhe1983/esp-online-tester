#!/usr/bin/env python3
"""Alternating CPU-time pairs for two emulator builds, as CONTRIBUTING.md's "Speed" asks.

    cpu_pairs.py --base DIR --new DIR [--pairs 7] [--workloads s3-hello,c3-hello,c6-hello,pocket-tank] --out FILE.json

DIR holds esp32sim, esp32sim-c3, esp32sim-c6 and esp32sim-s31 from `cargo build --release --bins`. Run from the
repository root (the workloads use web/wasm/fw/public and web/wasm/fw/local). Per workload: one
warmup pair (new, base), then PAIRS measured pairs alternating base->new and new->base. Each run's
child user CPU comes from wait4's rusage; its instruction count from the emulator's stop line and
its console from the SHA-256 of stdout. The change is the ratio of the medians.
"""
import argparse, hashlib, json, os, re, statistics, subprocess, sys, time

ROM = os.environ.get('ESP32SIM_ROM_DIR', '')
FW, LOCAL = 'web/wasm/fw/public', 'web/wasm/fw/local'

def workloads():
    r = lambda n: os.path.join(ROM, n)
    return {
        's3-hello': ('esp32sim', ['--boot', 'rom', '--rom', r('esp32s3_rev0_rom.elf'), '--board', 'none', '--bootloader', f'{FW}/hello-bootloader.bin',
                                  '--ptable', f'{FW}/hello-ptable.bin', '--app', f'{FW}/hello_world.bin', '--max-seconds', '30', '--no-dump']),
        'c3-hello': ('esp32sim-c3', ['--boot', 'rom', '--rom', r('esp32c3_rev3_rom.elf'), '--flash-mb', '4', '--bootloader', f'{FW}/c3-hello-bootloader.bin',
                                     '--ptable', f'{FW}/c3-hello-ptable.bin', '--app', f'{FW}/c3-hello_world.bin', '--max-seconds', '30', '--no-dump']),
        'c6-hello': ('esp32sim-c6', ['--boot', 'rom', '--rom', r('esp32c6_rev0_rom.elf'), '--flash-mb', '4', '--bootloader', f'{FW}/c6-hello-bootloader.bin',
                                     '--ptable', f'{FW}/c6-hello-ptable.bin', '--app', f'{FW}/c6-hello_world.bin', '--max-seconds', '30', '--no-dump']),
        'pocket-tank': ('esp32sim', ['--boot', 'rom', '--rom', r('esp32s3_rev0_rom.elf'), '--board', 'waveshare-amoled18-v2', '--flash-mb', '16', '--psram-mb', '8',
                                     '--bootloader', f'{FW}/pocket-tank-bootloader.bin', '--ptable', f'{FW}/pocket-tank-ptable.bin', '--app', f'{FW}/pocket-tank.bin',
                                     '--flash-at', f'0x290000={LOCAL}/model_q4.bin', '--max-seconds', '8', '--no-dump']),
        # EX265: a private flash dump; the caller names it and the USB-host script (never committed)
        's31-mosaico': ('esp32sim-s31', ['--flash-image', os.environ.get('S31_FLASH_IMAGE', ''), '--boot', 'rom', '--rom', r('esp32s31_rev0_rom.elf'),
                                          '--board', 'mosaico', '--flash-mb', '16', '--max-seconds', '10', '--usb-host', os.environ.get('S31_USB_HOST', ''), '--no-dump']),
        # EX265 round 4: the same, to 40 s, with a touch script that opens the music app (S31_MUSIC_SCRIPT, private)
        's31-music': ('esp32sim-s31', ['--flash-image', os.environ.get('S31_FLASH_IMAGE', ''), '--boot', 'rom', '--rom', r('esp32s31_rev0_rom.elf'),
                                        '--board', 'mosaico', '--flash-mb', '16', '--max-seconds', '40', '--usb-host', os.environ.get('S31_USB_HOST', ''),
                                        '--script', os.environ.get('S31_MUSIC_SCRIPT', ''), '--no-dump']),
        'c3-wifi-station': ('esp32sim-c3', ['--rom', r('esp32c3_rev3_rom.elf'), '--boot', 'rom', '--flash-mb', '4', '--no-dump',
                                            '--bootloader', f'{FW}/c3-wifi-bootloader.bin', '--ptable', f'{FW}/c3-wifi-ptable.bin', '--app', f'{FW}/c3-wifi_station.bin',
                                            '--wifi', 'ssid=esp32sim,psk=esp32sim-pass', '--net', 'none', '--max-seconds', '14']),
        'c6-wifi-station': ('esp32sim-c6', ['--rom', r('esp32c6_rev0_rom.elf'), '--boot', 'rom', '--flash-mb', '4', '--console', 'usb', '--no-dump',
                                            '--bootloader', f'{FW}/c6-wifi-bootloader.bin', '--ptable', f'{FW}/c6-wifi-ptable.bin', '--app', f'{FW}/c6-wifi_station.bin',
                                            '--wifi', 'ssid=esp32sim,psk=esp32sim-pass', '--net', 'none', '--max-seconds', '14']),
    }

def timed(binary, args):
    t0 = os.times()
    p = subprocess.run([binary] + args, capture_output=True)
    t1 = os.times()
    user = t1.children_user - t0.children_user
    err = p.stderr.decode(errors='replace')
    m = re.search(r'stop: .*?(?:core0 (\d+) \+ core1 (\d+)|(\d+)) insns', err)
    if not m: sys.exit(f'{binary}: no stop line\n{err[-1500:]}')
    insns = [int(m.group(1)), int(m.group(2))] if m.group(1) else [int(m.group(3))]
    return {'user_s': round(user, 4), 'insns': insns, 'console_sha256': hashlib.sha256(p.stdout).hexdigest(), 'exit': p.returncode}

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--base', required=True); ap.add_argument('--new', required=True)
    ap.add_argument('--pairs', type=int, default=7)
    ap.add_argument('--workloads', default='s3-hello,c3-hello,c6-hello,pocket-tank')
    ap.add_argument('--out', required=True)
    a = ap.parse_args()
    table = workloads()
    result = {'started': time.strftime('%Y-%m-%dT%H:%M:%S%z'), 'pairs': a.pairs, 'workloads': {}}
    for name in a.workloads.split(','):
        exe, args = table[name]
        b, n = os.path.join(a.base, exe), os.path.join(a.new, exe)
        rec = {'command': [exe] + args, 'load_before': os.getloadavg(), 'runs': []}
        def one(label, path, pair, order):
            r = timed(path, args); r.update(side=label, pair=pair, order=order); rec['runs'].append(r)
            print(f'  {name} {pair:>6} {order} {label:>4}: {r["user_s"]:8.3f} s  {r["insns"]}', file=sys.stderr, flush=True)
        one('new', n, 'warmup', 'N->B'); one('base', b, 'warmup', 'N->B')
        for i in range(1, a.pairs + 1):
            if i % 2: one('base', b, f'pair{i}', 'B->N'); one('new', n, f'pair{i}', 'B->N')
            else: one('new', n, f'pair{i}', 'N->B'); one('base', b, f'pair{i}', 'N->B')
        rec['load_after'] = os.getloadavg()
        meas = [r for r in rec['runs'] if r['pair'] != 'warmup']
        base = [r['user_s'] for r in meas if r['side'] == 'base']; new = [r['user_s'] for r in meas if r['side'] == 'new']
        mb, mn = statistics.median(base), statistics.median(new)
        slower = sum(1 for i in range(1, a.pairs + 1)
                     if [r['user_s'] for r in meas if r['pair'] == f'pair{i}' and r['side'] == 'new'][0] > [r['user_s'] for r in meas if r['pair'] == f'pair{i}' and r['side'] == 'base'][0])
        same = len({(tuple(r['insns']), r['console_sha256']) for r in rec['runs']}) == 1
        rec['summary'] = {'base_median_s': mb, 'new_median_s': mn, 'change_pct': round((mn / mb - 1) * 100, 2), 'base_range_s': [min(base), max(base)],
                          'new_range_s': [min(new), max(new)], 'new_slower_pairs': slower, 'identical_work_and_console': same,
                          'insns': rec['runs'][0]['insns'], 'console_sha256': rec['runs'][0]['console_sha256']}
        print(f'{name}: base {mb:.3f} new {mn:.3f} ({rec["summary"]["change_pct"]:+.2f}%), new slower {slower}/{a.pairs}, identical {same}', file=sys.stderr, flush=True)
        result['workloads'][name] = rec
    result['finished'] = time.strftime('%Y-%m-%dT%H:%M:%S%z')
    with open(a.out, 'w') as f: json.dump(result, f, indent=1)

main()
