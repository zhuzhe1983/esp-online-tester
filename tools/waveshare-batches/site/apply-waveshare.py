#!/usr/bin/env python3
"""Verify and apply the ordered Waveshare git diff chain from this directory."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import sys


def git(*args, env=None, data=None):
    return subprocess.check_output(['git', *args], input=data, env=env)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--check', action='store_true', help='Validate all remaining patches without changing the worktree or real index')
    mode.add_argument('--apply', action='store_true', help='Validate first, then apply in order with git apply --index')
    args = parser.parse_args()
    folder = Path(__file__).resolve().parent
    manifest = json.loads((folder / 'manifest.json').read_text())
    root = Path(git('rev-parse', '--show-toplevel').decode().strip())
    os.chdir(root)
    if subprocess.run(['git', 'diff', '--quiet']).returncode:
        raise RuntimeError('Tracked worktree changes are not staged. Save them yourself before applying this exact patch chain.')
    tree = git('write-tree').decode().strip()
    trees = [manifest['baseTree']] + [b['tree'] for b in manifest['batches']]
    if tree not in trees:
        raise RuntimeError('The current index is not the expected base or a completed batch. Start a clean branch at ' + manifest['baseCommit'] + ', or integrate your own changes separately.')
    done = trees.index(tree)
    pending = manifest['batches'][done:]
    print(f'Current tree matches batch {done:03d}; {len(pending)} pending batches.')
    if not pending:
        return
    with tempfile.TemporaryDirectory(prefix='waveshare-apply-') as temporary:
        env = dict(os.environ, GIT_INDEX_FILE=str(Path(temporary) / 'index'))
        git('read-tree', tree, env=env)
        paths = []
        for b in pending:
            source = folder / b['filename']
            if source.exists():
                patch = source.read_bytes()
            else:
                source = folder / b['gzipFilename']
                compressed = source.read_bytes()
                if len(compressed) != b['gzipBytes'] or hashlib.sha256(compressed).hexdigest() != b['gzipSha256']:
                    raise RuntimeError('Compressed SHA-256 mismatch: ' + source.name)
                patch = gzip.decompress(compressed)
            if len(patch) != b['bytes'] or hashlib.sha256(patch).hexdigest() != b['sha256']:
                raise RuntimeError('Patch SHA-256 mismatch: ' + b['filename'])
            path = Path(temporary) / b['filename']
            path.write_bytes(patch)
            git('apply', '--cached', '--check', str(path), env=env)
            git('apply', '--cached', str(path), env=env)
            if git('write-tree', env=env).decode().strip() != b['tree']:
                raise RuntimeError('Resulting tree mismatch: ' + b['filename'])
            paths.append((b, path))
            print('Verified ' + b['filename'] + ' -> ' + b['tree'])
        if args.check:
            print('All remaining batches verified. Worktree and real index unchanged.')
            return
        for b, path in paths:
            # Git refuses to overwrite conflicting untracked files. No resets,
            # commits, pushes, stashes or automatic conflict resolutions occur.
            git('apply', '--check', '--index', str(path))
            git('apply', '--index', str(path))
            if git('write-tree').decode().strip() != b['tree']:
                raise RuntimeError('Unexpected real index after ' + b['filename'])
            print('Applied ' + b['filename'])
        print('Applied successfully. Changes are staged for your review and commit; rebuild WASM using tools/wasm-build.sh.')


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as exc:
        print('Stopped: ' + str(exc), file=sys.stderr)
        sys.exit(1)
