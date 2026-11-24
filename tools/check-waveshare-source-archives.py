#!/usr/bin/env python3
"""Verify retained official RAR members and meaningful malformed-source cases."""
import argparse
import datetime
import gzip
import hashlib
import io
import json
from pathlib import Path
import zipfile

from waveshare_source_archives import rar_members


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    assets = Path(__file__).resolve().parents[1] / 'web/platform/assets'
    checks = []
    for suffix in ['', '-m']:
        product_id = 'waveshare-esp32-c6-touch-lcd-1-47' + suffix
        directory = assets / product_id
        plan = json.loads((directory / 'model-plan.json').read_text())
        sources = {s['id']: s for s in plan['sources']}
        for entry in (s for s in sources.values() if s.get('origin') == 'rar-member'):
            container = (directory / sources[entry['containerSourceId']]['file']).read_bytes()
            assert hashlib.sha256(container).hexdigest() == entry['archive']['containerSha256']
            member = entry['archive']['entry']
            data = rar_members(container, {member})[member]
            retained = (directory / entry['file']).read_bytes()
            original = gzip.decompress(retained) if entry.get('retained', {}).get('encoding') == 'gzip' else retained
            assert data == original
            assert len(data) == entry['bytes']
            assert hashlib.sha256(data).hexdigest() == entry['sha256']
            checks.append({'case': 'exact official member', 'productId': product_id,
                           'member': member, 'bytes': len(data), 'sha256': entry['sha256'], 'pass': True})
    # Use the real last archive for missing/limited/truncated-member failures.
    wrong_format = io.BytesIO()
    with zipfile.ZipFile(wrong_format, 'w') as archive:
        archive.writestr(member, b'wrong archive format')
    cases = [
        ('missing member', container, {'include': {'not-a-member.stp'}}),
        ('decoded size limit', container, {'include': {member}, 'max_member_bytes': 1024}),
        ('wrong format', wrong_format.getvalue(), {'include': {member}}),
        ('truncated archive', container[:256], {'include': {member}}),
    ]
    for name, payload, options in cases:
        try:
            rar_members(payload, **options)
        except (ValueError, RuntimeError) as error:
            checks.append({'case': name, 'pass': True, 'rejectedWith': str(error)})
        else:
            raise AssertionError('Expected source rejection: ' + name)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps({'checkedAt': datetime.datetime.now(datetime.timezone.utc).isoformat(),
                                     'checks': checks}, indent=2) + '\n')
    print(f'{len(checks)} RAR source checks passed')


if __name__ == '__main__':
    main()
