#!/usr/bin/env python3
"""Publishable, binary-complete incremental snapshots with sequential-apply proof.

The development checkout can contain several parallel lanes. A delivery tree
includes only the explicitly released product identities, their model assets,
and a source snapshot of their common implementation. No GitHub ref is pushed.
"""
import argparse
import datetime
import gzip
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
CATALOGS = {'web/platform/catalog/products.json', 'web/platform/catalog/firmwares.json',
            'web/platform/catalog/firmware-watchlist.json', 'web/platform/knowledge/index.json'}


def git(*args, env=None, data=None, text=True):
    return subprocess.check_output(['git', *args], cwd=ROOT, env=env, input=data, text=text)


def obj(ref, path):
    return json.loads(git('show', ref + ':' + path))


def sha(data):
    return hashlib.sha256(data).hexdigest()


def write_json(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + '\n')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--state', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--base', required=True)
    parser.add_argument('--source', default='HEAD')
    parser.add_argument('--selection', required=True, type=Path)
    parser.add_argument('--final', action='store_true')
    args = parser.parse_args()
    selection = json.loads(args.selection.read_text())
    base = git('rev-parse', args.base).strip()
    source = git('rev-parse', args.source).strip()
    state = json.loads(args.state.read_text()) if args.state.exists() else {
        'schemaVersion': 1, 'baseCommit': base, 'latestCommit': base, 'productIds': [], 'listings': [], 'batches': []}
    assert state['baseCommit'] == base
    assert len(selection['listings']) == 5 or args.final, 'Each normal batch contains five storefront model families.'
    assert not set(state['listings']) & set(selection['listings']), 'Do not release a listing twice.'
    allowed = set(state['productIds']) | set(selection['productIds'])
    all_listings = state['listings'] + selection['listings']
    parent = state['latestCommit']
    number = len(state['batches']) + 1
    products = obj(source, 'web/platform/catalog/products.json')
    products['items'] = [p for p in products['items'] if p.get('manufacturer') != 'Waveshare' or p['id'] in allowed]
    assert allowed <= {p['id'] for p in products['items']}
    selected_products = [p for p in products['items'] if p['id'] in allowed]
    source_paths = set(git('ls-tree', '-r', '--name-only', '-z', source).split('\0'))
    for product in selected_products:
        # The working-tree validator also sees generated, untracked knowledge.
        # A downloadable diff must contain each reference in its committed source.
        for relative in [product.get('knowledgeUrl'), (product.get('model') or {}).get('url')]:
            if relative:
                required = 'web/platform/' + relative.removeprefix('./')
                assert required in source_paths, (product['id'], 'uncommitted or missing release reference', required)
    all_product_ids = {p['id'] for p in products['items']}
    model_dirs = {p['model']['url'].split('/assets/')[1].split('/')[0]
                  for p in selected_products if p.get('model')}
    for p in selected_products:
        if p.get('visualVariantOf'):
            assert p['visualVariantOf'] in all_product_ids, p['id']
        if p.get('firmwareTargetId'):
            assert p['firmwareTargetId'] in all_product_ids, p['id']
    firmwares = obj(source, 'web/platform/catalog/firmwares.json')
    kept = []
    for entry in firmwares['items']:
        ids = entry.get('productIds', [])
        if ids and not set(ids) & all_product_ids:
            continue
        if ids:
            entry['productIds'] = [pid for pid in ids if pid in all_product_ids]
        kept.append(entry)
    firmwares['items'] = kept
    firmware_ids = {f['id'] for f in kept}
    for p in selected_products:
        if p.get('defaultFirmwareId'):
            assert p['defaultFirmwareId'] in firmware_ids, p['id']
    watch = obj(source, 'web/platform/catalog/firmware-watchlist.json')
    entries = []
    for entry in watch['entries']:
        ids = entry.get('compatibility', {}).get('productIds', [])
        if ids and not set(ids) & all_product_ids:
            continue
        if ids:
            entry['compatibility']['productIds'] = [pid for pid in ids if pid in all_product_ids]
        if entry.get('catalogTargets'):
            entry['catalogTargets'] = [t for t in entry['catalogTargets'] if t.get('firmwareId') in firmware_ids]
        entries.append(entry)
    watch['entries'] = entries
    knowledge = obj(source, 'web/platform/knowledge/index.json')
    knowledge['entries'] = [e for e in knowledge['entries'] if not e['catalogId'].startswith('waveshare-') or e['catalogId'] in allowed]
    overlays = {
        'web/platform/catalog/products.json': products,
        'web/platform/catalog/firmwares.json': firmwares,
        'web/platform/catalog/firmware-watchlist.json': watch,
        'web/platform/knowledge/index.json': knowledge}
    # Display families have many shared source receipts. These remain valid common
    # implementation evidence even when their sibling product is released later.
    chips = {p['chip'].removeprefix('esp32') or 'classic' for p in selected_products}

    def include(path):
        if path in CATALOGS:
            return False
        if path.startswith('web/platform/assets/waveshare-'):
            return path.split('/')[3] in model_dirs
        if path.startswith('web/platform/knowledge/waveshare-'):
            return Path(path).stem in allowed
        if path.startswith('tools/waveshare-batches/') and path.endswith('.json'):
            try:
                fragment = obj(source, path)
                return fragment.get('listingId') in all_listings
            except (subprocess.CalledProcessError, json.JSONDecodeError):
                return False
        if path in ['docs/platform/waveshare-progress-2026-11-23.md',
                    'docs/platform/research/waveshare-progress-2026-11-23.json']:
            return False
        batch_evidence = re.match(r'docs/platform/evidence/waveshare-batch(\d+)-', path)
        if batch_evidence:
            # Root acceptance evidence is named by delivery batch, not chip.
            # Carry earlier receipts forward without rewriting immutable patches.
            return int(batch_evidence.group(1)) <= number
        if path.startswith('docs/platform/evidence/waveshare-root-extras-'):
            # These shared receipts cover the classic display, thermal module,
            # and controller-containing Basic Kit; their directory is not named
            # after one chip. Release them when the first covered family ships.
            return bool(set(all_listings) & {
                'esp32-touch-lcd-3.5', 'thermal-45-camera-esp32-module',
                'esp32-xx-basic-kit'})
        if path.startswith(('docs/evidence/waveshare-', 'docs/research/waveshare-', 'docs/platform/evidence/waveshare-')):
            return any(('waveshare-' + chip) in path for chip in chips) or 'store-inventory' in path
        if path == 'tools/platform-waveshare-test.mjs':
            # This source contains exact seven-profile P4 assertions. Include when
            # its full P4 fixture selection is present, otherwise defer its file.
            return sum(p.get('defaultFirmwareId') == 'waveshare-p4-rev3-idf-hello' for p in selected_products) == 7
        return True

    args.output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='waveshare-index-') as directory:
        env = dict(os.environ, GIT_INDEX_FILE=str(Path(directory) / 'index'),
                   GIT_AUTHOR_NAME='Codex', GIT_AUTHOR_EMAIL='codex@users.noreply.github.com',
                   GIT_COMMITTER_NAME='Codex', GIT_COMMITTER_EMAIL='codex@users.noreply.github.com')
        git('read-tree', parent, env=env)
        changed = git('diff', '--name-only', '-z', parent, source).split('\0')
        source_entries = {}
        for row in git('ls-tree', '-r', '-z', source).split('\0'):
            if row:
                info, path = row.split('\t', 1)
                mode, kind, oid = info.split(' ')
                source_entries[path] = (mode, oid)
        updates = []
        for path in changed:
            if not path or not include(path):
                continue
            if path in source_entries:
                mode, oid = source_entries[path]
                updates.append(f'{mode} {oid}\t{path}\0')
            else:
                updates.append('0 ' + '0' * 40 + '\t' + path + '\0')
        for path, value in overlays.items():
            blob = (json.dumps(value, ensure_ascii=False, indent=2) + '\n').encode()
            oid = git('hash-object', '-w', '--stdin', data=blob, text=False).decode().strip()
            updates.append(f'100644 {oid}\t{path}\0')
        if updates:
            git('update-index', '-z', '--index-info', env=env, data=''.join(updates))
        tree = git('write-tree', env=env).strip()
        message = f'Waveshare incremental batch {number:02d}: ' + ', '.join(selection['listings']) + '\n'
        commit = git('commit-tree', tree, '-p', parent, env=env, data=message).strip()
        filename = f'waveshare-{number:03d}.diff'
        patch = git('diff', '--binary', '--full-index', '--no-ext-diff', parent, commit, text=False)
        assert patch, 'Empty batch is not a delivery.'
        patch_path = args.output / filename
        patch_path.write_bytes(patch)
        git('read-tree', parent, env=env)
        git('apply', '--cached', '--check', str(patch_path.resolve()), env=env)
        git('apply', '--cached', str(patch_path.resolve()), env=env)
        assert git('write-tree', env=env).strip() == tree, 'Sequential application must reconstruct the exact tree.'
        compressed = gzip.compress(patch, compresslevel=9, mtime=0)
        (args.output / (filename + '.gz')).write_bytes(compressed)
        batch = {'number': number, 'filename': filename, 'bytes': len(patch), 'sha256': sha(patch),
            'gzipFilename': filename + '.gz', 'gzipBytes': len(compressed), 'gzipSha256': sha(compressed),
            'baseCommit': parent, 'commit': commit, 'tree': tree, 'developmentSourceCommit': source,
            'listings': selection['listings'], 'productIds': selection['productIds'],
            'listingCount': len(selection['listings']), 'skuCount': len({p.get('sku', p['id']) for p in selected_products if p['id'] in selection['productIds']}),
            'profileCount': len(selection['productIds']),
            'cumulativeListingCount': len(all_listings), 'cumulativeSkuCount': len({p.get('sku', p['id']) for p in selected_products}),
            'cumulativeProfileCount': len(allowed),
            'sequentialApplyVerified': True,
            'createdAt': datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds'),
            'validation': selection.get('validation', []), 'limitations': selection.get('limitations', [])}
        git('update-ref', f'refs/heads/waveshare/delivery-{number:03d}', commit)
        state.update(latestCommit=commit, productIds=sorted(allowed), listings=all_listings,
                     skuCount=len({p.get('sku', p['id']) for p in selected_products}), profileCount=len(allowed))
        state['batches'].append(batch)
        write_json(args.state, state)
        write_json(args.output / f'waveshare-{number:03d}.json', batch)
        print(json.dumps(batch, ensure_ascii=False))


if __name__ == '__main__':
    main()
