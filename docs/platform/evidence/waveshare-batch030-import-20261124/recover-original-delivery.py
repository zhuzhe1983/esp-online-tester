#!/usr/bin/env python3
"""Verify batch030 against public source29 in a disposable private Git index."""
import argparse
import hashlib
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[4]
BASE = '78ac0278f7418556f4afa3327b5b6ce28ae453c9'
TREE = '3652fc8251fdd129a285ef4a508800f04cb44f7c'
PATCH_SHA256 = '0e9b7f955ca673746b36bb5cbd82cc0da3b21a5cdd3c4b7ad40439dfbc54f4ee'
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--patch', type=Path, required=True)
parser.add_argument('--output', type=Path, help='New ignored directory for the original evidence; omit to check only.')
parser.add_argument('--local-ref', help='Create a new local unpublished refs/heads/... delivery ref for continuing batch031; never push it.')
args = parser.parse_args()
patch = args.patch.resolve()
with patch.open('rb') as source:
    assert patch.stat().st_size == 61978770
    assert hashlib.file_digest(source, 'sha256').hexdigest() == PATCH_SHA256
objects = subprocess.check_output(['git', 'rev-parse', '--git-path', 'objects'], cwd=ROOT, text=True).strip()
objects = (ROOT / objects).resolve()
if args.local_ref:
    assert args.local_ref.startswith('refs/heads/')
    subprocess.run(['git', 'check-ref-format', args.local_ref], cwd=ROOT, check=True)
    assert subprocess.run(['git', 'show-ref', '--verify', '--quiet', args.local_ref], cwd=ROOT).returncode == 1
with tempfile.TemporaryDirectory(prefix='batch030-recovery-') as temporary:
    scratch = Path(temporary)
    (scratch / 'objects').mkdir()
    env = dict(os.environ, GIT_INDEX_FILE=str(scratch / 'index'),
               GIT_OBJECT_DIRECTORY=str(objects if args.local_ref else scratch / 'objects'),
               GIT_ALTERNATE_OBJECT_DIRECTORIES=str(objects))
    def git(*command, data=None):
        return subprocess.check_output(['git', *command], input=data, cwd=ROOT, env=env)
    git('read-tree', BASE)
    git('apply', '--cached', str(patch))
    assert git('write-tree').decode().strip() == TREE
    if args.local_ref:
        commit = git('commit-tree', TREE, '-p', BASE, data=b'Reconstruct original Waveshare batch030 locally; do not publish raw delivery ancestry.\n').decode().strip()
        git('update-ref', args.local_ref, commit, '0' * 40)
    if args.output:
        output = args.output.resolve()
        output.mkdir(parents=True, exist_ok=False)
        paths = ['docs/platform/evidence/waveshare-final-20261124',
                 'docs/platform/evidence/waveshare-batch030-models-20261124',
                 'docs/research/waveshare-s3-industrial-2026-11-24/batch-04-catalog-recovery']
        with subprocess.Popen(['git', 'archive', TREE, *paths], cwd=ROOT, env=env, stdout=subprocess.PIPE) as process:
            with tarfile.open(fileobj=process.stdout, mode='r|') as archive:
                archive.extractall(output, filter='data')
            assert process.wait() == 0
print('Verified original batch030 tree; no public refs or worktree files were modified. Any requested delivery ref is local and must remain unpublished.')
