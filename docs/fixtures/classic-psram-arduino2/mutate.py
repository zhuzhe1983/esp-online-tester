#!/usr/bin/env python3
"""P5 semantic assertion mutations. Run only in a clean disposable worktree.
Usage: mutate.py TREE OUTPUT; set ARDUINO2_FIRMWARE_DIR, ESP32SIM_ROM_DIR,
and caller-owned CARGO_TARGET_DIR. Compilation failures are not kills.
"""
import json
import os
from pathlib import Path
import subprocess
import sys

repo, out = (Path(value).resolve() for value in sys.argv[1:])
for key in ('ARDUINO2_FIRMWARE_DIR', 'ESP32SIM_ROM_DIR', 'CARGO_TARGET_DIR'):
    assert os.environ.get(key), f'set {key}'
assert not subprocess.check_output(['git', 'status', '--porcelain'], cwd=repo)
out.mkdir(parents=True, exist_ok=True)
present = 'external_arduino2_psram_startup_and_readback'
absent = 'external_arduino2_no_device_is_rejected'
cases = [
 ('deny-lowhigh', 'esp32/src/psram.rs', 'Ok(lowhigh)', 'if lowhigh { Err(Fault::Prohibited) } else { Ok(false) }', present),
 ('byte-store-corruption', 'esp32/src/psram.rs', 'self.psram[off] = *byte;', 'self.psram[off] = *byte ^ u8::from(N == 1);', present),
 ('byte-load-corruption', 'esp32/src/psram.rs', 'offsets.map(|o| self.psram[o])', 'offsets.map(|o| self.psram[o] ^ u8::from(N == 1))', present),
 ('half-device-backing', 'esp32/src/psram.rs', 'self.psram = vec![0; bytes];', 'self.psram = vec![0; bytes / 2];', present),
 ('physical-page-alias', 'esp32/src/psram.rs', '(entry as usize & 255) * SRAM_PAGE', '(entry as usize & 0) * SRAM_PAGE', present),
 ('reject-live-qpi', 'esp32/src/psram.rs', '!self.periph.spi1.0.classic_qpi_enabled()', 'self.periph.spi1.0.classic_qpi_enabled()', present),
 ('sram-slot-offset', 'esp32/src/psram.rs', 'const SRAM_SLOT: usize = 1152;', 'const SRAM_SLOT: usize = 1153;', present),
 ('sram-page-shift', 'esp32/src/psram.rs', 'const SRAM_SHIFT: u32 = 15;', 'const SRAM_SHIFT: u32 = 16;', present),
 ('absent-id-zero', 'esp-periph/src/spi_mem.rs', 'if psize == 0 {', None, absent),
]
results = []
for name, file, old, new, test in cases:
    path = repo / file
    original = path.read_text()
    assert original.count(old) == 1, name
    if new is None:
        first, rest = original.split(old, 1)
        needle = 'self.set_w_bytes(&vec![0xff; miso_bytes]);'
        assert needle in rest
        edited = first + old + rest.replace(needle, 'self.set_w_bytes(&vec![0; miso_bytes]);', 1)
    else:
        edited = original.replace(old, new)
    cmd = ['nice', '-n', '10', 'cargo', '+1.99.0', 'test', '--release', '-j4', '-p', 'esp32sim',
           '--test', 'classic_psram_arduino2', test, '--', '--ignored', '--exact', '--nocapture']
    try:
        path.write_text(edited)
        run = subprocess.run(cmd, cwd=repo, capture_output=True, text=True, timeout=300)
        text = run.stdout + run.stderr
        (out / (name + '.log')).write_text(text)
        killed = (run.returncode == 101 and f'{test} ... FAILED' in text
                  and 'panicked at cli/tests/classic_psram_arduino2.rs:' in text
                  and 'could not compile' not in text and 'error[E' not in text)
        results.append({'mutation': name, 'test': test, 'exit': run.returncode, 'assertion_killed': killed})
        (out / 'mutations.json').write_text(json.dumps(results, indent=2) + '\n')
        print(name, killed, flush=True)
    finally:
        path.write_text(original)
assert not subprocess.check_output(['git', 'diff', '--name-only'], cwd=repo)
assert all(any(row['test'] == test and row['assertion_killed'] for row in results)
           for test in (present, absent)), 'both named tests require semantic kills'
# Restored controls must use a fresh target: never trust cross-worktree Cargo reuse.
env = os.environ.copy()
env['CARGO_TARGET_DIR'] += '-restored'
cmd = ['nice', '-n', '10', 'cargo', '+1.99.0', 'test', '--release', '-j4', '-p', 'esp32sim',
       '--test', 'classic_psram_arduino2', '--', '--ignored', '--nocapture', '--test-threads=1']
run = subprocess.run(cmd, cwd=repo, env=env, capture_output=True, text=True, timeout=300)
(out / 'restored.log').write_text(run.stdout + run.stderr)
assert run.returncode == 0, 'fresh restored control must pass'
print('restored PASS; kills', sum(row['assertion_killed'] for row in results), '/', len(results))
