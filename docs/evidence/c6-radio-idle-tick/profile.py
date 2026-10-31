"""Profile a C6 binary on each fixture: 10 s guest smoke + a 5 s host `sample` of a long run."""
import subprocess, sys, time, hashlib, json, re
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent))
from fixtures import fixture_args, FIXTURES
binp, out = Path(sys.argv[1]), Path(sys.argv[2]); out.mkdir(parents=True, exist_ok=True)
names = sys.argv[3:] or FIXTURES
for n in names:
    cmd = ['nice', '-n', '10', str(binp)] + fixture_args(n) + ['--max-seconds', '600']
    so, se = open(out / f'{n}.stdout', 'wb'), open(out / f'{n}.stderr', 'wb')
    p = subprocess.Popen(cmd, stdout=so, stderr=se)
    time.sleep(1.0)
    subprocess.run(['/usr/bin/sample', str(p.pid), '5', '1', '-file', str(out / f'{n}.sample.txt')], capture_output=True)
    p.kill(); p.wait()
    txt = (out / f'{n}.sample.txt').read_text(errors='replace')
    m = txt.split('Sort by top of stack')[-1]
    rows = re.findall(r'^\s+(\S.*?)\s+\(in [^)]+\)\s+(\d+)\s*$', m, re.M)
    tot = sum(int(c) for _, c in rows)
    print('==', n, 'top-of-stack total', tot)
    for s, c in rows[:14]:
        print(f'  {int(c)*100/tot:5.1f}%  {c:>5}  {s[:150]}')
