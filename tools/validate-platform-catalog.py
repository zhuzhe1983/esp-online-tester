#!/usr/bin/env python3
"""Validate public catalogs against their local JSON Schemas; never resolve remote refs."""
import json
from pathlib import Path
from jsonschema import Draft202012Validator
from referencing import Registry, Resource

root = Path(__file__).resolve().parents[1] / 'web/platform'
schemas = {path.name: json.loads(path.read_text()) for path in (root / 'schemas').glob('*.json')}
registry = Registry().with_resources((name, Resource.from_contents(value)) for name, value in schemas.items())
validator = Draft202012Validator(schemas['catalog.schema.json'], registry=registry)
errors = []
for category in ('products', 'accessories', 'firmwares'):
    document = json.loads((root / 'catalog' / f'{category}.json').read_text())
    for error in validator.iter_errors(document):
        errors.append(f'{category}/{"/".join(map(str, error.path))}: {error.message[:240]}')
    kind = {'products': 'product', 'accessories': 'accessory', 'firmwares': 'firmware'}[category]
    item_validator = Draft202012Validator({'$ref': f'catalog.schema.json#/$defs/{kind}'}, registry=registry)
    for item in document['items']:
        for error in item_validator.iter_errors(item):
            errors.append(f'{category}/{item.get("id", "?")}: {error.message[:240]}')
    ids = [item['id'] for item in document['items']]
    if len(ids) != len(set(ids)):
        errors.append(f'{category}: duplicate entry IDs')
    print(f'{category}: {len(ids)} entries')
if errors:
    raise SystemExit('\n'.join(errors))
print('Catalog schemas and unique entry IDs passed.')
