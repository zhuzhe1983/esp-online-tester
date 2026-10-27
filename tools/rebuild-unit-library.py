#!/usr/bin/env python3
"""Rebuild the audited Unit models from committed CAD, photos and plans.

Requires tools/requirements-model.txt. Network is never used. Outputs go to a
caller-selected scratch directory; catalog/knowledge files are never mutated.
"""
import argparse
import importlib.util
import json
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
IDS = ('m5stack-unit-button-u027', 'm5stack-unit-angle-u005', 'm5stack-unit-rgb-u003', 'm5stack-unit-hub-u006')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('id', choices=IDS)
    parser.add_argument('--scratch', type=Path, required=True)
    args = parser.parse_args()
    spec = importlib.util.spec_from_file_location('model_builder', ROOT / 'tools/onboard-lane-01.py')
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)
    source = ROOT / 'web/platform/assets' / args.id
    scratch = args.scratch.resolve()
    if scratch == source or source in scratch.parents:
        parser.error('Use a scratch directory outside the published asset directory.')
    audit = json.loads((ROOT / 'docs/platform/evidence' / args.id / 'source-audit.json').read_text())
    plan = json.loads((source / 'model-plan.json').read_text())
    helper.TEMP = scratch / 'inputs'
    folder = helper.TEMP / args.id
    folder.mkdir(parents=True, exist_ok=True)
    prepared = {'id': args.id, 'sources': audit['sources']}
    required = {plan['sourceId'], 'cad-license'}
    required.update(item['sourceId'] for item in plan.get('photos', []))
    required.update(item['sourceId'] for item in plan.get('textureSources', []))
    # Some retained photos support derived knobs/connectors rather than a review camera.
    # Preserve those pinned reference assets too, so the metadata hash round-trips.
    required.update(item['id'] for item in audit['sources']
                    if item.get('archiveName') and (source / ('reference-' + item['archiveName'])).is_file())
    for receipt in audit['sources']:
        if receipt['id'] not in required:
            continue
        name = receipt.get('archiveName')
        if receipt['id'] == plan['sourceId']:
            candidate = source / 'source.stl'
        elif receipt['id'] == 'cad-license':
            candidate = source / 'LICENSE-M5Stack.txt'
        else:
            candidate = source / ('reference-' + name)
        if not candidate.is_file() or helper.sha(candidate.read_bytes()) != receipt['sha256']:
            raise ValueError('Missing or altered pinned input: ' + str(candidate.relative_to(ROOT)))
        shutil.copyfile(candidate, folder / name)
    helper.write(folder / 'components.json', audit['components'])
    output = scratch / 'assets' / args.id
    output.mkdir(parents=True, exist_ok=True)
    for receipt in audit['sources']:
        name = receipt.get('archiveName')
        if name and (source / ('reference-' + name)).is_file():
            shutil.copyfile(folder / name, output / ('reference-' + name))
    helper.build_model(prepared, plan, scratch / 'assets' / args.id,
                       evidence_root=scratch / 'evidence', license_path=source / 'LICENSE-M5Stack.txt')
    print(scratch / 'assets' / args.id / 'model.json')


if __name__ == '__main__':
    main()
