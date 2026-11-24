#!/usr/bin/env python3
"""Validate the five family fragments and their source-bound knowledge records."""
from pathlib import Path
import argparse, hashlib, json
from jsonschema import Draft202012Validator
from referencing import Registry, Resource

ROOT = Path(__file__).resolve().parents[3]
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--schemas', type=Path, default=ROOT/'web/platform/schemas')
args = p.parse_args()
schemas = {x.name:json.loads(x.read_text()) for x in args.schemas.glob('*.json')}
registry = Registry().with_resources((n,Resource.from_contents(s)) for n,s in schemas.items())
evidence = ROOT/'docs/research/waveshare-s3-industrial-2026-11-24/batch-03'
manifest = json.loads((evidence/'manifest.json').read_text())
results = []
product_ids, firmware_ids, watch_ids, skus = set(), set(), set(), set()
for item in manifest['items']:
    fragment = json.loads((ROOT/item['fragment']).read_text())
    for key, ref, seen in [('products','catalog.schema.json#/$defs/product',product_ids),('firmwares','catalog.schema.json#/$defs/firmware',firmware_ids),('watchlist','firmware-watchlist.schema.json#/properties/entries/items',watch_ids)]:
        validator = Draft202012Validator({'$ref':ref},registry=registry)
        for record in fragment[key]:
            validator.validate(record)
            assert record['id'] not in seen
            seen.add(record['id'])
    family_products={p['id'] for p in fragment['products']}
    family_firmwares={f['id']:f for f in fragment['firmwares']}
    for product in fragment['products']:
        assert product['sku'] not in skus
        skus.add(product['sku'])
        assert product['id'] in family_firmwares[product['defaultFirmwareId']]['productIds']
        cap=fragment['boardCapabilities'][product['runtime']['board']]
        assert (cap['flashMb'],cap['psramMb']) == (product['runtime']['flashMb'],product['runtime']['psramMb'])
        assert cap['display'] == (product['displayStatus']=='known')
    for firmware in fragment['firmwares']:
        assert set(firmware['productIds']) <= family_products
        assert firmware['simulationVerified'] is False and firmware['hardwareValidated'] is False
        if firmware['source']:
            assert firmware['status']=='trial'
            assert firmware['source']['archive']['format']=='zip-bundle'
            assert firmware['source']['bootMode']=='rom'
    for k in fragment['knowledge']:
        record=json.loads((ROOT/k['path']).read_text())
        Draft202012Validator(schemas['knowledge.schema.json'],registry=registry).validate(record)
        assert record['catalogId'] in family_products
        source_ids={s['id'] for s in record['sources']}
        assert len(source_ids)==len(record['sources'])
        for fact in record['facts']+record['unknowns']:
            assert set(fact['sourceIds']) <= source_ids
    results.append({'listingId':item['listingId'],'products':len(fragment['products']),'firmwares':len(fragment['firmwares']),'watchlist':len(fragment['watchlist']),'knowledge':len(fragment['knowledge']),'status':'passed'})
assert len(product_ids)==len(skus)==19 and len(firmware_ids)==len(watch_ids)==18
receipt={'scope':'Current integration JSON schemas plus exact SKU uniqueness, memory/default/target bindings, manual watchlist and source ID checks. Runtime/UI integration remains a separate gate.','schemas':{n:hashlib.sha256((args.schemas/n).read_bytes()).hexdigest() for n in ['catalog.schema.json','knowledge.schema.json','firmware-watchlist.schema.json']},'results':results}
(evidence/'schema-checks.json').write_text(json.dumps(receipt,indent=2)+'\n')
print('PASS: 19 products / 19 knowledge records / 18 firmwares / 18 watch entries; exact IDs, memory, defaults, source bindings and integration schemas.')
