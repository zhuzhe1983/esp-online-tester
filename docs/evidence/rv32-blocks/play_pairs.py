#!/usr/bin/env python3
"""Playback-phase CPU pairs for the S31 Mosaico music run (EX265, round 4).

    play_pairs.py --base DIR --new DIR [--pairs 7] [--t0 23] [--t1 40] --out FILE.json

Needs S31_FLASH_IMAGE, S31_USB_HOST, S31_SCRIPT (the USB host and touch scripts, private, never
committed) and ESP32SIM_ROM_DIR. Each side runs the same command to T0 and to T1 emulated seconds;
the phase cost is user(T1) - user(T0) of the same pair and side, and the phase speed is
(T1 - T0) / that cost (1.0 = real time on one host core). One warm-up pair, then PAIRS pairs
alternating base->new and new->base (each side's two runs back to back). The record keeps user CPU,
both cores' instruction counts and whether the console reached and stayed in the playing state;
exception and interrupt counts and console hashes are compared, not recorded (private workload).
"""
import argparse, hashlib, json, os, re, statistics, subprocess, sys, time

def args_for(seconds):
    rom = os.path.join(os.environ['ESP32SIM_ROM_DIR'], 'esp32s31_rev0_rom.elf')
    return ['--flash-image', os.environ['S31_FLASH_IMAGE'], '--boot', 'rom', '--rom', rom, '--board', 'mosaico', '--flash-mb', '16',
            '--max-seconds', str(seconds), '--usb-host', os.environ['S31_USB_HOST'], '--script', os.environ['S31_SCRIPT'], '--no-dump']

def timed(binary, seconds):
    t0 = os.times()
    p = subprocess.run([binary] + args_for(seconds), capture_output=True)
    t1 = os.times()
    err, out = p.stderr.decode(errors='replace'), p.stdout.decode(errors='replace')
    m = re.search(r'stop: .*?core0 (\d+) \+ core1 (\d+) insns.*?(\d+) exceptions, (\d+) interrupts', err)
    if not m: sys.exit(f'{binary}: no stop line\n{err[-1500:]}')
    playing = 'PREPARING -> PLAYING' in out and 'PLAYING -> ' not in out
    return {'user_s': round(t1.children_user - t0.children_user, 4), 'insns': [int(m.group(1)), int(m.group(2))],
            'exceptions': int(m.group(3)), 'interrupts': int(m.group(4)), 'playing_at_end': playing,
            '_console': hashlib.sha256(p.stdout).hexdigest()}

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--base', required=True); ap.add_argument('--new', required=True)
    ap.add_argument('--pairs', type=int, default=7); ap.add_argument('--t0', type=float, default=23); ap.add_argument('--t1', type=float, default=40)
    ap.add_argument('--out', required=True)
    a = ap.parse_args()
    exe = 'esp32sim-s31'
    res = {'started': time.strftime('%Y-%m-%dT%H:%M:%S%z'), 'pairs': a.pairs, 't0': a.t0, 't1': a.t1, 'load_before': os.getloadavg(), 'runs': []}
    def side(label, d, pair, order):
        r0 = timed(os.path.join(d, exe), a.t0); r1 = timed(os.path.join(d, exe), a.t1)
        rec = {'side': label, 'pair': pair, 'order': order, 't0': r0, 't1': r1, 'phase_s': round(r1['user_s'] - r0['user_s'], 4)}
        res['runs'].append(rec)
        print(f'  {pair:>6} {order} {label:>4}: to {a.t0:g} s {r0["user_s"]:7.2f}, to {a.t1:g} s {r1["user_s"]:7.2f}, phase {rec["phase_s"]:6.2f} s'
              f' ({(a.t1 - a.t0) / rec["phase_s"]:.2f}x)', file=sys.stderr, flush=True)
    side('new', a.new, 'warmup', 'N->B'); side('base', a.base, 'warmup', 'N->B')
    for i in range(1, a.pairs + 1):
        if i % 2: side('base', a.base, f'pair{i}', 'B->N'); side('new', a.new, f'pair{i}', 'B->N')
        else: side('new', a.new, f'pair{i}', 'N->B'); side('base', a.base, f'pair{i}', 'N->B')
    res['load_after'] = os.getloadavg()
    meas = [r for r in res['runs'] if r['pair'] != 'warmup']
    summ = {}
    for key in ('t0', 't1', 'phase'):
        get = (lambda r: r['phase_s']) if key == 'phase' else (lambda r, k=key: r[k]['user_s'])
        b = [get(r) for r in meas if r['side'] == 'base']; n = [get(r) for r in meas if r['side'] == 'new']
        mb, mn = statistics.median(b), statistics.median(n)
        slower = sum(1 for i in range(1, a.pairs + 1)
                     if get([r for r in meas if r['pair'] == f'pair{i}' and r['side'] == 'new'][0]) > get([r for r in meas if r['pair'] == f'pair{i}' and r['side'] == 'base'][0]))
        summ[key] = {'base_median_s': mb, 'new_median_s': mn, 'change_pct': round((mn / mb - 1) * 100, 2), 'base_range_s': [min(b), max(b)],
                     'new_range_s': [min(n), max(n)], 'new_slower_pairs': slower}
    span = a.t1 - a.t0
    summ['phase']['base_speed_x'] = round(span / summ['phase']['base_median_s'], 3)
    summ['phase']['new_speed_x'] = round(span / summ['phase']['new_median_s'], 3)
    for s in ('base', 'new'):
        rs = [r for r in res['runs'] if r['side'] == s]
        summ[s + '_identical_work_and_console'] = all(
            len({(tuple(r[k]['insns']), r[k]['exceptions'], r[k]['interrupts'], r[k]['_console']) for r in rs}) == 1 for k in ('t0', 't1'))
        summ[s + '_playing_at_end'] = all(r['t1']['playing_at_end'] for r in rs)
    res['summary'] = summ
    for r in res['runs']:
        for k in ('t0', 't1'):
            for f in ('_console', 'exceptions', 'interrupts'): r[k].pop(f)
    print(json.dumps(summ, indent=1), file=sys.stderr)
    res['finished'] = time.strftime('%Y-%m-%dT%H:%M:%S%z')
    with open(a.out, 'w') as f: json.dump(res, f, indent=1)

main()
