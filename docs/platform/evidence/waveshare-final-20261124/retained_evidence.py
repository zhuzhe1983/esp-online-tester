"""Read byte-identical historical JSON receipts retained with gzip transport."""
import gzip
import json
from pathlib import Path


def read_bytes(path):
    path = Path(path)
    if path.is_file():
        return path.read_bytes()
    with gzip.open(path.with_name(path.name + '.gz'), 'rb') as stream:
        return stream.read()


def read_json(path):
    return json.loads(read_bytes(path))
