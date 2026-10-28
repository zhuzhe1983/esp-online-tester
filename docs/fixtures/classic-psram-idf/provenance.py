#!/usr/bin/env python3
"""Record public SDK/linked input hashes without retaining absolute source paths."""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

build, config, output = map(Path, sys.argv[1:])
def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

result = {
    'idf': '38eeba213aa695aabfd6d89aa9f5078dbe5a94c3',
    'image_index': 'sha256:52bc81e7f212b6cc63b31ea57b8270badb3236e44df8567a57e2d1a6c74c5000',
    'compiler': subprocess.check_output(['xtensa-esp32-elf-gcc', '--version'], text=True).splitlines()[0],
    'sdkconfig_sha256': sha(config),
    'source_sha256': {name: sha(config.parent / name) for name in [
        'CMakeLists.txt', 'sdkconfig.defaults', 'main/CMakeLists.txt', 'main/main.c']},
    'firmware': {name: sha(build / name) for name in ['psram.bin', 'bootloader/bootloader.bin', 'partition_table/partition-table.bin']},
    'elf_sha256': sha(build / 'psram.elf'),
    'linked_archives': {},
}
for name in ['psram.map', 'bootloader/bootloader.map']:
    text = (build / name).read_text().split('Discarded input sections')[0]
    archives = {}
    for archive in sorted(set(re.findall(r'(\S+\.a)\(', text))):
        path = Path(archive)
        if not path.is_absolute():
            path = (build / name).parent / path
        archives[path.name] = sha(path)
    result['linked_archives'][name] = archives
symbols = subprocess.check_output(['xtensa-esp32-elf-nm', '-n', str(build / 'psram.elf')], text=True)
result['symbols'] = [line for line in symbols.splitlines() if any(line.endswith(' ' + name) for name in [
    'app_main', 'esp_spiram_init', 'esp_spiram_init_cache', 'esp_spiram_test', 'esp_spiram_add_to_heapalloc',
    'esp_spiram_get_size', 'heap_caps_malloc', 'heap_caps_get_total_size', 'cache_sram_mmu_set', 'cache_sram_mmu_set_rom'])]
output.write_text(json.dumps(result, indent=2) + '\n')
idf = Path(os.environ['IDF_PATH'])
notices = []
for name, header in [
    ('LICENSE', False),
    ('components/freertos/tasks.c', True),
    ('components/heap/heap_tlsf.c', True),
    ('components/xtensa/include/xtensa/hal.h', True),
]:
    text = (idf / name).read_text()
    if header:
        text = text.split('*/', 1)[0] + '*/\n'
    notices.append(name + '\n' + text)
output.with_name('licenses.txt').write_text('\n\n'.join(notices))
print(json.dumps(result, indent=2))
