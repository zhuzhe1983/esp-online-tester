#!/usr/bin/env python3
"""Inspect the caller-supplied C126-ECHO EasyLoader without executing it.

uv run --with dnfile==0.18.0 --with dncil==1.0.2 python tools/inspect-voices3r-easyloader.py <exe> <new-output-bin>
Fixed hashes constrain this evidence probe to the investigated release. Output stays ignored.
"""
import hashlib
import json
import sys
from pathlib import Path

import dnfile
from dncil.cil.body import CilMethodBody
from dncil.cil.body.reader import CilMethodBodyReaderBytes

root = Path(__file__).resolve().parents[1]
source, destination = map(Path, sys.argv[1:])
if not destination.resolve().is_relative_to(root / 'output') or destination.exists():
    raise SystemExit('Use a new path under this checkout output/.')
if source.stat().st_size > 64 * 1024 * 1024:
    raise SystemExit('Container exceeds evidence-tool limit.')
container = source.read_bytes()
container_hash = hashlib.sha256(container).hexdigest()
assert container_hash == '8c9f874398bb3d3f09165bed5c7b7ca756e995898154165df526e55ba7f65aa1'
pe = dnfile.dnPE(data=container)
sets = [r for r in pe.net.resources if r.name == 'MiniBurner.Resource1.resources']
assert len(sets) == 1
resources = {entry.name: entry.value for entry in sets[0].data.entries}
image = resources['firmware']
assert isinstance(image, bytes)
image_hash = hashlib.sha256(image).hexdigest()
assert image_hash == 'e4e4b272a7f33b022554b02e41edada89c284dc7030c2e4ff51bd14a11c7f664'
methods = []
for method in pe.net.mdtables.MethodDef:
    if str(method.Name) not in ('button1_Click', 'SaveBinAsTemp'):
        continue
    body = CilMethodBody(CilMethodBodyReaderBytes(pe.get_data(method.Rva)))
    strings = []
    for instruction in body.instructions:
        if str(instruction.opcode) == 'ldstr':
            value = pe.net.user_strings.get(instruction.operand.value & 0xffffff)
            if value is not None and ('write_flash' in value.value or value.value.startswith('0x')):
                strings.append(value.value)
    methods.append({'name': str(method.Name), 'rva': method.Rva, 'flashArgumentLiterals': strings})
assert image[:0x1000] == b'\xff' * 0x1000
assert image[0x1000] == 0xe9 and image[0x9000:0x9002] == b'\xaa\x50' and image[0x11000] == 0xe9
apps = [int.from_bytes(image[p + 4:p + 8], 'little') for p in range(0x9000, 0x9c00, 32)
    if image[p:p + 3] == b'\xaa\x50\x00']
assert apps and apps[0] == 0x10000
destination.parent.mkdir(parents=True, exist_ok=True)
with destination.open('xb') as stream:
    stream.write(image)
print(json.dumps({'containerSha256': container_hash, 'imageSha256': image_hash, 'imageBytes': len(image),
    'resourceStartAddress': resources['StartAddress'], 'methods': methods,
    'bootloaderFileOffset': '0x1000', 'partitionTableFileOffset': '0x9000', 'applicationFileOffset': '0x11000',
    'applicationPartitionTargets': [hex(offset) for offset in apps],
    'unmodified': True, 'executedWindowsCode': False}, indent=2))
