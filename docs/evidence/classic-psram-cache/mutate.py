#!/usr/bin/env python3
"""P3 semantic mutations. Use an idle disposable source tree; never concurrent builds."""
import json, os, subprocess, sys
from pathlib import Path
root=Path(sys.argv[1]).resolve(); out=Path(sys.argv[2]).resolve(); out.mkdir(parents=True,exist_ok=True)
coherence='cache_command_cpu_coherence_remap_and_flash_dirty_isolation'
gates='cache_gates_fault_and_restore_without_losing_bytes'
reboot='cache_reboot_resets_controls_device_mappings_and_bytes_in_place'
trace='cache_trace_registers_never_alias_fifo_or_other_controller'
mutants=[
('command-backing','esp32/src/bus.rs','execute(&mut self.flash, &mut self.psram)','execute(&mut self.flash, &mut [])',coherence),
('cpu-store','esp32/src/psram.rs','self.psram[off] = *byte;','self.psram[off] = byte.wrapping_add(1);',coherence),
('cpu-load','esp32/src/psram.rs','offsets.map(|o| self.psram[o])','offsets.map(|o| self.psram[o].wrapping_add(1))',coherence),
('dirty-memory','esp32/src/bus.rs','if memory == esp_periph::DirtyMem::Flash','if memory == esp_periph::DirtyMem::Psram',coherence),
('cache-enable','esp32/src/psram.rs','ctrl & 8 == 0 || ','false || ',gates),
('cache-masks','esp32/src/psram.rs','ctrl1 & ((7 << 6) | 0x28) != 0','ctrl1 & (7 << 6) != 0',gates),
('page-mode','esp32/src/psram.rs','ctrl1 & ((7 << 6) | 0x28) != 0','ctrl1 & 0x28 != 0',gates),
('address-mode','esp32/src/psram.rs','ctrl & modes != 0','ctrl & modes != 0 && false',gates),
('app-cache','esp32/src/psram.rs','for core in 0..cores {','for core in 0..cores.min(1) {',gates),
('device-qpi','esp32/src/psram.rs','!self.periph.spi1.0.classic_qpi_enabled() || ','false || ',gates),
('spi0-cs1','esp32/src/periph.rs','self.0.regs.read(0x34) & 2 == 0','true',gates),
('sctrl-qio','esp32/src/periph.rs','let required = (1 << 2) | (1 << 4) |','let required = (1 << 4) |',gates),
('sctrl-dio','esp32/src/periph.rs','s & (1 << 1) == 0','true',gates),
('sctrl-dummy','esp32/src/periph.rs','let required = (1 << 2) | (1 << 4) |','let required = (1 << 2) |',gates),
('sctrl-address','esp32/src/periph.rs','(s >> 22) & 63 == 23','true',gates),
('command-length','esp32/src/periph.rs','command >> 28 == 7','true',gates),
('read-enable','esp32/src/periph.rs','(1 << if write { 28 } else { 5 })','(if write { 1 << 28 } else { 0 })',gates),
('write-enable','esp32/src/periph.rs','(1 << if write { 28 } else { 5 })','(if write { 0 } else { 1 << 5 })',gates),
('command-value-gate','esp32/src/periph.rs','command & 0xffff == if write { 0x38 } else { 0xeb }','true',gates),
('read-opcode','esp32/src/periph.rs','if write { 0x38 } else { 0xeb }','if write { 0x38 } else { 0x03 }',gates),
('write-opcode','esp32/src/periph.rs','if write { 0x38 } else { 0xeb }','if write { 0x02 } else { 0xeb }',gates),
('read-gate','esp32/src/psram.rs','self.psram_cache_gate(false).map(|()| o)','Ok::<_, Fault>(o)',gates),
('write-gate','esp32/src/psram.rs','self.psram_cache_gate(true).map(|()| o)','Ok::<_, Fault>(o)',gates),
('register-read','esp32/src/periph.rs','if (0x54..=0x7c).contains(&off) { return self.0.regs.read(off); }','if false { return self.0.regs.read(off); }',trace),
('register-write','esp32/src/periph.rs','if (0x54..=0x7c).contains(&off) {\n            self.0.regs.write(off, v);','if false {\n            self.0.regs.write(off, v);',trace),
('reboot-bytes','esp32/src/psram.rs','self.psram.fill(0);','// retain bytes',reboot),
('reboot-mmu','esp32/src/psram.rs','table[SRAM_SLOT..SRAM_SLOT + 128].fill(0x100);','let _ = table;',reboot),
('reboot-spi0','esp32/src/soc.rs','self.periph.spi0.0.jedec = old.spi0.0.jedec;','self.periph.spi0 = old.spi0;',reboot),
('reboot-device','esp32/src/soc.rs','self.periph.spi1.0.jedec = old.spi1.0.jedec;','self.periph.spi1 = old.spi1;',reboot),
('reboot-dport','esp32/src/soc.rs','self.periph.efuse = old.efuse;','self.periph.dport = old.dport;\n        self.periph.efuse = old.efuse;',reboot),
]
if len(sys.argv) > 3:
    names = set(sys.argv[3:])
    mutants = [m for m in mutants if m[0] in names]
    assert len(mutants) == len(names), 'unknown mutation name'
original={f:(root/f).read_text() for _,f,_,_,_ in mutants}
results=[]
try:
    for name,f,old,new,test in mutants:
        p=root/f; text=original[f]; assert text.count(old)==1,(name,text.count(old))
        p.write_text(text.replace(old,new))
        cmd=['nice','-n','10','cargo','+1.99.0','test','--release','-p','esp32','--lib',test,'--','--nocapture']
        with (out/(name+'.log')).open('w') as stream:
            r=subprocess.run(cmd,cwd=root,stdout=stream,stderr=subprocess.STDOUT)
        log=(out/(name+'.log')).read_text()
        killed=r.returncode==101 and f'{test} ... FAILED' in log and 'panicked at' in log
        results.append(dict(name=name,file=f,old=old,new=new,test=test,exit_code=r.returncode,killed=killed))
        (out/'mutations.json').write_text(json.dumps(results,indent=2)+'\n')
        p.write_text(text)
        print(name,killed,flush=True)
        if not killed: print(log[-5000:]); raise SystemExit('survived/nonsemantic failure')
finally:
    for f,text in original.items(): (root/f).write_text(text)
