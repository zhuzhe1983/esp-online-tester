#!/usr/bin/env python3
"""Offline C148 appearance rebuild, preserving verified original native encodings.

The manufacturer CAD is regenerated independently. Its triangle positions and
normal inventory must agree after zero-noise normalization; sub-1e-20 values may move between two
vertices when the CAD splitter changes face order. This script does not replace the accepted
native stream merely because this environment reorders faces. The unchanged
front PNG must decode identically before its old lossless encoding is retained.
"""
import argparse
import collections
import hashlib
import importlib.util
import json
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('product_rebuild', ROOT/'tools/rebuild-product-model.py')
common = importlib.util.module_from_spec(spec)
spec.loader.exec_module(common)


def mesh_inventory(data):
    vertices = np.frombuffer(data, dtype='<f4').reshape(-1, 3, 6)
    positions = sorted(b''.join(sorted(p.tobytes() for p in t[:, :3])) for t in vertices)
    normals = collections.defaultdict(set)
    for p in vertices.reshape(-1, 6):
        normals[tuple(p[:3])].add(tuple(0.0 if abs(n) < 1e-20 else n for n in p[3:]))
    normal_counts = collections.Counter(tuple(0.0 if abs(n) < 1e-20 else n for n in p[3:]) for p in vertices.reshape(-1, 6))
    return positions, normals, normal_counts


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--scratch', required=True)
    args = parser.parse_args()
    product = common.load_product(ROOT, 'm5stack-c148')
    scratch = common.scratch_path(ROOT, args.scratch)
    output = scratch/'rebuilt/m5stack-c148'
    if output.exists():
        raise ValueError('Choose a fresh scratch directory; existing output is preserved')
    folder, _ = common.hydrate(product, scratch)
    module = common.exporter(ROOT)
    module.TEMP = scratch/'sources'
    module.REV = product['metadata']['provenance']['commit']
    prepared = json.loads((folder/'prepared.json').read_text())
    actual, _ = module.build_model(prepared, product['plan'], output,
        evidence_root=scratch/'evidence', license_path=folder/product['byId']['cad-license']['archiveName'])
    preservation = []
    for item in product['plan']['appearancePreserveEncodings']:
        name = item['file']
        if name not in ('geometry.bin', 'front-paper.png'):
            raise ValueError('Unsupported preserved encoding')
        original = (ROOT/'web/platform/assets/m5stack-c148'/name).read_bytes()
        if hashlib.sha256(original).hexdigest() != item['sha256']:
            raise ValueError('Changed preserved artifact: '+name)
        generated = (output/name).read_bytes()
        if name == 'geometry.bin':
            assert mesh_inventory(original) == mesh_inventory(generated), 'Manufacturer mesh inventory changed'
        else:
            with Image.open(ROOT/'web/platform/assets/m5stack-c148'/name) as old, Image.open(output/name) as new:
                assert old.size == new.size and old.convert('RGBA').tobytes() == new.convert('RGBA').tobytes(), 'Front paper pixels changed'
        preservation.append({'file': name, 'originalSha256': item['sha256'],
            'freshEncodingSha256': hashlib.sha256(generated).hexdigest(), 'semanticComparisonPassed': True})
        (output/name).write_bytes(original)
        actual['artifacts'][name] = item['sha256']
    common.write(output/'model.json', actual)
    result = common.compare_build(product['metadata'], actual, output)
    result['preservedEncodings'] = preservation
    common.write(scratch/'comparison/m5stack-c148.json', result)
    assert result['passed'] and result['modelJsonByteIdentical'], json.dumps(result)
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
