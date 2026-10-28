#!/usr/bin/env python3
"""Verify committed recipe source, SDK config and firmware input hashes."""
import hashlib
import json
from pathlib import Path

here = Path(__file__).resolve().parent
root = here.parents[2]
provenance = json.loads((here / 'provenance.json').read_text())
def check(path, expected):
    actual = hashlib.sha256(path.read_bytes()).hexdigest()
    assert actual == expected, str(path.relative_to(root))

for name, expected in provenance['source_sha256'].items():
    check(here / name, expected)
check(here / 'sdkconfig.boot-init', provenance['sdkconfig_sha256'])
for name, kind in [('psram.bin', 'app'), ('bootloader/bootloader.bin', 'bootloader'), ('partition_table/partition-table.bin', 'ptable')]:
    check(root / 'web/wasm/fw/public' / ('classic-psram-idf-' + kind + '.bin'), provenance['firmware'][name])
print('PASS: four source inputs, effective SDK config and three firmware hashes')
