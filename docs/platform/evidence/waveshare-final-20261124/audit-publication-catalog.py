#!/usr/bin/env python3
"""Read-only coverage, preservation and final exporter-overlay audit.

Only the report JSON is written. There is no build, test execution, Git object
write, index update, catalog edit or network request. Retained official sources
define the audited snapshot; this is not a fresh inventory crawl.
"""
import ast
from collections import Counter, defaultdict
import copy
import datetime as dt
import hashlib
import json
from pathlib import Path
import subprocess
import time
from retained_evidence import read_bytes, read_json

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
INVENTORY = 'docs/platform/research/waveshare-20261124/waveshare-complete-inventory.json'
CENSUS = 'docs/platform/research/waveshare-20261124/waveshare-current-census.json'
GUARDS = 'docs/research/waveshare-s3-industrial-2026-11-24/batch-04-catalog-recovery/published-catalog-guards.json'
CATALOGS = ['web/platform/catalog/products.json', 'web/platform/catalog/firmwares.json',
            'web/platform/catalog/firmware-watchlist.json', 'web/platform/knowledge/index.json']


def read(path):
    return read_json(ROOT / path)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode()


def identity(path):
    raw = read_bytes(ROOT / path)
    return dict(path=path, bytes=len(raw), sha256=sha(raw))


def exporter_overlays(allowed):
    """Execute the exporter's actual catalog-only AST, entirely in memory.

    The source-tree membership check is deferred to the real exporter after
    the development commit. Its filtering and serialization are not re-created
    in a second implementation, and no Git calls are included here.
    """
    path = ROOT / 'tools/export-waveshare-increment.py'
    module = ast.parse(path.read_text())
    main = next(n for n in module.body if isinstance(n, ast.FunctionDef) and n.name == 'main')
    nodes = []
    for node in main.body:
        if isinstance(node, ast.Assign) and isinstance(node.value, ast.Call) and isinstance(node.value.func, ast.Name) and node.value.func.id == 'obj':
            nodes.append(node)
        elif isinstance(node, ast.Assign) and any(isinstance(t, ast.Name) and t.id in {'source_paths', 'model_dirs'} for t in node.targets):
            continue
        elif nodes:
            committed_path_loop = isinstance(node, ast.For) and isinstance(node.iter, ast.Name) and node.iter.id == 'selected_products' and any(
                isinstance(n, ast.Assert) and isinstance(n.test, ast.Compare) and any(isinstance(c, ast.Name) and c.id == 'source_paths' for c in n.test.comparators)
                for n in ast.walk(node))
            if committed_path_loop:
                continue
            nodes.append(node)
            if isinstance(node, ast.Assign) and any(isinstance(t, ast.Name) and t.id == 'overlays' for t in node.targets):
                break
    assert len(nodes) == 19, 'Exporter filtering changed; review AST selection before rerunning.'
    block = ast.Module(body=nodes, type_ignores=[])
    for node in ast.walk(block):
        if isinstance(node, ast.Call):
            assert ((isinstance(node.func, ast.Name) and node.func.id in {'obj', 'set'}) or
                    (isinstance(node.func, ast.Attribute) and node.func.attr in {'get', 'append', 'startswith'})), ast.unparse(node)
    def obj(_ref, name):
        assert name in CATALOGS
        return read(name)
    context = dict(obj=obj, source='read-only-worktree', allowed=allowed)
    exec(compile(block, str(path), 'exec'), context)
    serializer = next(n for n in ast.walk(main) if isinstance(n, ast.Assign) and any(isinstance(t, ast.Name) and t.id == 'blob' for t in n.targets))
    serializer_block = ast.Module(body=[serializer], type_ignores=[])
    checks = []
    for name, value in context['overlays'].items():
        serialization = dict(json=json, value=value)
        exec(compile(serializer_block, str(path), 'exec'), serialization)
        raw = serialization['blob']
        original = (ROOT / name).read_bytes()
        assert raw == original, ('Exporter changes tested catalog bytes', name)
        checks.append(dict(path=name, bytes=len(raw), sha256=sha(raw), sourceBytesIdentical=True))
    return dict(exporter=identity('tools/export-waveshare-increment.py'), allowedProfiles=len(allowed),
                extractedStatementCount=len(nodes), filterAstSha256=sha(ast.dump(block).encode()),
                serializerExpression=ast.unparse(serializer.value), catalogs=checks,
                missingCommittedSourceMembershipCheck='Deferred to the actual exporter after source commit; this audit reads current worktree files.',
                gitWrites=False, catalogWrites=False, allSerializedBytesIdentical=True)


