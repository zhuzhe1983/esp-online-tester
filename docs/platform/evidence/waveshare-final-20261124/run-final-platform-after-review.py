#!/usr/bin/env python3
"""Bind the final platform gate to catalogs, all16 assets and frozen Rust."""
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time
from retained_evidence import read_bytes, read_json

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
CATALOGS = sorted(['web/platform/catalog/products.json', 'web/platform/catalog/firmwares.json',
                   'web/platform/catalog/firmware-watchlist.json', 'web/platform/knowledge/index.json'])
CODE_FREEZE = read_json(OUT / 'rust-source-freeze.json')
MANIFEST = json.loads((ROOT / 'docs/research/waveshare-s3-industrial-2026-11-24/batch-04/manifest.json').read_text())
IDS = [pid for item in MANIFEST['items'] for pid in item['modelIds']]
assert len(IDS) == 16 and len(set(IDS)) == 16


def digest(path):
    return hashlib.sha256(read_bytes(path)).hexdigest()


def freeze(paths):
    body = ''.join(f'{digest(ROOT / path)}  {path}\n' for path in sorted(paths))
    return dict(fileCount=len(paths), manifestSha256=hashlib.sha256(body.encode()).hexdigest(), sha256sum=body)


def runtime_freeze():
    paths = subprocess.check_output(['git', 'ls-files'], cwd=ROOT, text=True).splitlines()
    paths = sorted(path for path in paths if path.endswith('.rs') or Path(path).name in ('Cargo.toml', 'Cargo.lock'))
    actual = freeze(paths)
    return dict(fileCount=actual['fileCount'], manifestSha256=actual['manifestSha256'],
                matchesFrozenSource=actual == CODE_FREEZE and actual['fileCount'] == 855)


def snapshot():
    paths = set(CATALOGS + ['web/platform/locales/en.js', 'web/wasm/esp32sim.wasm',
                           'web/platform/waveshare-board-families.js'])
    for pid in IDS:
        directory = ROOT / 'web/platform/assets' / pid
        paths.update(str(path.relative_to(ROOT)) for path in directory.rglob('*') if path.is_file())
        paths.add('web/platform/knowledge/' + pid + '.json')
    paths.update(item['fragment'] for item in MANIFEST['items'])
    return {path: dict(bytes=(ROOT / path).stat().st_size, sha256=digest(ROOT / path)) for path in sorted(paths)}


def save(record):
    path = OUT / 'final-platform-after-model-review.json'
    temporary = path.with_suffix('.json.tmp')
    temporary.write_text(json.dumps(record, indent=2, ensure_ascii=False) + '\n')
    temporary.replace(path)


def file_snapshot(paths):
    return {path: dict(bytes=(ROOT / path).stat().st_size, sha256=digest(ROOT / path)) for path in sorted(paths)}


catalog_freeze = freeze(CATALOGS)
(OUT / 'catalog-freeze.json').write_text(json.dumps(catalog_freeze, indent=2) + '\n')
files = sorted(str(path.relative_to(ROOT)) for path in (ROOT / 'tools').glob('platform-*-test.mjs'))
harness_paths = files + [str(Path(__file__).resolve().relative_to(ROOT))]
argv = ['node', '--test', '--test-concurrency=2', *files]
env = dict(os.environ)
env.pop('UPDATE_GOLDENS', None)
env['ESP32SIM_ROM_DIR'] = str(ROOT / 'web/wasm/fw')
env['ROM'] = str(ROOT / 'web/wasm/fw')
before = snapshot()
runtime_before = runtime_freeze()
assert runtime_before['matchesFrozenSource'], 'Rust/Cargo differs from frozen CI source before testing'
record = dict(schemaVersion=2, command='node --test --test-concurrency=2 tools/platform-*-test.mjs',
              argv=argv, testFileCount=len(files), cwd='$REPO',
              environment=dict(ESP32SIM_ROM_DIR='$REPO/web/wasm/fw', ROM='$REPO/web/wasm/fw', UPDATE_GOLDENS='unset'),
              node=subprocess.check_output(['node', '--version'], cwd=ROOT, text=True).strip(),
              startedAt=dt.datetime.now(dt.timezone.utc).isoformat(), complete=False,
              catalogFreeze=catalog_freeze, runtimeBefore=runtime_before, before=before,
              frozenRuntimeSourceFile='rust-source-freeze.json', frozenRuntimeSourceFileSha256=digest(OUT / 'rust-source-freeze.json'),
              harnessBefore=file_snapshot(harness_paths),
              priorAttempt='final-platform-after-model-review-attempt1-review.json',
              scope='Final platform tests after three source-photo material/registration revisions. Full16 models, source assets, knowledge, fragments, four catalogs, English dictionary, production WASM and runtime identity are compared before/after. Node/Worker/CPU geometry does not establish browser WebGL or physical hardware acceptance.')
save(record)
print(f'START final platform gate: {len(files)} test files; 855 frozen Rust/Cargo inputs; {len(before)} retained data/assets.', flush=True)
started = time.monotonic()
log_path = OUT / 'final-platform-after-model-review.log'
with log_path.open('w') as log:
    process = subprocess.Popen(argv, cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                               text=True, errors='replace', bufsize=1)
    for line in process.stdout:
        log.write(line.replace(str(ROOT), '$REPO'))
        log.flush()
    status = process.wait()
# Save the actual process outcome before checking identities so any concurrent
# edits remain a distinct observation and cannot hide a real failed test.
record.update(exitCode=status, durationSeconds=round(time.monotonic() - started, 6),
              finishedAt=dt.datetime.now(dt.timezone.utc).isoformat(), log=log_path.name,
              logSha256=digest(log_path), logBytes=log_path.stat().st_size)
text = log_path.read_text()
record['totals'] = {name: int(value) for name, value in re.findall(r'^(?:#|ℹ) (tests|suites|pass|fail|cancelled|skipped|todo) (\d+)\s*$', text, re.M)}
record['testsPassed'] = status == 0 and record['totals'].get('tests') == 507 and record['totals'].get('pass') == 507 and record['totals'].get('fail') == 0
save(record)
record.update(after=snapshot(), runtimeAfter=runtime_freeze(), harnessAfter=file_snapshot(harness_paths), complete=True)
record['dataAssetsUnchanged'] = record['before'] == record['after']
record['runtimeUnchanged'] = record['runtimeBefore'] == record['runtimeAfter'] and record['runtimeAfter']['matchesFrozenSource']
record['harnessUnchanged'] = record['harnessBefore'] == record['harnessAfter']
record['unchanged'] = record['dataAssetsUnchanged'] and record['runtimeUnchanged'] and record['harnessUnchanged']
record['changedPaths'] = [path for path in sorted(set(record['before']) | set(record['after'])) if record['before'].get(path) != record['after'].get(path)]
record['changedHarnessPaths'] = [path for path in sorted(set(record['harnessBefore']) | set(record['harnessAfter'])) if record['harnessBefore'].get(path) != record['harnessAfter'].get(path)]
record['passed'] = record['testsPassed'] and record['unchanged']
save(record)
print(json.dumps({key: record[key] for key in ['exitCode', 'durationSeconds', 'totals', 'unchanged', 'passed']}, ensure_ascii=False), flush=True)
raise SystemExit(0 if record['passed'] else status or 1)
