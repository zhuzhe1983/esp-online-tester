#!/usr/bin/env python3
"""Retain commands, complete logs, exit status and source identity for final gates."""
import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import platform
import shlex
import subprocess
import time

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--repo', type=Path, required=True)
p.add_argument('--output', type=Path, required=True)
p.add_argument('--lane', choices=['wasm-build', 'host', 'wasm-tests', 'wasm-jit'], required=True)
p.add_argument('--platform-ready', type=Path, help='Wait for the coordinator gate before catalog/model-dependent platform checks')
p.add_argument('--resume-from', help='Restart at a failed step, retaining all prior attempts and unaffected preceding steps')
p.add_argument('--resume-note', help='Explain the scoped correction before resuming')
a = p.parse_args()
repo = a.repo.resolve()
out = a.output.resolve()
out.mkdir(parents=True, exist_ok=True)

def git(*args):
    return subprocess.check_output(['git', *args], cwd=repo, text=True).strip()

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def source_manifest():
    paths = sorted(path for path in git('ls-files').splitlines()
                   if path.endswith('.rs') or Path(path).name in ('Cargo.toml', 'Cargo.lock'))
    body = ''.join(f'{digest(repo / path)}  {path}\n' for path in paths)
    return {'fileCount': len(paths), 'manifestSha256': hashlib.sha256(body.encode()).hexdigest(), 'sha256sum': body}

freeze = source_manifest()
freeze_path = out / 'rust-source-freeze.json'
if freeze_path.exists():
    assert json.loads(freeze_path.read_text()) == freeze, 'Source differs from frozen runtime'
else:
    freeze_path.write_text(json.dumps(freeze, indent=2) + '\n')

def item(key, command):
    return {'id': key, 'command': command}

steps = {
    'wasm-build': [
        item('wasm-clippy', 'cargo clippy --release --target wasm32-unknown-unknown -p esp32sim-wasm --features jit-tests -- -D warnings'),
        item('production-wasm', 'bash tools/wasm-build.sh'),
    ],
    'host': [
        item('host-clippy', 'cargo clippy --workspace --all-targets -- -D warnings'),
        item('host-workspace-build', 'cargo build --release --workspace'),
        item('host-workspace-tests', 'cargo test --release --workspace -- --include-ignored --skip external_'),
        item('native-vq-tests', 'ESP32SIM_VQ_NATIVE=1 cargo test --release -p esp32s3 --test machine --test virtual_stops'),
        item('candidate-perf-bins', 'cargo build --release --bins'),
    ],
    'wasm-tests': [
        item('wasm-sections', 'node tools/check-wasm-sections.mjs web/wasm/esp32sim.wasm'),
        item('wasm-page-manifests', 'node tools/wasm-test.mjs hello c3-hello c6-hello c6-xiao p4-hello p4-c6-hosted p4-lcd35-synth c6-energy-scan c6-contiki c6-contiki-net c6-rpl-net panel tdisplay-prices tdisplay-schematik'),
        item('platform-contracts', 'node --test --test-concurrency=2 tools/platform-*-test.mjs'),
        item('platform-real-recording', 'node tools/platform-recording-test.mjs --real'),
        item('stopwatch-wasm', 'node tools/wasm-test.mjs demo-stopwatch'),
        item('stopwatch-hardware-diagnostics', 'node tools/probe-hardware-diagnostics.mjs tools/fixtures/stopwatch-hardware-checks.json "$FINAL_EVIDENCE/stopwatch-hardware-checks.json"'),
        item('timing-abi', 'node wasm/tests/timing-api.mjs web/wasm/esp32sim.wasm'),
        item('ble-abi', 'node wasm/tests/ble-api.mjs web/wasm/esp32sim.wasm web/wasm/fw/public web/wasm/fw/esp32c3_rev3_rom.elf'),
        item('ble-connection', 'node wasm/tests/ble-connection.mjs web/wasm/esp32sim.wasm web/wasm/fw/public web/wasm/fw/esp32c3_rev3_rom.elf'),
        item('worker-pacing', 'node tools/worker-pacing-test.mjs'),
        item('worker-channel', 'node tools/worker-channel-test.mjs'),
        item('touch-stopwatch-ui', 'node --test tools/touch-input-test.mjs tools/stopwatch-ui-test.mjs'),
        item('wasm-build-policy', 'node --test tools/wasm-build-policy.test.mjs'),
        item('browser-benchmark-js', 'node --test tools/browser-benchmark/*.test.mjs'),
        item('browser-benchmark-python', "python3 -m unittest discover -s tools/browser-benchmark -p 'test_*.py'"),
        item('network-helper', 'node tools/network-helper-test.mjs'),
        item('network-relay', 'node --test tools/network-relay-test.mjs'),
        item('x5-receipt-tests', 'node --test docs/evidence/perf-x5-2026-09-22/verify-results.test.mjs'),
        item('x5-receipts', 'node docs/evidence/perf-x5-2026-09-22/verify-results.mjs'),
        item('m1-receipt-tests', 'node --test docs/evidence/perf-x5-2026-09-22/m1-before-after/verify.test.mjs'),
        item('m1-receipts', 'node docs/evidence/perf-x5-2026-09-22/m1-before-after/verify.mjs'),
        item('historical-receipt-tests', 'node --test docs/evidence/perf-x5-2026-09-22/historical-before-after/verify.test.mjs'),
        item('historical-receipts', 'node docs/evidence/perf-x5-2026-09-22/historical-before-after/verify.mjs'),
        item('retained-ci-privacy', "node tools/check-evidence-privacy.mjs docs/evidence 'docs/platform/evidence/seeed*'"),
    ],
    'wasm-jit': [
        item('wasm-jit-differential', 'bash tools/wasm-jit-test.sh'),
        item('wasm-profile-build', '. ./tools/wasm-rustflags.sh; cargo build --release --target wasm32-unknown-unknown -p esp32sim-wasm --features jit-profile,cpu-profile'),
    ],
}[a.lane]

