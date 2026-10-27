#!/usr/bin/env python3
"""Recheck pinned manufacturer metadata and ESP images in RAM; never execute firmware."""
import argparse
import concurrent.futures
import hashlib
import json
import struct
import sys
import subprocess
from pathlib import Path
from urllib.parse import urlparse


def fetch(url):
    parsed = urlparse(url)
    if parsed.scheme != 'https' or parsed.username or parsed.password:
        raise ValueError('Source URL must be public HTTPS without credentials')
    result = subprocess.run(['curl', '--fail', '--location', '--silent', '--show-error',
                             '--max-time', '120', '--max-filesize', '33554432', url],
                            capture_output=True, check=True)
    return result.stdout


def image(data, offset):
    if offset + 24 > len(data) or data[offset] != 0xe9 or not 1 <= data[offset + 1] <= 16:
        raise ValueError(f'Invalid ESP image at {offset:#x}')
    at, checksum = offset + 24, 0xef
    for _ in range(data[offset + 1]):
        if at + 8 > len(data):
            raise ValueError('Truncated segment header')
        size = struct.unpack_from('<I', data, at + 4)[0]
        at += 8
        if at + size > len(data):
            raise ValueError('Truncated segment data')
        for value in data[at:at + size]:
            checksum ^= value
        at += size
    checksum_at = at | 15
    if checksum_at >= len(data) or data[checksum_at] != checksum:
        raise ValueError(f'ESP segment checksum mismatch at {offset:#x}')
    if data[offset + 23] == 1:
        end = checksum_at + 1
        if data[end:end + 32] != hashlib.sha256(data[offset:end]).digest():
            raise ValueError(f'ESP appended digest mismatch at {offset:#x}')
    return struct.unpack_from('<H', data, offset + 12)[0]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--receipts', required=True, type=Path)
    parser.add_argument('--metadata-url', required=True)
    parser.add_argument('--firmware-id', action='append', help='Only inspect these exact pinned entries')
    args = parser.parse_args()
    metadata = json.loads(fetch(args.metadata_url))
    entries = {entry['fid']: entry for entry in metadata}
    # Compare the publisher to a previously independently verified official image.
    # The account identifier is used in RAM only and never included in receipts.
    official = entries['02f082a61479570df0a2063c9bce7b57']['author']
    receipts = json.loads((args.receipts / 'image-source-index.json').read_text())
    if args.firmware_id:
        receipts = [receipt for receipt in receipts if receipt['firmwareId'] in args.firmware_id]
        if len(receipts) != len(set(args.firmware_id)):
            raise ValueError('Unknown firmware ID')

    def verify(receipt):
        entry = entries[receipt['firmwareId']]
        if entry['author'] != official:
            raise ValueError('Publisher no longer matches verified official publisher')
        versions = [version for version in entry['versions'] if version.get('published')]
        if not any(version['version'] == receipt['version'] and receipt['url'].endswith('/' + version['file']) for version in versions):
            raise ValueError('Pinned version/file no longer in official metadata')
        data = fetch(receipt['url'])
        if len(data) != receipt['bytes'] or hashlib.sha256(data).hexdigest() != receipt['sha256']:
            raise ValueError('Pinned source bytes/hash changed')
        images = receipt['layout']['bootHeaders'] + [partition['image']
                  for table in receipt['layout']['partitionTables'] for partition in table['partitions']
                  if partition.get('image')]
        for item in images:
            if image(data, item['offset']) != item['chipId']:
                raise ValueError('Unexpected image chip')
        for product in receipt['validation']['products']:
            if len(data) > product['flashBytes'] or product['partitionEnd'] > product['flashBytes']:
                raise ValueError('Image or partition exceeds Flash')
        return {'firmwareId': receipt['firmwareId'], 'version': receipt['version'],
                'bytes': len(data), 'sha256': receipt['sha256'], 'imagesChecked': len(images),
                'result': 'source-layout-checksums-pass', 'executed': False}

    def checked(receipt):
        try:
            return verify(receipt)
        except Exception as error:
            return {'firmwareId': receipt['firmwareId'], 'version': receipt['version'],
                    'result': 'failed', 'error': str(error), 'executed': False}

    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        results = list(pool.map(checked, receipts))
    print(json.dumps({'metadataUrl': args.metadata_url, 'images': results, 'executed': False}, indent=2))
    if any(result['result'] == 'failed' for result in results):
        sys.exit(1)


if __name__ == '__main__':
    main()
