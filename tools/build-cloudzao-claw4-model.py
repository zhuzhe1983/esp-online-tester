#!/usr/bin/env python3
"""Rebuild the Claw4 illustration-derived preview without network access.

Usage: python tools/build-cloudzao-claw4-model.py --output /tmp/claw4-model
The output argument is a parent directory. Known source files are hash checked;
geometry is in millimetres, but estimated dimensions are not manufacturing CAD.
Requires numpy and Pillow, already used by the platform's model tooling.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
from PIL import Image
import numpy as np

ROOT = Path(__file__).resolve().parents[1]
PRODUCT = 'cloudzao-claw4'
ASSETS = ROOT / 'web/platform/assets' / PRODUCT


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def build(output):
    plan = json.loads((ASSETS / 'model-plan.json').read_text())
    for source in plan['provenance']['sources']:
        path = ASSETS / source['archiveName']
        if sha256(path) != source['sha256']:
            raise ValueError('Retained source hash mismatch: ' + path.name)
    # Isolate only the printed white marking, preserving the original reference.
    # This does not turn the manufacturer's promotional artwork into a photograph.
    crop = plan['photoDerivation']['logoCropPixels']
    rgb = np.asarray(Image.open(ASSETS / 'reference-front-back.jpg').convert('RGB').crop(crop))
    alpha = np.clip((rgb.min(axis=2).astype(float) - 160) / 65, 0, 1)
    rgba = np.full((*rgb.shape[:2], 4), 255, dtype=np.uint8)
    rgba[:, :, 3] = np.rint(alpha * 255).astype(np.uint8)
    spec = importlib.util.spec_from_file_location('platform_model_exporter', ROOT / 'tools/build-lilygo-expansion-models.py')
    exporter = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(exporter)
    # The generic exporter reads only model plans and retained files under ASSETS.
    # Use a temporary source tree so reconstruction never edits its input assets.
    with tempfile.TemporaryDirectory(prefix='claw4-source-') as temporary:
        base = Path(temporary)
        source = base / PRODUCT
        source.mkdir()
        for name in ['model-plan.json', *plan['retainedFiles']]:
            if name == 'rear-logo.png':
                continue
            (source / name).write_bytes((ASSETS / name).read_bytes())
        Image.fromarray(rgba).save(source / 'rear-logo.png', optimize=True)
        exporter.ASSETS = base
        receipt = exporter.build(PRODUCT, output)
    return receipt


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(build(args.output), ensure_ascii=False, indent=2))