env = dict(os.environ)
env.pop('UPDATE_GOLDENS', None)
env.update(CARGO_BUILD_JOBS='2', CARGO_INCREMENTAL='0', ESP32SIM_ROM_DIR=str(repo / 'web/wasm/fw'), FINAL_EVIDENCE=str(out))
record = {
    'schemaVersion': 1, 'lane': a.lane, 'sourceCommitAtStart': git('rev-parse', 'HEAD'),
    'runtimeSourceSha256': freeze['manifestSha256'], 'runtimeSourceFileCount': freeze['fileCount'],
    'environment': {'architecture': platform.machine(), 'system': platform.system(),
                    'logicalCpus': os.cpu_count(), 'cargoBuildJobs': 2, 'cargoIncremental': 0,
                    'rustc': subprocess.check_output(['rustc', '--version'], env=env, text=True).strip(),
                    'cargo': subprocess.check_output(['cargo', '--version'], env=env, text=True).strip(),
                    'node': subprocess.check_output(['node', '--version'], env=env, text=True).strip(),
                    'updateGoldens': False, 'releaseProfileOverrides': False},
    'limitations': ['Linux x86_64 only; no native AArch64 CI execution.', 'Node/Worker checks do not establish browser WebGL or physical hardware acceptance.'],
    'steps': [], 'complete': False,
}
report = out / (a.lane + '.json')
start_index = 0
if a.resume_from:
    assert a.resume_note and report.exists()
    start_index = next(i for i, step in enumerate(steps) if step['id'] == a.resume_from)
    record = json.loads(report.read_text())
    assert record['runtimeSourceSha256'] == freeze['manifestSha256']
    retained = record['steps'][:start_index]
    assert len(retained) == start_index and all(row.get('exitCode') == 0 for row in retained)
    history = record.setdefault('priorAttempts', [])
    for row in record['steps'][start_index:]:
        assert 'exitCode' in row, 'Cannot resume a running attempt'
        attempt = 1 + sum(old['id'] == row['id'] for old in history)
        old_path = out / row['log']
        historical_name = old_path.stem + '-attempt' + str(attempt) + old_path.suffix
        old_path.rename(out / historical_name)
        history.append({**row, 'log': historical_name})
    record['steps'] = retained
    record.setdefault('resumptions', []).append({'at': dt.datetime.now(dt.timezone.utc).isoformat(), 'from': a.resume_from, 'reason': a.resume_note,
                                                'englishDictionarySha256': digest(repo / 'web/platform/locales/en.js')})

def save():
    temp = report.with_suffix('.json.tmp')
    temp.write_text(json.dumps(record, indent=2, ensure_ascii=False) + '\n')
    temp.replace(report)

save()
for step in steps[start_index:]:
    if step['id'] == 'platform-contracts' and a.platform_ready:
        if not a.platform_ready.exists():
            print('WAIT catalog/model final integration gate', flush=True)
        while not a.platform_ready.exists():
            time.sleep(5)
    assert source_manifest() == freeze, 'Runtime changed before ' + step['id']
    filename = a.lane + '-' + step['id'] + '.log'
    path = out / filename
    row = {**step, 'log': filename, 'startedAt': dt.datetime.now(dt.timezone.utc).isoformat()}
    record['steps'].append(row)
    save()
    print('START ' + a.lane + '/' + step['id'], flush=True)
    started = time.monotonic()
    with path.open('w') as log:
        proc = subprocess.Popen(['bash', '-c', step['command']], cwd=repo, env=env,
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, errors='replace', bufsize=1)
        for line in proc.stdout:
            line = line.replace(str(repo), '$REPO').replace(str(out), '$FINAL_EVIDENCE')
            log.write(line)
            log.flush()
        status = proc.wait()
    row.update(exitCode=status, durationSeconds=round(time.monotonic()-started, 3),
               finishedAt=dt.datetime.now(dt.timezone.utc).isoformat(), logSha256=digest(path), logBytes=path.stat().st_size)
    assert source_manifest() == freeze, 'Runtime changed during ' + step['id']
    save()
    print(f"END {a.lane}/{step['id']}: exit={status} seconds={row['durationSeconds']}", flush=True)
    if status:
        raise SystemExit(status)
if a.lane in ('wasm-build', 'wasm-tests', 'wasm-jit'):
    production = repo / 'web/wasm/esp32sim.wasm'
    record['productionWasm'] = {'bytes': production.stat().st_size, 'sha256': digest(production)}
record['complete'] = True
record['passed'] = all(row['exitCode'] == 0 for row in record['steps'])
save()
print(f'COMPLETE {a.lane}: {len(steps)} passed', flush=True)
