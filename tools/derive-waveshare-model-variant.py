#!/usr/bin/env python3
"""Select unchanged, already tessellated native parts for an exact SKU variant.

The variant keeps the original source files and complete omission accounting.
Only whole exported parts may be selected; source selections, rigid transforms,
and procedural geometry must match the pinned base plan exactly. New estimated
details belong in explicit renderer-derived parts, never in a modified CAD mesh.
"""
import argparse
import copy
import gzip
import hashlib
import json
from pathlib import Path
import shutil

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / 'web/platform/assets'


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read(path):
    return json.loads(path.read_text())


def write(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def identity(item, plan):
    return {key: item.get(key) for key in
            ['sourceFile', 'sourceComponent', 'sourceComponents', 'geometry']} | {
        'matrixRows': item.get('matrixRows', plan.get('matrixRows'))}


def derive(base_id, variant_id):
    assert base_id != variant_id
    assert all(Path(x).name == x and x.startswith('waveshare-') for x in [base_id, variant_id])
    base, target = ASSETS / base_id, ASSETS / variant_id
    old, new = read(base / 'model-plan.json'), read(target / 'model-plan.json')
    metadata, check = read(base / 'model.json'), read(base / 'build-check.json')
    assert new['id'] == variant_id and old['id'] == metadata['id'] == base_id
    assert sha((base / 'model.json').read_bytes()) == check['modelSha256']
    assert old['sources'] == new['sources'], 'Variant must retain identical pinned sources'
    assert old['tessellation'] == new['tessellation'], 'Retained mesh settings are unchanged'
    for filename, digest in metadata['artifacts'].items():
        assert sha((base / filename).read_bytes()) == digest, filename
    raw = gzip.decompress((base / metadata['binary']).read_bytes())
    assert len(raw) == check['geometry']['rawBytes'] and sha(raw) == check['geometry']['rawSha256']
    old_parts = {item['name']: item for item in old['parts']}
    old_meshes = {item['name']: item for item in metadata['parts']}
    old_checks = {item['name']: item for item in check['checks']}
    records, checks, chunks, selections, names = [], [], [], {}, set()
    offset = 0
    for item in new['parts']:
        name = item['name']
        assert name not in names and name in old_parts
        names.add(name)
        assert identity(item, new) == identity(old_parts[name], old), 'Geometry changed: ' + name
        mesh = old_meshes[name]
        size = mesh['vertexCount'] * 6 * 4
        chunk = raw[mesh['byteOffset']:mesh['byteOffset'] + size]
        assert len(chunk) == size
        record = {k: v for k, v in item.items() if k not in ['geometry', 'matrixRows']}
        record.update(byteOffset=offset, vertexCount=mesh['vertexCount'], boundsMm=mesh['boundsMm'])
        records.append(record)
        checks.append(old_checks[name])
        chunks.append(chunk)
        offset += size
        if 'sourceFile' in item:
            selections.setdefault(item['sourceFile'], []).extend(
                item.get('sourceComponents', [item.get('sourceComponent')]))
    for source, coverage in new['sourceCoverage'].items():
        selected, omitted = selections.get(source, []), coverage.get('omittedIndices', [])
        assert len(selected) == len(set(selected))
        assert sorted(selected) == coverage['includedIndices']
        assert not set(selected).intersection(omitted)
        assert sorted(selected + omitted) == list(range(new['sourceTopologyCounts'][source]))
        assert not omitted or coverage.get('omissionReason'), 'An omission requires its source-based reason'
    assert records and len(records) <= 512 and offset <= 64 * 1024 * 1024
    for source in new['sources']:
        shutil.copyfile(base / source['file'], target / source['file'])
    for filename in ['source-manifest.json', 'SOURCE-RIGHTS.txt']:
        shutil.copyfile(base / filename, target / filename)
    manifest = read(target / 'source-manifest.json')
    manifest['productId'] = variant_id
    write(target / 'source-manifest.json', manifest)
    binary = b''.join(chunks)
    compressed = gzip.compress(binary, compresslevel=9, mtime=0)
    (target / 'geometry.bin.gz').write_bytes(compressed)
    files = [x['file'] for x in new['sources']] + [
        'model-plan.json', 'source-manifest.json', 'SOURCE-RIGHTS.txt', 'geometry.bin.gz']
    result = {k: new[k] for k in ['id', 'dimensions', 'presentation', 'limitations',
              'referencePhotos', 'method', 'sourceSearch', 'photoRegistration',
              'omittedSourceComponents', 'sourceCoverage'] if k in new}
    provenance = copy.deepcopy(metadata['provenance'])
    provenance.update(generator='tools/derive-waveshare-model-variant.py',
                      generatorSha256=sha(Path(__file__).read_bytes()),
                      baseModel={'id': base_id, 'modelSha256': check['modelSha256'],
                                 'geometryRawSha256': check['geometry']['rawSha256'],
                                 'generator': metadata['provenance']['generator'],
                                 'generatorSha256': metadata['provenance']['generatorSha256']})
    result.update(schemaVersion='1', unit='mm', strideFloats=6, binary='geometry.bin.gz',
                  binaryCompression='gzip', parts=records, derivedParts=new.get('derivedParts', []),
                  artifacts={f: sha((target / f).read_bytes()) for f in files},
                  provenance=provenance, sourceTransform=metadata['sourceTransform'])
    write(target / 'model.json', result)
    write(target / 'build-check.json', {
        'id': variant_id, 'modelSha256': sha((target / 'model.json').read_bytes()),
        'geometry': {'rawBytes': len(binary), 'rawSha256': sha(binary),
                     'gzipBytes': len(compressed), 'gzipSha256': sha(compressed)},
        'generatorSha256': sha(Path(__file__).read_bytes()), 'engine': check['engine'],
        'baseModel': provenance['baseModel'], 'checks': checks,
        'dimensionTargets': new.get('dimensionTargets', []),
        'scope': 'Whole native parts selected byte-for-byte from a hash-verified base export. '
                 'Source omissions and renderer-derived estimates are explicit; actual loader '
                 'geometry/ray checks, visual review and hardware fit remain separate.'})
    print(json.dumps({'id': variant_id, 'parts': len(records), 'rawBytes': len(binary),
                      'gzipBytes': len(compressed), 'status': 'derived'}), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('base')
    parser.add_argument('variant')
    args = parser.parse_args()
    derive(args.base, args.variant)
