#!/usr/bin/env python3
"""Assertion-check the memory-miss dispatch in an isolated checkout."""
import json
import os
from pathlib import Path
import subprocess
import sys

root, out = (Path(p).resolve() for p in sys.argv[1:])
out.mkdir(parents=True, exist_ok=True)
bus = root / 'esp32/src/bus.rs'
psram = root / 'esp32/src/psram.rs'
original = {p: p.read_text() for p in (bus, psram)}
load = 'if (crate::psram::LOW..crate::psram::HIGH).contains(&a) { return self.psram_load(a); }'
store = 'if (crate::psram::LOW..crate::psram::HIGH).contains(&a) { return self.psram_store(a, bytes); }'
mutants = [
    ('drop-load-dispatch', bus, load, '', 'PSRAM load miss dispatch'),
    ('drop-store-dispatch', bus, store, '', 'PSRAM store miss dispatch'),
    ('wrong-load-bound', bus, load, load.replace('crate::psram::HIGH', '(crate::psram::HIGH - 4)'), 'PSRAM load miss dispatch'),
    ('wrong-store-bound', bus, store, store.replace('crate::psram::HIGH', '(crate::psram::HIGH - 4)'), 'PSRAM store miss dispatch'),
    ('wrong-fetch-bound', bus, '(crate::psram::LOW..crate::psram::HIGH).contains(&pc)', '(crate::psram::LOW..=crate::psram::HIGH).contains(&pc)', 'outside PSRAM window'),
    ('absent-as-present', psram, 'self.psram = vec![0; bytes];', 'self.psram = vec![0; if bytes == 0 { 4 << 20 } else { bytes }];', 'absent device has no backing'),
]
env = os.environ.copy()
env.setdefault('CARGO_BUILD_JOBS', '2')
env.setdefault('CARGO_TARGET_DIR', str(out / 'target'))
cmd = ['nice', '-n', '10', 'cargo', '+1.99.0', 'test', '--release', '-p', 'esp32', 'sram_miss_dispatch_preserves_window_and_absent_faults', '--', '--nocapture']
results = []
try:
    for name, path, old, new, assertion in mutants:
        assert original[path].count(old) == 1, (name, 'unique mutation anchor')
        path.write_text(original[path].replace(old, new))
        with (out / (name + '.log')).open('w') as log:
            rc = subprocess.run(cmd, cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT).returncode
        text = (out / (name + '.log')).read_text()
        killed = rc == 101 and 'test result: FAILED' in text and assertion in text and 'panicked at' in text and 'error: could not compile' not in text
        results.append(dict(mutation=name, exit_code=rc, assertion=assertion, assertion_killed=killed))
        path.write_text(original[path])
        (out / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
        assert killed, name
finally:
    for path, text in original.items():
        path.write_text(text)
with (out / 'restored.log').open('w') as log:
    subprocess.run(cmd, cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
print(json.dumps(results, indent=2))
