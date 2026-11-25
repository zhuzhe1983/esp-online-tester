#!/usr/bin/env python3
"""Read-only verification of C5 evidence curation; never execute guest commands."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import subprocess


def digest(data):
    return hashlib.sha256(data).hexdigest()


def at(value, pointer):
    for part in pointer.strip('/').split('/') if pointer else []:
        part = part.replace('~1', '/').replace('~0', '~')
        value = value[int(part)] if isinstance(value, list) else value[part]
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, default=Path(__file__).resolve().parents[3])
    parser.add_argument('--integration-root', type=Path)
    parser.add_argument('--cache-root', type=Path)
    args = parser.parse_args()
    repo = args.repo.resolve()
    directory = repo / 'docs/evidence/c5-evidence-retention-final-2026-11-25'

    def read(name):
        return json.loads((directory / name).read_text())

    def original(revision, path):
        return subprocess.check_output(['git', 'show', revision + ':' + path], cwd=repo)

    def private_path(label):
        choices = {'${INTEGRATION_ROOT}/': args.integration_root, '${C5_CACHE_ROOT}/': args.cache_root}
        for prefix, root in choices.items():
            if label.startswith(prefix):
                if root is None:
                    return None
                root = root.resolve()
                path = (root / label[len(prefix):]).resolve()
                assert path.is_relative_to(root), 'capture path escapes supplied root'
                return path
        raise AssertionError('unknown capture root label')

    def verify_private(row, path_key, byte_key, hash_key):
        path = private_path(row[path_key])
        if path is None:
            return False
        data = path.read_bytes()
        assert len(data) == row[byte_key] and digest(data) == row[hash_key], row[path_key]
        return True

    stdout = read('stdout-manifest.json')
    mappings = read('receipt-hash-mapping.json')['changes']
    artifacts = read('log-artifacts.json')['artifacts']
    references = sum((read('log-references-' + day + '.json')['references']
                      for day in ['2026-11-24', '2026-11-25']), [])
    components = read('component-output-hashes.json')['outputs']
    base = stdout['snapshotRevision']
    hashes = {row['sha256']: row for row in artifacts}
    assert len(hashes) == len(artifacts) == 143
    assert len(references) == 164 and len(components) == 47
    expected_additions = {(row['receipt'], row['objectPointer'] + '/' + row['bytesKey'])
                          for row in references}
    expected_additions |= {(row['sourceReceipt']['path'], '/stdoutRetention')
                           for row in stdout['archives']}
    seen_additions = set()
    json_count = readme_count = private_receipts = 0
    old_json = {}
    for row in mappings:
        before = original(row['originalRevision'], row['path'])
        after = (repo / row['path']).read_bytes()
        assert len(before) == row['originalBytes'] and digest(before) == row['originalSha256']
        assert len(after) == row['updatedBytes'] and digest(after) == row['updatedSha256']
        git_blob = hashlib.sha1(b'blob ' + str(len(before)).encode() + b'\0' + before).hexdigest()
        assert git_blob == row['gitBlob']
        if row['path'].endswith('.json'):
            old = json.loads(before)
            new = json.loads(after)
            old_json[row['path']] = old
            restored = copy.deepcopy(new)
            for pointer in row['addedPointers']:
                parent, key = pointer.rsplit('/', 1)
                key = key.replace('~1', '/').replace('~0', '~')
                container = at(restored, parent)
                assert key in container and key not in at(old, parent)
                assert (row['path'], pointer) in expected_additions
                del container[key]
                seen_additions.add((row['path'], pointer))
            # Encoding the parsed values also distinguishes bools, ints and strings.
            assert json.dumps(old, sort_keys=True) == json.dumps(restored, sort_keys=True), row['path']
            json_count += 1
        else:
            text = before.decode()
            for replacement in row['readmeEdits']['replacements']:
                assert replacement['old'] in text
                text = text.replace(replacement['old'], replacement['new'])
            assert (text + row['readmeEdits']['append']).encode() == after
            readme_count += 1
        private_receipts += verify_private(row, 'privateOriginal', 'originalBytes', 'originalSha256')
    assert json_count == 42 and readme_count == 11 and seen_additions == expected_additions

    private_stdout = 0
    assert len(stdout['archives']) == stdout['totalFiles'] == 12
    assert sum(row['bytes'] for row in stdout['archives']) == stdout['totalBytes'] == 22791
    for row in stdout['archives']:
        assert not (repo / row['originalPath']).exists()
        data = original(row['artifactRevision'], row['originalPath'])
        assert len(data) == row['bytes'] and digest(data) == row['sha256']
        assert row['reproductionCommands']
        source = row['sourceReceipt']
        assert digest(original(source['revision'], source['path'])) == source['sha256']
        private_stdout += verify_private(row, 'privatePath', 'bytes', 'sha256')
        path = private_path(row['privatePath'])
        if path is not None:
            assert subprocess.run(['git', 'check-ignore', '--', str(path)],
                                  cwd=args.integration_root, capture_output=True).returncode == 0

    private_logs = sum(verify_private(row, 'privatePath', 'bytes', 'sha256') for row in artifacts)
    for row in references:
        before = at(old_json[row['receipt']], row['objectPointer'])
        after = at(json.loads((repo / row['receipt']).read_text()), row['objectPointer'])
        assert before[row['hashKey']] == row['sha256']
        assert after[row['hashKey']] == row['sha256']
        assert after[row['bytesKey']] == hashes[row['sha256']]['bytes']
        assert before[row['logKey']] == after[row['logKey']]

    private_components = unknown = 0
    for row in components:
        data = json.loads(original(base, row['receipt']))
        assert at(data, row['pointer']) == row['sha256']
        assert at(json.loads((repo / row['receipt']).read_text()), row['pointer']) == row['sha256']
        if row['availability'] == 'byte-matched':
            assert isinstance(row['bytes'], int)
            private_components += verify_private(row, 'privatePath', 'bytes', 'sha256')
        else:
            assert row['availability'] == 'not-located-in-scoped-capture-roots' and row['bytes'] is None
            unknown += 1
    assert unknown == 13
    for path in directory.iterdir():
        assert path.is_file() and path.stat().st_size < 50000, path.name
    print(json.dumps({'scope': 'static metadata/capture verification only',
                      'jsonOldValuesPreserved': json_count, 'readmeEditsReplayed': readme_count,
                      'stdoutArchives': 12, 'stdoutBytes': 22791, 'logReferences': 164,
                      'distinctLogHashes': 143, 'componentHashReferences': 47,
                      'componentBytesKnown': 34, 'componentBytesUnknown': unknown,
                      'privateOldReceiptsVerified': private_receipts,
                      'privateStdoutVerified': private_stdout, 'privateLogArtifactsVerified': private_logs,
                      'privateComponentContentsVerified': private_components}, ensure_ascii=False))


if __name__ == '__main__':
    main()
