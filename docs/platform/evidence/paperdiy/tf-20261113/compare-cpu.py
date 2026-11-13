"""Native regression check; all trees and firmware inputs are caller-selected."""
import argparse, datetime, hashlib, json, pathlib, re, statistics, subprocess

p = argparse.ArgumentParser()
for name in ['base', 'candidate', 'root', 'baseline-commit', 'candidate-revision']:
    p.add_argument('--' + name, required=True)
p.add_argument('--pairs', type=int, default=3)
a = p.parse_args()
root = pathlib.Path(a.root)
sha = lambda path: hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()
r = {'checkedAt': datetime.datetime.now(datetime.timezone.utc).isoformat(),
     'baselineCommit': a.baseline_commit, 'candidateRevision': a.candidate_revision,
     'method': 'Alternating main/candidate native runs; median /usr/bin/time -p user seconds. Exact console and instruction totals must match every run.',
     'pairs': a.pairs, 'guestSeconds': 30, 'binaries': {}, 'workloads': {}}
for manifest, binary in [('hello', 'esp32sim'), ('c3-hello', 'esp32sim-c3'), ('c6-hello', 'esp32sim-c6'), ('pocket-tank', 'esp32sim')]:
    cfg = json.loads((root / 'web/wasm/fw' / (manifest + '.json')).read_text())
    args = ['--board', cfg['board'] if manifest == 'pocket-tank' else 'none', '--boot', 'rom', '--max-seconds', '30', '--flash-mb', str(cfg['flash_mb']), '--no-dump']
    if binary == 'esp32sim': args += ['--psram-mb', str(cfg['psram_mb'])]
    for kind, path in cfg['files'].items(): args += ['--' + kind, str(root / 'web/wasm/fw' / path)]
    for offset, path in cfg.get('flash_at', {}).items(): args += ['--flash-at', offset + '=' + str(root / 'web/wasm/fw' / path)]
    samples = {'main': [], 'candidate': []}
    correct = []
    for pair in range(a.pairs):
        for kind, folder in [('main', a.base), ('candidate', a.candidate)]:
            exe = pathlib.Path(folder) / binary
            r['binaries'][kind + '-' + binary] = sha(exe)
            out = subprocess.run(['/usr/bin/time', '-p', str(exe)] + args, capture_output=True, check=True)
            stderr = out.stderr.decode()
            samples[kind].append(float(re.search(r'^user ([0-9.]+)$', stderr, re.M)[1]))
            stop = next(x for x in stderr.splitlines() if '[emu] stop:' in x).split(' in ')[0]
            correct.append((hashlib.sha256(out.stdout).hexdigest(), stop))
    assert len(set(correct)) == 1, (manifest, correct)
    med = {k: statistics.median(v) for k, v in samples.items()}
    inputs = {k: sha(root / 'web/wasm/fw' / v) for k, v in cfg['files'].items()}
    inputs.update({'flash_at_' + k: sha(root / 'web/wasm/fw' / v) for k, v in cfg.get('flash_at', {}).items()})
    r['workloads'][manifest] = {'userSeconds': samples, 'medianUserSeconds': med,
                              'candidateDeltaPercent': (med['candidate'] / med['main'] - 1) * 100,
                              'stdoutSha256': correct[0][0], 'instructions': correct[0][1], 'inputs': inputs}
print(json.dumps(r, indent=2))
