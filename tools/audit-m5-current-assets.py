#!/usr/bin/env python3
"""Audit current M5 asset bytes, JSON references and raster decoding, without loaders."""
import argparse
from collections import Counter
from datetime import datetime, timezone
import gzip
import hashlib
import json
from pathlib import Path
import re
import subprocess

from PIL import Image, __version__ as pillow_version

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--source-root', type=Path, required=True)
p.add_argument('--out', type=Path, required=True)
a = p.parse_args()
root = a.source_root.resolve()
platform = root/'web/platform'
asset_root = platform/'assets'
out = a.out.resolve()
out.parent.mkdir(parents=True, exist_ok=True)
sha = lambda b: hashlib.sha256(b).hexdigest()
canonical = lambda obj: (json.dumps(obj, ensure_ascii=False, sort_keys=True, indent=2)+'\n').encode()
git = lambda *args: subprocess.check_output(['git', '-C', str(root), *args], text=True).strip()
source_start = git('rev-parse', 'HEAD')
files = {}
documents = {}
errors = []
warnings = []
json_checked = set()
decoded_gzip = {}


def relative(path):
    return path.relative_to(root).as_posix()


def read(path):
    path = path.resolve()
    if not path.is_relative_to(root):
        raise ValueError('Local asset escapes source root')
    if path not in files:
        raw = path.read_bytes()
        stat = path.stat()
        files[path] = {'path': relative(path), 'sha256': sha(raw), 'bytes': len(raw), '_stat': [stat.st_size, stat.st_mtime_ns]}
        return raw
    return path.read_bytes()


