#!/usr/bin/env python3
"""Normalize personal build path prefixes in derived ZIP inventories only.

Original ZIPs, firmware members, SDK sources, hashes and sizes are untouched.
The manifest retains original and normalized inventory hashes and counts.
"""
import gzip
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / 'docs/platform/research/waveshare-thermal-20261124'
PATTERN = re.compile(r'/(Users|home)/(?!alice(?:[/\s"\'`\\<>]|$))[^/\s"\'`\\<>]+')


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    path = OUT / 'source-manifest.json'
    manifest = json.loads(path.read_text())
    for row in manifest['cadSearch']['archives']:
        inventory = OUT / row['retainedMemberList']
        original = inventory.read_bytes()
        assert sha(original) == row['retainedSha256']
        decoded = gzip.decompress(original)
        normalized, count = PATTERN.subn(lambda m: '/' + m.group(1) + '/alice', decoded.decode())
        if not count:
            continue
        data = normalized.encode()
        assert len(json.loads(data)) == row['memberCount']
        encoded = gzip.compress(data, mtime=0)
        inventory.write_bytes(encoded)
        row['retainedSha256'] = sha(encoded)
        row['inventoryPrivacyNormalization'] = {
            'scope': 'Derived ZIP inventory member names only; normalize personal home-directory label to alice. Member count, suffixes, file sizes and archive/firmware bytes are unchanged.',
            'replacedPrefixes': count,
            'originalCompressedSha256': sha(original),
            'originalDecodedSha256': sha(decoded),
            'normalizedCompressedSha256': sha(encoded),
            'normalizedDecodedSha256': sha(data),
            'effect': 'Personal build-directory prefixes are no longer exact archive member names. Factory firmware entries and CAD suffix classification are unaffected.'}
    path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n')


if __name__ == '__main__':
    main()