def main():
    started = time.monotonic()
    inputs = read(str((OUT / 'publication-audit-inputs.json').relative_to(ROOT)))['inputs']
    published = inputs['published029']['value']
    selection = inputs['selection030']['value']
    inventory, census, guards = read(INVENTORY), read(CENSUS), read(GUARDS)
    families = {v['listingId']: v for v in inventory['items'] if v['scope'] == 'included-controller-or-controller-kit'}
    assert len(inventory['items']) == len({v['listingId'] for v in inventory['items']}) == 180
    assert len(families) == 149 and len(published['batches']) == 29
    assert len(selection['listings']) == 4 and len(selection['productIds']) == 16
    assert not set(published['listings']) & set(selection['listings'])
    assert not set(published['productIds']) & set(selection['productIds'])
    batches = published['batches'] + [dict(number=30, **selection)]
    assert [b['number'] for b in batches] == list(range(1, 31))
    assert all(len(b['listings']) == (4 if b['number'] == 30 else 5) for b in batches)
    ordered_listings = [v for b in batches for v in b['listings']]
    ordered_profiles = [v for b in batches for v in b['productIds']]
    assert len(ordered_listings) == len(set(ordered_listings)) == 149
    assert len(ordered_profiles) == len(set(ordered_profiles)) == 407
    assert set(ordered_listings) == set(families)
    allowed = set(ordered_profiles)
    catalog = read(CATALOGS[0])
    products = {p['id']: p for p in catalog['items']}
    waveshare = {k: v for k, v in products.items() if v.get('manufacturer') == 'Waveshare'}
    assert len(catalog['items']) == len(products) == 676 and set(waveshare) == allowed
    fragments = {}
    for path in sorted((ROOT / 'tools/waveshare-batches').rglob('*.json')):
        value = json.loads(path.read_text())
        if isinstance(value, dict) and value.get('listingId'):
            assert value['listingId'] not in fragments, value['listingId']
            fragments[value['listingId']] = (path.relative_to(ROOT).as_posix(), value)
    assert set(fragments) == set(families)
    fragment_ids, raw_skus, family_rows = [], [], []
    for listing in ordered_listings:
        path, fragment = fragments[listing]
        source = families[listing]
        row_skus = [str(v['sku']) for v in source['variants']]
        raw_skus.extend(row_skus)
        expected_skus = set(row_skus) - {'33355'}
        ids = [p['id'] for p in fragment['products']]
        assert all(p == products[p['id']] for p in fragment['products']), listing
        assert {str(p['sku']) for p in fragment['products']} == expected_skus, listing
        assert set(ids) == {pid for b in batches if listing in b['listings'] for pid in b['productIds'] if pid in ids}
        fragment_ids.extend(ids)
        family_rows.append(dict(listingId=listing, officialUrl=source['url'], documentationLinks=source.get('documentationLinks', []),
                                fragment=identity(path), profileIds=ids, controllerSkus=sorted(expected_skus),
                                officialProductResponse=source.get('productSource'),
                                productsCanonicalSha256=sha(canonical(fragment['products']))))
    assert len(fragment_ids) == len(set(fragment_ids)) == 407 and set(fragment_ids) == allowed
    assert len(raw_skus) == len(set(raw_skus)) == 384
    actual_skus = {str(p['sku']) for p in waveshare.values()}
    assert actual_skus == set(raw_skus) - {'33355'} and len(actual_skus) == 383
    final_fragments = [fragments[name][1] for name in selection['listings']]
    assert [p['id'] for f in final_fragments for p in f['products']] == selection['productIds']
    final_ids = set(selection['productIds'])
    owned = {
        'catalog/products.json': final_ids,
        'catalog/firmwares.json': {v['id'] for f in final_fragments for v in f['firmwares']},
        'catalog/firmware-watchlist.json': {v['id'] for f in final_fragments for v in f['watchlist']},
        'knowledge/index.json': final_ids,
    }
    preserved = []
    for suffix, guard in guards['files'].items():
        path = 'web/platform/' + suffix
        key = guard['recordKey']
        current = read(path)
        retained = copy.deepcopy(current)
        retained[key] = [v for v in retained[key] if v['id'] not in owned[suffix]]
        if suffix == 'knowledge/index.json':
            retained['updatedAt'] = guard['metadata']['updatedAt']
        assert len(retained[key]) == guard['records']
        assert sha(canonical(retained)) == guard['canonicalSha256']
        old_raw = subprocess.check_output(['git', 'show', guards['publishedCommit'] + ':' + path], cwd=ROOT)
        assert sha(old_raw) == guard['rawSha256']
        assert retained == json.loads(old_raw), path
        preserved.append(dict(path=path, originalRecords=guard['records'], addedRecords=len(owned[suffix]),
                              publishedRawSha256=guard['rawSha256'], retainedCanonicalSha256=sha(canonical(retained)),
                              matchesPublished029Object=True))
    freeze = read(str((OUT / 'catalog-freeze.json').relative_to(ROOT)))
    lines = ''.join(f'{sha((ROOT / p).read_bytes())}  {p}\n' for p in sorted(CATALOGS))
    assert freeze == dict(fileCount=4, manifestSha256=sha(lines.encode()), sha256sum=lines)
    export = exporter_overlays(allowed)
    final_platform = read(str((OUT / 'final-platform-after-model-review.json').relative_to(ROOT)))
    assert final_platform['passed'] and final_platform['totals']['pass'] == 507
    assert all(final_platform['after'][p]['sha256'] == sha((ROOT / p).read_bytes()) for p in CATALOGS)
    firmwares = [f for f in read(CATALOGS[1])['items'] if allowed & set(f.get('productIds', []))]
    watches = [w for w in read(CATALOGS[2])['entries'] if allowed & set(w.get('compatibility', {}).get('productIds', []))]
    assert all(not f.get('simulationVerified') and not f.get('hardwareValidated') for f in firmwares)
    assert all(p['runtime']['status'] == p['model']['status'] == 'partial' for p in waveshare.values())
    assert all(p.get('chronology', {}).get('date') is None for p in waveshare.values())
    sku_groups = defaultdict(list)
    for p in waveshare.values():
        sku_groups[str(p['sku'])].append(dict(id=p['id'], hardwareRevision=p.get('hardwareRevision'), board=p['runtime']['board']))
    sources = [dict(label='Official category defining the retained scope', url=inventory['scopeUrl'], evidence=INVENTORY)]
    final_rows = []
    for family in final_fragments:
        p = family['products'][0]
        source_map = {v['id']: v for v in p['sources']}
        for key in ['storefront', 'overview', 'resources', 'schematic']:
            sources.append(dict(label=family['listingId'] + ': ' + key, url=source_map[key]['url'], evidence=fragments[family['listingId']][0]))
        final_rows.append(dict(listingId=family['listingId'], products=[dict(id=q['id'], name=q['name'], sku=q['sku'],
                              flashMb=q['runtime']['flashMb'], psramMb=q['runtime']['psramMb'],
                              modelProvenance=q['model']['provenance']) for q in family['products']]))
    sources.extend([
        dict(label='Basic Kit: accessory-only SKU33355 exclusion', url='https://www.waveshare.com/product/esp32-related/esp32-xx-basic-kit.htm', evidence='tools/waveshare-batches/root-extras/esp32-xx-basic-kit.json'),
        dict(label='Zero native mechanical source (base geometry; not N8R8 identity)', url='https://files.waveshare.com/wiki/ESP32-S3-Zero/ESP32-S3-Zero_v2-138eb3753cf14afa3252058c380ca416.stp', evidence='web/platform/assets/waveshare-esp32-s3-zero-m/model-plan.json'),
        dict(label='DEV-KIT N8R8 native manufacturer CAD archive', url='https://files.waveshare.com/wiki/ESP32-S3-DEV-KIT-N8R8/ESP32-S3-DEV-KIT-N8R8_20260110.zip', evidence='web/platform/assets/waveshare-esp32-s3-dev-kit-n8r8/model-plan.json'),
    ])
    cache_paths = [p for f in families.values() for p in f.get('documentationCacheFiles', [])]
    priority_batches = [dict(number=b['number'], displayPriorityHints=sum(bool(families[v].get('displayPriorityHint')) for v in b['listings']), listingCount=len(b['listings'])) for b in batches]
    report = dict(schemaVersion=1, checkedAt=dt.datetime.now(dt.timezone.utc).isoformat(),
        script=identity(str(Path(__file__).resolve().relative_to(ROOT))),
        inputSnapshots=identity(str((OUT / 'publication-audit-inputs.json').relative_to(ROOT))),
        officialInventory=identity(INVENTORY), officialCensus=identity(CENSUS), publishedGuards=identity(GUARDS),
        snapshotCheckedAt=inventory['checkedAt'], sourceCensusCheckedAt=census['checkedAt'],
        scope=dict(officialCategoryListings=180, includedControllerFamilies=149, excludedCategoryListings=31,
                   rawIncludedSkuRows=384, excludedAccessorySkus=['33355'], controllerSkus=383, exactProfiles=407,
                   distinctReferencedModelUrls=len({p['model']['url'] for p in waveshare.values()}),
                   profilesByChip=dict(sorted(Counter(p['chip'] for p in waveshare.values()).items())),
                   missingFamilies=[], extraFamilies=[], missingControllerSkus=[], extraControllerSkus=[]),
        selection030=dict(listings=selection['listings'], productIds=selection['productIds'], families=4, profiles=16, controllerSkus=16,
                          prior029Families=145, prior029Profiles=391, prior029ControllerSkus=367, disjointFromPublished029=True),
        batches=[dict(number=b['number'], listings=b['listings'], profileCount=len(b['productIds'])) for b in batches],
        finalFamilies=final_rows, familyCoverage=family_rows,
        skuRevisionProfiles=[dict(sku=k, profiles=v) for k, v in sorted(sku_groups.items()) if len(v) > 1],
        excludedListings=[dict(listingId=i['listingId'], url=i['url'], scope=i['scope'], basis=i['scopeBasis']) for i in inventory['items'] if i['scope'] != 'included-controller-or-controller-kit'],
        preserved029=preserved, catalogFreeze=freeze, exporterSimulation=export,
        testedCatalogBinding=dict(receipt=identity(str((OUT / 'final-platform-after-model-review.json').relative_to(ROOT))), tests=507, passed=507, currentCatalogBytesMatch=True),
        capabilityBoundary=dict(runtimeStatus={'partial': 407}, modelStatus={'partial': 407},
                                modelProvenanceByProfile=dict(sorted(Counter(p['model']['provenance'] for p in waveshare.values()).items())),
                                waveshareFirmwareEntries=len(firmwares), firmwareStatus=dict(Counter(f['status'] for f in firmwares)),
                                waveshareWatchEntries=len(watches), firmwareSimulationVerifiedTrue=0, firmwareHardwareValidatedTrue=0,
                                browserWebglAcceptance=False, physicalHardwareAcceptance=False),
        priority=dict(releaseDatesEstablished=0, initialDisplayPriorityHints=99, actualBatches=priority_batches,
                      strictlyDisplayFirst=False, strictlyStorefrontPositionOrdered=False,
                      statement='Display and new-family work was prioritized but batches also grouped shared implementation work. All407 release dates remain unknown; Position is a source-labeled proxy, not a verified release chronology.'),
        firstPartyLinks=sources,
        metadataInterpretationNotes=[
            dict(id='historical-inventory-stage', evidence=[INVENTORY, CENSUS], note='boardVariantValidationComplete=false and census skuExpansionComplete=false describe retained initial source stages, not the final coverage result. Initial documentPagesFailed includes Thermal Camera and C6 DEV-KIT NX; later fragments cite the actual working wiki/N8 source endpoints. These initial records are preserved.'),
            dict(id='ephemeral-cache-pointers', evidence=[INVENTORY], historicalCachePointerCount=len(cache_paths), absentCachePointerCount=sum(not Path(p).exists() for p in cache_paths), note='Original documentationCacheFiles record the old working cache. They are historical provenance, not the current retained source bodies or a prerequisite for this audit.'),
            dict(id='rust-unchanged-scope', evidence=[fragments[x][0] for x in selection['listings']] + ['docs/platform/evidence/waveshare-final-20261124/classic-cli-correction.json'], note='Final fragments say Rust/Cargo sources remain unchanged in the metadata-reconstruction integrationNotes. This applies only to that operation. The final delivery additionally corrects cli/src/lib.rs and cli/tests/classic_psram_option.rs; final855 freeze and rebuilt-WASM proof govern final source identity.'),
            dict(id='historical-model-revision', evidence=['docs/research/waveshare-s3-industrial-2026-11-24/batch-04/README.md', 'docs/platform/evidence/waveshare-batch030-models-20261124/'], note='Old baseline-four unchanged wording and old36 model images are retained historical revision evidence. The README front matter explicitly supersedes them for15 reconstructed models; only the published baseZero assets were preserved byte-for-byte.'),
            dict(id='model-type-vs-provenance', evidence=[CATALOGS[0]], note='manufacturer-cad is the loader model type, not proof that every profile is an unmodified native CAD model. Actual provenance is explicit:195 official-cad,22 official-cad-partial,3 official-cad-with-dimensioned-optical-reference,187 dimension-photo-reconstruction;407 profiles reference259 distinct model URLs.'),
            dict(id='priority-hints-not-final-display-facts', evidence=[INVENTORY, 'tools/waveshare-batches/root-extras/thermal-45-camera-esp32-module.json'], note='The99 initial displayPriorityHint values are scheduling hints, not99 certified onboard-screen families. For example, the final Thermal Camera profile explicitly has no onboard display; its sensor resolution is not a display resolution.'),
            dict(id='nano-led-source-disagreement', evidence=['web/platform/knowledge/waveshare-esp32-s3-nano.json'], note='Historical LED package pin labels conflict with the ROHM datasheet. The software GPIO/color mapping follows the retained official program and fixed Arduino mapping; measured optical color remains unverified.'),
            dict(id='pico-memory-source-disagreement', evidence=['web/platform/knowledge/waveshare-esp32-s3-pico.json'], note='The documented S3R2/2MiB identity conflicts with an Octal-PSRAM paragraph. The linked4MiB MicroPython software layout does not redefine the physical16MiB Flash. The uncertainty remains explicit.'),
            dict(id='devkit-native-dimension-difference', evidence=['web/platform/assets/waveshare-esp32-s3-dev-kit-n8r8/model-plan.json'], note='Published25.4×63.3mm and native25.42068576×63.31069634mm PCB extents differ slightly. Native geometry was not rescaled to hide the difference; physical fit is unverified.'),
        ],
        reportLimitations=[
            'Coverage is complete for the retained2026-11-23 category snapshot, not future products or products outside that category.',
            '407 profiles are383 controller SKUs with24 additional explicit hardware/controller-revision profiles; do not call this407 distinct storefront devices.',
            'Every runtime/model remains partial. Digital/firmware observations are per exact profile and image; they do not imply all peripheral features or complete factory applications.',
            'All246 Waveshare firmware entries remain simulationVerified=false/hardwareValidated=false:180 trial,66 reference. Remote links and fixed manifests are not full firmware acceptance or hosted rewritten binaries.',
            'The final16 original-image evidence is not inherited by new header/memory/antenna variants. ZeroN8R8 andN32R16V lack substituted near-capacity default images.',
            'No current browser WebGL or physical-board acceptance is claimed. Node production-loader/geometry/source-photo reviews are separately bounded evidence.',
            'Photo geometry has explicit UNKNOWN or estimated surfaces/heights and no manufacturing/connector-fit certification. Generic RESET uses restart rather than invented GPIO controls.',
            'This audit does not establish that030 has already been published; the final exporter, complete sequential-apply verifier and Sites readback remain separate publication steps.',
        ],
        mutations=dict(catalog=False, models=False, rust=False, tests=False, git=False, output='publication-catalog-audit.json only'),
        passed=True, durationSeconds=round(time.monotonic() - started, 6))
    (OUT / 'publication-catalog-audit.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps(dict(passed=True, families=149, profiles=407, controllerSkus=383, fragmentObjectsEqual=True,
                         published029Preserved=True, exporterCatalogBytesIdentical=True, durationSeconds=report['durationSeconds']), ensure_ascii=False))


if __name__ == '__main__':
    main()