def unique_pairs(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('Duplicate JSON key: '+key)
        result[key] = value
    return result


def document(path):
    path = path.resolve()
    if path not in documents:
        raw = read(path)
        if path.suffix == '.gz':
            raw = gzip.decompress(raw)
        documents[path] = json.loads(raw, object_pairs_hook=unique_pairs)
        json_checked.add(relative(path))
    return documents[path]


def local(folder, name):
    if not isinstance(name, str) or '://' in name:
        raise ValueError('Invalid local asset name')
    path = (folder/name).resolve()
    if not path.is_relative_to(asset_root):
        raise ValueError('Local dependency escapes platform assets')
    return path


def file_row(path):
    if path.resolve() not in files:
        read(path)
    return {k: v for k, v in files[path.resolve()].items() if not k.startswith('_')}


def source_references(obj, declared, path=''):
    results = []
    if isinstance(obj, dict):
        for key, value in obj.items():
            pointer = path+'/'+key
            if key == 'sourceId' and isinstance(value, str):
                results.append({'pointer': pointer, 'sourceId': value, 'resolved': value in declared})
            elif key == 'sourceIds' and isinstance(value, list):
                for i, identity in enumerate(value):
                    results.append({'pointer': pointer+'/'+str(i), 'sourceId': identity, 'resolved': identity in declared})
            else:
                results.extend(source_references(value, declared, pointer))
    elif isinstance(obj, list):
        for i, value in enumerate(obj):
            results.extend(source_references(value, declared, path+'/'+str(i)))
    return results


def file_fields(obj, path=''):
    results = []
    if isinstance(obj, dict):
        for key, value in obj.items():
            pointer = path+'/'+key
            if key in ('file', 'sourceArtifact', 'binary') and isinstance(value, str):
                results.append((pointer, value))
            elif key == 'texture' and isinstance(value, str):
                results.append((pointer, value))
            else:
                results.extend(file_fields(value, pointer))
    elif isinstance(obj, list):
        for i, value in enumerate(obj):
            results.extend(file_fields(value, path+'/'+str(i)))
    return results


def gzip_row(path):
    path = path.resolve()
    if path not in decoded_gzip:
        digest = hashlib.sha256()
        size = 0
        with gzip.open(path, 'rb') as stream:
            while chunk := stream.read(1024*1024):
                digest.update(chunk)
                size += len(chunk)
        decoded_gzip[path] = {'path': relative(path), 'expandedSha256': digest.hexdigest(), 'expandedBytes': size}
    return decoded_gzip[path]


catalog_path = platform/'catalog/products.json'
catalog = document(catalog_path)
products = [product for product in catalog['items'] if product.get('manufacturer') == 'M5Stack']
assert len(products) == 141, 'The final M5 main-controller denominator must remain 141'
rows = []
directories = set()
artifact_rows = {}
for product in products:
    identity = product['id']
    loader = {'m5stack-papermono': 'dedicated-papermono', 'm5stack-stopwatch': 'dedicated-stopwatch'}.get(identity, 'manufacturer')
    url = product.get('model', {}).get('url')
    effective_url = url or './assets/'+identity+'/model.json'
    model_path = (platform/effective_url).resolve()
    folder = model_path.parent
    directories.add(folder)
    own_assets = asset_root/identity
    if own_assets.is_dir():
        directories.add(own_assets.resolve())
    row = {'id': identity, 'catalogModelUrl': url, 'effectiveModelPath': relative(model_path), 'loader': loader, 'issues': []}
    rows.append(row)
    try:
        model = document(model_path)
        row['modelSha256'] = files[model_path]['sha256']
        row['metadataId'] = model.get('id')
        plan_path = folder/'model-plan.json'
        plan = document(plan_path) if plan_path.exists() else None
        row['planSha256'] = files[plan_path]['sha256'] if plan else None
        row['planStatus'] = 'present' if plan else 'not-present-legacy-metadata-contract'
        if plan is None:
            warnings.append({'id': identity, 'kind': 'legacy-plan-absent', 'modelPath': relative(model_path)})
        manifest = model.get('artifacts')
        assert isinstance(manifest, dict) and manifest, 'Missing declared artifact manifest'
        verified = []
        for name, expected in sorted(manifest.items()):
            assert isinstance(expected, str) and re.fullmatch('[0-9a-f]{64}', expected), 'Invalid artifact digest'
            path = local(folder, name)
            info = file_row(path)
            assert info['sha256'] == expected, 'Artifact SHA-256 differs: '+name
            artifact_rows[info['path']] = info
            verified.append(info)
            if path.name.endswith('.json') or path.name.endswith('.json.gz'):
                document(path)
            if path.suffix == '.gz':
                gzip_row(path)
        row['artifactCount'] = len(verified)
        row['artifactBytes'] = sum(item['bytes'] for item in verified)
        row['artifactManifestSha256'] = sha(canonical(verified))
        dependencies = []
        for kind, metadata in [('model', model), ('plan', plan or {})]:
            for pointer, name in file_fields(metadata):
                if name.startswith(('https:', 'http:')):
                    continue
                # Gallery receipt filenames identify original downloads. They
                # are not dependencies promised by the production manifest.
                if kind == 'plan' and pointer.startswith('/sourceRecheck/galleryReceipts/'):
                    row.setdefault('externalGalleryReceiptFilenames', []).append({'pointer': pointer, 'file': name, 'scope': 'external-source-receipt-name; not a declared local asset'})
                    continue
                path = local(folder, name)
                if not path.exists():
                    raise ValueError('Declared local dependency missing: '+name+' at '+kind+pointer)
                info = file_row(path)
                dependencies.append({'document': kind, 'pointer': pointer, **info, 'manifestBound': name in manifest})
        row['localDependencies'] = dependencies
        binary_path = local(folder, model['binary'])
        binary = file_row(binary_path)
        assert model['binary'] in manifest, 'Geometry binary absent from artifact manifest'
        decoded = gzip_row(binary_path) if model.get('binaryCompression') == 'gzip' else {'expandedSha256': binary['sha256'], 'expandedBytes': binary['bytes']}
        row['geometry'] = {**binary, **decoded, 'compression': model.get('binaryCompression')}
        assert model.get('strideFloats') == 6
        assert decoded['expandedBytes'] % 24 == 0
        for part in model.get('parts', []):
            offset, count = part['byteOffset'], part['vertexCount']
            assert isinstance(offset, int) and isinstance(count, int) and offset >= 0 and offset % 4 == 0 and count >= 3 and count % 3 == 0
            assert offset+count*24 <= decoded['expandedBytes'], 'Part geometry range outside binary'
        source_hash = model.get('provenance', {}).get('sourceSha256')
        source_bindings = [v['path'] for v in verified if v['sha256'] == source_hash]
        expanded_bindings = [v['path'] for pth, v in decoded_gzip.items() if pth.parent == folder and v['expandedSha256'] == source_hash]
        decoded_hash = model.get('provenance', {}).get('decodedSha256')
        decoded_bindings = [v['path'] for pth, v in decoded_gzip.items() if pth.parent == folder and v['expandedSha256'] == decoded_hash]
        row['sourceArtifact'] = {'sourceSha256': source_hash, 'storedMatches': source_bindings, 'expandedMatches': expanded_bindings, 'decodedSourceSha256': decoded_hash, 'decodedSourceMatches': decoded_bindings, 'state': 'stored-exact' if source_bindings else 'gzip-expanded-exact' if expanded_bindings else 'decoded-source-exact-original-wrapper-not-retained' if decoded_bindings else 'photo-reconstruction-or-receipt-only'}
        if source_hash and not source_bindings and not expanded_bindings:
            warnings.append({'id': identity, 'kind': 'source-retention-scope', **row['sourceArtifact']})
        knowledge_path = (platform/product.get('knowledgeUrl', './knowledge/'+identity+'.json')).resolve()
        knowledge = document(knowledge_path) if knowledge_path.exists() else {}
        registries = {}
        def register_sources(metadata, path, mode, declared_via=None):
            sources = metadata.get('provenance', {}).get('sources', metadata.get('sources', []))
            for source in sources:
                if 'id' in source and source['id'] not in registries:
                    registries[source['id']] = {'resolution': mode, 'registryPath': relative(path), 'registrySha256': file_row(path)['sha256'], 'sourceRecord': {k: v for k, v in source.items() if k in ('id', 'url', 'resolvedUrl', 'sha256', 'bytes', 'kind', 'status', 'revision')}, 'declaredVia': declared_via}
        register_sources(model, model_path, 'resolved-in-model')
        if knowledge:
            register_sources(knowledge, knowledge_path, 'resolved-via-own-knowledge', {'path': relative(catalog_path), 'pointer': '/items/'+str(catalog['items'].index(product))+'/knowledgeUrl', 'value': product.get('knowledgeUrl')})
        # Only explicitly declared parent and immutable snapshot chains are
        # followed. A global same-name lookup would silently mix product sources.
        for pointer, note in [('/sourceTransform/note', (model.get('sourceTransform') or {}).get('note', '')), ('/cad/notes', ((plan or {}).get('cad') or {}).get('notes', ''))]:
            match = re.search(r'Manufacturer source is (m5stack-[a-z0-9-]+)\.', note)
            if match and match.group(1) != identity:
                parent_path = asset_root/match.group(1)/'model.json'
                parent = document(parent_path)
                register_sources(parent, parent_path, 'resolved-via-explicit-parent', {'document': 'model' if pointer.startswith('/sourceTransform') else 'plan', 'pointer': pointer, 'value': note})
        def register_snapshots(value, pointer=''):
            if isinstance(value, dict):
                for key, child in value.items():
                    here = pointer+'/'+key
                    if key == 'metadataSnapshot' and isinstance(child, dict) and isinstance(child.get('file'), str):
                        snapshot_path = local(folder, child['file'])
                        assert file_row(snapshot_path)['sha256'] == child['sha256'], 'Declared metadata snapshot SHA mismatch'
                        if child.get('uncompressedSha256'):
                            assert gzip_row(snapshot_path)['expandedSha256'] == child['uncompressedSha256'], 'Declared metadata snapshot expanded SHA mismatch'
                        register_sources(document(snapshot_path), snapshot_path, 'resolved-via-explicit-metadata-snapshot', {'document': 'plan', 'pointer': here, 'value': child})
                    else:
                        register_snapshots(child, here)
            elif isinstance(value, list):
                for i, child in enumerate(value):
                    register_snapshots(child, pointer+'/'+str(i))
        register_snapshots(plan or {})
        asset_refs = []
        for kind, metadata in [('model', model), ('plan', plan or {})]:
            for reference in source_references(metadata, registries):
                reference['document'] = kind
                if reference['resolved']:
                    reference.update({k: v for k, v in registries[reference['sourceId']].items() if k != 'sourceRecord'})
                asset_refs.append(reference)
        unresolved = [r for r in asset_refs if not r['resolved']]
        row['sourceRegistries'] = list(registries.values())
        row['assetSourceReferences'] = {'checked': len(asset_refs), 'unresolved': unresolved, 'resolutionCounts': dict(Counter(r.get('resolution', 'unresolved') for r in asset_refs)), 'references': asset_refs}
        if unresolved:
            warnings.append({'id': identity, 'kind': 'asset-source-reference', 'unresolved': unresolved})
        product_sources = set(registries) | {s['id'] for s in product.get('sources', []) if 'id' in s}
        product_refs = source_references(product, product_sources)
        row['catalogSourceReferences'] = {'checked': len(product_refs), 'unresolved': [r for r in product_refs if not r['resolved']]}
        if row['catalogSourceReferences']['unresolved']:
            warnings.append({'id': identity, 'kind': 'catalog-source-reference', 'unresolved': row['catalogSourceReferences']['unresolved']})
        unknown = []
        for part in model.get('derivedParts', []):
            if part.get('kind') == 'pcb-placeholder' or part.get('label') == 'UNKNOWN':
                unknown.append({'id': part.get('id'), 'kind': part.get('kind'), 'declaredLabel': part.get('label'), 'effectiveLoaderLabel': 'UNKNOWN' if part.get('kind') == 'pcb-placeholder' else part.get('label'), 'basis': part.get('basis'), 'labelSide': part.get('geometry', {}).get('labelSide', 'both')})
        row['unknownDeclarations'] = unknown
        row['status'] = 'declared-assets-verified'
    except Exception as error:
        row['status'] = 'failed'
        row['issues'].append(str(error))
        errors.append({'id': identity, 'kind': 'asset-contract', 'error': str(error)})

image_paths = sorted({path.resolve() for directory in directories for path in directory.rglob('*') if path.is_file() and path.suffix.lower() in {'.png', '.jpg', '.jpeg', '.webp', '.gif', '.bmp', '.tif', '.tiff', '.avif', '.ico'}})
image_rows = []
for path in image_paths:
    row = file_row(path)
    try:
        with Image.open(path) as image:
            image.verify()
        frame_count = 0
        with Image.open(path) as image:
            count = getattr(image, 'n_frames', 1)
            row.update(format=image.format, width=image.width, height=image.height, mode=image.mode, frames=count)
            for frame in range(count):
                image.seek(frame)
                image.load()
                frame_count += 1
            image.seek(0)
            if 'A' in image.getbands() or 'transparency' in image.info:
                alpha = image.convert('RGBA').getchannel('A').histogram()
                row['firstFrameAlpha'] = {'transparentPixels': alpha[0], 'nonzeroPixels': sum(alpha[1:]), 'opaquePixels': alpha[255]}
        row.update(decoded=True, decodedFrames=frame_count)
    except Exception as error:
        row.update(decoded=False, error=str(error))
        errors.append({'kind': 'image-decode', 'path': row['path'], 'error': str(error)})
    image_rows.append(row)

# Check other retained metadata too, including assets not listed in manifests.
for path in sorted({path.resolve() for directory in directories for path in directory.rglob('*.json')}):
    try:
        document(path)
    except Exception as error:
        errors.append({'kind': 'json-metadata', 'path': relative(path), 'error': str(error)})

changed_files = []
for path, info in files.items():
    stat = path.stat()
    if [stat.st_size, stat.st_mtime_ns] != info['_stat']:
        changed_files.append(relative(path))
if changed_files:
    errors.append({'kind': 'source-mutated-during-audit', 'paths': changed_files})


def shards(label, values, max_rows):
    result = []
    pending = [values[i:i+max_rows] for i in range(0, len(values), max_rows)]
    while pending:
        chunk = pending.pop(0)
        raw = canonical(chunk)
        packed = gzip.compress(raw, compresslevel=9, mtime=0)
        if len(packed) >= 50_000:
            assert len(chunk) > 1, 'Single record exceeds the compressed evidence limit'
            midpoint = len(chunk)//2
            pending[0:0] = [chunk[:midpoint], chunk[midpoint:]]
            continue
        name = f'{out.stem}-{label}-{len(result)+1:03}.json.gz'
        (out.parent/name).write_bytes(packed)
        result.append({'file': name, 'rows': len(chunk), 'sha256': sha(packed), 'bytes': len(packed), 'rawSha256': sha(raw), 'rawBytes': len(raw)})
    return result


artifact_values = sorted(artifact_rows.values(), key=lambda row: row['path'])
details = {'models': shards('models', rows, 15), 'artifacts': shards('artifacts', artifact_values, 150), 'images': shards('images', image_rows, 150), 'warnings': shards('warnings', warnings, 100)}
summary = {'schemaVersion': 1, 'checkedAt': datetime.now(timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ'), 'sourceStartCommit': source_start, 'sourceEndCommit': git('rev-parse', 'HEAD'), 'catalogSha256': files[catalog_path.resolve()]['sha256'], 'harnessSha256': sha(Path(__file__).read_bytes()), 'pillowVersion': pillow_version, 'counts': {'products': len(rows), 'modelPaths': len({r['effectiveModelPath'] for r in rows}), 'manufacturer': sum(r['loader'] == 'manufacturer' for r in rows), 'dedicated': sum(r['loader'] != 'manufacturer' for r in rows), 'passedAssetContracts': sum(r['status'] == 'declared-assets-verified' for r in rows), 'artifactDeclarations': sum(r.get('artifactCount', 0) for r in rows), 'uniqueArtifactPaths': len(artifact_values), 'uniqueArtifactBytes': sum(r['bytes'] for r in artifact_values), 'plansPresent': sum(r.get('planSha256') is not None for r in rows), 'legacyPlansAbsent': sum(r.get('planSha256') is None for r in rows), 'jsonDocumentsCheckedForDuplicateKeys': len(json_checked), 'gzipArtifactsExpandedAndHashed': len(decoded_gzip), 'imageDirectories': len(directories), 'images': len(image_rows), 'decodedImages': sum(r['decoded'] for r in image_rows), 'decodedFrames': sum(r.get('decodedFrames', 0) for r in image_rows), 'unknownDeclarations': sum(len(r.get('unknownDeclarations', [])) for r in rows), 'productsWithUnknown': sum(bool(r.get('unknownDeclarations')) for r in rows), 'unresolvedAssetSourceReferences': sum(len(r.get('assetSourceReferences', {}).get('unresolved', [])) for r in rows), 'unresolvedCatalogSourceReferences': sum(len(r.get('catalogSourceReferences', {}).get('unresolved', [])) for r in rows), 'warnings': len(warnings), 'errors': len(errors), 'sourceReferenceResolutionCounts': dict(sum((Counter(r.get('assetSourceReferences', {}).get('resolutionCounts', {})) for r in rows), Counter()))}, 'actualArtifactManifestSha256': sha(canonical(artifact_values)), 'actualImageManifestSha256': sha(canonical(image_rows)), 'actualModelDetailsSha256': sha(canonical(rows)), 'legacyPlansAbsent': [r['id'] for r in rows if r.get('planSha256') is None], 'modelSha256ByProduct': {r['id']: r.get('modelSha256') for r in rows}, 'shardEncoding': 'UTF-8 sorted-key JSON with indent=2 and final LF; gzip level9 mtime=0; each compressed file <50000 bytes; raw bytes and SHA-256 allow lossless recovery', 'details': details, 'errors': errors, 'sourceFilesStableDuringAudit': not changed_files, 'passed': not errors, 'scope': '141 current catalog controllers, including the two explicitly identified dedicated asset directories. Exact manifest file existence/SHA-256, local texture/reference/source-file dependencies, decoded gzip hashes, binary byte ranges, duplicate JSON keys, source-reference reporting, static UNKNOWN declarations and Pillow full decode of every retained raster frame. No Three loader instantiation, rendering, GPU, visual alignment, firmware or physical acceptance. Source receipt archiveName fields do not promise downloaded HTML/PDF copies. Unresolved source IDs are reported separately and are not silently promoted to verified evidence. Historical image reports are untouched.'}
raw = canonical(summary)
assert len(raw) < 50_000, 'Main evidence summary must remain below 50 KB'
out.write_bytes(raw)
print(json.dumps({'passed': summary['passed'], 'sourceStartCommit': source_start, 'sourceEndCommit': summary['sourceEndCommit'], 'counts': summary['counts']}))
raise SystemExit(0 if summary['passed'] else 1)
