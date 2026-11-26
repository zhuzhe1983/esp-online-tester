#!/usr/bin/env python3
"""Package or reassemble the complete final M5 control audit without losing bytes."""
import argparse
import copy
import gzip
import hashlib
import io
import json
from pathlib import Path
import re

LIMIT = 50_000
RAW = 'all-141-final.json'
COVERAGE = 'all-141-coverage.json'
COLUMNS = ['productId', 'file', 'sha256', 'bytes', 'uncompressedSha256', 'uncompressedBytes']

def sha(data):
    return hashlib.sha256(data).hexdigest()

def packed(data):
    stream = io.BytesIO()
    with gzip.GzipFile(filename='', mode='wb', fileobj=stream, compresslevel=9, mtime=0) as handle:
        handle.write(data)
    result = stream.getvalue()
    assert result[4:8] == b'\0' * 4
    assert gzip.decompress(result) == data
    return result

def artifact(file, compressed, original):
    return dict(file=file, sha256=sha(compressed), bytes=len(compressed),
                uncompressedSha256=sha(original), uncompressedBytes=len(original))

def write_small(path, data):
    if len(data) >= LIMIT:
        raise ValueError(f'Evidence file must be below {LIMIT} bytes: {path.name} ({len(data)})')
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)

def json_bytes(value):
    return (json.dumps(value, ensure_ascii=False, indent=2) + '\n').encode()

def manifest_bytes(value):
    # One compact, column-labelled product row per line keeps the index reviewable.
    fields = []
    for key, item in value.items():
        if key == 'products':
            body = '[\n' + ',\n'.join('    ' + json.dumps(row, ensure_ascii=False, separators=(',', ':'))
                                      for row in item) + '\n  ]'
        else:
            body = json.dumps(item, ensure_ascii=False, indent=2).replace('\n', '\n  ')
        fields.append('  ' + json.dumps(key) + ': ' + body)
    return ('{\n' + ',\n'.join(fields) + '\n}\n').encode()

def safe_file(root, name):
    relative = Path(name)
    if relative.is_absolute() or '..' in relative.parts:
        raise ValueError('Unsafe artifact path')
    return root / relative

def checked_gzip(folder, record):
    data = safe_file(folder, record['file']).read_bytes()
    assert len(data) == record['bytes'] and sha(data) == record['sha256']
    assert data[4:8] == b'\0' * 4
    original = gzip.decompress(data)
    assert len(original) == record['uncompressedBytes']
    assert sha(original) == record['uncompressedSha256']
    return original

def package(source, output):
    original = (source / RAW).read_bytes()
    coverage_bytes = (source / COVERAGE).read_bytes()
    raw, coverage = json.loads(original), json.loads(coverage_bytes)
    assert coverage['rawReportSha256'] == sha(original)
    text, decoder = original.decode(), json.JSONDecoder()
    marker = '\n  "products": ['
    cursor = text.index(marker) + len(marker)
    while text[cursor].isspace():
        cursor += 1
    prefix, rows, separators = text[:cursor], [], []
    for index, expected in enumerate(raw['products']):
        value, end = decoder.raw_decode(text, cursor)
        assert value == expected
        identity = value['productId']
        if not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]*', identity):
            raise ValueError('Unsafe product identity')
        chunk = text[cursor:end].encode()
        filename = f'all-141-final-products/{identity}.json.gz'
        compressed = packed(chunk)
        write_small(output / filename, compressed)
        info = artifact(filename, compressed, chunk)
        rows.append([identity, *[info[key] for key in COLUMNS[1:]]])
        if index + 1 < len(raw['products']):
            cursor = end
            while text[cursor].isspace():
                cursor += 1
            assert text[cursor] == ','
            cursor += 1
            while text[cursor].isspace():
                cursor += 1
            separators.append(text[end:cursor])
        else:
            suffix = text[end:]
    assert len(set(separators)) <= 1
    separator = separators[0] if separators else ''
    manifest = dict(schemaVersion=2, format='per-product-gzip-exact-reassembly',
                    sourceStartCommit=raw['sourceStartCommit'], sourceCommit=raw['sourceCommit'],
                    catalogSha256=raw['catalogSha256'], harnessSha256=raw['harnessSha256'],
                    summary=raw['summary'],
                    original=dict(file=RAW, bytes=len(original), sha256=sha(original)),
                    productColumns=COLUMNS, products=rows,
                    layout=dict(prefix=prefix, separator=separator, suffix=suffix),
                    scope='Every original byte, including all ray values and negatives, is retained. '
                          'Reassemble prefix + product slices joined by separator + suffix. '
                          'No measured values, geometry hashes or historical receipts are rewritten.')
    manifest_data = manifest_bytes(manifest)
    write_small(output / RAW, manifest_data)
    coverage_gzip = packed(coverage_bytes)
    coverage_artifact = artifact(COVERAGE + '.gz', coverage_gzip, coverage_bytes)
    write_small(output / coverage_artifact['file'], coverage_gzip)
    summary = copy.deepcopy(coverage)
    summary.pop('products')
    summary.update(schemaVersion=2, format='compact-summary-with-gzip-detail',
                   rawReport=RAW, rawReportFormat=manifest['format'],
                   rawReportSha256=sha(manifest_data),
                   originalRawReportSha256=sha(original), originalRawReportBytes=len(original),
                   completeCoverage=coverage_artifact)
    write_small(output / COVERAGE, json_bytes(summary))
    return dict(products=len(rows), originalRawSha256=sha(original),
                rawManifestSha256=sha(manifest_data), coverage=coverage_artifact)

def unpack(source, output):
    manifest_data = (source / RAW).read_bytes()
    manifest = json.loads(manifest_data)
    summary = json.loads((source / COVERAGE).read_bytes())
    assert manifest['format'] == 'per-product-gzip-exact-reassembly'
    assert manifest['productColumns'] == COLUMNS
    assert summary['rawReportSha256'] == sha(manifest_data)
    chunks = []
    for row in manifest['products']:
        record = dict(zip(COLUMNS, row, strict=True))
        data = checked_gzip(source, record)
        assert json.loads(data)['productId'] == record['productId']
        chunks.append(data)
    layout = manifest['layout']
    raw = layout['prefix'].encode() + layout['separator'].encode().join(chunks) + layout['suffix'].encode()
    assert len(raw) == manifest['original']['bytes']
    assert sha(raw) == manifest['original']['sha256']
    coverage = checked_gzip(source, summary['completeCoverage'])
    assert json.loads(coverage)['rawReportSha256'] == sha(raw)
    assert len(json.loads(raw)['products']) == len(chunks)
    output.mkdir(parents=True, exist_ok=True)
    (output / RAW).write_bytes(raw)
    (output / COVERAGE).write_bytes(coverage)
    return dict(products=len(chunks), rawSha256=sha(raw), rawBytes=len(raw),
                coverageSha256=sha(coverage), coverageBytes=len(coverage))

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action', choices=['pack', 'unpack'])
    parser.add_argument('--input', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if args.input.resolve() == args.out.resolve():
        parser.error('Input and output directories must be separate')
    result = package(args.input, args.out) if args.action == 'pack' else unpack(args.input, args.out)
    print(json.dumps(result, indent=2))
