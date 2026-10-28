#!/usr/bin/env python3
"""Capture installed SDK provenance and linked PSRAM symbols for recipe outputs."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess

p = argparse.ArgumentParser()
p.add_argument('inputs', type=Path)
p.add_argument('output', type=Path)
a = p.parse_args()
a.output.mkdir(parents=True, exist_ok=True)
index = {}
for label, version in [('A2','2.0.17'), ('A30','3.0.7')]:
    root = a.inputs / ('arduino-' + label)
    platform = root / 'data/packages/esp32/hardware/esp32' / version
    tools = root / 'data/packages/esp32/tools'
    sdk = platform / 'tools/sdk/esp32' if label == 'A2' else tools / 'esp32-arduino-libs/idf-release_v5.1-632e0c2a/esp32'
    out = a.output / label
    out.mkdir(exist_ok=True)
    files = {'sdkconfig': sdk / 'sdkconfig', 'boards.txt': platform / 'boards.txt',
             'platform.txt': platform / 'platform.txt'}
    for key, f in files.items():
        shutil.copyfile(f, out / key)
    compiler = next(tools.glob('xtensa-esp32-elf-gcc/*/bin/xtensa-esp32-elf-gcc')) if label == 'A2' else next(tools.glob('esp-x32/*/bin/xtensa-esp32-elf-gcc'))
    nm = compiler.with_name('xtensa-esp32-elf-nm')
    elf = a.inputs / ('fw-' + label) / 'p0_probe.ino.elf'
    symbols = subprocess.check_output([str(nm), '-n', str(elf)], text=True)
    relevant = '\n'.join(l for l in symbols.splitlines() if any(k in l.lower() for k in ['psram', 'spiram', 'cache_sram_mmu', 'heap_caps_malloc', 'heap_caps_get_total_size'])) + '\n'
    (out / 'symbols.txt').write_text(relevant)
    linkmap = (a.inputs / ('fw-' + label) / 'p0_probe.ino.map').read_text()
    relevant_map = '\n'.join(l.replace(str(a.inputs), '$INPUTS') for l in linkmap.splitlines() if any(k in l.lower() for k in ['psram', 'spiram', 'cache_sram_mmu'])) + '\n'
    (out / 'linked-map.txt').write_text(relevant_map)
    archives = sorted(set(re.findall(r'([^\s()]+\.a)\(', linkmap)))
    archive_hashes = {}
    for name in archives:
        file = Path(name)
        if file.is_file():
            archive_hashes[name.replace(str(a.inputs), '$INPUTS')] = hashlib.sha256(file.read_bytes()).hexdigest()
    package_index = json.loads((root / 'data/package_esp32_index.json').read_text())
    package = next(x for x in package_index['packages'] if x['name'] == 'esp32')
    release = next(x for x in package['platforms'] if x['version'] == version)
    (out / 'package-release.json').write_text(json.dumps(release, indent=2) + '\n')
    installed = {str(f.relative_to(root)): hashlib.sha256(f.read_bytes()).hexdigest()
                 for f in root.glob('downloads/packages/*') if f.is_file()}
    fw_hashes = {f.name: hashlib.sha256(f.read_bytes()).hexdigest()
                 for f in (a.inputs / ('fw-' + label)).glob('*') if f.is_file()}
    index[label] = {'version': version, 'compiler': subprocess.check_output([str(compiler),'--version'],text=True).splitlines()[0],
                    'sdk': str(sdk.relative_to(root)), 'linked_archives': archive_hashes,
                    'downloaded_archives': installed, 'firmware': fw_hashes,
                    'sdkconfig_sha256': hashlib.sha256(files['sdkconfig'].read_bytes()).hexdigest()}
    assert archive_hashes and installed and 'cache_sram_mmu_set' in relevant
(a.output / 'provenance.json').write_text(json.dumps(index, indent=2) + '\n')
print(json.dumps({k: {'linked_archives': len(v['linked_archives']), 'downloaded_archives': len(v['downloaded_archives']), 'sdkconfig_sha256': v['sdkconfig_sha256']} for k,v in index.items()}, indent=2))
