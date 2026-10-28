#!/usr/bin/env python3
"""Run in a disposable P7 worktree, with CARGO_TARGET_DIR outside the checkout.
Every mutant must build for both targets, then fail the SAME named assertion.
Does not reset Git or touch any file outside the caller-supplied worktree/output.
"""
import json
import os
from pathlib import Path
import subprocess
import sys

root = Path.cwd()
out = Path(sys.argv[1]).resolve()
out.mkdir(parents=True, exist_ok=True)
mutations = [
    ('factory-omits-psram', 'wasm/src/lib.rs', 'm.bus.set_psram_size(classic_psram_bytes) {', 'm.bus.set_psram_size(if classic_psram_bytes == 4 << 20 { 0 } else { classic_psram_bytes }) {', 'fresh physical pages contain no stale bytes'),
    ('factory-rejects-unsupported-size', 'wasm/src/lib.rs', '            0\n        };\n        if let Err(reason)', '            return std::ptr::null_mut();\n        };\n        if let Err(reason)', 'unsupported classic PSRAM size must start without PSRAM'),
    ('sram-slot-offset', 'esp32/src/psram.rs', 'const SRAM_SLOT: usize = 1152;', 'const SRAM_SLOT: usize = 1153;', 'fresh physical pages contain no stale bytes'),
    ('sram-page-shift', 'esp32/src/psram.rs', 'const SRAM_SHIFT: u32 = 15;', 'const SRAM_SHIFT: u32 = 16;', 'fresh physical pages contain no stale bytes'),
    ('invalid-entry-ignored', 'esp32/src/psram.rs', 'if entry & 0x100 != 0 ||', 'if false ||', 'assertion `left == right` failed'),
    ('cache-enable-ignored', 'esp32/src/psram.rs', 'if ctrl & 8 == 0 ||', 'if false ||', 'cache disabled'),
    ('cache-mask-ignored', 'esp32/src/psram.rs', 'ctrl1 & ((7 << 6) | 0x28) != 0', 'ctrl1 & (7 << 6) != 0', 'cache masked'),
    ('store-drops-byte', 'esp32/src/psram.rs', 'self.psram[off] = *byte;', 'self.psram[off] = 0;', 'assertion `left == right` failed'),
    ('reset-retains-bytes', 'esp32/src/psram.rs', '        self.psram.fill(0);', '        // mutant retains backing', 're-enabled reset mapping reads zero'),
    ('reset-retains-maps', 'esp32/src/psram.rs', 'for table in &mut self.mmu { table[SRAM_SLOT..SRAM_SLOT + 128].fill(0x100); }', '// mutant retains tables', 'reset clears every SRAM map'),
    ('reboot-skips-psram-reset', 'esp32/src/soc.rs', '        self.reset_psram();', '        // mutant skips PSRAM reset', 'reset clears every SRAM map'),
]

def run(cmd, label):
    with (out / (label + '.log')).open('w') as log:
        r = subprocess.run(cmd, stdout=log, stderr=subprocess.STDOUT, env=os.environ)
    return r.returncode, (out / (label + '.log')).read_text()

rows = []
for name, file, old, new, assertion in mutations:
    path = root / file
    original = path.read_text()
    assert original.count(old) == 1, (name, original.count(old))
    try:
        path.write_text(original.replace(old, new))
        rc, text = run(['nice','-n','10','cargo','+1.99.0','test','--release','-p','esp32sim-wasm','--lib','classic_psram_abi_lifecycle','--','--nocapture'], name + '-native')
        native_killed = rc != 0 and 'error: could not compile' not in text and assertion in text
        rc_build, _ = run(['nice','-n','10','sh','-c', '. ./tools/wasm-rustflags.sh; export RUSTC="$(rustup +1.99.0 which rustc)"; export DYLD_FALLBACK_LIBRARY_PATH="$(dirname "$(dirname "$RUSTC")")/lib"; cargo +1.99.0 build --release --target wasm32-unknown-unknown -p esp32sim-wasm --features jit-tests'], name + '-wasm-build')
        rc_wasm, wasm_text = run(['nice','-n','10','node','tools/psram-wasm-test.mjs','--lifecycle'], name + '-wasm') if rc_build == 0 else (0, '')
        wasm_killed = rc_wasm != 0 and assertion in wasm_text
        row = dict(name=name, assertion=assertion, native_rc=rc, wasm_build_rc=rc_build, wasm_rc=rc_wasm, native_killed=native_killed, wasm_killed=wasm_killed)
        rows.append(row)
        (out/'results.json').write_text(json.dumps(rows,indent=2)+'\n')
        print(json.dumps(row),flush=True)
        assert native_killed and wasm_killed, 'retain logs; mutant must fail assertion on both targets'
    finally:
        path.write_text(original)
print(f'PASS {len(rows)} mutants killed on native and WASM')
