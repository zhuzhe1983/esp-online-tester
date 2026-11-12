"""802.15.4 idle-tick (EX262) mutation campaign: each mutant edits one changed line of
esp32c6/src/radio.rs in a scratch worktree and runs the esp32c6 library tests, then (if none
failed) every esp32c6 integration test. Killed only when a test FAILS at run time; a compile error
is reported as such. Sources are restored after each case.
usage: mutate.py <mutation-worktree> <out-dir>   (DRY=1: print anchor counts only)"""
import subprocess, sys, json, os
from pathlib import Path
tree, out = Path(sys.argv[1]), Path(sys.argv[2]); out.mkdir(parents=True, exist_ok=True)
T = [(['-p', 'esp32c6', '--lib'], 'esp32c6-lib'), (['-p', 'esp32c6', '--tests'], 'esp32c6-tests')]
L = 'esp32c6/src/radio.rs'
GATE = '        if !self.live && self.ed_left.is_none() { return; }\n'
WR = '    fn write(&mut self, off: u32, v: u32) -> WriteEffect {\n        self.live = true;\n'
RX = ' started_ago: Option<u64>) -> bool {\n        self.live = true;\n'
TX = '    pub fn tx_loaded(&mut self, psdu: Vec<u8>) {\n        self.live = true;\n'
REC = ('        self.live = self.ed_left.is_some() || self.timer.iter().any(|t| t.running)\n'
       '            || [&self.tx_sfd, &self.tx_done, &self.rx_sfd, &self.rx_done, &self.ack_timeout, &self.ack_turnaround, &self.ack_done].iter().any(|c| c.armed());\n')
NEW = 'scene_acc: 0, scene_ticks: 0, live: false,'
def drop(cd): return REC.replace(f'&self.{cd}, ', '').replace(f', &self.{cd}]', ']')
cases = [
 ('control-unmutated', GATE, GATE),
 ('gate-removed', GATE, ''),
 ('gate-inverted', GATE, GATE.replace('!self.live', 'self.live')),
 ('public-ed-ignored', GATE, '        if !self.live { return; }\n'),
 ('write-sets-no-live', WR, WR.replace('        self.live = true;\n', '')),
 ('receive-sets-no-live', RX, RX.replace('        self.live = true;\n', '')),
 ('tx-loaded-sets-no-live', TX, TX.replace('        self.live = true;\n', '')),
 ('recompute-always-false', REC, '        self.live = false;\n'),
 ('recompute-always-true', REC, '        self.live = true;\n'),
 ('recompute-no-ed', REC, REC.replace('self.ed_left.is_some() || ', '')),
 ('recompute-timer0-only', REC, REC.replace('self.timer.iter().any(|t| t.running)', 'self.timer[0].running')),
 ('recompute-no-timers', REC, REC.replace(' || self.timer.iter().any(|t| t.running)', '')),
 ('recompute-and', REC, REC.replace('self.ed_left.is_some() ||', 'self.ed_left.is_some() &&')),
] + [(f'recompute-no-{c}', REC, drop(c)) for c in ['tx_sfd', 'tx_done', 'rx_sfd', 'rx_done', 'ack_timeout', 'ack_turnaround', 'ack_done']] + [
 ('new-live-true', NEW, NEW.replace('live: false', 'live: true')),
]
if os.environ.get('DRY'):
    src = (tree / L).read_text()
    for name, old, new in cases: print(name, src.count(old), old != new or name.startswith('control'), src.replace(old, new).count('self.live') )
    sys.exit(0)
env = dict(os.environ, CARGO_TARGET_DIR=str(out / 'target'))
rows = []
for name, old, new in cases:
    p = tree / L; src = p.read_text()
    assert src.count(old) == 1, (name, old)
    p.write_text(src.replace(old, new))
    log, failed, compiled, rc = '', [], True, 0
    try:
        for args, crate in T:
            r = subprocess.run(['nice', '-n', '10', 'cargo', '+1.99.0', 'test', '--release', '-j3', '--no-fail-fast'] + args, cwd=tree, env=env, capture_output=True, text=True, timeout=2400)
            log += f'== {crate}\n' + r.stdout + r.stderr
            rc |= r.returncode
            compiled &= 'error[E' not in r.stderr and 'could not compile' not in r.stderr
            failed += [f'{crate}::{l.split()[1]}' for l in r.stdout.splitlines() if l.startswith('test ') and l.rstrip().endswith('FAILED')]
            if failed or not compiled: break   # killed (or broken): the remaining stages are not run
    finally:
        p.write_text(src)
    (out / f'{name}.log').write_text(log)
    row = {'mutant': name, 'file': L, 'compiled': compiled, 'killed': compiled and rc != 0 and bool(failed), 'failing_tests': failed}
    rows.append(row); print(json.dumps(row), flush=True)
(out / 'mutations.json').write_text(json.dumps(rows, indent=1) + '\n')
print(sum(r['killed'] for r in rows), '/', len(rows), 'killed')
