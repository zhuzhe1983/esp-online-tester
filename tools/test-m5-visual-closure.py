#!/usr/bin/env python3
"""Exercise closure validator rejection boundaries without changing source assets."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--source-root', type=Path, default=ROOT)
p.add_argument('--matrix', type=Path, default=ROOT/'docs/platform/evidence/m5-visual-closure-20261126/matrix.json')
p.add_argument('--integrated-positive', help='Optional already-integrated SKU to exercise actual tree reads')
p.add_argument('--output', type=Path)
a = p.parse_args()
raw = a.matrix.read_bytes()
base = json.loads(raw)
rows = []

def entry(matrix, identity):
    return next(e for e in matrix['entries'] if e['id'] == identity)


def run(name, matrix, identity, expected, snapshot=True, source_root=None, expected_message=None):
    with tempfile.TemporaryDirectory(prefix='m5-closure-control-') as folder:
        path = Path(folder)/'matrix.json'
        path.write_text(json.dumps(matrix, ensure_ascii=False, indent=2)+'\n')
        command = [sys.executable, str(ROOT/'tools/verify-m5-visual-closure.py'), '--source-root', str(source_root or a.source_root), '--matrix', str(path), '--ids', identity]
        if snapshot:
            command.append('--snapshot')
        result = subprocess.run(command, capture_output=True, text=True)
    assert result.returncode == expected, (name, result.returncode, result.stdout, result.stderr)
    if expected_message:
        assert expected_message in result.stdout+result.stderr, (name, result.stdout, result.stderr)
    try:
        document = json.loads(result.stdout)
        outcome = document.get('errors') or document.get('pending') or 'passed'
    except json.JSONDecodeError:
        outcome = result.stderr.strip()
    rows.append({'case': name, 'expectedExit': expected, 'actualExit': result.returncode, 'outcome': outcome})

run('valid contribution snapshot', base, 'm5stack-c145', 0)
run('valid P2 input-route-only equivalence', base, 'm5stack-k016-p2', 0)
run('valid Core2 v1.1 invisible input equivalence', base, 'm5stack-k010-v11', 0)
if a.integrated_positive:
    run('valid integrated working tree subset', base, a.integrated_positive, 0, snapshot=False)
with tempfile.TemporaryDirectory(prefix='m5-closure-duplicate-') as folder:
    fixture = Path(folder)
    screening = base['screening']['path']
    target = fixture/screening; target.parent.mkdir(parents=True)
    screen_file = a.source_root/screening
    screen_raw = screen_file.read_bytes() if screen_file.is_file() else subprocess.check_output(['git', '-C', str(a.source_root), 'show', base['screening']['sourceRevision']+':'+screening])
    target.write_bytes(screen_raw)
    target = fixture/entry(base, 'm5stack-c145')['model']['path']; target.parent.mkdir(parents=True)
    target.write_text('{"parts":[{"controlId":"button","controlId":"button"}]}\n')
    run('duplicate same-value controlId rejected before hash acceptance', base, 'm5stack-c145', 1,
        snapshot=False, source_root=fixture, expected_message='Duplicate JSON key: controlId')
m = copy.deepcopy(base); m['entries'].pop(); run('missing denominator entry rejected even for a selected subset', m, 'm5stack-c145', 1)
m = copy.deepcopy(base); entry(m, 'm5stack-c145')['model']['sha256'] = '0'*64; run('wrong final model digest rejected', m, 'm5stack-c145', 1)
m = copy.deepcopy(base); entry(m, 'm5stack-c145')['model']['binarySha256'] = '0'*64; run('wrong geometry identity rejected', m, 'm5stack-c145', 1)
m = copy.deepcopy(base); entry(m, 'm5stack-c145')['proofs'][0]['sha256'] = '0'*64; run('changed proof digest rejected', m, 'm5stack-c145', 1)
m = copy.deepcopy(base)
for proof in entry(m, 'm5stack-c145')['proofs']:
    proof['modelHashBindings'] = []
run('CPU receipt without final model binding rejected', m, 'm5stack-c145', 1)
m = copy.deepcopy(base); e = entry(m, 'm5stack-c145'); e['proofs'] = [{'path': base['screening']['path'], 'sourceRevision': base['screening']['sourceRevision'], 'sha256': base['screening']['sha256'], 'modelHashBindings': [], 'cpuAppearances': []}]; run('original screening cannot substitute for final evidence', m, 'm5stack-c145', 1)
m = copy.deepcopy(base); entry(m, 'm5stack-k005')['retainedImages'][0]['sha256'] = '0'*64; run('changed retained image rejected', m, 'm5stack-k005', 1)
m = copy.deepcopy(base); entry(m, 'm5stack-k148')['sharedHostIdentity']['firmwareTargetId'] = 'm5stack-invented'; run('wrong Kit shared identity rejected', m, 'm5stack-k148', 1)
m = copy.deepcopy(base); entry(m, 'm5stack-k140')['decisionBasis'] = None; run('source-limited result without decision basis rejected', m, 'm5stack-k140', 1)
m = copy.deepcopy(base); e = entry(m, 'm5stack-k151'); e['closureState'] = 'closed-scoped'; e.pop('requiredEvidenceAssertions', None); run('K151 cannot close without nested inspection proof', m, 'm5stack-k151', 1)
if entry(base, 'm5stack-k151').get('assembledEquivalence'):
    m = copy.deepcopy(base); entry(m, 'm5stack-k151')['assembledEquivalence']['priorModelSha256'] = '0'*64; run('incorrect assembled-equivalence chain rejected', m, 'm5stack-k151', 1)
m = copy.deepcopy(base); entry(m, 'm5stack-k016-p2')['assembledEquivalence']['removedJsonPointers'] = ['/parts/4/byteOffset']; run('geometry byte offsets cannot be exempted as metadata', m, 'm5stack-k016-p2', 1, expected_message='Equivalence cannot remove visual geometry/material/texture data')
m = copy.deepcopy(base); entry(m, 'm5stack-k132')['assembledEquivalence']['semanticDigestPairs'][0]['priorProofPath'] = entry(m, 'm5stack-k132')['assembledEquivalence']['proofPath']; run('separate prior loader signature cannot bind the final model', m, 'm5stack-k132', 1, expected_message='Cross-proof visual signature lacks its exact old/final model binding')
m = copy.deepcopy(base); e = entry(m, 'm5stack-k140'); e['visibleGeometryDelta']['previousBinarySha256'] = e['model']['binarySha256']; run('real geometry delta cannot masquerade as unchanged binary', m, 'm5stack-k140', 1, expected_message='Real geometry delta cannot use a metadata-only equivalence exemption')
m = copy.deepcopy(base); entry(m, 'm5stack-k140')['visibleGeometryDelta']['finalCpuRecords'] = entry(m, 'm5stack-k140')['visibleGeometryDelta']['finalCpuRecords'][:1]; run('geometry delta needs all three final CPU appearances', m, 'm5stack-k140', 1, expected_message='Geometry delta lacks final model-bound CPU view records for three appearances')
m = copy.deepcopy(base); entry(m, 'm5stack-k140')['visibleGeometryDelta']['finalCpuRecords'][0]['renderReceiptPointer'] = '/cpu/images/0/renderReceipt'; run('old geometry CPU record cannot certify new geometry', m, 'm5stack-k140', 1, expected_message='Geometry delta CPU record does not bind the final model')
m = copy.deepcopy(base); entry(m, 'm5stack-k140')['historicalVisualStages'][0]['retainedImages'][0]['sha256'] = '0'*64; run('superseded source history remains hash checked', m, 'm5stack-k140', 1, expected_message='Historical review image changed')
receipt = {'matrixSha256': hashlib.sha256(raw).hexdigest(), 'validatorSha256': hashlib.sha256((ROOT/'tools/verify-m5-visual-closure.py').read_bytes()).hexdigest(), 'passed': True, 'cases': rows, 'scope': 'Read-only guard checks. No model/catalog/renderer mutation, Cargo or rendering.'}
if a.output:
    a.output.parent.mkdir(parents=True, exist_ok=True)
    a.output.write_text(json.dumps(receipt, ensure_ascii=False, indent=2)+'\n')
print(json.dumps({'passed': True, 'cases': len(rows), 'matrixSha256': receipt['matrixSha256']}, indent=2))
