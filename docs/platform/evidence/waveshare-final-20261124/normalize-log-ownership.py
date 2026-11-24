#!/usr/bin/env python3
"""Remove ls owner/group labels while preserving measurements and original hashes."""
from pathlib import Path
import datetime
import hashlib
import json
import re

root = Path(__file__).resolve().parent
ownership = re.compile(r'^([bcdlps-][rwxstST-]{9}[+.]?\s+\d+)\s+\S+\s+\S+(\s+\d+\s+[A-Z][a-z]{2}\s+\d+\s+\S+\s+.+)$', re.MULTILINE)
sha = lambda body: hashlib.sha256(body).hexdigest()
changes = {}
for path in sorted(root.rglob('*.log')):
    before = path.read_bytes()
    after = ownership.sub(r'\1 [USER] [GROUP]\2', before.decode()).encode()
    if before == after:
        continue
    changes[str(path.relative_to(root))] = {'beforeSha256': sha(before), 'afterSha256': sha(after),
                                          'beforeBytes': len(before), 'afterBytes': len(after)}
    path.write_bytes(after)

updated_reports = []
for path in sorted(root.rglob('*.json')):
    value = json.loads(path.read_text())
    changed = [False]

    def update(node):
        if isinstance(node, dict):
            log = node.get('log')
            if isinstance(log, str):
                relative = str((path.parent / log).relative_to(root))
                correction = changes.get(relative)
                if correction and 'logSha256' in node:
                    assert node['logSha256'] == correction['beforeSha256'], relative
                    node['ownershipNormalizationInputSha256'] = correction['beforeSha256']
                    node['logSha256'] = correction['afterSha256']
                    if 'logBytes' in node:
                        assert node['logBytes'] == correction['beforeBytes'], relative
                        node['logBytes'] = correction['afterBytes']
                    changed[0] = True
            for child in node.values():
                update(child)
        elif isinstance(node, list):
            for child in node:
                update(child)

    update(value)
    if changed[0]:
        path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')
        updated_reports.append(str(path.relative_to(root)))

receipt = root / 'log-ownership-normalization.json'
if changes:
    previous = json.loads(receipt.read_text()) if receipt.exists() else {'schemaVersion': 1, 'operations': []}
    previous['operations'].append({'recordedAt': datetime.datetime.now(datetime.timezone.utc).isoformat(),
                                  'method': 'Replace only ls -l owner/group columns with [USER] [GROUP]. Preserve file size, mode, timestamps, commands, test results and measured values. Original normalized-path log hashes are retained below and in updated report rows.',
                                  'files': changes, 'updatedReports': updated_reports})
    receipt.write_text(json.dumps(previous, indent=2) + '\n')
print(json.dumps({'normalizedLogs': len(changes), 'updatedReports': updated_reports}))
