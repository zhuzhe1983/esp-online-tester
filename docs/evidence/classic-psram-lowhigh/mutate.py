#!/usr/bin/env python3
"""P4b semantic mutations; run only in an idle caller-owned source tree.

CARGO_TARGET_DIR must point outside the source tree. Sources are restored in
finally. A kill requires the named test panic, never just a compiler error.
"""
import json
import os
from pathlib import Path
import subprocess
import sys

root, out = (Path(p).resolve() for p in sys.argv[1:3])
out.mkdir(parents=True, exist_ok=True)
assert os.environ.get('CARGO_TARGET_DIR'), 'set your own CARGO_TARGET_DIR'
widths = 'lowhigh_both_lx6_cpus_load_store_all_widths_both_halves'
cross = 'lowhigh_cross_half_atomic_validation_and_table_selection'
gates = 'lowhigh_cache_disable_masks_and_page_mode_are_per_half'
modes = 'lowhigh_split_combined_and_live_mixed_modes_rejected'
mutations = [
 ('swap-tables', 'self.mmu[core][slot]', 'self.mmu[1 - core][slot]', widths),
 ('always-pro', 'self.mmu[core][slot]', 'self.mmu[0][slot]', widths),
 ('boundary-off-by-one', 'a >= LOW + (2 << 20)', 'a > LOW + (2 << 20)', widths),
 ('fold-upper-slots', 'self.mmu[core][slot]', 'self.mmu[core][slot - 64 * core]', widths),
 ('drop-hl-acceptance', 'Ok(lowhigh)', 'if lowhigh { Err(Fault::Prohibited) } else { Ok(false) }', widths),
 ('force-matched-tables', '!lowhigh && app_live', 'app_live', widths),
 ('drop-invalid', 'entry & 0x100 != 0 || ', 'false || ', cross),
 ('drop-half-gate', 'if lowhigh { self.psram_half_gate(core)?; }', 'if lowhigh { let _ = core; }', gates),
 ('drop-cache-enable', 'ctrl & 8 == 0 || ', 'false || ', gates),
 ('drop-dram1-mask', '((7 << 6) | 0x28)', '((7 << 6) | 0x20)', gates),
 ('drop-opsdram-mask', '((7 << 6) | 0x28)', '((7 << 6) | 8)', gates),
 ('drop-page-mode', '((7 << 6) | 0x28)', '0x28', gates),
 ('gate-both-halves', 'if lowhigh { self.psram_half_gate(core)?; }', 'if lowhigh { self.psram_half_gate(0)?; self.psram_half_gate(1)?; }', gates),
 ('drop-pro-split', 'pro & (1 << 11) != 0', 'false', modes),
 ('drop-app-split', 'app & (1 << 11) != 0', 'false', modes),
 ('drop-mixed-rejection', 'lowhigh != (app & (1 << 14) != 0)', 'false', modes),
 ('partial-crossing-store', 'fn psram_store_width<const N: usize>(&mut self, addr: u32, bytes: &[u8; N]) -> Result<(), Fault> {',
  'fn psram_store_width<const N: usize>(&mut self, addr: u32, bytes: &[u8; N]) -> Result<(), Fault> {\n'
  '        for (i, byte) in bytes.iter().enumerate() { let o = self.psram_offsets::<1>(addr + i as u32)?; self.psram[o[0]] = *byte; }', cross),
]
path = root / 'esp32/src/psram.rs'
original = path.read_text()
results = []
try:
 for name, old, new, test in mutations:
  assert original.count(old) == 1, (name, original.count(old))
  path.write_text(original.replace(old, new))
  cmd = ['nice', '-n', '10', 'cargo', '+1.99.0', 'test', '--release', '-j4', '-p', 'esp32', '--lib', test, '--', '--nocapture']
  with (out/(name+'.log')).open('w') as log:
   run = subprocess.run(cmd, cwd=root, stdout=log, stderr=subprocess.STDOUT)
  text = (out/(name+'.log')).read_text()
  killed = run.returncode == 101 and f'{test} ... FAILED' in text and 'panicked at' in text
  results.append(dict(name=name, old=old, new=new, test=test, exit=run.returncode, killed=killed))
  (out/'mutations.json').write_text(json.dumps(results, indent=2)+'\n')
  print(name, killed, flush=True)
  if not killed:
   print(text[-4000:])
   raise SystemExit('survivor or nonsemantic failure')
finally:
 path.write_text(original)
