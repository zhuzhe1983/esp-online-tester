#!/usr/bin/env python3
"""Check a diagnose-stopwatch-tearing export. Requires Pillow; no network access."""
import argparse
import base64
import hashlib
import io
import json
from PIL import Image


def rgba(data_url):
    return Image.open(io.BytesIO(base64.b64decode(data_url.split(',', 1)[1]))).convert('RGBA').tobytes()


def split_rows(previous, current, following, width, height):
    stride = width * 4
    rows = range(height)
    before = [current[y * stride:(y + 1) * stride] == previous[y * stride:(y + 1) * stride] for y in rows]
    after = [current[y * stride:(y + 1) * stride] == following[y * stride:(y + 1) * stride] for y in rows]
    return [y for y in range(1, height) if all(after[:y]) and all(before[y:])
            and not all(before[:y]) and not all(after[y:])]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('export_json', help='JSON from probe.export(); retain under ignored output/')
    args = parser.parse_args()
    with open(args.export_json, encoding='utf-8') as stream:
        data = json.load(stream)
    frames = data['frames']
    images = [rgba(frame['rawPng']) for frame in frames]
    assert all(hashlib.sha256(image).hexdigest() == frame['sha256'] for image, frame in zip(images, frames))
    canvas_equal = [image == rgba(frame['canvasPng']) for image, frame in zip(images, frames)]
    candidates = []
    for i in range(1, len(images) - 1):
        frame = frames[i]
        boundaries = split_rows(images[i - 1], images[i], images[i + 1], frame['width'], frame['height'])
        if boundaries:
            candidates.append({'index': i, 'frameNumber': frame['number'], 'splitRows': boundaries,
                'previousSha256': frames[i - 1]['sha256'], 'sha256': frame['sha256'],
                'followingSha256': frames[i + 1]['sha256'], 'stats': frame['stats']})
    print(json.dumps({'frameCount': len(frames), 'canvasEqualCount': sum(canvas_equal),
        'allRawHashesVerified': True, 'exactPreviousNextSplices': candidates}, indent=2))


if __name__ == '__main__':
    main()
