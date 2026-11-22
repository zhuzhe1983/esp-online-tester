#!/usr/bin/env python3
"""Alternating native hello checks required by AGENTS.md / CONTRIBUTING.md.

Build both trees with the same stable toolchain and `cargo build --release --bins`
first. Outputs retain all warmups and measurements; raw console logs stay in the
caller-selected ignored directory. No timing sample is discarded after recording.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import resource
import statistics
import subprocess
import time


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def source_receipt(root):
    def git(*args):
        return subprocess.check_output(['git', '-C', str(root), *args])
    paths = set(git('ls-files', '-z').decode().split('\0'))
    paths.update(git('ls-files', '--others', '--exclude-standard', '-z').decode().split('\0'))
    files = {path: sha256((root / path).read_bytes()) for path in sorted(paths)
             if path and (path.endswith('.rs') or Path(path).name in {'Cargo.toml', 'Cargo.lock'})}
    changed = set(git('diff', '--name-only', 'HEAD').decode().splitlines())
    changed.update(git('ls-files', '--others', '--exclude-standard').decode().splitlines())
    return {
        'head': git('rev-parse', 'HEAD').decode().strip(),
        'rustSourceManifestSha256': sha256(json.dumps(files, sort_keys=True, separators=(',', ':')).encode()),
        'rustSourceFiles': len(files),
        'rustSourceScope': 'All tracked/unignored Rust files, Cargo.toml and Cargo.lock; paths and content SHA-256 sorted as compact JSON',
        'uncommittedRustFiles': {path: files[path] for path in sorted(changed) if path in files},
    }


STOP = re.compile(r'\[emu\] stop: (.*?) — (.*?) insns .*?emulated ([\d.]+)s \((\d+) cycles\); (\d+) exceptions, (\d+) interrupts')
WORK = ('stop', 'instructions', 'guestSeconds', 'cycles', 'exceptions', 'interrupts', 'stdoutSha256')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', type=Path, required=True, help='Baseline checkout with target/release binaries')
    parser.add_argument('--candidate', type=Path, required=True, help='Candidate checkout with target/release binaries')
    parser.add_argument('--assets', type=Path, required=True, help='Directory containing hello.json and its ROM/firmware files')
    parser.add_argument('--output', type=Path, required=True, help='Compact JSON receipt')
    parser.add_argument('--raw-dir', type=Path, required=True, help='Ignored output directory for stdout/stderr')
    parser.add_argument('--rounds', type=int, default=5)
    parser.add_argument('--conditions', default='Shared host; no dedicated isolation asserted', help='Known concurrent work or measurement isolation, retained verbatim')
    args = parser.parse_args()
    if args.rounds < 3:
        parser.error('At least three rounds are required for a median comparison')
    args.raw_dir.mkdir(parents=True, exist_ok=True)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    roots = {'baseline': args.baseline.resolve(), 'candidate': args.candidate.resolve()}
    report = {
        'schemaVersion': 1,
        'startedAt': datetime.now(timezone.utc).isoformat(timespec='seconds'),
        'method': 'Sequential RUSAGE_CHILDREN user/system CPU deltas; 30 emulated seconds; one excluded warmup per arm; alternating AB/BA measurement pairs',
        'rounds': args.rounds,
        'environment': {'os': platform.system(), 'architecture': platform.machine(), 'logicalCpus': os.cpu_count(),
                        'rustc': subprocess.check_output(['rustc', '--version']).decode().strip(),
                        'python': platform.python_version(), 'measurementConditions': args.conditions},
        'buildCommand': 'CARGO_BUILD_JOBS=1 cargo build --release --bins',
        'source': {label: source_receipt(root) for label, root in roots.items()},
        'harnessSha256': sha256(Path(__file__).read_bytes()),
        'workloads': {},
    }

    def save():
        args.output.write_text(json.dumps(report, indent=2) + '\n')

    for workload, binary in [('hello', 'esp32sim'), ('c3-hello', 'esp32sim-c3'), ('c6-hello', 'esp32sim-c6')]:
        manifest = json.loads((args.assets / (workload + '.json')).read_text())
        files = {role: (args.assets / path).resolve() for role, path in manifest['files'].items()}
        command = ['--boot', 'rom', '--flash-mb', str(manifest['flash_mb']), '--max-seconds', '30', '--no-dump']
        if workload == 'hello':
            command += ['--board', manifest['board'], '--psram-mb', str(manifest['psram_mb'])]
        for role in ('rom', 'bootloader', 'ptable', 'app'):
            command += ['--' + role, str(files[role])]
        normalized = [value.replace(str(args.assets.resolve()), '{assets}') for value in command]
        row = {
            'command': '{arm}/target/release/' + binary + ' ' + ' '.join(normalized),
            'inputs': {role: {'path': manifest['files'][role], 'sha256': sha256(path.read_bytes()), 'bytes': path.stat().st_size}
                       for role, path in files.items()},
            'binarySha256': {label: sha256((root / 'target/release' / binary).read_bytes()) for label, root in roots.items()},
            'warmups': [], 'samples': [],
        }
        report['workloads'][workload] = row
        expected = None
        for round_number in range(args.rounds + 1):
            order = ['baseline', 'candidate'] if round_number % 2 else ['candidate', 'baseline']
            for label in order:
                aggregate_load = os.getloadavg()
                before = resource.getrusage(resource.RUSAGE_CHILDREN)
                start = time.perf_counter()
                result = subprocess.run([str(roots[label] / 'target/release' / binary), *command], capture_output=True, timeout=120)
                wall = time.perf_counter() - start
                after = resource.getrusage(resource.RUSAGE_CHILDREN)
                stem = f'{workload}-{round_number}-{label}'
                (args.raw_dir / (stem + '.stdout')).write_bytes(result.stdout)
                (args.raw_dir / (stem + '.stderr')).write_bytes(result.stderr)
                matches = STOP.findall(result.stderr.decode(errors='replace'))
                sample = {
                    'round': round_number, 'label': label, 'exitCode': result.returncode,
                    'userSeconds': after.ru_utime - before.ru_utime,
                    'systemSeconds': after.ru_stime - before.ru_stime, 'wallSeconds': wall,
                    'aggregateLoadBefore': list(aggregate_load),
                    'stdoutSha256': sha256(result.stdout), 'stderrSha256': sha256(result.stderr),
                }
                if len(matches) == 1:
                    stop, instructions, seconds, cycles, exceptions, interrupts = matches[0]
                    sample.update(stop=stop, instructions=instructions, guestSeconds=seconds,
                                  cycles=int(cycles), exceptions=int(exceptions), interrupts=int(interrupts))
                row['warmups' if round_number == 0 else 'samples'].append(sample)
                save()
                if result.returncode != 0 or len(matches) != 1 or sample['stop'] != 'Halted' or sample['guestSeconds'] != '30.000':
                    raise RuntimeError(f'{stem}: invalid completion; see raw output and retained receipt')
                work = tuple(sample[key] for key in WORK)
                if expected is None:
                    expected = work
                if work != expected:
                    row['identicalWorkAndOutput'] = False
                    save()
                    raise RuntimeError(f'{stem}: guest work/output differs; timing comparison is invalid')
                print(f'{workload} round {round_number} {label}: user {sample["userSeconds"]:.6f}s, wall {wall:.6f}s', flush=True)
        row['identicalWorkAndOutput'] = True
        row['medianUserSeconds'] = {label: statistics.median(sample['userSeconds'] for sample in row['samples'] if sample['label'] == label) for label in roots}
        row['userSecondsRange'] = {label: [min(sample['userSeconds'] for sample in row['samples'] if sample['label'] == label), max(sample['userSeconds'] for sample in row['samples'] if sample['label'] == label)] for label in roots}
        row['candidatePercentChange'] = 100 * (row['medianUserSeconds']['candidate'] / row['medianUserSeconds']['baseline'] - 1)
        row['slowerCandidatePairs'] = sum(next(sample['userSeconds'] for sample in row['samples'] if sample['round'] == n and sample['label'] == 'candidate') > next(sample['userSeconds'] for sample in row['samples'] if sample['round'] == n and sample['label'] == 'baseline') for n in range(1, args.rounds + 1))
        save()
        print(f'{workload}: {row["candidatePercentChange"]:+.3f}% median user CPU; identical guest work and stdout', flush=True)
    report['finishedAt'] = datetime.now(timezone.utc).isoformat(timespec='seconds')
    save()


if __name__ == '__main__':
    main()
