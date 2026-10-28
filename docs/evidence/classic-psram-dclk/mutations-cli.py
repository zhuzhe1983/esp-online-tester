#!/usr/bin/env python3
"""Exercise CLI size wiring by removing it, restoring source even on failure.
Run only in an idle, caller-owned source worktree with ESP32SIM_ROM_DIR set.
"""
import json
import os
from pathlib import Path
import subprocess
import sys

root = Path(sys.argv[1]).resolve()
out = Path(sys.argv[2]).resolve()
out.mkdir(parents=True, exist_ok=True)
path = root / 'cli/src/lib.rs'
original = path.read_text()
old = 'm.bus.set_psram_size(mb << 20).unwrap_or_else(|e| usage_error(&e));'
assert original.count(old) == 1
try:
    path.write_text(original.replace(old, 'let _ = mb;'))
    cmd = ['nice', '-n', '10', 'cargo', '+1.99.0', 'test', '--release', '-j2',
           '-p', 'esp32sim', '--test', 'classic_psram_option',
           'classic_psram_cli_reaches_guest_allocator', '--', '--include-ignored', '--nocapture']
    with (out / 'cli-size-wiring.log').open('w') as log:
        run = subprocess.run(cmd, cwd=root, env=os.environ.copy(), stdout=log, stderr=subprocess.STDOUT)
    log = (out / 'cli-size-wiring.log').read_text()
    killed = run.returncode != 0 and 'classic_psram_cli_reaches_guest_allocator ... FAILED' in log and 'panicked at' in log
    result = dict(mutation='cli-size-wiring', test='classic_psram_cli_reaches_guest_allocator',
                  returncode=run.returncode, killed=killed)
    (out / 'results.json').write_text(json.dumps([result], indent=2) + '\n')
    print(result)
    assert killed, log[-4000:]
finally:
    path.write_text(original)
