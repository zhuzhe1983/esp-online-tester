#!/usr/bin/env python3
"""Semantic guest-fixture negatives; disposable clean worktree required.
Usage: mutate.py MUTANT_WORKTREE OUTPUT_DIRECTORY
Set CARGO_TARGET_DIR and ESP32SIM_ROM_DIR. Compiler errors are NOT kills.
"""
import json
import os
from pathlib import Path
import subprocess
import sys

repo, out = map(lambda s: Path(s).resolve(), sys.argv[1:])
assert os.environ.get('CARGO_TARGET_DIR'), 'set caller-owned CARGO_TARGET_DIR'
assert os.environ.get('ESP32SIM_ROM_DIR'), 'set ESP32SIM_ROM_DIR'
assert not subprocess.check_output(['git', 'status', '--porcelain'], cwd=repo), 'require clean disposable worktree'
out.mkdir(parents=True, exist_ok=True)
source = repo / 'esp32/src/psram.rs'
original = source.read_text()
cases = [
    ('sram-slot-offset', 'const SRAM_SLOT: usize = 1152;', 'const SRAM_SLOT: usize = 1153;'),
    ('sram-page-shift', 'const SRAM_SHIFT: u32 = 15;', 'const SRAM_SHIFT: u32 = 16;'),
    ('byte-store-corruption', 'self.psram[off] = *byte;', 'self.psram[off] = *byte ^ u8::from(N == 1);'),
    ('byte-load-corruption', 'offsets.map(|o| self.psram[o])', 'offsets.map(|o| self.psram[o] ^ u8::from(N == 1))'),
    ('half-device-backing', 'self.psram = vec![0; bytes];', 'self.psram = vec![0; bytes / 2];'),
    ('physical-page-alias', '(entry as usize & 255) * SRAM_PAGE', '(entry as usize & 0) * SRAM_PAGE'),
    ('reject-live-qpi', '!self.periph.spi1.0.classic_qpi_enabled()', 'self.periph.spi1.0.classic_qpi_enabled()'),
]
cmd = ['nice', '-n', '10', 'cargo', '+1.99.0', 'test', '--release', '-p', 'esp32sim', '--test',
       'classic_psram_idf', 'classic_psram_idf_boot_allocator', '--', '--ignored', '--exact', '--nocapture']
results = []
try:
    for name, old, new in cases:
        assert original.count(old) == 1, name
        source.write_text(original.replace(old, new))
        run = subprocess.run(cmd, cwd=repo, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=240)
        (out / (name + '.log')).write_text(run.stdout)
        killed = (run.returncode == 101 and 'missing guest milestone' in run.stdout
                  and 'panicked at cli/tests/classic_psram_idf.rs:' in run.stdout
                  and 'test classic_psram_idf_boot_allocator ... FAILED' in run.stdout
                  and 'error[E' not in run.stdout and 'could not compile' not in run.stdout)
        results.append({'mutation': name, 'old': old, 'new': new, 'test': 'classic_psram_idf_boot_allocator',
                        'exit_code': run.returncode, 'assertion_killed': killed})
        (out / 'mutations.json').write_text(json.dumps(results, indent=2) + '\n')
        print(name, killed, flush=True)
        source.write_text(original)
finally:
    source.write_text(original)
run = subprocess.run(cmd, cwd=repo, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=240)
(out / 'restored.log').write_text(run.stdout)
assert run.returncode == 0, 'restored fixture must pass'
assert not subprocess.check_output(['git', 'diff', '--name-only'], cwd=repo), 'mutant tree must be restored'
count = sum(row['assertion_killed'] for row in results)
assert count >= 5, 'require at least five runtime assertion kills'
print('restored PASS; semantic assertion kills', count, 'of', len(results))
