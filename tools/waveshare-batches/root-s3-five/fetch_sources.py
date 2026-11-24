#!/usr/bin/env python3
"""Fetch explicitly selected public primary inputs with original-byte receipts."""
import argparse
import concurrent.futures
import datetime
import hashlib
import json
from pathlib import Path
import urllib.request


def utc():
    return datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00', 'Z')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cache', type=Path, required=True)
    parser.add_argument('--queue', type=Path, required=True)
    args = parser.parse_args()

    def fetch(item):
        path = args.cache / item['name'] / item['filename']
        path.parent.mkdir(parents=True, exist_ok=True)
        receipt = path.with_name(path.name + '.receipt.json')
        if path.exists() and receipt.exists():
            old = json.loads(receipt.read_text())
            if old.get('sha256') == hashlib.sha256(path.read_bytes()).hexdigest():
                return dict(item, status='cached', bytes=path.stat().st_size)
        row = {'requestedUrl': item['url'], 'requestedAt': utc()}
        try:
            request = urllib.request.Request(item['url'], headers={'User-Agent': 'Mozilla/5.0'})
            with urllib.request.urlopen(request, timeout=45) as response:
                data = response.read()
                row.update(finalUrl=response.url, fetchedAt=utc(), httpStatus=response.status,
                           contentType=response.headers.get('Content-Type'), bytes=len(data),
                           sha256=hashlib.sha256(data).hexdigest(), status='fetched')
            path.write_bytes(data)
            receipt.write_text(json.dumps(row, indent=2) + '\n')
            return dict(item, status='fetched', bytes=len(data))
        except Exception as error:
            row.update(status='failed', attemptedAt=utc(), error=str(error))
            receipt.write_text(json.dumps(row, indent=2) + '\n')
            return dict(item, status='failed', error=str(error))

    results = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        for row in pool.map(fetch, json.loads(args.queue.read_text())):
            results.append(row)
            print(json.dumps(row), flush=True)
            (args.cache / 'download-status.json').write_text(json.dumps(results, indent=2) + '\n')


if __name__ == '__main__':
    main()
