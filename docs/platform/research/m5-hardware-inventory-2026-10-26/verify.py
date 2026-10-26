#!/usr/bin/env python3
"""Check the inventory offline; optionally reproduce Git and official-HTML evidence.

No network calls or repository mutations. The caller supplies temporary source paths.
"""
import argparse
import collections
import csv
import hashlib
import json
import pathlib
import re
import subprocess
from html.parser import HTMLParser

HERE = pathlib.Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--git-dir', type=pathlib.Path, help='Temporary bare M5_Hardware clone')
parser.add_argument('--html-cache', type=pathlib.Path, help='Downloaded HTML named first16(SHA256(url)).html')
args = parser.parse_args()
load = lambda name: json.loads((HERE / name).read_text())
data, history = load('inventory.json'), load('commits.json')
products, metadata = data['products'], data['metadata']
summary, sources = metadata['summary'], {s['id']: s for s in load('sources.json')['sources']}
names = {p['directory'] for p in products}
assert len(products) == len(names) == summary['currentDirectories'] == 270
assert len(sources) == 270
assert dict(collections.Counter(p['group'] for p in products)) == summary['groups']
assert summary['groups'] == {'esp32-controller': 83, 'unit': 154, 'non-esp32-controller': 5, 'other-accessory-or-module': 28}
assert len(history['changes']) == summary['window']['productsCommits'] == 58
assert len(set(history['uniqueTouched'])) == 248
assert sum(p['isUnit'] and p['esp32OnboardStatus'] == 'verified-present' for p in products) == 13
assert sum(p['esp32OnboardStatus'] == 'verified-present' for p in products) == 97
assert summary['window']['endpointAdded'] == 217
assert summary['window']['endpointModifiedExisting'] == 29
assert summary['window']['endpointDeleted'] == 0
assert summary['window']['directoryEventsUniqueCounts'] == {'added': 219, 'modified': 133, 'deleted': 2}
assert len({c['sha'] for c in history['changes']}) == 58
for p in products:
    source = sources[p['officialDocumentSourceId']]
    assert source['status'] == 'fetched'
    assert re.fullmatch(r'[0-9a-f]{64}', source['sha256'])
    assert source['fetchedAt'].endswith('Z')
    assert source['title'] == p['name']
    assert source['sku'] == p['skuAsDocumented']
    if p['group'] == 'esp32-controller':
        assert p['primaryChip'].startswith('ESP32') and p['primaryChipIsEsp32'] and p['chipEvidence']
    if p['isUnit']:
        assert '/en/unit/' in source['url']
    for fact in p['chipEvidence']:
        assert fact['sourceId'] == source['id']
    assert sum(len(f['quote'].split()) for f in p['chipEvidence']) <= 25
assert len(list(csv.DictReader((HERE / 'inventory.csv').open()))) == 270
files = [f for p in products for f in p['files']]
assert len(files) == summary['filesUnderProducts'] == 324
assert dict(collections.Counter(f['extension'] for f in files)) == summary['fileExtensions']
assert sum(bool(p['modelFormats']) for p in products) == 264
for status, expected in [('added', history['added']), ('modified-existing', history['modifiedExisting'])]:
    assert {p['directory'] for p in products if p['endpointChange'] == status} == set(expected)

if args.git_dir:
    def git(*parts):
        return subprocess.check_output(['git', '-c', 'core.quotepath=false', '--git-dir=' + str(args.git_dir), *parts], text=True).strip()
    def tree(revision):
        return {line.split('\t')[1]: line.split()[2] for line in git('ls-tree', revision + ':Products').splitlines()}
    current, baseline = tree(metadata['head']), tree(metadata['baseline'])
    assert set(current) == names
    assert len(baseline) == 53
    assert git('rev-parse', metadata['head'] + ':Products') == metadata['productsTreeGitSha1']
    commits = git('log', '--format=%H', '--since=' + metadata['windowStart'], '--until=' + metadata['windowEnd'], metadata['head'], '--', 'Products').splitlines()
    assert set(commits) == {c['sha'] for c in history['changes']}
    assert len(git('log', '--format=%H', '--since=' + metadata['windowStart'], '--until=' + metadata['windowEnd'], metadata['head']).splitlines()) == 58
    assert not git('log', '--merges', '--format=%H', '--since=' + metadata['windowStart'], '--until=' + metadata['windowEnd'], metadata['head'])
    assert set(current) - set(baseline) == set(history['added'])
    assert set(baseline) - set(current) == set(history['deleted'])
    assert {n for n in current.keys() & baseline.keys() if current[n] != baseline[n]} == set(history['modifiedExisting'])
    objects = {line.split('\t')[1]: line.split()[2] for line in git('ls-tree', '-r', metadata['head'], '--', 'Products').splitlines()}
    assert objects == {f['path']: f['gitBlobSha1'] for f in files}
    for commit in history['changes']:
        current, previous = tree(commit['sha']), tree(commit['sha'] + '^')
        expected = {'added': sorted(current.keys() - previous.keys()), 'deleted': sorted(previous.keys() - current.keys()), 'modified': sorted(n for n in current.keys() & previous.keys() if current[n] != previous[n])}
        assert expected == commit['directoryChanges']
    print('Git revision, 270 directories, 324 blob IDs, 58 commits and all per-commit directory deltas verified.')

class Article(HTMLParser):
    def __init__(self):
        super().__init__(); self.depth = 0; self.skip = 0; self.parts = []
    def handle_starttag(self, tag, attrs):
        if tag == 'div':
            if self.depth: self.depth += 1
            elif 'markdown' in dict(attrs).get('class', '').split(): self.depth = 1
        if self.depth and tag in ['script', 'style', 'svg']: self.skip += 1
        if self.depth and not self.skip and tag in ['h1', 'h2', 'h3', 'p', 'tr', 'li']: self.parts.append('\n')
        if self.depth and not self.skip and tag in ['td', 'th']: self.parts.append(' | ')
    def handle_endtag(self, tag):
        if self.depth and tag == 'div': self.depth -= 1
        if self.skip and tag in ['script', 'style', 'svg']: self.skip -= 1
    def handle_data(self, text):
        if self.depth and not self.skip: self.parts.append(text)
    def text(self):
        return '\n'.join(re.sub(r'[ \t]+', ' ', line).strip() for line in ''.join(self.parts).splitlines() if line.strip())

if args.html_cache:
    for p in products:
        s = sources[p['officialDocumentSourceId']]
        key = hashlib.sha256(s['url'].encode()).hexdigest()[:16]
        raw = (args.html_cache / (key + '.html')).read_bytes()
        assert hashlib.sha256(raw).hexdigest() == s['sha256']
        article = Article(); article.feed(raw.decode()); body = article.text()
        assert body.splitlines()[0] == p['name']
        assert 'SKU:' + p['skuAsDocumented'] in body
        for fact in p['chipEvidence']: assert fact['quote'] in body, (p['directory'], fact)
    print('270 official HTML SHA256 values, article titles/SKUs and retained chip evidence verified.')
print('Inventory integrity checks passed.')
