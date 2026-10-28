#!/usr/bin/env python3
"""Capture public P5 recipe outputs without retaining installation paths.
Usage: provenance.py RECIPE_OUTPUT JSON_OUTPUT
"""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

root, output = (Path(x).resolve() for x in sys.argv[1:])
platform = root / 'arduino-A2/data/packages/esp32/hardware/esp32/2.0.17'
sdk = platform / 'tools/sdk/esp32'
tools = root / 'arduino-A2/data/packages/esp32/tools'
compiler = next(tools.glob('xtensa-esp32-elf-gcc/*/bin/xtensa-esp32-elf-gcc'))
fw = root / 'fw-A2'

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

linkmap = (fw / 'p5_probe.ino.map').read_text()
archives = sorted(set(re.findall(r'([^\s()]+\.a)\(', linkmap)))
archive_hashes = {name.replace(str(root), '$INPUTS'): sha(Path(name))
                  for name in archives if Path(name).is_file()}
assert archive_hashes
cfg = (sdk / 'sdkconfig').read_text()
for text in ('# CONFIG_FREERTOS_UNICORE is not set', '# CONFIG_SPIRAM_BOOT_INIT is not set',
             'CONFIG_SPIRAM_SPEED_80M=y', 'CONFIG_ESPTOOLPY_FLASHFREQ_80M=y',
             'CONFIG_HEAP_POISONING_LIGHT=y'):
    assert text in cfg, text
nm = compiler.with_name('xtensa-esp32-elf-nm')
symbols = subprocess.check_output([str(nm), '-n', str(fw / 'p5_probe.ino.elf')], text=True)
objdump = compiler.with_name('xtensa-esp32-elf-objdump')
disasm = subprocess.check_output([str(objdump), '-d', str(fw / 'p5_probe.ino.elf')], text=True)
name = 'esp_himem_reserved_area_size'
match = re.search(r'^([0-9a-f]+) <' + name + r'>:\n(.*?)(?=\n[0-9a-f]+ <)', disasm, re.M | re.S)
assert match and 'movi.n\ta2, 0' in match[2], 'linked weak himem returns zero'
getters = platform / 'cores/esp32/Esp.cpp'
result = {
 'version': '2.0.17', 'fqbn': 'esp32:esp32:esp32cam',
 'compiler': subprocess.check_output([str(compiler), '--version'], text=True).splitlines()[0],
 'firmware': {path.name: sha(path) for path in sorted(fw.iterdir()) if path.is_file()},
 'sdkconfig_sha256': sha(sdk / 'sdkconfig'),
 'sdkconfig_psram': [line for line in cfg.splitlines() if any(key in line for key in ('SPIRAM', 'UNICORE', 'FLASHFREQ', 'HEAP_POISONING'))],
 'source_hashes': {str(path.relative_to(platform)): sha(path) for path in
                   (getters, platform/'cores/esp32/esp32-hal-psram.c', platform/'boards.txt', platform/'platform.txt')},
 'linked_archives': archive_hashes,
 'symbols': [line for line in symbols.splitlines() if any(key in line for key in
             ('psram', 'spiram', 'cache_sram_mmu', 'esp_himem', 'getPsramSize', 'heap_caps_get_total_size'))],
 'himem': {'address': match[1], 'disassembly': match[2].strip(), 'reserved_bytes': 0},
}
output.parent.mkdir(parents=True, exist_ok=True)
output.write_text(json.dumps(result, indent=2) + '\n')
print('provenance PASS; linked archives', len(archive_hashes), 'weak himem reserve=0')
