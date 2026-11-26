#!/usr/bin/env python3
"""Read-only K136 G0 source registration and native cap support audit."""
import argparse
import hashlib
import itertools
import json
from pathlib import Path
import re
import subprocess

import numpy as np
from scipy.spatial import cKDTree

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--source-root', type=Path, required=True)
p.add_argument('--photo-root', type=Path, required=True)
p.add_argument('--footprint', type=Path, required=True)
p.add_argument('--out', type=Path, required=True)
a = p.parse_args()
root = a.source_root.resolve()
sha = lambda b: hashlib.sha256(b).hexdigest()
revision = subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip()
catalog_raw = (root/'web/platform/catalog/products.json').read_bytes()
product = next(p for p in json.loads(catalog_raw)['items'] if p['id'] == 'm5stack-k136')
assert product['name'] == 'M5Stack VAMeter' and product['chip'] == 'esp32s3'
assert next(c for c in product['controls'] if c['id'] == 'boot')['pin'] == 0
assert next(c for c in product['controls'] if c['id'] == 'encoder-press')['pin'] == 2


def cover(identity, name):
    folder = root/'web/platform/assets'/identity
    model_raw = (folder/'model.json').read_bytes()
    metadata = json.loads(model_raw)
    part = next(p for p in metadata['parts'] if p['sourceComponent'] == 0 and p['name'] == name)
    raw = (folder/metadata['binary']).read_bytes()
    assert sha(raw) == metadata['artifacts'][metadata['binary']]
    values = np.frombuffer(raw, dtype='<f4', count=part['vertexCount']*6, offset=part['byteOffset']).reshape(-1, 3, 6)
    points = np.unique(values[:, :, :3].reshape(-1, 3), axis=0)
    stl = (folder/'source.stl').read_bytes()
    assert sha(stl) == metadata['provenance']['sourceSha256']
    return metadata, values, points, {'id': identity, 'modelSha256': sha(model_raw), 'geometrySha256': sha(raw), 'sourceStlSha256': sha(stl), 'part': name, 'sourceComponent': 0, 'byteOffset': part['byteOffset'], 'vertexCount': part['vertexCount'], 'uniqueVertices': len(points), 'boundsMm': part['boundsMm'], 'sourceTransform': metadata['sourceTransform']}


k136, target_triangles, target, target_source = cover('m5stack-k136', 'stamp-cover')
s007, source_triangles, source, module_source = cover('m5stack-s007', 'cad-component-0')
source_center = (source.min(0)+source.max(0))/2
target_center = (target.min(0)+target.max(0))/2
tree = cKDTree(target)
candidates = []
for permutation in itertools.permutations(range(3)):
    for signs in itertools.product([-1, 1], repeat=3):
        rotation = np.eye(3)[list(permutation)]*np.array(signs)[:, None]
        if np.linalg.det(rotation) < .5:
            continue
        translation = target_center-rotation@source_center
        transformed = source@rotation.T+translation
        distances, _ = tree.query(transformed)
        candidates.append({'rotation': rotation.astype(int).tolist(), 'translationMm': translation.tolist(), 'rmsNearestVertexMm': float(np.mean(distances*distances)**.5), 'distanceQuantilesMm': dict(zip(['median', 'p90', 'p99', 'max'], np.quantile(distances, [.5, .9, .99, 1]).tolist()))})
candidates.sort(key=lambda row: row['rmsNearestVertexMm'])
best = candidates[0]
assert best['rotation'] == [[1, 0, 0], [0, -1, 0], [0, 0, -1]]
assert best['rmsNearestVertexMm'] < .02 and candidates[1]['rmsNearestVertexMm'] > 1

# A conservative region entirely on K136's own flat moving tongue cap. The
# whole module is registered above; no footprint size-only substitution is used.
region = {'centerMm': [3.4, -3.6, 5.53], 'sizeMm': [2.4, 2.2], 'normal': [0, 0, -1], 'rotationRadians': [float(np.pi), 0, 0], 'part': 'stamp-cover'}
cross2 = lambda x, y: x[0]*y[1]-x[1]*y[0]
samples = []
for dx, dy in [(0, 0), (-.4, -.4), (.4, -.4), (-.4, .4), (.4, .4)]:
    point = np.array(region['centerMm'])+np.array([dx*2.4, dy*2.2, 0])
    hits = []
    for index, triangle in enumerate(target_triangles):
        if not (np.all(triangle[:, 5] < -.99) and np.all(abs(triangle[:, 2]-5.55) < .005)):
            continue
        aa, bb, cc = triangle[:, :2]
        denominator = cross2(bb-aa, cc-aa)
        if abs(denominator) < 1e-10:
            continue
        u = cross2(point[:2]-aa, cc-aa)/denominator
        v = cross2(bb-aa, point[:2]-aa)/denominator
        if u >= -1e-5 and v >= -1e-5 and u+v <= 1.00001:
            support = triangle[0, :3]+u*(triangle[1, :3]-triangle[0, :3])+v*(triangle[2, :3]-triangle[0, :3])
            distance = float(np.linalg.norm(point-support))
            assert distance < .021
            hits.append({'nativePartTriangleIndex': index, 'nativeVerticesMm': triangle[:, :3].tolist(), 'barycentric': [float(1-u-v), float(u), float(v)], 'supportingPointMm': support.tolist(), 'planeDistanceMm': distance})
    assert hits, 'The conservative cap region must lie on actual K136 flat cap triangles'
    samples.append({'fraction': [dx, dy], 'pointMm': point.tolist(), 'nativeTriangleSupport': hits})

