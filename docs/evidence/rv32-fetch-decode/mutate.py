"""EX249 mutation campaign: each mutant edits the C6 fetch window in a scratch worktree, runs the
named tests, and counts as killed only when a test FAILS at run time (a compile error is reported
as such, never as a kill). Sources are restored after each case.
usage: mutate.py <mutation-worktree> <out-dir>"""
import subprocess, sys, json, os
from pathlib import Path
tree, out = Path(sys.argv[1]), Path(sys.argv[2]); out.mkdir(parents=True, exist_ok=True)
B = 'esp32c6/src/bus.rs'
T = ['-p', 'esp32c6', '--lib', 'fetch_tests']
FAST = 'if let Some(w) = b.get(o..o + 4) { return Ok([w[0], w[1], w[2], w[3]]); }'
cases = [
 ('fast-byte-order', B, FAST, 'if let Some(w) = b.get(o..o + 4) { return Ok([w[1], w[0], w[2], w[3]]); }', T),
 ('fast-window-shifted', B, FAST, 'if let Some(w) = b.get(o + 1..o + 5) { return Ok([w[0], w[1], w[2], w[3]]); }', T),
 ('fast-last-byte-dropped', B, FAST, 'if let Some(w) = b.get(o..o + 4) { return Ok([w[0], w[1], w[2], 0]); }', T),
 ('fast-window-clamped', B, FAST, 'if let Some(w) = b.get(o..(o + 4).min(b.len())) { return Ok([w[0], w[1], w[2], w[3]]); }', T),
 ('fast-window-short-ok', B, FAST, 'if let Some(w) = b.get(o..(o + 4).min(b.len())) { return Ok([w[0], *w.get(1).unwrap_or(&0xff), *w.get(2).unwrap_or(&0xff), *w.get(3).unwrap_or(&0xff)]); }', T),
 ('tail-not-zero-filled', B, '                for i in 0..4 { if o + i < b.len() { r[i] = b[o + i]; } }\n', '                for i in 0..4 { if o + i < b.len() { r[i] = b[o + i]; } else { r[i] = 0xff; } }\n', T),
]
env = dict(os.environ, CARGO_TARGET_DIR=str(out / 'target'))
rows = []
for name, path, old, new, test in cases:
    p = tree / path; src = p.read_text()
    assert src.count(old) == 1, (name, old)
    p.write_text(src.replace(old, new))
    try:
        r = subprocess.run(['nice', '-n', '10', 'cargo', '+1.99.0', 'test', '--release'] + test, cwd=tree, env=env, capture_output=True, text=True)
    finally:
        p.write_text(src)
    log = r.stdout + r.stderr
    (out / f'{name}.log').write_text(log)
    compiled = 'error[E' not in log and 'could not compile' not in log
    failed = [l.split()[1] for l in log.splitlines() if l.startswith('test ') and l.rstrip().endswith('FAILED')]
    row = {'mutant': name, 'file': path, 'compiled': compiled, 'killed': compiled and r.returncode != 0 and bool(failed), 'failing_tests': failed}
    rows.append(row); print(json.dumps(row), flush=True)
(out / 'mutations.json').write_text(json.dumps(rows, indent=1) + '\n')
print(sum(r['killed'] for r in rows), '/', len(rows), 'killed')
