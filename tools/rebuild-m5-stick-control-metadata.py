#!/usr/bin/env python3
"""Replay source-bound control metadata without re-encoding fixed CAD/PNG bytes.

This is an input-routing amendment, not a replacement for the original full
geometry builders. The compressed prior metadata and every runtime artifact are
independently hash checked. Only additive, declared nonvisual controls can change.
"""
import argparse
import copy
import gzip
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def pointer(value, path):
    for key in path.split('/')[1:]:
        value = value[int(key)] if isinstance(value, list) else value[key]
    return value


def build(identity, output, source_root=ROOT):
    folder = source_root/'web/platform/assets'/identity
    if output.resolve() == folder.resolve() or output.resolve().is_relative_to(folder.resolve()):
        raise ValueError('Use an independent scratch output')
    plan = json.loads((folder/'model-plan.json').read_bytes())
    recipe = plan['controlMetadataAmendment']
    base = recipe['base']
    if Path(base['file']).name != base['file']:
        raise ValueError('Unsafe metadata checkpoint name')
    packed = (folder/base['file']).read_bytes()
    if len(packed) != base['bytes'] or sha(packed) != base['sha256']:
        raise ValueError('Metadata checkpoint changed')
    raw = gzip.decompress(packed)
    if len(raw) != base['uncompressedBytes'] or sha(raw) != base['uncompressedSha256']:
        raise ValueError('Original metadata changed')
    metadata = json.loads(raw)
    if metadata['id'] != identity:
        raise ValueError('Wrong metadata identity')
    catalog = json.loads((source_root/'web/platform/catalog/products.json').read_bytes())['items']
    product = next(p for p in catalog if p['id'] == identity)
    controls = {c['id'] for c in product['controls'] if c['type'] == 'button' and c.get('simulationSupported') is not False}
    prior = copy.deepcopy(metadata)
    paths = recipe['addedJsonPointers']
    if len(set(paths)) != len(paths) or any(a != b and a.startswith(b+'/') for a in paths for b in paths):
        raise ValueError('Overlapping amendment operations')
    for path in paths:
        value = copy.deepcopy(pointer(plan, path))
        if path == '/presentation/buttons' or re.fullmatch(r'/presentation/buttons/\d+', path):
            regions = value if isinstance(value, list) else [value]
            if not all(isinstance(r, dict) and r.get('id') in controls and len(r.get('positionMm', [])) == 3 and len(r.get('sizeMm', [])) == 2 and r.get('sourceIds') for r in regions):
                raise ValueError('Input region lacks a declared control or source')
        elif re.fullmatch(r'/parts/\d+/controlId', path):
            if value not in controls:
                raise ValueError('Native route lacks a declared control')
        elif re.fullmatch(r'/presentation/inspectionGroups/\d+/parts/\d+', path):
            if value not in {'cover-key:'+r['id'] for r in plan['presentation']['buttons']}:
                raise ValueError('Only invisible input companions may amend inspection')
        else:
            raise ValueError('Visual geometry/material/texture amendment forbidden: '+path)
        parent, key = path.rsplit('/', 1)
        container = pointer(metadata, parent)
        if isinstance(container, list):
            if int(key) != len(container):
                raise ValueError('Input array operation must append')
            container.append(value)
        else:
            if key in container:
                raise ValueError('Routing amendment cannot overwrite existing fields')
            if key == 'controlId' and 'byteOffset' in container:
                # Match the original CAD builder's stable native-record order.
                pairs = []
                for old_key, old_value in container.items():
                    if old_key == 'byteOffset':
                        pairs.append((key, value))
                    pairs.append((old_key, old_value))
                container.clear()
                container.update(pairs)
            else:
                container[key] = value
    if metadata['presentation'] != plan['presentation']:
        raise ValueError('Undeclared presentation change')
    output.mkdir(parents=True, exist_ok=True)
    verified = {}
    for name, expected in prior['artifacts'].items():
        if Path(name).name != name:
            raise ValueError('Unsafe native artifact name')
        artifact = (folder/name).read_bytes()
        if sha(artifact) != expected:
            raise ValueError('Fixed CAD/source/texture bytes changed: '+name)
        (output/name).write_bytes(artifact)
        verified[name] = expected
    result = (json.dumps(metadata, ensure_ascii=False, indent=2)+'\n').encode()
    (output/'model.json').write_bytes(result)
    expected = (folder/'model.json').read_bytes()
    receipt = {'productId': identity, 'previousModelSha256': sha(raw), 'modelSha256': sha(result),
               'planSha256': sha((folder/'model-plan.json').read_bytes()),
               'metadataByteIdentical': result == expected, 'verifiedArtifacts': verified,
               'addedJsonPointers': paths,
               'scope': 'Exact offline replay of nonvisual routing over hash-fixed prior native/source artifacts. No regenerated CAD tessellation, PNG encoding, render or firmware run is claimed.'}
    if result != expected:
        raise ValueError('Replayed metadata differs from committed target')
    return receipt


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('product_id')
    p.add_argument('--source-root', type=Path, default=ROOT)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--receipt', type=Path)
    a = p.parse_args()
    receipt = build(a.product_id, a.output, a.source_root.resolve())
    if a.receipt:
        a.receipt.write_text(json.dumps(receipt, ensure_ascii=False, indent=2)+'\n')
    print(json.dumps({'productId': a.product_id, 'metadataByteIdentical': receipt['metadataByteIdentical'], 'verifiedArtifacts': len(receipt['verifiedArtifacts'])}))