knowledge = json.loads((root/'web/platform/knowledge/m5stack-k136.json').read_text())
sources = {s['id']: s for s in knowledge['sources']}
photos = []
for number, identity in [('4', 'product-photo-3483e547cb'), ('7', 'product-photo-8d18bbf35e'), ('16', 'product-photo-ec48f54a42')]:
    raw = (a.photo_root/f'vameter-{number}.webp').read_bytes()
    assert sha(raw) == sources[identity]['sha256']
    photos.append({'sourceId': identity, 'url': sources[identity]['url'], 'bytes': len(raw), 'sha256': sha(raw)})
gif = (a.photo_root/'vameter.gif').read_bytes()
assert sha(gif) == 'f7a95587fdb04a6cf6aab0a2e5a0d3de16a1deaf075d063b2a98f03aa50d2ed5'
step_path = root/'web/platform/assets/m5stack-s007/source-manufacturer-cad-2a9b25ea49.step'
step = step_path.read_bytes()
assert sha(step) == '7f29337cdac90c80400897f52795ed0709548d4e74dc4436efb3e4f3873d2ebf'
footprint = a.footprint.read_bytes()
assert sha(footprint) == '2435abbd80d39803f29e8b57f4b52174339716b28633237103af512c521f7f83'
assert '(offset (xyz 0 0 1.1))' in footprint.decode()
receipt = {'schemaVersion': 1, 'productId': product['id'], 'name': product['name'], 'chip': product['chip'], 'sourceCommit': revision, 'catalogSha256': sha(catalog_raw), 'harnessSha256': sha(Path(__file__).read_bytes()), 'sources': {'target': target_source, 'moduleReference': module_source, 'officialPhotos': photos, 'officialDownloadGif': {'bytes': len(gif), 'sha256': sha(gif), 'url': 'https://m5stack.oss-cn-shenzhen.aliyuncs.com/resource/docs/products/core/VA%20Meter/M5PPT%20%281135%20x%201080%20%E5%83%8F%E7%B4%A0%29.gif'}, 'stampStep': {'repositoryPath': str(step_path.relative_to(root)), 'sha256': sha(step), 'productNames': re.findall(r"PRODUCT\('([^']*)'", step.decode())}, 'stampFootprint': {'url': 'https://github.com/m5stack/M5_Hardware/blob/e29c139cbc7c80b5ccc5557e5ab8025a7a19a109/KiCad/Footprints/M5Stack.pretty/Stamp-S3-SMD.kicad_mod', 'sha256': sha(footprint), 'gitBlobSha': '56a8ea7af9529a9e36686d35d90127952f1b454d', 'modelReference': '${KICAD_M5STACK}/M5KiCad.3d/M5STAMP-S3.step', 'offsetMm': [0, 0, 1.1], 'electricalG0CapNetNotEstablishedByFootprint': True}}, 'rigidRegistration': {'method': 'All 24 proper signed axis permutations, bbox-center translation, every source unique vertex queried against target unique vertices. No scale, deformation or model mutation.', 'testedRotations': len(candidates), 'best': best, 'runnerUp': candidates[1], 'allRmsNearestVertexMm': [c['rmsNearestVertexMm'] for c in candidates]}, 'nativeK136CapRegion': region, 'nativeSupportSamples': samples, 'terminalSourceConclusion': {'physicalBootExists': True, 'module': 'Stamp-S3', 'capIdentity': 'Native connected shield flexure tongue on K136 component 0; external face is -Z', 'defaultAssembledSurfaceClickNotClaimed': True, 'appropriateSpatialMode': 'inspection-only', 'mixIsoIsSeparateSwitch': True, 'encoderPressIsSeparateG2': True, 'logicalOnlyOrNonexistentDispositionRejected': True, 'requiredSpatialEvidence': 'Production-loader nearest-cap rays belong to the separate inspection mapping receipt; opaque shell and UNKNOWN PCB must remain occluders.'}, 'scope': 'Independent source and native surface registration only. Official exact-SKU gallery 4 labels menu G0/BOOT and gallery 7 exposes the Stamp module after Base removal; GIF demonstrates holding G0 for download. Existing KiCad STEP/footprint corroborate module geometry, but no K136 board layout or fixed full PCB reconstruction is asserted. No model edits, Cargo, renderer, CPU/GPU, guest or physical hardware validation.'}
a.out.parent.mkdir(parents=True, exist_ok=True)
a.out.write_text(json.dumps(receipt, ensure_ascii=False, indent=2)+'\n')
print(json.dumps({'physicalBootExists': True, 'nativeCapDirection': '-Z', 'inspectionOnlyCandidate': region['centerMm'], 'rmsNearestVertexMm': best['rmsNearestVertexMm'], 'nativeSupportSamples': len(samples)}))
