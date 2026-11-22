#!/usr/bin/env python3
"""Summarize retained LilyGO model evidence without upgrading its confidence."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--audit', type=Path, default=ROOT / 'docs/platform/research/lilygo-adaptation-sources-2026-11-22.json')
parser.add_argument('--discovery', type=Path, default=ROOT / 'docs/platform/research/lilygo-cad-discovery-2026-11-22.json')
parser.add_argument('--output', type=Path, default=ROOT / 'docs/platform/research/lilygo-model-expansion-2026-11-22.json')
args = parser.parse_args()
audit = json.loads(args.audit.read_text())
by_id = {item['id']: item for item in audit['items']}
discovery = json.loads(args.discovery.read_text()) if args.discovery.exists() else {'repositories': []}
patch_path = ROOT / 'docs/platform/research/lilygo-model-catalog-patches-2026-11-22.json'
patches = json.loads(patch_path.read_text())
items = []
asset_bytes = 0
retained_photo_bytes = 0
retained_cad_bytes = 0
for patch in patches['items']:
    product_id = patch['id']
    folder = ROOT / 'web/platform/assets' / product_id
    plan = json.loads((folder / 'model-plan.json').read_text())
    model = json.loads((folder / 'model.json').read_text())
    check = json.loads((folder / 'build-check.json').read_text())
    assert check['modelSha256'] == hashlib.sha256((folder / 'model.json').read_bytes()).hexdigest()
    source_item = by_id.get(product_id, {})
    cad_sources = [source for source in plan['provenance']['sources'] if source.get('kind') == 'manufacturer-cad']
    inspected = [row for row in discovery['repositories'] if product_id in row.get('productIds', [])]
    candidates = [{'repository': row['repository'], 'paths': [candidate['path'] for candidate in row.get('cadCandidates', [])]}
                  for row in inspected if row.get('cadCandidates')]
    if cad_sources:
        selection = {'status': 'officially-hosted-cad-retained', 'reason': plan.get('representationScope', 'Only geometry actually present in the pinned source is represented; inspect source and limitations for revision scope.')}
    elif candidates:
        selection = {'status': 'photo-preview-with-cad-candidates-unresolved', 'reason': 'Official repository CAD candidates exist. Exact SKU/revision and usable assembly scope have not been established for these files; this exterior remains a provisional photo projection.', 'candidates': candidates}
    elif inspected and all(row.get('status') == 'fetched' and not row.get('truncated') for row in inspected):
        selection = {'status': 'photo-preview-no-cad-in-inspected-repositories', 'reason': 'No CAD or embedded-footprint candidate was found in the listed complete repository snapshots. This does not prove that no model exists elsewhere.'}
    else:
        selection = {'status': 'photo-preview-cad-search-incomplete', 'reason': 'Exact official photographs are retained; no suitable exact CAD was established. Some linked repository aliases or variant mappings remain unresolved.'}
    photo_sources = [source for source in plan['provenance']['sources'] if source.get('kind') == 'manufacturer-photo' and source.get('file')]
    for file in folder.iterdir():
        if file.is_file():
            asset_bytes += file.stat().st_size
            if file.name.startswith('reference-'): retained_photo_bytes += file.stat().st_size
            if file.name.startswith('manufacturer-'): retained_cad_bytes += file.stat().st_size
    items.append({
        'id': product_id, 'priorityRank': plan.get('priorityRank'), 'name': source_item.get('name', product_id),
        'chronology': source_item.get('chronology'), 'mode': plan['mode'], 'modelStatus': 'partial',
        'model': f'web/platform/assets/{product_id}/model.json', 'sourceSelection': selection,
        'representationScope': plan.get('representationScope'), 'dimensions': plan['dimensions'],
        'dimensionChecks': plan.get('dimensionChecks', []),
        'photoDerivation': plan.get('photoDerivation'), 'manualPhotoCalibration': plan.get('manualPhotoCalibration'),
        'photoCount': len({source['file'] for source in photo_sources}),
        'retainedCad': [{key: source[key] for key in ('id', 'url', 'file', 'sha256', 'bytes', 'storedSha256', 'sourceUnit', 'revision', 'revisionType', 'commit', 'path') if key in source} for source in cad_sources],
        'sourceIds': [source['id'] for source in plan['provenance']['sources']],
        'inspectedRepositories': [row['repository'] for row in inspected],
        'displayHint': patch.get('displayHint'), 'displayEvidence': plan.get('displayEvidence'),
        'runtimeValidation': False, 'physicalValidation': 'Not established by mesh/export checks; per-source bounds and partial photo checks only.',
        'limitations': plan['limitations'], 'buildCheck': check,
    })
items.sort(key=lambda item: (item['priorityRank'] is None, item['priorityRank'] or 10000, item['id']))
report = {
    'schemaVersion': '1', 'evidenceDate': '2026-11-22',
    'scope': 'All 122 previously missing LilyGO model assets; the original five models are preserved.',
    'orderingPolicy': audit['orderingPolicy'],
    'summary': {'catalogProducts': 127, 'preservedModels': 5, 'newModels': len(items),
                'modeCounts': dict(Counter(item['mode'] for item in items)),
                'sourceSelectionCounts': dict(Counter(item['sourceSelection']['status'] for item in items)),
                'triangles': sum(item['buildCheck']['triangleCount'] for item in items),
                'storedGeometryBytes': sum(item['buildCheck']['storedGeometryBytes'] for item in items),
                'expandedGeometryBytes': sum(item['buildCheck']['expandedGeometryBytes'] for item in items),
                'retainedPhotoBytes': retained_photo_bytes, 'retainedCadArchiveBytes': retained_cad_bytes,
                'totalAssetBytes': asset_bytes},
    'verification': {'productionLoaderCommand': 'node tools/verify-lilygo-expansion-models.mjs',
                     'exportedModels': len(items), 'allExportChecksPassed': all(item['buildCheck']['exportPassed'] for item in items),
                     'maximumFloat32ExportErrorMm': max(item['buildCheck']['maxFloat32ExportErrorMm'] for item in items),
                     'physicalAccuracyPassed': False, 'firmwareCompatibilityPassed': False,
                     'browserRendering': 'The production declarative loader is checked. Real WebGL rendering is a separate integration/CI gate; do not infer it from a mesh test.'},
    'items': items,
}
args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
print(json.dumps(report['summary'], ensure_ascii=False))
