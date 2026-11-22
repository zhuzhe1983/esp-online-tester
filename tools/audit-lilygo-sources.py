#!/usr/bin/env python3
"""Build a source-backed, newest-first LILYGO adaptation research inventory.

Input: the existing public product catalog, downloaded official Shopify product
JSON and hash-pinned official documentation Markdown in --cache. This command
does not mutate the product catalog, invent specifications, or fetch firmware.
Use --fetch to refresh public metadata/documentation; binaries are never fetched.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import hashlib
import html
import json
from pathlib import Path
import re
from urllib.parse import quote, urljoin, urlsplit
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parents[1]
REPO = 'Xinyuan-LilyGO/documentation'
ALIAS = {
    't-deck-plus-1': 't-deck-plus', 't-display-s3-amoled-1-64': 't-display-s3-amoled-1.64',
    'ts-s3-epaper': 't3-s3-e-paper', 't3-s3-v1-3': 't3-s3-v1.3',
    't3s3-v1-0': 't3-s3', 't3-tcxo': 't3-txco',
    't-dongle-esp32-s2-1-14-inch-lcd-board': 't-dongle',
    't-eth-elite-1': 't-eth-elite', 't-encoder-esp32': 't-encoder',
    't-mini-epaper-s3': 't-mini-e-paper-s3', 't-pico': 't-pico-2350',
    't7-v1-3-mini-32-esp32': 't7', 't8-c3': 't8',
    't5-4-7-inch-e-paper-v2-3': 't5-e-paper',
    't5-2-13inch-e-paper': 't5-epaper-2.13inch', 't5s-2-7inch-e-paper': 't5-epaper-2.7inch',
    't-sim-7670g-s3': 't-sim7670g-s3', 't-sim7080-s3': 't-sim7080x-s3',
    't-sim7000g': 't-sim7000', 't-sim7600e': 'sim7600e', 't-sim7600': 't-sim7600x',
    'a-t-pcie': 't-pcie', 't-sim-a7670e': 't-a7670',
    't-eth-lite': 't-eth-lite-s3',
}


def read(path):
    return json.loads(path.read_text())


def write(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + '\n')


def stamp():
    return datetime.now(timezone.utc).isoformat().replace('+00:00', 'Z')


def fetch(url, path):
    request = Request(url, headers={'User-Agent': 'esp-online-tester-public-metadata-audit'})
    with urlopen(request, timeout=35) as response:
        body = response.read(8 * 1024 * 1024 + 1)
        if len(body) > 8 * 1024 * 1024:
            raise ValueError('Metadata response exceeds 8 MiB')
        record = {'url': url, 'resolvedUrl': response.url, 'fetchedAt': stamp(),
                  'mediaType': response.headers.get_content_type(), 'bytes': len(body),
                  'sha256': hashlib.sha256(body).hexdigest()}
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(body)
    write(path.with_suffix('.source.json'), record)
    return body, record


def clean(text):
    text = re.sub(r'<[^>]*>', ' ', text)
    text = re.sub(r'!\[[^]]*\]\([^)]*\)', '', text)
    text = re.sub(r'\[([^]]*)\]\([^)]*\)', r'\1', text)
    return ' '.join(html.unescape(text).replace('**', '').replace('`', '').split())


def md_rows(text):
    rows = []
    section = ''
    for line in text.splitlines():
        if line.startswith('#'):
            section = clean(line.lstrip('#'))
        if not line.startswith('|'):
            continue
        values = [clean(v) for v in line.strip('|').split('|')]
        if len(values) < 2 or re.fullmatch(r'[-: ]+', values[0] or '-'):
            continue
        rows.append({'section': section, 'values': values})
    return rows


def product_facts(body):
    # HTML tables only: prose can describe an accessory or an alternative screen.
    rows = []
    for raw in re.findall(r'<tr\b[^>]*>(.*?)</tr>', body, re.S | re.I):
        cells = [clean(x) for x in re.findall(r'<t[dh]\b[^>]*>(.*?)</t[dh]>', raw, re.S | re.I)]
        if len(cells) >= 2 and cells[0] and any(cells[1:]):
            rows.append({'label': cells[0], 'value': ' | '.join(cells[1:])})
    return rows


def links(text, base):
    found = re.findall(r'(?:href|src)=[\"\']([^\"\']+)', text, re.I)
    found += re.findall(r'\bsrc:\s*[\"\']([^\"\']+)', text)
    found += re.findall(r'!?\[[^]]*\]\(([^)\s]+)(?:\s+[^)]*)?\)', text)
    return sorted({urljoin(base, html.unescape(u)) for u in found if u and not u.startswith(('#', 'mailto:', 'javascript:'))})


def release_history(rows):
    history = []
    for row in rows:
        if not re.search(r'version history|revision history', row['section'], re.I):
            continue
        values = row['values']
        if len(values) < 2:
            continue
        if not re.fullmatch(r'(?:19|20)\d{2}[-/]\d{1,2}(?:[-/]\d{1,2})?', values[1]):
            continue
        date = values[1].replace('/', '-')
        history.append({'hardwareRevision': values[0], 'date': date,
                        'precision': 'day' if len(date.split('-')) == 3 else 'month',
                        'description': ' | '.join(values[2:])})
    return history


def normalized_specifications(rows):
    """Expose candidates, preserving conflicting revisions and rotations."""
    resolutions, diagonals, dimensions, flash, psram = {}, {}, [], {}, {}
    for row in rows:
        label, value, sid = row['label'], row['value'], row['sourceId']
        if re.search(r'display|screen|resolution|lcd|amoled|oled|e.paper', label, re.I):
            for a, b in re.findall(r'(?<![\d.])(\d{2,4})\s*(?:\([HV]\))?\s*(?:RGB\s*)?[xX×*]\s*(?:RGB\s*[xX×*]?\s*)?(\d{2,4})(?!\d)', value):
                width, height = int(a), int(b)
                if width < 64 or height < 64:
                    continue
                key = (width, height)
                item = resolutions.setdefault(key, {'width': width, 'height': height, 'unit': 'px', 'sourceIds': [], 'labels': []})
                if sid not in item['sourceIds']:
                    item['sourceIds'].append(sid)
                if label not in item['labels']:
                    item['labels'].append(label)
            for n in re.findall(r'(?<!\d)(\d+(?:\.\d+)?)\s*(?:-?inch(?:es)?|[\"″])', value, re.I):
                number = float(n)
                if 0.1 <= number <= 30:
                    diagonals.setdefault(number, set()).add(sid)
        if re.search(r'dimension|\bsize\b', label, re.I) and re.search(r'\bmm\b', value, re.I):
            for group in re.findall(r'(\d+(?:\.\d+)?\s*[xX×*]\s*\d+(?:\.\d+)?(?:\s*[xX×*]\s*\d+(?:\.\d+)?)?)\s*mm', value, re.I):
                dimensions.append({'values': [float(x) for x in re.findall(r'\d+(?:\.\d+)?', group)], 'unit': 'mm',
                                   'sourceId': sid, 'label': label, 'scope': 'as labeled; do not infer axis order or thickness'})
        for role, destination in [('flash', flash), (r'ps\s*ram', psram)]:
            if re.fullmatch(r'.*' + role + r'.*', label, re.I) and not re.search(r'frequency|mode|clock|pin|interface', label, re.I):
                for n in re.findall(r'(?<!\d)(\d+)\s*M(?:B|iB)\b', value):
                    destination.setdefault(int(n), set()).add(sid)
            elif re.search(r'memory|onboard', label, re.I):
                for n in re.findall(role + r'\s*[:：]?\s*(\d+)\s*M(?:B|iB)\b', value, re.I):
                    destination.setdefault(int(n), set()).add(sid)
                for n in re.findall(r'(\d+)\s*M(?:B|iB)\s*' + role, value, re.I):
                    destination.setdefault(int(n), set()).add(sid)
    values = list(resolutions.values())
    return {'resolutionCandidates': values,
            'singleResolution': {k: values[0][k] for k in ('width', 'height')} if len(values) == 1 else None,
            'resolutionStatus': 'single-declared' if len(values) == 1 else 'ambiguous-or-rotated' if values else 'not-found-in-tables',
            'diagonalCandidates': [{'value': n, 'unit': 'inch', 'sourceIds': sorted(sids)} for n, sids in diagonals.items()],
            'bodyDimensionCandidates': dimensions,
            'flashCandidates': [{'value': n, 'unit': 'MiB', 'sourceIds': sorted(sids)} for n, sids in flash.items()],
            'psramCandidates': [{'value': n, 'unit': 'MiB', 'sourceIds': sorted(sids)} for n, sids in psram.items()],
            'note': 'Candidates extracted only from labeled official specification tables. Multiple variants/controllers remain unresolved; missing display evidence does not mean no display. MiB normalizes manufacturer MB capacity notation, not a measured memory test.'}


def build(cache, output):
    catalog = read(ROOT / 'web/platform/catalog/products.json')
    products = [p for p in catalog['items'] if p['id'].startswith('lilygo-')]
    sources, shops, wiki = {}, {}, {}
    for file in sorted(cache.glob('shopify-*.raw')):
        source_id = 'official-store-' + file.stem
        source = read(file.with_suffix('.source.json'))
        assert source['bytes'] == len(file.read_bytes()) and source['sha256'] == hashlib.sha256(file.read_bytes()).hexdigest(), f'Cached source changed: {file.name}'
        sources[source_id] = {'id': source_id, 'status': 'fetched', **source}
        for item in read(file)['products']:
            shops[item['handle']] = (item, source_id)
    for file in sorted(cache.glob('t*.raw')):
        payload = read(file)
        if 'product' not in payload:
            continue
        source_id = 'official-store-' + file.stem
        source = read(file.with_suffix('.source.json'))
        assert source['bytes'] == len(file.read_bytes()) and source['sha256'] == hashlib.sha256(file.read_bytes()).hexdigest(), f'Cached source changed: {file.name}'
        sources[source_id] = {'id': source_id, 'status': 'fetched', **source}
        shops[payload['product']['handle']] = (payload['product'], source_id)
    for file in sorted((cache / 'wiki').glob('*.md')):
        source = read(file.with_suffix('.source.json'))
        body_bytes = file.read_bytes()
        assert source['bytes'] == len(body_bytes) and source['sha256'] == hashlib.sha256(body_bytes).hexdigest(), f'Cached source changed: {file.name}'
        if source.get('gitBlobSha'):
            assert source['gitBlobSha'] == hashlib.sha1(b'blob ' + str(len(body_bytes)).encode() + b'\0' + body_bytes).hexdigest(), f'Git blob mismatch: {file.name}'
        path = source.get('path', file.name.replace('__', '/'))
        if len(path.split('/')) < 5:
            continue
        slug = path.split('/')[-2]
        source_id = 'official-wiki-' + slug
        sources[source_id] = {'id': source_id, 'status': 'fetched', **source}
        wiki[slug] = (file.read_text(), source_id, path)
    rows = []
    for product in products:
        product_id = product['id']
        handle = product['providers']['lilygo-spark']['productId']
        entry = {'id': product_id, 'name': product['name'], 'handle': handle,
                 'wasRuntimeUnavailable': product['runtime']['status'] == 'unavailable',
                 'productUrl': 'https://lilygo.cc/products/' + handle, 'sourceIds': [],
                 'storefront': None, 'chronology': None, 'documentedVersions': [],
                 'specifications': [], 'officialRepositories': [], 'sourceLinks': [],
                 'photos': [], 'cadCandidates': [], 'factoryFirmwareInvestigation': {},
                 'conflicts': [], 'unknowns': []}
        docs, body = '', ''
        shop, shop_source = shops.get(handle, (None, None))
        if shop:
            entry['sourceIds'].append(shop_source)
            entry['storefront'] = {'createdAt': shop['created_at'], 'publishedAt': shop['published_at'],
                                   'sourceId': shop_source, 'title': shop['title'],
                                   'options': shop.get('options', []),
                                   'variantTitles': [v['title'] for v in shop.get('variants', [])]}
            body = shop.get('body_html', '')
            entry['specifications'] += [{'sourceId': shop_source, 'basis': 'official-store-table', **r} for r in product_facts(body)]
            entry['photos'] = [{'url': im['src'], 'width': im.get('width'), 'height': im.get('height'),
                                'status': 'linked', 'sourceId': shop_source} for im in shop.get('images', [])]
        else:
            entry['unknowns'].append('Exact product handle absent from fetched storefront inventory; no sibling listing substituted.')
        slug = ALIAS.get(handle, handle)
        doc, doc_source, doc_path = wiki.get(slug, ('', None, None))
        if doc:
            docs = doc
            entry['sourceIds'].append(doc_source)
            entry['documentationUrl'] = 'https://wiki.lilygo.cc/' + doc_path.removeprefix('en/').removesuffix('index.md')
            entry['documentationMapping'] = {'handle': handle, 'slug': slug, 'kind': 'exact' if handle == slug else 'named-alias',
                                             'note': 'Specific variants still require independent pin/layout verification.'}
            tables = md_rows(doc)
            entry['documentedVersions'] = release_history(tables)
            # Omit instruction/firmware tables: they are not hardware specifications.
            for row in tables:
                if re.search(r'specification|display|pin definition|hardware|parameter|technical|memory', row['section'], re.I):
                    entry['specifications'].append({'sourceId': doc_source, 'basis': 'official-documentation-table',
                                                     'section': row['section'], 'label': row['values'][0],
                                                     'value': ' | '.join(row['values'][1:])})
        for source_text, source_id, base in [(body, shop_source, entry['productUrl']),
                                            (docs, doc_source, entry.get('documentationUrl', 'https://wiki.lilygo.cc/'))]:
            if not source_id:
                continue
            for url in links(source_text, base):
                if re.search(r'github\.com/Xinyuan-LilyGO/[^/]+', url, re.I):
                    repo = '/'.join(urlsplit(url).path.strip('/').split('/')[:2])
                    candidate = {'repository': repo, 'url': 'https://github.com/' + repo, 'sourceId': source_id,
                                 'status': 'linked', 'mapping': 'official-page-link; exact SKU still requires review'}
                    if not any(x['repository'].lower() == repo.lower() for x in entry['officialRepositories']):
                        entry['officialRepositories'].append(candidate)
                if re.search(r'github\.com|wiki\.lilygo\.cc|\.pdf(?:\?|$)|\.st[el]p(?:\?|$)|\.stl(?:\?|$)|\.3mf(?:\?|$)', url, re.I):
                    entry['sourceLinks'].append({'url': url, 'sourceId': source_id, 'status': 'linked'})
                if re.search(r'(?:3d|model|dimensions|case)|\.(?:step|stp|stl|3mf|kicad_mod|7z)(?:\?|$)', url, re.I):
                    entry['cadCandidates'].append({'url': url, 'sourceId': source_id, 'status': 'linked',
                                                   'scope': 'candidate; exact variant, geometry and license not verified by this metadata audit'})
                if re.search(r'\.(?:png|jpe?g|webp)(?:\?|$)', url, re.I) and source_id == doc_source:
                    entry['photos'].append({'url': url, 'sourceId': source_id, 'status': 'linked'})
        # A concrete V1.2 catalog identity must not borrow V0.7/V1.1 dates.
        versions = [v for v in entry['documentedVersions'] if v['precision'] == 'day'
                    and not re.search(r'preview|keyboard|battery|backplane|expansion', v['hardwareRevision'] + ' ' + v['description'], re.I)]
        if product_id == 'lilygo-t-beam-v1-2':
            versions = [v for v in versions if re.search(r'V1\.2\b', v['hardwareRevision'], re.I)]
        if versions:
            first = min(versions, key=lambda x: x['date'])
            entry['chronology'] = {'date': first['date'], 'basis': 'hardware-release',
                                    'sourceUrl': entry['documentationUrl'], 'sourceId': doc_source,
                                    'confidence': 'medium', 'hardwareRevision': first['hardwareRevision'],
                                    'notes': 'Earliest dated hardware version explicitly listed in the official Version History. This is the manufacturer documentation date, not independently verified retail launch; undated and preview variants remain separate.'}
            if first['date'].endswith('-01-01'):
                entry['chronology']['confidence'] = 'low'
                entry['conflicts'].append({'kind': 'date-precision-unconfirmed', 'sourceIds': [doc_source],
                                           'notes': 'Official table gives January 1 for this revision; day-level accuracy is not independently established. Retain the published value without claiming a confirmed retail launch date.'})
        elif shop and shop.get('published_at'):
            entry['chronology'] = {'date': shop['published_at'][:10], 'basis': 'storefront-publication',
                                    'sourceUrl': entry['productUrl'], 'sourceId': shop_source,
                                    'confidence': 'low', 'hardwareRevision': product.get('hardwareRevision', 'unknown'),
                                    'notes': 'Official Shopify published_at is a provisional listing-order proxy, not a confirmed hardware release. It can reflect republishing and software-preloaded editions; created_at is retained separately.'}
        else:
            entry['chronology'] = {'date': None, 'basis': 'unknown', 'sourceUrl': entry['productUrl'],
                                    'sourceId': None, 'confidence': 'unknown',
                                    'hardwareRevision': product.get('hardwareRevision', 'unknown'),
                                    'notes': 'No exact dated release evidence found in the inspected sources; no date inferred from a similar device.'}
        factory_links = [x for x in entry['sourceLinks'] if re.search(r'firmware|factory|release', x['url'], re.I)]
        factory_sections = [clean(x) for x in re.findall(r'^.*(?:factory firmware|factory program|factory test|factory example|firmware download|restore factory).*$', docs, re.M | re.I)]
        entry['factoryFirmwareInvestigation'] = {
            'status': 'official-entry-linked' if factory_links else 'official-documentation-only' if factory_sections else 'not-found-in-inspected-pages',
            'sourceIds': list(entry['sourceIds']), 'links': factory_links,
            'evidenceNotes': factory_sections[:5], 'binaryFetched': False, 'simulationVerified': False,
            'notes': 'Product page and official documentation inspected. Linked firmware directories are discovery evidence only; this audit does not establish an exact complete image, digest, factory identity for a sibling variant, or runtime compatibility.'}
        if not entry['cadCandidates']:
            entry['unknowns'].append('No direct CAD/model link in inspected product/documentation pages; linked manufacturer repositories require tree-level model inspection before choosing photo or screen fallback.')
        if not docs:
            entry['unknowns'].append('No exact or curated named-alias page in the inspected official documentation index.')
        if handle == 't-eth-lite':
            entry['conflicts'].append({'kind': 'multiple-hardware-options', 'sourceIds': list(entry['sourceIds']),
                                       'notes': 'Storefront includes both ESP32 + RTL8201 and ESP32-S3 + W5500. The catalog MCU is ESP32-S3, so documentation is explicitly mapped to t-eth-lite-s3. Do not combine columns or inherit classic ESP32 pins.'})
        if handle in ('t-display-s3-pro', 't-display-s3-pro-external'):
            entry['conflicts'].append({'kind': 'screen-controller-review', 'sourceIds': list(entry['sourceIds']),
                                       'notes': 'The original Pro official wiki describes a 2.2-inch ST7789V2 panel; the storefront describes 2.33-inch ST7796 at 222x480. External is a separate listing. Resolve against the exact official source header/hardware revision before selecting a controller; do not apply a sibling Pro panel by name.'})
        if handle == 't-beam-1w':
            entry['conflicts'].append({'kind': 'storefront-copy-error', 'sourceIds': [shop_source],
                                       'notes': 'Storefront MCU text says ESP32-53FN8 and also contains a malformed unrelated Mini-Epaper repository link. Preserve the original source text; exact pins/SoC come from the correct LoRa-Series hardware files.'})
        entry['unknowns'].append('Pins, alternative screen/SoC revisions, physical dimensions and ports require exact variant review; table extraction is source evidence, not runtime configuration.')
        entry['normalizedSpecifications'] = normalized_specifications(entry['specifications'])
        rows.append(entry)
    rows.sort(key=lambda x: (x['chronology']['date'] is not None, x['chronology']['date'] or '', x['id']), reverse=True)
    for rank, row in enumerate(rows, 1):
        row['priorityRank'] = rank
    used = {sid for row in rows for sid in row['sourceIds']}
    report = {'schemaVersion': '1', 'generatedAt': stamp(), 'catalogRevision': catalog['revision'],
              'scope': 'All 127 LILYGO entries already present in the public product catalog; newer external products not silently added.',
              'orderingPolicy': 'Newest to oldest by exact documented hardware release when present; otherwise explicitly labeled storefront publication proxy. Unknown dates follow dated entries. Never use current repository commit time as launch date.',
              'sourcePolicy': 'Original public metadata is cached locally; only facts, links, retrieval receipts and limited discovery notes are retained. Linked images/CAD/firmware have not been fetched by this audit.',
              'sourceCount': len(used), 'sources': [sources[sid] for sid in sorted(used)], 'items': rows,
              'newestFirst': [r['id'] for r in rows],
              'unadaptedNewestFirst': [r['id'] for r in rows if r['wasRuntimeUnavailable']],
              'summary': {'products': len(rows), 'previouslyUnadapted': sum(x['wasRuntimeUnavailable'] for x in rows),
                          'storefrontMatched': sum(bool(x['storefront']) for x in rows),
                          'documentationMatched': sum('documentationUrl' in x for x in rows),
                          'documentedHardwareRelease': sum(x['chronology']['basis'] == 'hardware-release' for x in rows),
                          'storefrontDateProxy': sum(x['chronology']['basis'] == 'storefront-publication' for x in rows),
                          'unknownDate': sum(x['chronology']['date'] is None for x in rows)}}
    write(output, report)
    print(json.dumps(report['summary']))
    for row in rows[:20]:
        print(row['chronology']['date'], row['chronology']['basis'], row['id'])


def refresh(cache):
    for page in (1, 2):
        fetch(f'https://lilygo.cc/products.json?limit=250&page={page}', cache / f'shopify-{page}.raw')
    meta, _ = fetch(f'https://api.github.com/repos/{REPO}', cache / 'wiki-repo.raw')
    branch = json.loads(meta)['default_branch']
    raw, _ = fetch(f'https://api.github.com/repos/{REPO}/git/trees/{branch}?recursive=1', cache / 'wiki-tree.raw')
    tree = json.loads(raw)
    if tree.get('truncated'):
        raise ValueError('Incomplete official documentation tree')
    write(cache / 'wiki-tree.json', tree)
    entries = [x for x in tree['tree'] if x['type'] == 'blob' and x['path'].startswith('en/products/') and x['path'].endswith('index.md')]

    def get(entry):
        path = entry['path']
        destination = cache / 'wiki' / path.replace('/', '__')
        url = f'https://raw.githubusercontent.com/{REPO}/{tree["sha"]}/' + quote(path)
        _, source = fetch(url, destination)
        source.update(commit=tree['sha'], gitBlobSha=entry['sha'], path=path)
        write(destination.with_suffix('.source.json'), source)
        return source

    with ThreadPoolExecutor(max_workers=6) as pool:
        records = list(pool.map(get, entries))
    write(cache / 'wiki-sources.json', records)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cache', type=Path, default=ROOT / 'target/lilygo-audit')
    parser.add_argument('--output', type=Path, default=ROOT / 'docs/platform/research/lilygo-adaptation-sources-2026-11-22.json')
    parser.add_argument('--fetch', action='store_true')
    args = parser.parse_args()
    if args.fetch:
        refresh(args.cache)
    build(args.cache, args.output)
