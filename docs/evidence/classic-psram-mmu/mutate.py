#!/usr/bin/env python3
"""Run P2 semantic negatives in a disposable, clean worktree. No reset/checkout."""
import argparse
import json
import os
from pathlib import Path
import subprocess

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--repo', type=Path, required=True)
p.add_argument('--out', type=Path, required=True)
a = p.parse_args()
root = a.repo.resolve()
out = a.out.resolve()
out.mkdir(parents=True, exist_ok=True)
env = os.environ.copy()
env.update(CARGO_TARGET_DIR=str(out / 'target'), CARGO_BUILD_JOBS='2')
mutations = [
    ('sram-slot', 'psram.rs', 'const SRAM_SLOT: usize = 1152;', 'const SRAM_SLOT: usize = 1151;'),
    ('sram-shift', 'psram.rs', 'const SRAM_SHIFT: u32 = 15;', 'const SRAM_SHIFT: u32 = 16;'),
    ('ignore-invalid', 'psram.rs', 'entry & 0x100 != 0 || ', 'false || '),
    ('ignore-app', 'psram.rs', '(app_live && entry != self.mmu[1][slot])', '(app_live && false)'),
    ('flat-bypass', 'psram.rs', '(entry as usize & 255) * SRAM_PAGE', '((a - LOW) as usize / SRAM_PAGE) * SRAM_PAGE'),
    ('no-zero-reboot', 'psram.rs', 'self.psram.fill(0);', '// no zero'),
    ('no-mmu-reset', 'psram.rs', 'table[SRAM_SLOT..SRAM_SLOT + 128].fill(0x100);', 'let _ = table;'),
    ('replace-on-reboot', 'psram.rs', 'self.psram.fill(0);', 'self.psram = vec![0; self.psram.len()];'),
    ('allow-fetch', 'bus.rs', 'return Err(Fault::Prohibited);', 'return self.psram_load(pc);'),
    ('ignore-bounds', 'psram.rs', 'if *off >= self.psram.len() { return Err(Fault::Unmapped); }', '// no bounds fault'),
    ('partial-store', 'psram.rs', 'fn psram_store_width<const N: usize>(&mut self, addr: u32, bytes: &[u8; N]) -> Result<(), Fault> {',
     'fn psram_store_width<const N: usize>(&mut self, addr: u32, bytes: &[u8; N]) -> Result<(), Fault> {\n'
     '        for (i, byte) in bytes.iter().enumerate() { let o = self.psram_offsets::<1>(addr + i as u32)?; self.psram[o[0]] = *byte; }'),
]
original = {f: (root / 'esp32/src' / f).read_text() for _, f, _, _ in mutations}
results = []
for name, f, old, new in mutations:
    path = root / 'esp32/src' / f
    text = original[f]
    assert text.count(old) == 1, (name, text.count(old))
    path.write_text(text.replace(old, new))
    try:
        log = out / (name + '.log')
        with log.open('w') as stream:
            r = subprocess.run(['nice', '-n', '10', 'cargo', '+1.99.0', 'test', '--release', '-p', 'esp32', '--lib', 'psram'],
                               cwd=root, env=env, stdout=stream, stderr=subprocess.STDOUT)
        lines = log.read_text().splitlines()
        killed = r.returncode == 101 and any('test result: FAILED' in l for l in lines)
        results.append(dict(name=name, file=f, old=old, new=new, exit_code=r.returncode, killed=killed,
                            failures=[l for l in lines if l.startswith('test psram') and l.endswith('FAILED')]))
        (out / 'mutations.json').write_text(json.dumps(results, indent=2) + '\n')
        print(name, killed, flush=True)
    finally:
        path.write_text(text)
    if not killed:
        raise SystemExit('Mutation survived or did not compile: ' + name)
