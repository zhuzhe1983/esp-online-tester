"""Screening: host instructions retired / cycles / user CPU (/usr/bin/time -l) per binary and fixture.
usage: ir.py <seconds> <bin>... (first bin is the reference); FIXTURES env selects fixtures.
Also checks the emulator's retired count and console hash against the reference."""
import subprocess, sys, re, json, hashlib
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent))
from fixtures import fixture_args, FIXTURES
secs, bins = sys.argv[1], sys.argv[2:]
for n in FIXTURES:
    ref = None
    for b in bins:
        r = subprocess.run(['/usr/bin/time', '-l', b] + fixture_args(n) + ['--max-seconds', secs], capture_output=True)
        e = r.stderr.decode(errors='replace')
        rec = {'fixture': n, 'bin': b.split('/bins/')[-1], 'rc': r.returncode,
               'host_insns': int(re.search(r'(\d+)\s+instructions retired', e)[1]),
               'host_cycles': int(re.search(r'(\d+)\s+cycles elapsed', e)[1]),
               'user': float(re.search(r'([\d.]+) user', e)[1]),
               'stop': re.sub(r' in [0-9.]+s wall = [0-9.]+ Minsn/s', '', (re.findall(r'\[emu\] stop: .*', e) or [''])[0]),
               'console': hashlib.sha256(r.stdout).hexdigest()[:12]}
        if ref is None: ref = rec
        rec['insn_pct'] = round(100 * (rec['host_insns'] / ref['host_insns'] - 1), 2)
        rec['same_work'] = rec['stop'] == ref['stop'] and rec['console'] == ref['console']
        print(json.dumps(rec), flush=True)
