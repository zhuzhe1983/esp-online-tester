#!/usr/bin/env python3
"""Pin stock A30 configuration, actual linked archives, public sources and inputs.
Usage: provenance.py RECIPE_OUTPUT JSON_OUTPUT
"""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
root, output = (Path(x).resolve() for x in sys.argv[1:])
packages = root / 'arduino-A30/data/packages/esp32'
platform = packages / 'hardware/esp32/3.0.7'
sdk = packages / 'tools/esp32-arduino-libs/idf-release_v5.1-632e0c2a/esp32'
compiler = next((packages / 'tools').glob('esp-x32/*/bin/xtensa-esp32-elf-gcc'))
fw = root / 'fw-p6'
def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()
config = sdk / 'sdkconfig'
assert sha(config) == 'cc18179bae15798fb66668c72cc246b9d596790806d54d717326fcf17ad2d311'
cfg = config.read_text()
for text in ('# CONFIG_SPIRAM_BOOT_INIT is not set', '# CONFIG_FREERTOS_UNICORE is not set',
             'CONFIG_SPIRAM_SPEED_80M=y', 'CONFIG_HEAP_POISONING_LIGHT=y'):
    assert text in cfg, text
archives = sorted(set(re.findall(r'([^\s()]+\.a)\(', (fw/'p6_probe.ino.map').read_text())))
linked = {p.replace(str(root), '$INPUTS'): sha(Path(p)) for p in archives if Path(p).is_file()}
assert linked
nm = subprocess.check_output([str(compiler.with_name('xtensa-esp32-elf-nm')), '-n', str(fw/'p6_probe.ino.elf')], text=True)
result = {'version': '3.0.7', 'idf': 'idf-release_v5.1-632e0c2a', 'fqbn': 'esp32:esp32:esp32cam',
 'compiler': subprocess.check_output([str(compiler), '--version'], text=True).splitlines()[0],
 'firmware': {p.name: sha(p) for p in sorted(fw.iterdir()) if p.is_file()},
 'sdkconfig_sha256': sha(config), 'sdkconfig_psram': [l for l in cfg.splitlines() if any(k in l for k in ('SPIRAM', 'UNICORE', 'FLASHFREQ', 'HEAP_POISONING'))],
 'sources': {str(p.relative_to(platform)): sha(p) for p in [platform/'cores/esp32/Esp.cpp', platform/'cores/esp32/esp32-hal-psram.c', platform/'cores/esp32/esp32-hal-misc.c', platform/'boards.txt', platform/'platform.txt']},
 'linked_archives': linked, 'symbols': [l for l in nm.splitlines() if any(k in l for k in ('psram', 'spiram', 'cache_sram_mmu', 'heap_caps_get_total_size'))]}
output.parent.mkdir(parents=True, exist_ok=True)
output.write_text(json.dumps(result, indent=2)+'\n')
print('PASS stock BOOT_INIT=n configuration and', len(linked), 'linked archive hashes')
