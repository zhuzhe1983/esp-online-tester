#!/usr/bin/env python3
"""Recheck the source-bound Audio battery export exception without network access."""
from pathlib import Path
import argparse, gzip, hashlib, importlib.util, json, tempfile
import numpy as np
from OCP.BRepTools import BRepTools

ROOT = Path(__file__).resolve().parents[3]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
spec = importlib.util.spec_from_file_location('builder', ROOT / 'tools/build-waveshare-models.py')
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)
raw = gzip.decompress((ROOT / 'web/platform/assets/waveshare-s3-audio-board-mechanical/manufacturer.step.gz').read_bytes())
assert hashlib.sha256(raw).hexdigest() == '0a2557e0a975f29049ab5e55222c01f13d2559d79ffade488106107f0bea521d'
with tempfile.TemporaryDirectory() as directory:
    path = Path(directory) / 'manufacturer.step'
    path.write_bytes(raw)
    shape = builder.topology(path)[2]
    source_bounds = builder.bounds(shape)
    results = []
    for deflection, angle in [(.03, .25), (.01, .1), (.003, .05)]:
        BRepTools.Clean_s(shape)
        vertices = builder.tessellate(shape, deflection, angle)
        measured = np.asarray([vertices[:, :3].min(0), vertices[:, :3].max(0)])
        results.append({'deflection': deflection, 'angle': angle, 'triangles': len(vertices)//3,
                        'vertexCount': len(vertices), 'triangulationCacheCleared': True,
                        'sourceBounds': source_bounds.tolist(), 'meshBounds': measured.tolist(),
                        'maxDifference': float(np.abs(source_bounds-measured).max())})
args.output.write_text(json.dumps(results, indent=2)+'\n')
