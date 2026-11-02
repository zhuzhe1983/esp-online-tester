#!/usr/bin/env python3
"""Keep the existing OEM 1 data; remove unused legacy OCR components.

Usage: python3 tools/trim-ocr-traineddata.py original-eng.traineddata.gz output.gz
Source identity is pinned; output is deterministic gzip with no filename/time.
Component IDs: tesseract 5.5.3 src/ccutil/tessdatamanager.h (TessdataType).
https://github.com/tesseract-ocr/tesseract/blob/5.5.3/src/ccutil/tessdatamanager.h
"""
import gzip
import hashlib
import io
import json
import struct
import sys
from pathlib import Path

SOURCE_SHA256 = 'ed350f3752f81ee8f38769edc14d92d997dababe23b565c59879372cc46a2468'
NAMES = ['lstm', 'lstm-punc-dawg', 'lstm-word-dawg', 'lstm-number-dawg', 'lstm-unicharset', 'lstm-recoder', 'version']

def main():
    source = Path(sys.argv[1]).read_bytes()
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA256:
        raise ValueError('Source does not match the pinned original English model')
    data = gzip.decompress(source)
    count = struct.unpack_from('<i', data)[0]
    if count != 24:
        raise ValueError('Unexpected traineddata format')
    offsets = struct.unpack_from('<24q', data, 4)
    parts = []
    for index in range(17, 24):
        start = offsets[index]
        end = next((offset for offset in offsets[index + 1:] if offset >= 0), len(data))
        if not 196 <= start < end <= len(data):
            raise ValueError('Invalid component boundaries')
        parts.append(data[start:end])
    output_offsets = [-1] * 24
    position = 196
    for index, part in enumerate(parts, 17):
        output_offsets[index] = position
        position += len(part)
    output = struct.pack('<i24q', 24, *output_offsets) + b''.join(parts)
    buffer = io.BytesIO()
    with gzip.GzipFile(fileobj=buffer, mode='wb', filename='', mtime=0, compresslevel=9) as stream:
        stream.write(output)
    compressed = buffer.getvalue()
    Path(sys.argv[2]).write_bytes(compressed)
    print(json.dumps({'sourceSha256': SOURCE_SHA256, 'sourceBytes': len(source), 'bytes': len(compressed),
        'sha256': hashlib.sha256(compressed).hexdigest(), 'components': [
            {'id': i + 17, 'name': name, 'bytes': len(part), 'sha256': hashlib.sha256(part).hexdigest()}
            for i, (name, part) in enumerate(zip(NAMES, parts))]}, indent=2))

if __name__ == '__main__':
    main()
