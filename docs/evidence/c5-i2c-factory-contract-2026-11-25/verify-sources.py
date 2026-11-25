#!/usr/bin/env python3
"""Verify fixed C5 I2C source bytes and optional caller-supplied typed inputs.

This is a static audit: it does not execute a controller, CPU or firmware.
"""
import argparse
import datetime
import hashlib
import json
from pathlib import Path
import re
import urllib.request


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-root', type=Path, required=True)
    parser.add_argument('--fetch', action='store_true', help='fetch the fixed official source URLs')
    parser.add_argument('--app-typed', type=Path)
    parser.add_argument('--iram-typed', type=Path)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    receipt = json.loads(Path(__file__).with_name('source-receipt.json').read_text())
    sources = {}
    for source in receipt['sources']:
        key = source['version'], source['path']
        local = args.source_root / source['version'] / source['path'].replace('/', '__')
        if args.fetch:
            with urllib.request.urlopen(source['url'], timeout=30) as response:
                data = response.read()
            local.parent.mkdir(parents=True, exist_ok=True)
            local.write_bytes(data)
        else:
            data = local.read_bytes()
        assert len(data) == source['bytes'], f'byte count: {key}'
        assert hashlib.sha256(data).hexdigest() == source['sha256'], f'hash: {key}'
        sources[key] = data
    paths = {path for _, path in sources}
    for path in paths:
        assert sources['v553', path] == sources['v554', path], f'version difference: {path}'
    expected = {'NACK': 10, 'TIME_OUT': 8, 'TRANS_COMPLETE': 7, 'ARBITRATION_LOST': 5, 'END_DETECT': 3}
    for version in ('v553', 'v554'):
        header = sources[version, 'components/soc/esp32c5/register/soc/i2c_reg.h'].decode()
        for event, shift in expected.items():
            for suffix in ('RAW', 'CLR', 'ENA', 'ST'):
                match = re.search(rf'^#define I2C_{event}_INT_{suffix}_S\s+(\d+)\s*$', header, re.M)
                assert match and int(match[1]) == shift, f'{version}: {event}/{suffix}'
    mask = sum(1 << bit for bit in expected.values())
    assert mask == 0x5a8
    typed = []
    for label, local in (('appTyped', args.app_typed), ('iramTyped', args.iram_typed)):
        if local is None:
            continue
        expected_input = next(item for item in receipt['inputs'] if item['label'] == label)
        data = local.read_bytes()
        assert len(data) == expected_input['bytes'], f'byte count: {label}'
        assert hashlib.sha256(data).hexdigest() == expected_input['sha256'], f'hash: {label}'
        text = data.decode()
        for pc, word in expected_input['instructionWords'].items():
            assert re.search(rf'^{pc}:\s+{word}\s', text, re.M), f'instruction: {label}/{pc}'
        typed.append({'label': label, 'sha256': expected_input['sha256'], 'instructionWordsChecked': len(expected_input['instructionWords'])})
    result = {
        'kind': 'static-source-and-typed-instruction-contract',
        'checkedAtUtc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
        'sourcesChecked': len(sources), 'identicalVersionPairs': len(paths),
        'masterEventMask': hex(mask), 'typedInputs': typed,
        'controllerTestsExecuted': False, 'factoryFirmwareExecuted': False,
        'result': 'pass',
    }
    output = json.dumps(result, indent=2) + '\n'
    if args.output:
        args.output.write_text(output)
    print(output, end='')


if __name__ == '__main__':
    main()
