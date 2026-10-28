#!/usr/bin/env python3
"""Run independently compiled command-engine mutations; restore source even on failure.
Run with an idle source worktree (never concurrent with qualification/build commands).
"""
import json
import os
from pathlib import Path
import subprocess
import sys

root = Path(sys.argv[1]).resolve()
out = Path(sys.argv[2]).resolve()
out.mkdir(parents=True, exist_ok=True)
shared = root / 'esp-periph/src/spi_mem.rs'
adapter = root / 'esp32/src/periph.rs'
original = {p: p.read_text() for p in (shared, adapter)}
# Every row requires a named Rust assertion failure, not merely a compiler error.
mutants = [
 ('adapter-opt-in', adapter, 's.enable_classic_quad();', 's.has_psram = false;', 'psram_spi_pinned_probe_eight_bytes_and_absence'),
 ('cs-select', shared, 'misc & 1 != 0 && misc & 2 == 0', 'misc & 2 == 0 && false', 'psram_spi_pinned_probe_eight_bytes_and_absence'),
 ('ignore-cs0', shared, 'misc & 1 != 0 && misc & 2 == 0', 'true', 'psram_spi_cs_selection_flash_isolation_and_disabled_cs'),
 ('disabled-cs', shared, 'misc & 3 == 3', 'false', 'psram_spi_cs_selection_flash_isolation_and_disabled_cs'),
 ('stale-user2', shared, 'match self.classic_quad_opcode(user, user1, user2)', 'match Some(user2 & 0xffff)', 'psram_spi_address_entry_mosi_exit_and_reentry'),
 ('address-opcode', shared, 'self.regs.read(4) >> 24\n        } else if !command', 'self.regs.read(4) & 0xff\n        } else if !command', 'psram_spi_address_entry_mosi_exit_and_reentry'),
 ('mosi-opcode', shared, 'self.w[0] & 0xff\n', 'user2 & 0xff\n', 'psram_spi_address_entry_mosi_exit_and_reentry'),
 ('command-phase', shared, 'let command = user & (1 << 31) != 0;', 'let command = true;', 'psram_spi_phase_lengths_and_payload_modes'),
 ('command-length', shared, '(user2 >> 28) & 15 == 7', 'true', 'psram_spi_phase_lengths_and_payload_modes'),
 ('id-address-length', shared, '(user1 >> 26) & 63 == 23', 'true', 'psram_spi_phase_lengths_and_payload_modes'),
 ('entry-state', shared, 'state.qpi = true;', 'state.qpi = false;', 'psram_spi_address_entry_mosi_exit_and_reentry'),
 ('exit-state', shared, '0xf5 if control => { state.qpi = false; None }', '0xf5 if control => { None }', 'psram_spi_address_entry_mosi_exit_and_reentry'),
 ('reset-arm', shared, 'state.reset_enabled = true;', 'state.reset_enabled = false;', 'psram_spi_reset_requires_enable_and_is_consumed'),
 ('reset-unarmed', shared, 'if reset_enabled { state.qpi = false; }', 'if !reset_enabled { state.qpi = false; }', 'psram_spi_reset_requires_enable_and_is_consumed'),
 ('reset-consume', shared, 'std::mem::replace(&mut state.reset_enabled, false)', 'state.reset_enabled', 'psram_spi_reset_requires_enable_and_is_consumed'),
 ('absence', shared, 'self.set_w_bytes(&vec![0xff; miso_bytes]);', 'self.set_w_bytes(&vec![0; miso_bytes]);', 'psram_spi_pinned_probe_eight_bytes_and_absence'),
 ('density', shared, '0x400000 => 0x22', '0x400000 => 0x42', 'psram_spi_pinned_probe_eight_bytes_and_absence'),
 ('eight-byte-tail', shared, 'for w in self.w.iter_mut() { *w = 0; }', 'self.w[0] = 0;', 'psram_spi_pinned_probe_eight_bytes_and_absence'),
 ('qpi-write-mode', shared, '0x38 if state.qpi &&', '0x38 if !state.qpi &&', 'psram_spi_phase_lengths_and_payload_modes'),
 ('spi-write-mode', shared, '0x02 if !state.qpi &&', '0x02 if state.qpi &&', 'psram_spi_phase_lengths_and_payload_modes'),
 ('control-phase', shared, 'let control = !miso && (!command || (delay_command && address && !mosi) || (!address && !mosi));', 'let control = true;', 'psram_spi_phase_lengths_and_payload_modes'),
 ('entry-length', shared, '(user1 >> 26) & 63 == 7', 'true', 'psram_spi_phase_lengths_and_payload_modes'),
 ('exit-length', shared, 'self.regs.read(0x24) == 7', 'true', 'psram_spi_phase_lengths_and_payload_modes'),
 ('address-alignment', adapter, 's.addr_msb_aligned = true;', 's.addr_msb_aligned = false;', 'psram_spi_cs_selection_flash_isolation_and_disabled_cs'),
 ('dclk-id', shared, 'let opcode = if delay_id {', 'let opcode = if false {', 'psram_spi_delay_clock_id_returns_device_id_and_rejects_wrong_phases'),
 ('dclk-id-command-length', shared, '(user2 >> 28) & 15 == 1', 'true', 'psram_spi_delay_clock_id_returns_device_id_and_rejects_wrong_phases'),
 ('dclk-id-command-zero', shared, 'user2 & 0xffff == 0', 'true', 'psram_spi_delay_clock_id_returns_device_id_and_rejects_wrong_phases'),
 ('dclk-id-address-length', shared, '(user1 >> 26) & 63 == 31', 'true', 'psram_spi_delay_clock_id_returns_device_id_and_rejects_wrong_phases'),
 ('dclk-id-miso-length', shared, 'self.regs.read(0x28) == 63', 'true', 'psram_spi_delay_clock_id_returns_device_id_and_rejects_wrong_phases'),
 ('dclk-exit', shared, 'self.w[0] & 0xffff == 0xf500', 'false', 'psram_spi_delay_clock_mosi_exit_restores_spi'),
 ('dclk-entry', shared, '(!command || delay_command) && address', '!command && address', 'psram_spi_delay_clock_entry_enters_qpi'),
]
results = []
env = os.environ.copy()
try:
    for name, path, old, new, test in mutants:
        text = original[path]
        if name == 'absence':
            # First match is the all-CS-disabled guard, second is the absence branch.
            anchor = 'if psize == 0 {'
            a, b = text.split(anchor, 1)
            assert old in b
            edited = a + anchor + b.replace(old, new, 1)
        else:
            assert text.count(old) == 1, (name, text.count(old))
            edited = text.replace(old, new)
        path.write_text(edited)
        cmd = ['nice', '-n', '10', 'cargo', '+1.99.0', 'test', '--release', '-p', 'esp32', '--lib', test, '--', '--nocapture']
        with (out / (name + '.log')).open('w') as f:
            run = subprocess.run(cmd, cwd=root, env=env, stdout=f, stderr=subprocess.STDOUT)
        log = (out / (name + '.log')).read_text()
        killed = run.returncode != 0 and f'{test} ... FAILED' in log and 'panicked at' in log
        row = dict(mutation=name, test=test, returncode=run.returncode, killed=killed)
        results.append(row)
        (out / 'results.json').write_text(json.dumps(results, indent=2) + '\n')
        path.write_text(original[path])
        print(row, flush=True)
        if not killed:
            print(log[-5000:])
            raise SystemExit('survived or did not reach its named assertion')
finally:
    for p, text in original.items():
        p.write_text(text)
