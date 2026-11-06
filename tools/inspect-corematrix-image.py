#!/usr/bin/env python3
"""Inspect a caller-supplied original CoreMatrix image without executing it.

Requires littlefs-python==0.19.0. File inventories may contain private names in
user-modified firmware; review the output before sharing it.
"""
import argparse
import hashlib
import json
import runpy
import struct
from pathlib import Path
import littlefs


def inspect(path):
    data = path.read_bytes()
    image = runpy.run_path(str(Path(__file__).with_name('audit-factory-firmware-sources.py')))['image']
    headers = []
    for offset in (0, 0x10000):
        chip = image(data, offset)
        if data[offset + 23] != 1:
            raise ValueError('Expected an appended ESP image SHA-256')
        if chip != 20:
            raise ValueError('Expected ESP32-C61 chip ID 20 in both original images')
        headers.append({'offset': offset, 'chipId': chip, 'entry': struct.unpack_from('<I', data, offset + 4)[0], 'segmentChecksums': 'pass', 'appendedSha256': 'pass'})
    filesystems = []
    for at in range(0x8000, 0x9000, 32):
        if data[at:at + 2] != b'\xaaP':
            break
        _, kind, subtype, offset, size, label, _ = struct.unpack_from('<HBBII16sI', data, at)
        if offset + size > 16 * 1024 * 1024 or offset + size > len(data):
            raise ValueError('Partition exceeds the complete CoreMatrix image')
        if kind != 1 or subtype != 129:
            continue
        fs = littlefs.LittleFS(context=littlefs.UserContext(buffer=bytearray(data[offset:offset + size])), block_size=4096, block_count=size // 4096, read_size=32, prog_size=32)
        files = []

        def walk(directory):
            for name in fs.listdir(directory):
                name = directory.rstrip('/') + '/' + name
                info = fs.stat(name)
                if info.type == 2:
                    walk(name)
                else:
                    with fs.open(name, 'rb') as f:
                        contents = f.read()
                    files.append({'name': name, 'bytes': info.size, 'sha256': hashlib.sha256(contents).hexdigest()})

        walk('/')
        filesystems.append({'name': label.rstrip(b'\0').decode('ascii'), 'offset': offset, 'bytes': size, 'mounted': True, 'files': files, 'm5thingsFiles': [item['name'] for item in files if 'm5things' in item['name'].lower()]})
        fs.unmount()
    return {'sha256': hashlib.sha256(data).hexdigest(), 'bytes': len(data), 'sdk': data[0x10020 + 112:0x10020 + 144].split(b'\0', 1)[0].decode('ascii'), 'bootImages': headers, 'filesystems': filesystems, 'executed': False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('image', type=Path)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    result = json.dumps(inspect(args.image), indent=2) + '\n'
    if args.output:
        args.output.write_text(result)
    else:
        print(result, end='')
