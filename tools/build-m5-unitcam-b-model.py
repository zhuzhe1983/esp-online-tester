#!/usr/bin/env python3
"""Rebuild the exact-SKU U109-B photo/drawing preview from retained inputs."""
import argparse
import importlib.util
import json
from pathlib import Path

root = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('source_preview_exporter', root / 'tools/build-m5-poe-cameraf-models.py')
exporter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(exporter)

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(exporter.build('m5stack-u109-b', args.output), ensure_ascii=False, indent=2))
