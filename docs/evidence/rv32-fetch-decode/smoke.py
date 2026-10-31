"""Daytime A/B smoke: user CPU (wait4) per fixture (FIXTURES), 1 warmup pair + 3 alternating pairs
(order alternates per pair), load average before every run, console hash checked per run.
usage: smoke.py <out.json> <seconds> <base-bin> <cand-bin>"""
import subprocess, sys, os, json, hashlib, statistics, time
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent))
from fixtures import fixture_args, FIXTURES
out, secs, base, cand = Path(sys.argv[1]), sys.argv[2], sys.argv[3], sys.argv[4]
def run(b, n):
    load = os.getloadavg()[0]
    p = subprocess.Popen([b] + fixture_args(n) + ['--max-seconds', secs], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    data = p.stdout.read()
    _, st, ru = os.wait4(p.pid, 0)
    return {'user_s': ru.ru_utime, 'sys_s': ru.ru_stime, 'rc': st, 'load1': load, 'console_sha256': hashlib.sha256(data).hexdigest()}
res = {'seconds': secs, 'base': base, 'cand': cand, 'fixtures': {}}
for n in FIXTURES:
    runs = []
    for i in range(4):
        order = [('base', base), ('cand', cand)] if i % 2 == 0 else [('cand', cand), ('base', base)]
        pair = {}
        for arm, b in order: pair[arm] = run(b, n)
        pair['warmup'] = i == 0; pair['order'] = [a for a, _ in order]
        runs.append(pair)
        print(n, i, {a: round(pair[a]['user_s'], 3) for a in ('base', 'cand')}, 'load1', round(pair[order[0][0]]['load1'], 2), flush=True)
    measured = [r for r in runs if not r['warmup']]
    b = [r['base']['user_s'] for r in measured]; c = [r['cand']['user_s'] for r in measured]
    hashes = {r[a]['console_sha256'] for r in runs for a in ('base', 'cand')}
    res['fixtures'][n] = {'runs': runs, 'base_median_user_s': statistics.median(b), 'cand_median_user_s': statistics.median(c),
        'median_change_pct': (statistics.median(c) / statistics.median(b) - 1) * 100,
        'cand_slower_pairs': sum(ci > bi for bi, ci in zip(b, c)), 'pairs': len(b),
        'console_identical': len(hashes) == 1, 'all_rc0': all(r[a]['rc'] == 0 for r in runs for a in ('base', 'cand'))}
    print(n, json.dumps({k: v for k, v in res['fixtures'][n].items() if k != 'runs'}), flush=True)
    out.write_text(json.dumps(res, indent=1) + '\n')
print('done')
