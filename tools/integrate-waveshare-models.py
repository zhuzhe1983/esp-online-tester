#!/usr/bin/env python3
"""Connect reviewed model integration records and verify their retained sources."""
import datetime
import gzip
import hashlib
import json
from pathlib import Path
import sys

root = Path(__file__).resolve().parents[1]
base = root / 'web/platform'
now = datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00', 'Z')
read = lambda path: json.loads(path.read_text())
def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')

def current_model_note(note):
    return note.replace('3D 模型尚未完成，当前屏幕预览仅用于调试；', '3D 已根据可用官方资料建立；').replace(
        '目前使用屏幕调试视图，独立 3D 模型与实物朝向校准尚未完成；',
        '独立 3D 模型已接入；真实 IMU 板坐标与实物朝向标定仍未完成；')

catalog = read(base / 'catalog/products.json')
products = {p['id']: p for p in catalog['items']}
updated = set()
for argument in sys.argv[1:]:
    for record in read(root / argument)['models']:
        pid = record['productId']
        product = products[pid]
        directory = base / 'assets' / pid
        model = read(directory / 'model.json')
        path = base / 'knowledge' / (pid + '.json')
        knowledge = read(path)
        knowledge['sources'] = [s for s in knowledge['sources'] if not s['id'].startswith('model-')]
        knowledge['assets'] = [s for s in knowledge['assets'] if not s['id'].startswith('model-')]
        refs = []
        for source in model['provenance']['sources']:
            sid = 'model-' + source['id']
            refs.append(sid)
            body = (directory / source['file']).read_bytes()
            if source.get('retained', {}).get('encoding') == 'gzip':
                body = gzip.decompress(body)
            assert len(body) == source['bytes'] and hashlib.sha256(body).hexdigest() == source['sha256'], (pid, sid)
            timestamp = source.get('fetchedAt') or source.get('fetchedAtUTC') or source.get('containerFetchedAt') or source.get('capturedAt') or source.get('capturedAtUTC') or source.get('observedAt')
            if not timestamp and source.get('containerSourceId'):
                container = next(s for s in model['provenance']['sources'] if s['id'] == source['containerSourceId'])
                assert source['archive']['containerSha256'] == container['sha256'], (pid, sid, 'archive/container mismatch')
                timestamp = container.get('fetchedAt') or container.get('fetchedAtUTC')

            assert timestamp, (pid, sid, 'source retrieval/capture timestamp required')
            fetched = datetime.datetime.fromisoformat(timestamp.replace('Z', '+00:00')).astimezone(datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00', 'Z')
            notes = 'Original source retained in assets/' + pid + '/' + source['file'] + '. ' + source.get('rights', '')
            if source.get('origin') in ('zip-member','rar-member'):
                member = source.get('member') or source['archive'].get('entry')
                assert member, (pid, sid, 'ZIP member path required')
                notes += ' Hash and size identify extracted member ' + member + ' in '+source.get('origin','archive-member').split('-')[0].upper()+' SHA-256 ' + source['archive']['containerSha256'] + ', not a separate HTTP response.'
            elif source.get('captureKind') == 'github-connector-json-content':
                notes += ' Hash and size identify retained connector JSON content, not raw HTTP response bytes. fetchedAt records the connector capture time. ' + source.get('retrievalTimeNote', source.get('captureTimeNote', ''))
            elif source.get('captureKind') == 'derived-github-connector-field-selection':
                notes += ' Hash and size identify the retained selection of connector metadata fields, not the complete connector or HTTP response. fetchedAt records the observation time. ' + source.get('scope', '')
            else:
                notes += ' Hash and size identify decompressed original response bytes.'
            knowledge['sources'].append({'id': sid, 'title': source.get('title', source['id']), 'url': source['url'], 'kind': 'manufacturer-cad' if source['role'] == 'cad-source' else 'official-visual-reference', 'status': 'fetched', 'resolvedUrl': source.get('finalUrl', source['url']), 'fetchedAt': fetched, 'bytes': source['bytes'], 'sha256': source['sha256'], 'archiveName': source['file'].removesuffix('.gz'), 'notes': notes})
            knowledge['assets'].append({'id': sid, 'sourceId': sid, 'role': source['role'], 'url': source['url'], 'localUrl': './assets/' + pid + '/' + source['file']})
        cad = record['model']['provenance'] == 'official-cad'
        fact = {'id': 'model-dimensions', 'label': '3D 模型尺寸与适用版本', 'value': record['dimensions'], 'basis': 'cad-measurement' if cad else 'image-measurement', 'sourceIds': refs, 'method': 'Retained original CAD/dimensioned source measurements and registered photographs; unknown values and visual estimates are explicit.', 'notes': ' '.join(record['limitations'])}
        if not cad:
            fact['uncertainty'] = 'Known outlines retain manufacturer dimensions. Unknown thickness, package heights and debug layout estimates are not manufacturing measurements.'
        knowledge['facts'] = [f for f in knowledge['facts'] if f['id'] != 'model-dimensions'] + [fact]
        knowledge['updatedAt'] = now
        note = '3D 来源：' + ('官方 CAD 机械参考。' if cad else '官方尺寸与照片重建。') + ' 生产加载器几何与输入射线已检查；浏览器 WebGL 和实物装配尚未验收。'
        knowledge['collectionNotes'] = [current_model_note(n) for n in knowledge['collectionNotes'] if not n.startswith('3D 来源：')] + [note]
        product['model'] = record['model']
        product['limitations'] = [current_model_note(n) for n in product['limitations']]
        save(path, knowledge)
        updated.add(pid)
save(base / 'catalog/products.json', catalog)
index = read(base / 'knowledge/index.json')
index['updatedAt'] = now
for entry in index['entries']:
    if entry['id'] in updated:
        entry['updatedAt'] = now
save(base / 'knowledge/index.json', index)
print('Integrated models with verified source hashes:', ', '.join(sorted(updated)))
