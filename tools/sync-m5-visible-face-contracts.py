#!/usr/bin/env python3
"""Synchronize reviewed dynamic faces from committed plans; never edit CAD bytes.

This complements each product's full offline geometry generator. The explicit
per-SKU visibleFaceReview field records source hashes and what remains unknown.
"""
import argparse
import copy
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def synchronize(identity, write):
    assets = ROOT / 'web/platform/assets' / identity
    plan = json.loads((assets / 'model-plan.json').read_text())
    metadata = json.loads((assets / 'model.json').read_text())
    review = plan['visibleFaceReview']
    for name, expected in review['sourceHashes'].items():
        assert digest(assets / name) == expected, 'Reviewed source changed: ' + name
    binary = metadata['binary']; before = digest(assets / binary)
    for key in ['derivedParts', 'presentation', 'limitations', 'representationScope']:
        if key in plan:
            if write:
                metadata[key] = copy.deepcopy(plan[key])
            else:
                assert metadata.get(key) == plan[key], 'Plan differs from production metadata: ' + key
    if write:
        (assets / 'model.json').write_text(json.dumps(metadata, ensure_ascii=False, indent=2) + '\n')
    assert digest(assets / binary) == before, 'Source geometry was mutated'
    assert any(part['kind'] == 'pcb-placeholder' for part in metadata['derivedParts']), 'Unknown internal geometry must remain explicit'
    print(json.dumps({'productId': identity, 'mode': 'write' if write else 'check', 'geometrySha256': before, 'modelSha256': digest(assets / 'model.json'), 'unknownPolicy': review['unknownPolicy']}))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--ids', required=True, help='Comma-separated catalog IDs')
    parser.add_argument('--write', action='store_true')
    args = parser.parse_args()
    for identity in args.ids.split(','):
        synchronize(identity, args.write)
