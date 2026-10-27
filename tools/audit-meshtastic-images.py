#!/usr/bin/env python3
"""Manual onboarding audit, in RAM only; NOT invoked by the metadata update Action."""
import argparse
import datetime
import hashlib
import importlib.util
import json
import re
import struct
from pathlib import Path
from urllib.parse import urlparse

spec = importlib.util.spec_from_file_location('esp_image_audit', Path(__file__).with_name('audit-factory-firmware-sources.py'))
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)


def inspect(url, flash_bytes):
    parsed = urlparse(url)
    if parsed.scheme != 'https' or parsed.hostname != 'release.meshtastic.org' or parsed.username or parsed.password or parsed.query or parsed.fragment:
        raise ValueError('Use an official public Meshtastic target manifest URL')
    metadata = audit.fetch(url)
    if len(metadata) > 256 * 1024:
        raise ValueError('Manifest exceeds limit')
    manifest = json.loads(metadata)
    board, version, chip = manifest['platformioTarget'], manifest['version'], manifest['mcu']
    if not re.fullmatch(r'[a-z0-9-]{1,80}', board) or not re.fullmatch(r'[0-9][a-zA-Z0-9.-]{0,79}', version):
        raise ValueError('Invalid board or version')
    base = f'https://release.meshtastic.org/{version}/'
    name = f'firmware-{board}-{version}'
    if url != base + name + '.mt.json' or manifest['repo'] != 'meshtastic/firmware':
        raise ValueError('Manifest identity mismatch')
    images = [x for x in manifest['files'] if x['name'] == name + '.factory.bin']
    if len(images) != 1 or chip not in ('esp32c6', 'esp32s3'):
        raise ValueError('Missing unique supported factory image')
    image = images[0]
    body = audit.fetch(base + image['name'])
    if len(body) != image['bytes'] or hashlib.md5(body).hexdigest() != image['md5']:
        raise ValueError('Publisher size/MD5 mismatch')
    chip_id = {'esp32c6': 13, 'esp32s3': 9}[chip]
    if audit.image(body, 0) != chip_id or audit.image(body, 0x10000) != chip_id:
        raise ValueError('Boot or app chip mismatch')
    partitions = []
    for at in range(0x8000, 0x9000, 32):
        row = body[at:at + 32]
        if row[:2] != b'\xaa\x50':
            break
        offset, size = struct.unpack_from('<II', row, 4)
        partitions.append({'type': row[2], 'subtype': row[3], 'offset': offset, 'size': size, 'name': row[12:28].split(b'\0')[0].decode()})
    if not partitions:
        raise ValueError('No partition table')
    partition_end = max(row['offset'] + row['size'] for row in partitions)
    if len(body) > flash_bytes or partition_end > flash_bytes:
        raise ValueError('Image or partition exceeds the supplied physical Flash capacity')
    return {'boardTarget': board, 'version': version, 'url': base + image['name'],
            'fetchedAt': datetime.datetime.now(datetime.timezone.utc).isoformat(),
            'bytes': len(body), 'sha256': hashlib.sha256(body).hexdigest(), 'publisherMd5': image['md5'],
            'chipId': chip_id, 'bootOffset': 0, 'appOffset': 65536, 'partitions': partitions,
            'flashBytes': flash_bytes, 'partitionEnd': partition_end,
            'executed': False, 'storage': 'Firmware bytes inspected in RAM only; not written to disk',
            'metadata': {'url': url, 'sha256': hashlib.sha256(metadata).hexdigest(), 'bytes': len(metadata)}}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest-url', action='append', required=True)
    parser.add_argument('--flash-mib', type=int, required=True, help='Physical Flash capacity of every supplied target, verified from its hardware documentation')
    args = parser.parse_args()
    if not 1 <= args.flash_mib <= 256:
        parser.error('--flash-mib must be between 1 and 256')
    print(json.dumps([inspect(url, args.flash_mib * 1024 * 1024) for url in args.manifest_url], indent=2))
