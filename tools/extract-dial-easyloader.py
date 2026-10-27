#!/usr/bin/env python3
"""Read the documented Dial EasyLoader resource without executing Windows code.

Run with: uv run --with dnfile==0.18.0 python tools/extract-dial-easyloader.py ...
The input is caller supplied and both container and image hashes are mandatory.
The extracted image is test-only: destinations must be below ignored output/.
"""
import argparse
import hashlib
from pathlib import Path

import dnfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--input', type=Path, required=True)
parser.add_argument('--container-sha256', required=True)
parser.add_argument('--image-sha256', required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
destination = args.output.resolve()
if not destination.is_relative_to(root / 'output') or destination.exists():
    parser.error('Use a new file beneath this checkout\'s ignored output/ directory.')
if args.input.stat().st_size > 64 * 1024 * 1024:
    parser.error('EasyLoader container exceeds the 64 MiB test-tool limit.')
data = args.input.read_bytes()
if hashlib.sha256(data).hexdigest() != args.container_sha256:
    parser.error('Container SHA-256 differs; inspect the upstream revision first.')
pe = dnfile.dnPE(data=data)
sets = [r for r in pe.net.resources if r.name == 'MiniBurner.Resource1.resources']
if len(sets) != 1:
    parser.error('Expected exactly one documented MiniBurner resource set.')
resources = {entry.name: entry.value for entry in sets[0].data.entries}
image = resources.get('firmware')
if resources.get('StartAddress') != '0x0' or not isinstance(image, bytes):
    parser.error('Expected a byte-array firmware resource flashed at 0x0.')
if hashlib.sha256(image).hexdigest() != args.image_sha256:
    parser.error('Embedded firmware SHA-256 differs; do not silently adopt it.')
if len(image) < 0x9000 or image[0] != 0xe9 or image[0x8000:0x8002] != b'\xaa\x50':
    parser.error('Expected ESP bootloader and partition table at 0x8000.')
destination.parent.mkdir(parents=True, exist_ok=True)
with destination.open('xb') as stream:
    stream.write(image)
print(f'Extracted unchanged resource: {len(image)} bytes, SHA-256 {args.image_sha256}')
