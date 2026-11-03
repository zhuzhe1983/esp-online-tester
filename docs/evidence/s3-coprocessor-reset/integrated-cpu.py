#!/usr/bin/env python3
"""EX248 fixed-work integration comparison; retain hashes, never raw guest output."""
import argparse
import datetime
import hashlib
import json
import os
import pathlib
import platform
import re
import resource
import statistics
import subprocess
import time


def sha(data):
    return hashlib.sha256(data).hexdigest()


def source_fingerprint(repo, revision):
    rows = subprocess.check_output(['git', 'ls-tree', '-r', revision], cwd=repo).splitlines()
    selected = [row for row in rows if row.split(b'\t', 1)[1].endswith(
        (b'.rs', b'Cargo.toml', b'Cargo.lock'))]
    return sha(b'\n'.join(selected) + b'\n')


parser = argparse.ArgumentParser(description=__doc__)
for option in ('base', 'candidate', 'base-revision', 'candidate-revision', 'assets', 'repo', 'output'):
    parser.add_argument('--' + option, required=True)
parser.add_argument('--pairs', type=int, default=5)
parser.add_argument('--workloads', default='s3-hello,pocket-tank,c3-hello,c6-hello')
args = parser.parse_args()
root = pathlib.Path(args.repo).resolve()
assets = pathlib.Path(args.assets).resolve()
output = pathlib.Path(args.output)
result = {
    'experiment': 'EX248',
    'date_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'base_revision': args.base_revision,
    'candidate_revision': args.candidate_revision,
    'source_fingerprints_sha256': {
        'base': source_fingerprint(root, args.base_revision),
        'candidate': source_fingerprint(root, args.candidate_revision),
    },
    'source_fingerprint_method': 'SHA-256 of newline-terminated git ls-tree -r records whose paths end in .rs, Cargo.toml or Cargo.lock, in Git tree order',
    'build_command': 'cargo +stable build --release --locked --bin esp32sim --bin esp32sim-c3 --bin esp32sim-c6',
    'method': {
        'pairs': args.pairs,
        'warmup': 'one run per arm per workload, omitted from medians but retained in runs',
        'order': 'warmup base/candidate; measured pairs base/candidate then candidate/base, alternating',
        'user_cpu': 'resource.RUSAGE_CHILDREN ru_utime delta around one subprocess.run; no other children run during each interval',
        'output': 'SHA-256 of unmodified stdout from --console uart0; counts parsed from stderr stop report',
        'load': 'os.getloadavg before and after each run; machine-wide load averages, not CPU utilization',
    },
    'environment': {
        'system': platform.system(),
        'kernel_release': platform.release(),
        'architecture': platform.machine(),
        'logical_cpus': os.cpu_count(),
        'cpu': subprocess.check_output(['sysctl', '-n', 'machdep.cpu.brand_string'], text=True).strip(),
        'os_version': subprocess.check_output(['sw_vers', '-productVersion'], text=True).strip(),
        'rustc': subprocess.check_output(['rustc', '+stable', '-vV'], text=True).strip().splitlines(),
    },
    'inputs': {},
    'binaries': {},
    'workloads': {},
    'privacy': 'No raw consoles, process inventories, machine identity, local paths or session data retained. Input labels and command paths use $ASSETS/$BASE/$CANDIDATE placeholders. Original output bytes are hashed before omission.',
}


def save():
    output.write_text(json.dumps(result, indent=2) + '\n')


def input_file(relative):
    path = assets / relative
    data = path.read_bytes()
    result['inputs'][relative] = {'bytes': len(data), 'sha256': sha(data)}
    return str(path)


