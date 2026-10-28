#!/usr/bin/env python3
"""Rerun inherited P1/P2/P3 probes with P4b's renamed mode/match expressions.

Only mutation anchors change; inherited tests and expectations are untouched.
Use one idle caller-owned tree and an external CARGO_TARGET_DIR (four jobs).
"""
import os
from pathlib import Path
import subprocess
import sys

root, out = (Path(p).resolve() for p in sys.argv[1:3])
assert os.environ.get('CARGO_TARGET_DIR'), 'set your own CARGO_TARGET_DIR'
out.mkdir(parents=True, exist_ok=True)
env = os.environ.copy()
env['CARGO_BUILD_JOBS'] = '4'
commands = []
for lane, source in [('p1', 'classic-psram-spi/mutations.py'),
                     ('p2', 'classic-psram-mmu/mutate.py'),
                     ('p3', 'classic-psram-cache/mutate.py')]:
 text = (root/'docs/evidence'/source).read_text()
 if lane == 'p2':
  text = text.replace("'(app_live && entry != self.mmu[1][slot])', '(app_live && false)'",
                      "'(!lowhigh && app_live && entry != self.mmu[1][slot])', '(app_live && false)'")
  text = text.replace("env.update(CARGO_TARGET_DIR=str(out / 'target'), CARGO_BUILD_JOBS='2')",
                      "env.update(CARGO_BUILD_JOBS='4')")
 if lane == 'p3':
  text = text.replace("'ctrl & modes != 0','ctrl & modes != 0 && false'",
                      "'lowhigh != (app & (1 << 14) != 0)','false'")
 script = out/(lane+'.py')
 script.write_text(text)
 args = ['--repo', str(root), '--out', str(out/lane)] if lane == 'p2' else [str(root), str(out/lane)]
 with (out/(lane+'.log')).open('w') as log:
  p = subprocess.run([sys.executable, str(script)] + args, env=env, stdout=log, stderr=subprocess.STDOUT)
 print(lane, p.returncode, flush=True)
 if p.returncode: raise SystemExit(p.returncode)
