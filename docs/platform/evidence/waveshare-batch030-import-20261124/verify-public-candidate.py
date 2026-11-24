#!/usr/bin/env python3
"""Reject raw batch030 session values in new reachable blobs, including archives."""
import argparse
import gzip
import io
import json
import re
import subprocess
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--candidate', required=True)
parser.add_argument('--base', default='a00343fa48c6d0975d5ac9621495b2ba215e1aea')
parser.add_argument('--source-local', required=True, help='Unpublished original delivery ref, or a locally reconstructed equivalent.')
args = parser.parse_args()
def git(*command):
    return subprocess.check_output(['git', *command], cwd=ROOT)
parents = git('rev-list', '--parents', '-n', '1', args.candidate).decode().split()
base = git('rev-parse', args.base).decode().strip()
assert parents[1:] == [base], 'Candidate must have only the existing integration base as parent.'
progress = json.loads((ROOT / 'docs/platform/waveshare-patch-import-20261124.json').read_text())
ids = next(row['productIds'] for row in progress['batches'] if row['number'] == 30)
values = set()
for product_id in ids:
    path = 'web/platform/assets/' + product_id + '/store.html.gz'
    found = subprocess.run(['git', 'show', args.source_local + ':' + path], cwd=ROOT,
                           stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    if found.returncode:
        continue
    values.update(re.findall(rb'/form_key/([A-Za-z0-9_-]+)/', gzip.decompress(found.stdout)))
assert values, 'Original local source did not expose the expected anonymous fields.'
objects = git('rev-list', '--objects', base + '..' + args.candidate).decode().splitlines()
checked = {'blobs': 0, 'gzipBodies': 0, 'zipMembers': 0}
def scan(data, depth=0):
    assert all(value not in data for value in values), 'Original anonymous source value remains in candidate blobs.'
    if depth >= 3:
        return
    if data.startswith(b'\x1f\x8b'):
        checked['gzipBodies'] += 1
        scan(gzip.decompress(data), depth + 1)
    elif data.startswith(b'PK\x03\x04'):
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            for item in archive.infolist():
                if not item.is_dir():
                    checked['zipMembers'] += 1
                    scan(archive.read(item), depth + 1)
with subprocess.Popen(['git', 'cat-file', '--batch'], cwd=ROOT,
                      stdin=subprocess.PIPE, stdout=subprocess.PIPE) as process:
    for record in objects:
        oid = record.split(' ', 1)[0]
        process.stdin.write((oid + '\n').encode())
        process.stdin.flush()
        _, kind, size = process.stdout.readline().split()
        data = process.stdout.read(int(size))
        assert process.stdout.read(1) == b'\n'
        if kind == b'blob':
            scan(data)
            checked['blobs'] += 1
    process.stdin.close()
    assert process.wait() == 0
print(json.dumps(dict(candidate=parents[0], base=base, soleParentVerified=True,
                      originalSourceVisibility='local unpublished input; not candidate ancestry',
                      distinctOriginalAnonymousValues=len(values), checked=checked,
                      originalValuesFound=False, passed=True), indent=2))