workloads = {
    's3-hello': ('hello', 'esp32sim'),
    'pocket-tank': ('pocket-tank', 'esp32sim'),
    'c3-hello': ('c3-hello', 'esp32sim-c3'),
    'c6-hello': ('c6-hello', 'esp32sim-c6'),
}
for workload in args.workloads.split(','):
    manifest_name, executable = workloads[workload]
    manifest_path = assets / (manifest_name + '.json')
    manifest = json.loads(manifest_path.read_text())
    input_file(manifest_name + '.json')
    # C3/C6 manifest board names select the chip in the web loader. Their native
    # executables already select it. Neither native chip accepts --psram-mb;
    # --board is rejected on C3 and expects a physical board name on C6.
    options = ['--board', manifest['board'], '--psram-mb', str(manifest['psram_mb'])] if executable == 'esp32sim' else []
    options += ['--boot', 'rom', '--flash-mb', str(manifest['flash_mb'])]
    for kind, relative in manifest['files'].items():
        options.extend(['--' + kind, input_file(relative)])
    for address, relative in manifest.get('flash_at', {}).items():
        options.extend(['--flash-at', address + '=' + input_file(relative)])
    options.extend(['--max-seconds', '30', '--no-dump', '--console', 'uart0'])
    row = {'command': [executable] + [value.replace(str(assets), '$ASSETS') for value in options], 'runs': []}
    result['workloads'][workload] = row
    binaries = {}
    for arm in ('base', 'candidate'):
        binary = pathlib.Path(getattr(args, arm)).resolve() / executable
        data = binary.read_bytes()
        result['binaries'][arm + '/' + executable] = {'bytes': len(data), 'sha256': sha(data)}
        binaries[arm] = binary
    for pair in range(-1, args.pairs):
        order = ['base', 'candidate'] if pair < 0 or pair % 2 == 0 else ['candidate', 'base']
        for arm in order:
            load_before = os.getloadavg()
            before = resource.getrusage(resource.RUSAGE_CHILDREN)
            started = time.monotonic()
            run = subprocess.run([str(binaries[arm])] + options, capture_output=True, timeout=180)
            wall = time.monotonic() - started
            after = resource.getrusage(resource.RUSAGE_CHILDREN)
            load_after = os.getloadavg()
            stderr = run.stderr.decode('utf-8', errors='replace')
            report = re.search(r'\[emu\] stop: (.*?) — (.*?) insns in .*?; emulated ([0-9.]+)s \((\d+) cycles\); (\d+) exceptions, (\d+) interrupts', stderr)
            if run.returncode or not report:
                raise RuntimeError(f'{workload}/{arm}: exit {run.returncode} or missing complete stop report')
            stop, counts, seconds, cycles, exceptions, interrupts = report.groups()
            insns = [int(value) for value in re.findall(r'core\d+ (\d+)', counts)] or [int(counts)]
            if stop != 'Halted' or float(seconds) != 30:
                raise RuntimeError(f'{workload}/{arm}: expected Halted at 30 seconds, got {stop}/{seconds}')
            row['runs'].append({
                'pair': pair,
                'arm': arm,
                'user_s': round(after.ru_utime - before.ru_utime, 6),
                'system_s': round(after.ru_stime - before.ru_stime, 6),
                'wall_s': round(wall, 6),
                'instructions': insns,
                'cycles': int(cycles),
                'emulated_seconds': float(seconds),
                'exceptions': int(exceptions),
                'interrupts': int(interrupts),
                'console_bytes': len(run.stdout),
                'console_sha256': sha(run.stdout),
                'stop': stop,
                'exit': run.returncode,
                'load_before': list(load_before),
                'load_after': list(load_after),
            })
            save()
        print(workload, 'warmup' if pair < 0 else f'pair {pair + 1}', flush=True)
    signatures = {(tuple(run['instructions']), run['cycles'], run['emulated_seconds'],
                   run['exceptions'], run['interrupts'], run['console_bytes'], run['console_sha256'])
                  for run in row['runs']}
    row['exact_work_and_console'] = len(signatures) == 1
    if len(signatures) != 1:
        raise RuntimeError(f'{workload}: work or output mismatch')
    measured = [run for run in row['runs'] if run['pair'] >= 0]
    samples = {arm: [run['user_s'] for run in measured if run['arm'] == arm] for arm in ('base', 'candidate')}
    medians = {arm: statistics.median(values) for arm, values in samples.items()}
    slower = sum(next(run['user_s'] for run in measured if run['pair'] == pair and run['arm'] == 'candidate') >
                 next(run['user_s'] for run in measured if run['pair'] == pair and run['arm'] == 'base')
                 for pair in range(args.pairs))
    row['summary'] = {
        'median_user_s': medians,
        'change_percent': round(100 * (medians['candidate'] / medians['base'] - 1), 3),
        'range_user_s': {arm: [min(values), max(values)] for arm, values in samples.items()},
        'candidate_slower_pairs': slower,
    }
    save()
    print(workload, row['summary'], flush=True)
