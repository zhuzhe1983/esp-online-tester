#!/usr/bin/env python3
"""Verify retained/decoded model source pins and exact official ZIP members offline."""
from pathlib import Path
import gzip
import hashlib
import io
import json
import zipfile

ROOT = Path(__file__).resolve().parents[4]
progress = json.loads((ROOT / 'docs/platform/waveshare-patch-import-20261124.json').read_text())
ids = next(row['productIds'] for row in progress['batches'] if row['number'] == 30)
results, members = [], []
for product_id in ids:
    folder = ROOT / 'web/platform/assets' / product_id
    plan = json.loads((folder / 'model-plan.json').read_text())
    originals = {}
    for source in plan['sources']:
        raw = (folder / source['file']).read_bytes()
        retained = source.get('retained', source)
        assert len(raw) == retained['bytes']
        assert hashlib.sha256(raw).hexdigest() == retained['sha256']
        decoded = gzip.decompress(raw) if retained.get('encoding') == 'gzip' else raw
        assert len(decoded) == source['bytes']
        assert hashlib.sha256(decoded).hexdigest() == source['sha256']
        originals[source['id']] = decoded
    for source in plan['sources']:
        if source.get('origin') != 'zip-member':
            continue
        raw = originals[source['containerSourceId']]
        assert hashlib.sha256(raw).hexdigest() == source['archive']['containerSha256']
        with zipfile.ZipFile(io.BytesIO(raw)) as archive:
            name = source['archive']['entry']
            assert archive.namelist().count(name) == 1
            assert archive.read(name) == originals[source['id']]
        members.append(dict(productId=product_id, sourceId=source['id'], entry=name,
                            containerSha256=source['archive']['containerSha256'],
                            memberSha256=source['sha256'], passed=True))
    results.append(dict(productId=product_id, sources=len(plan['sources']),
                        allRetainedAndDecodedHashesMatch=True))
print(json.dumps(dict(scope='Offline retained/decoded bytes and exact original ZIP STEP members; no remeshing or network retrieval.',
                      models=results, archiveMembers=members,
                      checkedSources=sum(row['sources'] for row in results), allPassed=True), indent=2))
