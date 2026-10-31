"""Exact-work comparison: run each fixture (FIXTURES, default the four C6 ones) for N guest seconds on each binary; compare
retired count, scheduler work, interrupt report and console hash.
usage: exact.py <out-dir> <seconds> <bin>...  (first bin is the reference)"""
import subprocess, sys, hashlib, json, re
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent))
from fixtures import fixture_args, FIXTURES
out, secs, bins = Path(sys.argv[1]), sys.argv[2], [Path(b) for b in sys.argv[3:]]
out.mkdir(parents=True, exist_ok=True)
rows, ok = [], True
for n in FIXTURES:
    ref = None
    for b in bins:
        r = subprocess.run(['nice', '-n', '10', str(b)] + fixture_args(n) + ['--max-seconds', secs], capture_output=True)
        (out / f'{n}-{b.parent.name}-{b.name}.stdout').write_bytes(r.stdout); (out / f'{n}-{b.parent.name}-{b.name}.stderr').write_bytes(r.stderr)
        err = r.stderr.decode(errors='replace')
        err = re.sub(r'in [0-9.]+s wall = [0-9.]+ Minsn/s', 'in <wall>', err)
        err = '\n'.join(l for l in err.splitlines() if 'loaded from' not in l)
        rec = {'fixture': n, 'bin': f'{b.parent.name}/{b.name}', 'rc': r.returncode,
               'retired': (re.findall(r'retired=(\d+)', err) or [None])[0],
               'stop': (re.findall(r'\[emu\] stop: .*', err) or [None])[0],
               'console_sha256': hashlib.sha256(r.stdout).hexdigest(), 'stderr_sha256': hashlib.sha256(err.encode()).hexdigest()}
        if ref is None: ref = rec
        rec['same_as_ref'] = all(rec[k] == ref[k] for k in ('rc', 'retired', 'stop', 'console_sha256', 'stderr_sha256'))
        ok &= rec['same_as_ref']
        rows.append(rec); print(json.dumps(rec), flush=True)
(out / 'exact.json').write_text(json.dumps({'seconds': secs, 'all_identical': ok, 'rows': rows}, indent=1) + '\n')
print('ALL_IDENTICAL' if ok else 'DIFFERENT')
