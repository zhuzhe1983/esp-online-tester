#!/usr/bin/env python3
"""Verify the 35 screening candidates plus C008 against pinned final evidence.

Default mode reads the actual --source-root working tree. --snapshot instead
checks immutable contribution commits and reports Root model bytes separately;
it never describes contribution bytes as integrated. No renderer/build runs.
"""
import argparse
import collections
import copy
import datetime
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MATRIX = ROOT/'docs/platform/evidence/m5-visual-closure-20261126/matrix.json'
SHA = re.compile(r'^[0-9a-f]{64}$')
REV = re.compile(r'^[0-9a-f]{40}$')
DISPOSITIONS = {'repaired-visible-surface', 'shared-host-repaired', 'evidence-excluded', 'source-limited-unknown-retained'}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':')).encode()


def json_document(data, label):
    def unique_keys(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError('Duplicate JSON key: '+key)
            result[key] = value
        return result
    try:
        return json.loads(data, object_pairs_hook=unique_keys)
    except ValueError as error:
        raise ValueError(label+': '+str(error)) from error


def safe_path(path):
    value = PurePosixPath(path)
    if value.is_absolute() or '..' in value.parts or not value.parts or value.parts[0] not in ('docs', 'web', 'tools'):
        raise ValueError('Unsafe repository path')
    return value


def nodes(value, path=''):
    if isinstance(value, dict):
        for key, child in value.items():
            key = key.replace('~', '~0').replace('/', '~1')
            yield from nodes(child, path+'/'+key)
    elif isinstance(value, list):
        for index, child in enumerate(value):
            yield from nodes(child, path+'/'+str(index))
    else:
        yield path, value


def pointer(value, path):
    for key in path.split('/')[1:]:
        key = key.replace('~1', '/').replace('~0', '~')
        value = value[int(key)] if isinstance(value, list) else value[key]
    return value


def inverse_nonvisual_metadata(model, product, contract):
    """Permit only routing fields and invisible input/inspection declarations."""
    paths = contract['removedJsonPointers']
    if len(set(paths)) != len(paths) or any(a != b and a.startswith(b+'/') for a in paths for b in paths):
        raise ValueError('Overlapping or duplicate equivalence pointers')
    declared = {c['id'] for c in product['controls']}
    supported = {c['id'] for c in product['controls'] if c.get('simulationSupported') is not False and c['type'] in ('button', 'matrix-key')}
    def input_region(region):
        if not isinstance(region, dict) or region.get('id') not in supported or not region.get('positionMm') or not region.get('sizeMm'):
            raise ValueError('New invisible input region has no supported source control')
    for path in paths:
        value = pointer(model, path)
        if path == '/presentation/inspectionGroups':
            continue  # Assembly-only comparison; expanded state needs separate proof.
        if re.fullmatch(r'/(?:parts|derivedParts)/\d+/controlId', path) or path == '/presentation/screen/controlId':
            if value not in supported:
                raise ValueError('New input route is not a supported product control')
        elif re.fullmatch(r'/presentation/buttons/\d+', path):
            input_region(value)
        elif path == '/presentation/buttons':
            for region in value:
                input_region(region)
        elif re.fullmatch(r'/presentation/inspectionGroups/\d+/parts/\d+', path):
            valid = {'cover-key:'+r['id'] for r in model['presentation'].get('buttons', [])}
            if value not in valid:
                raise ValueError('Only invisible input companions may amend carrier membership')
        else:
            raise ValueError('Equivalence cannot remove visual geometry/material/texture data: '+path)
    result = copy.deepcopy(model)
    # Delete deepest values first and list indices in descending order.
    def removal_key(path):
        parent, key = path.rsplit('/', 1)
        return path.count('/'), parent, int(key) if key.isdigit() else -1, key
    for path in sorted(paths, key=removal_key, reverse=True):
        parent, key = path.rsplit('/', 1)
        container = pointer(result, parent)
        if isinstance(container, list):
            del container[int(key)]
        else:
            del container[key]
    for operation in contract.get('restoredJsonValues', []):
        path, value = operation['pointer'], operation['value']
        if not re.fullmatch(r'/(?:parts|derivedParts)/\d+/controlId', path) or value not in declared:
            raise ValueError('Unsupported restoration of historical routing metadata')
        parent, key = path.rsplit('/', 1)
        container = pointer(result, parent)
        if key in container:
            raise ValueError('Historical routing restoration would overwrite a value')
        container[key] = value
    return result


class Reader:
    def __init__(self, root, snapshot):
        self.root = root.resolve()
        self.snapshot = snapshot
        self.catalogs = {}
        self.revisions = set()

    def git(self, *args):
        result = subprocess.run(['git', '-C', str(self.root), *args], capture_output=True, check=False)
        if result.returncode:
            raise ValueError('Missing pinned git object: '+args[-1])
        return result.stdout

    def validate_revision(self, revision):
        if not REV.fullmatch(revision):
            raise ValueError('Contribution revision must be a full commit SHA')
        if self.snapshot and revision not in self.revisions:
            if self.git('cat-file', '-t', revision).strip() != b'commit':
                raise ValueError('Pinned revision is not a commit')
            self.revisions.add(revision)

    def read(self, path, revision):
        safe_path(path)
        self.validate_revision(revision)
        if self.snapshot:
            return self.git('show', revision+':'+path)
        file = self.root/path
        if not file.resolve().is_relative_to(self.root):
            raise ValueError('Repository file escapes source root')
        return file.read_bytes()

    def catalog(self, revision):
        key = revision if self.snapshot else 'working-tree'
        if key not in self.catalogs:
            self.catalogs[key] = {p['id']: p for p in json_document(self.read('web/platform/catalog/products.json', revision), 'products.json')['items']}
        return self.catalogs[key]


def verify_entry(entry, reader):
    identity = entry['id']
    revision = entry['contributionSnapshot']
    expected = entry['model']
    result = {'id': identity, 'disposition': entry['disposition'], 'plannedBatch': entry['plannedBatch'],
              'contributionSnapshot': revision, 'expectedModelSha256': expected['sha256'], 'errors': []}
    errors = result['errors']
    try:
        if entry['disposition'] not in DISPOSITIONS:
            raise ValueError('Unknown disposition')
        if entry['disposition'] in ('evidence-excluded', 'source-limited-unknown-retained') and not entry.get('decisionBasis'):
            raise ValueError('Exclusion/source-limited outcome needs an explicit source decision')
        if not REV.fullmatch(entry['dispositionCommit']) or not REV.fullmatch(revision):
            raise ValueError('Model contribution commits must be full SHAs')
        reader.validate_revision(entry['dispositionCommit'])
        data = reader.read(expected['path'], revision)
        model = json_document(data, expected['path'])
        actual = digest(data)
        result['actualModelSha256'] = actual
        if actual != expected['sha256']:
            raise ValueError('Model differs from evidence-bound contribution; refresh its final proof before accepting')
        if model['id'] != expected['assetId']:
            raise ValueError('Model identity mismatch')
        plan = reader.read(expected['planPath'], revision)
        json_document(plan, expected['planPath'])
        if digest(plan) != expected['planSha256']:
            raise ValueError('Model reconstruction plan changed')
        if digest(canonical(model['artifacts'])) != expected['artifactManifestSha256']:
            raise ValueError('Artifact manifest changed')
        if len(model['artifacts']) != expected['artifactCount']:
            raise ValueError('Artifact count mismatch')
        if model['binary'] != expected['binary'] or model['artifacts'][model['binary']] != expected['binarySha256']:
            raise ValueError('Native stored geometry identity changed')
        asset_dir = PurePosixPath(expected['path']).parent
        total = 0
        for name, sha in model['artifacts'].items():
            if PurePosixPath(name).name != name or not SHA.fullmatch(sha):
                raise ValueError('Unsafe artifact name or digest')
            payload = reader.read(str(asset_dir/name), revision)
            if digest(payload) != sha:
                raise ValueError('Artifact bytes changed: '+name)
            total += len(payload)
        result['verifiedArtifacts'] = len(model['artifacts'])
        result['verifiedArtifactBytes'] = total
        product = reader.catalog(revision)[identity]
        declared = product['model']['url'].removeprefix('./')
        if 'web/platform/'+declared != expected['path']:
            raise ValueError('Catalog model URL is not the evidence-bound model')
        for field, value in entry.get('sharedHostIdentity', {}).items():
            actual_value = product['model']['url'] if field == 'modelUrl' else product.get(field)
            if actual_value != value:
                raise ValueError('Shared-host identity mismatch: '+field)
        equivalent = entry.get('assembledEquivalence')
        if equivalent:
            before = inverse_nonvisual_metadata(model, product, equivalent)
            serialized = (json.dumps(before, ensure_ascii=False, indent=2)+'\n').encode()
            if digest(serialized) != equivalent['priorModelSha256']:
                raise ValueError('Final model differs from prior CPU-bound model beyond the permitted nonvisual metadata')
            engine = equivalent['engine']
            if digest(reader.read(engine['path'], engine['sourceRevision'])) != engine['sha256']:
                raise ValueError('Assembly-equivalence engine differs from the verified source')
        bindings = 0
        cpu_bound = []
        appearances = set()
        direct_appearances = set()
        inherited_appearances = set()
        proof_documents = {}
        for proof in entry['proofs']:
            if proof['path'].startswith('docs/platform/evidence/m5-cpu-visual-qa-20261126/'):
                raise ValueError('Original screening cannot substitute for final closure evidence')
            raw = reader.read(proof['path'], proof['sourceRevision'])
            if digest(raw) != proof['sha256']:
                raise ValueError('Proof bytes changed: '+proof['path'])
            document = json_document(raw, proof['path'])
            proof_documents[proof['path']] = document
            bound_model = proof.get('boundModelSha256', actual)
            if bound_model != actual and (not equivalent or bound_model != equivalent['priorModelSha256']):
                raise ValueError('Historical CPU model needs explicit, independently checked assembled equivalence')
            for binding in proof['modelHashBindings']:
                if pointer(document, binding) != bound_model:
                    raise ValueError('Proof no longer binds final model: '+binding)
            bindings += len(proof['modelHashBindings'])
            strings = [v for _, v in nodes(document) if isinstance(v, str)]
            has_cpu = any('cycles' in v.lower() and 'cpu' in v.lower() for v in strings)
            if proof['modelHashBindings'] and has_cpu:
                cpu_bound.append({'path': proof['path'], 'boundModelSha256': bound_model,
                                  'scope': 'direct' if bound_model == actual else 'assembled-equivalent only'})
            found = {v for p, v in nodes(document) if isinstance(v, str) and p.rsplit('/', 1)[-1] in ('appearance', 'preset') and v in ('original', 'gold', 'purple')}
            if sorted(found) != proof['cpuAppearances']:
                raise ValueError('Recorded appearance set changed: '+proof['path'])
            if proof['modelHashBindings'] and has_cpu:
                appearances.update(found)
                (direct_appearances if bound_model == actual else inherited_appearances).update(found)
        if not bindings or not cpu_bound:
            raise ValueError('No final-model-bound CPU proof; screening is not final acceptance')
        result['modelHashBindings'] = bindings
        result['boundCpuProofs'] = cpu_bound
        result['recordedCpuAppearances'] = sorted(appearances)
        result['directCpuAppearances'] = sorted(direct_appearances)
        result['inheritedAssembledCpuAppearances'] = sorted(inherited_appearances)
        result['cpuScope'] = ('assembled idle appearances inherited through independently checked nonvisual metadata/actual loader equivalence'
                              if inherited_appearances else 'three named appearances' if appearances == {'original', 'gold', 'purple'}
                              else 'final CPU comparison plus source/three-material loader checks; separate three-preset CPU renders not claimed')
        for image in entry['retainedImages']:
            image_model = image.get('boundModelSha256', actual)
            if image_model != actual and (not equivalent or image_model != equivalent['priorModelSha256']):
                raise ValueError('Historical review image has no assembled-equivalence chain')
            if digest(reader.read(image['path'], image['sourceRevision'])) != image['sha256']:
                raise ValueError('Retained review image changed: '+image['path'])
        result['verifiedRetainedImages'] = len(entry['retainedImages'])
        if not entry['retainedImages']:
            result['imageRetentionLimit'] = 'Final CPU view hashes/recipe retained in bound receipt; original PNG bytes are not retained in this contribution'
        historical_images = 0
        for stage in entry.get('historicalVisualStages', []):
            if not SHA.fullmatch(stage['modelSha256']):
                raise ValueError('Historical model identity must be explicit')
            for proof in stage['proofs']:
                raw = reader.read(proof['path'], proof['sourceRevision'])
                if digest(raw) != proof['sha256']:
                    raise ValueError('Historical proof bytes changed')
                old_document = json_document(raw, proof['path'])
                if any(pointer(old_document, p) != stage['modelSha256'] for p in proof['modelHashBindings']):
                    raise ValueError('Historical proof no longer binds its previous model')
            for historical in stage['retainedImages']:
                if digest(reader.read(historical['path'], historical['sourceRevision'])) != historical['sha256']:
                    raise ValueError('Historical review image changed')
                historical_images += 1
        if historical_images:
            result['verifiedHistoricalImages'] = historical_images
            result['historicalVisualScope'] = 'Preserved source history; never counted as final geometry acceptance'
        delta = entry.get('visibleGeometryDelta')
        if delta:
            if equivalent or expected['binarySha256'] == delta['previousBinarySha256']:
                raise ValueError('Real geometry delta cannot use a metadata-only equivalence exemption')
            if not any(s['modelSha256'] == delta['previousModelSha256'] and s['binarySha256'] == delta['previousBinarySha256']
                       for s in entry.get('historicalVisualStages', [])):
                raise ValueError('Geometry delta must identify its independently verified historical stage')
            if direct_appearances != {'original', 'gold', 'purple'}:
                raise ValueError('Geometry delta requires direct final CPU evidence in all three appearances')
            materials = set()
            images = {i['path']: i for i in entry['retainedImages']}
            for record in delta['finalCpuRecords']:
                document = proof_documents[record['proofPath']]
                render = pointer(document, record['renderReceiptPointer'])
                product_render = render['products'][0]
                if render.get('engine') != 'Cycles CPU' or product_render['sourceModelSha256'] != actual or product_render['id'] != identity:
                    raise ValueError('Geometry delta CPU record does not bind the final model')
                if pointer(document, record['imageHashPointer']) != images[record['imagePath']]['sha256']:
                    raise ValueError('Geometry delta CPU view does not bind retained review bytes')
                materials.add(product_render['appearance'])
            if materials != {'original', 'gold', 'purple'}:
                raise ValueError('Geometry delta lacks final model-bound CPU view records for three appearances')
            result['visibleGeometryDelta'] = {'previousModelSha256': delta['previousModelSha256'],
                'previousBinarySha256': delta['previousBinarySha256'], 'finalBinarySha256': expected['binarySha256'],
                'directFinalCpuViews': len(delta['finalCpuRecords'])}
        result['scopedClosure'] = entry['closureState'] == 'closed-scoped'
        if equivalent:
            document = proof_documents[equivalent['proofPath']]
            pairs = equivalent['semanticDigestPairs']
            if {p['preset'] for p in pairs} != {'original', 'gold', 'purple'} or len(pairs) != 3:
                raise ValueError('Assembly equivalence requires all three actual loader material digests')
            for pair in pairs:
                prior_path = pair.get('priorProofPath', equivalent['proofPath'])
                final_path = pair.get('finalProofPath', equivalent['proofPath'])
                for key, path, bound in [('priorProofPath', prior_path, equivalent['priorModelSha256']),
                                         ('finalProofPath', final_path, actual)]:
                    if key in pair:
                        source_proof = {p['path']: p for p in entry['proofs']}[path]
                        if not source_proof['modelHashBindings'] or source_proof.get('boundModelSha256', actual) != bound:
                            raise ValueError('Cross-proof visual signature lacks its exact old/final model binding')
                old_digest = pointer(proof_documents[prior_path], pair['priorPointer'])
                new_digest = pointer(proof_documents[final_path], pair['finalPointer'])
                if not isinstance(old_digest, str) or not SHA.fullmatch(old_digest) or new_digest != old_digest:
                    raise ValueError('Actual assembled mesh/UV/index/world/material semantic digest changed')
            result['assembledEquivalentTo'] = equivalent['priorModelSha256']
        assertions = entry.get('requiredEvidenceAssertions', [])
        for assertion in assertions:
            if pointer(proof_documents[assertion['proofPath']], assertion['pointer']) != assertion['expected']:
                raise ValueError('Required evidence assertion failed: '+assertion['kind'])
        if identity == 'm5stack-k151' and result['scopedClosure']:
            if not model['presentation'].get('inspectionGroups'):
                raise ValueError('K151 nested inspection is not declared')
            required = {'expanded-export', 'complete-inward-marker', 'exact-inspection-reset', 'servo-photo-attachment'}
            if not required.issubset({a['kind'] for a in assertions}):
                raise ValueError('K151 nested inspection needs expanded CPU, whole marker, reset and servo/photo evidence')
            if not any(p['scope'] == 'direct' for p in cpu_bound):
                raise ValueError('K151 needs a final-model-bound direct CPU inspection receipt')
        if not result['scopedClosure']:
            result['pending'] = entry['closureState']
        if reader.snapshot:
            file = reader.root/expected['path']
            observed = digest(file.read_bytes()) if file.is_file() else None
            result['sourceRootModelObservation'] = {'sha256': observed, 'matchesContributionBytes': observed == actual,
                'scope': 'Model-file byte observation only; not integrated artifact/catalog/proof validation'}
    except (ValueError, OSError, KeyError, IndexError, TypeError) as error:
        errors.append(str(error))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-root', type=Path, default=ROOT, help='Final repository tree, or git object database for --snapshot')
    parser.add_argument('--matrix', type=Path, default=DEFAULT_MATRIX)
    parser.add_argument('--snapshot', action='store_true', help='Verify contribution git snapshots; do not claim Root integration')
    parser.add_argument('--ids', help='Comma-separated subset; denominator is still validated against all36 entries')
    parser.add_argument('--output', type=Path, help='Write machine receipt without changing any model/catalog')
    args = parser.parse_args()
    matrix_bytes = args.matrix.read_bytes()
    matrix = json_document(matrix_bytes, 'closure matrix')
    reader = Reader(args.source_root, args.snapshot)
    screening = matrix['screening']
    source = reader.read(screening['path'], screening['sourceRevision'])
    if digest(source) != screening['sha256']:
        raise ValueError('Original screening receipt changed')
    original = json_document(source, screening['path'])
    candidates = {p['id'] for p in original['products'] if p['review'] == 'candidate-for-product-review'}
    entries = {e['id']: e for e in matrix['entries']}
    if len(candidates) != 35 or screening['candidateCount'] != 35 or screening['lateFollowups'] != ['m5stack-c008']:
        raise ValueError('Screening denominator must be35 plus the single C008 follow-up')
    if len(entries) != len(matrix['entries']) or set(entries) != candidates | {'m5stack-c008'}:
        raise ValueError('Missing, duplicate or extraneous closure entries')
    for identity, entry in entries.items():
        origin = 'screening-candidate' if identity in candidates else 'followup-candidate'
        if entry['origin'] != origin:
            raise ValueError('Incorrect screening/follow-up origin: '+identity)
    selected = args.ids.split(',') if args.ids else list(entries)
    if not selected or len(set(selected)) != len(selected) or set(selected)-set(entries):
        raise ValueError('Invalid selected SKU set')
    rows = [verify_entry(entries[identity], reader) for identity in selected]
    errors = [{'id': r['id'], 'detail': error} for r in rows for error in r['errors']]
    pending = [r['id'] for r in rows if not r['errors'] and not r.get('scopedClosure')]
    passed = not errors and not pending
    source_head = None
    if (reader.root/'.git').exists():
        source_head = reader.git('rev-parse', 'HEAD').decode().strip()
    receipt = {'schemaVersion': 1, 'checkedAt': datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ'),
        'mode': 'contribution-snapshot' if args.snapshot else 'integrated-working-tree',
        'sourceRootHead': source_head, 'contributionCommitObjectsVerified': args.snapshot,
        'scope': 'All36 candidate closures' if not args.ids else 'Selected subset; not an all36 final pass',
        'matrixSha256': digest(matrix_bytes), 'coverage': {'originalCandidates': 35, 'lateFollowups': 1, 'total': 36, 'uniqueModelPaths': len({e['model']['path'] for e in entries.values()}), 'checked': len(rows)},
        'evidenceVerified': not errors, 'scopedClosurePassed': passed, 'pending': pending, 'errors': errors,
        'dispositionCounts': dict(collections.Counter(r['disposition'] for r in rows)), 'products': rows,
        'limits': ['Hash/source-bound scoped visual closure, not complete manufacturing accuracy, hidden PCB reconstruction, browser GPU or firmware compatibility.',
            'Snapshot mode verifies contribution commits only; final Root must independently pass without --snapshot after the final model/inspection evidence is integrated.']}
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(receipt, ensure_ascii=False, indent=2)+'\n')
    print(json.dumps({k: v for k, v in receipt.items() if k != 'products'}, ensure_ascii=False, indent=2))
    return 0 if passed else 1


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (ValueError, OSError, KeyError, TypeError) as error:
        print('ERROR: '+str(error), file=sys.stderr)
        sys.exit(1)
