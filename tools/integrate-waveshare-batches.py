#!/usr/bin/env python3
"""Merge reviewed per-listing fragments without changing unrelated products.

Fragments retain exact SKU identities and explicit firmware relationships. This
does not infer adaptation from a shared board or visual model. The generated
Rust memory table and browser capability table use the same reviewed values.
"""
import argparse
import datetime
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path):
    return json.loads(path.read_text())


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')


def upsert(rows, additions):
    positions = {row['id']: i for i, row in enumerate(rows)}
    assert len(positions) == len(rows), 'Duplicate existing identities'
    for row in additions:
        if row['id'] in positions:
            rows[positions[row['id']]] = row
        else:
            positions[row['id']] = len(rows)
            rows.append(row)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('fragments', nargs='+', type=Path)
    parser.add_argument('--selection', type=Path)
    args = parser.parse_args()
    fragments = [read(path) for path in args.fragments]
    listings = [fragment['listingId'] for fragment in fragments]
    assert len(listings) == len(set(listings)), 'A listing may occur only once'
    products = read(ROOT / 'web/platform/catalog/products.json')
    firmwares = read(ROOT / 'web/platform/catalog/firmwares.json')
    watch = read(ROOT / 'web/platform/catalog/firmware-watchlist.json')
    index = read(ROOT / 'web/platform/knowledge/index.json')
    # An explicitly shared upstream example may target several existing exact
    # families. Preserve those reviewed relationships instead of shrinking the
    # firmware record each time one family is integrated.
    known_product_ids = {p['id'] for p in products['items']} | {
        p['id'] for fragment in fragments for p in fragment['products']}
    capability_path = ROOT / 'web/platform/waveshare-board-families.js'
    header, body = capability_path.read_text().split('Object.create(null),', 1)
    capabilities = json.loads(body.rsplit('));', 1)[0])
    added_ids = []
    for fragment in fragments:
        rows = fragment['products']
        ids = {p['id'] for p in rows}
        assert len(ids) == len(rows) and not ids.intersection(added_ids)
        # Some reviewed lanes keep memory only on each exact SKU and call the
        # virtual wafer choice siliconRevision. Normalize without guessing it.
        for board, cap in fragment.get('boardCapabilities', {}).items():
            memories = {(p['runtime']['flashMb'], p['runtime']['psramMb'])
                        for p in rows if p['runtime'].get('board') == board}
            assert len(memories) == 1, (board, memories)
            flash, psram = memories.pop()
            assert cap.get('flashMb', flash) == flash and cap.get('psramMb', psram) == psram
            cap.update(flashMb=flash, psramMb=psram)
            if 'siliconRevision' in cap:
                revision = cap.pop('siliconRevision')
                assert cap.get('virtualRevision', revision) == revision
                cap['virtualRevision'] = revision
        for p in rows:
            assert p['manufacturer'] == 'Waveshare'
            assert p.get('sku') and p.get('knowledgeUrl')
            board = p['runtime'].get('board')
            if board:
                cap = fragment['boardCapabilities'].get(board, capabilities.get(board))
                assert cap and cap['chip'] == p['chip'], (p['id'], board)
                assert all(cap[k] == p['runtime'][k] for k in ['flashMb', 'psramMb']), p['id']
            if p.get('model'):
                model = ROOT / 'web/platform' / p['model']['url'].removeprefix('./')
                assert model.is_file(), model
        knowledge = []
        for entry in fragment.get('knowledge', []):
            if 'path' in entry:
                k = read(ROOT / entry['path'])
            else:
                k = entry
                write(ROOT / 'web/platform/knowledge' / (k['id'] + '.json'), k)
            assert k['catalogId'] in ids
            knowledge.append({'id': k['id'], 'kind': 'product', 'catalogId': k['catalogId'],
                              'url': './' + k['id'] + '.json', 'updatedAt': k['updatedAt']})
        assert ids == {k['catalogId'] for k in knowledge}, 'Each SKU/revision needs its knowledge record'
        for f in fragment.get('firmwares', []):
            assert set(f.get('productIds', [])) <= known_product_ids
            assert not f.get('simulationVerified'), 'Verified firmware needs a separate final acceptance review'
        upsert(products['items'], rows)
        upsert(firmwares['items'], fragment.get('firmwares', []))
        for entry in fragment.get('watchlist', []):
            compatibility = entry['compatibility']
            if compatibility.get('status') in ('partial', 'unverified'):
                assert not compatibility.get('simulationVerified') and not entry.get('catalogTargets')
                # This is the watcher's automatic-update approval boundary,
                # independent of an original image's selectable trial status.
                compatibility['status'] = 'reference-only'
        upsert(watch['entries'], fragment.get('watchlist', []))
        upsert(index['entries'], knowledge)
        capabilities.update(fragment.get('boardCapabilities', {}))
        added_ids.extend(p['id'] for p in rows)
    now = datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00', 'Z')
    index['updatedAt'] = now
    for path, value in [('catalog/products.json', products), ('catalog/firmwares.json', firmwares),
                        ('catalog/firmware-watchlist.json', watch), ('knowledge/index.json', index)]:
        write(ROOT / 'web/platform' / path, value)
    capability_path.write_text(header + 'Object.create(null),' + json.dumps(capabilities, ensure_ascii=False, indent=2) + '));\n')
    rust = ['//! Generated by tools/integrate-waveshare-batches.py from reviewed exact board capabilities.',
            '//! No aliases, hardware-version inference, or full-device compatibility claim.',
            'pub fn memory_mib(board: &str) -> Option<(usize, usize)> {', '    match board {']
    for board, cap in sorted(capabilities.items()):
        assert board.startswith('waveshare-') and all(c.isalnum() or c in '-.' for c in board)
        rust.append(f'        "{board}" => Some(({int(cap["flashMb"])}, {int(cap["psramMb"])})),')
    rust += ['        _ => None,', '    }', '}', '',
             '/// Explicit virtual wafer family; never inferred from a SKU name or image.',
             'pub fn virtual_revision(board: &str) -> Option<u16> {', '    match board {']
    for board, cap in sorted(capabilities.items()):
        if 'virtualRevision' in cap:
            assert cap['chip'] == 'esp32p4' and cap['virtualRevision'] in (100, 301)
            rust.append(f'        "{board}" => Some({cap["virtualRevision"]}),')
    rust += ['        _ => None,', '    }', '}', '']
    (ROOT / 'esp-soc/src/waveshare_profiles.rs').write_text('\n'.join(rust))
    if args.selection:
        write(args.selection, {'listings': listings, 'productIds': added_ids,
                              'validation': [], 'limitations': []})
    print(json.dumps({'listings': listings, 'profiles': len(added_ids),
                      'skus': len({p['sku'] for f in fragments for p in f['products']})}, ensure_ascii=False))


if __name__ == '__main__':
    main()
