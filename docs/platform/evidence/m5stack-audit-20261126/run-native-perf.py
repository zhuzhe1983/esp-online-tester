#!/usr/bin/env python3
"""Compare fixed native CLI binaries with exact guest work and child user CPU."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import resource
import statistics
import subprocess
import tomllib


STOP = re.compile(
    r'^\[emu\] stop: (?P<reason>.+?) — (?P<work>.+?) insns in .+?s wall = .+? Minsn/s; '
    r'emulated (?P<seconds>\d+\.\d+)s \((?P<cycles>\d+) cycles\); '
    r'(?P<exceptions>\d+) exceptions, (?P<interrupts>\d+) interrupts$')
FATAL = re.compile(r'Guru Meditation|assert(?:ion)? failed|abort\(\)|stack overflow|'
                   r'\bpanic(?:ked|king)?\b|Interrupt wdt timeout|task_wdt|'
                   r'\bjit (?:failed|failure|error)\b', re.I)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def git(tree, *args):
    return subprocess.check_output(['git', *args], cwd=tree, text=True).strip()


def source_paths(tree):
    members = set(tomllib.loads((tree / 'Cargo.toml').read_text())['workspace']['members'])
    return sorted(path for path in git(tree, 'ls-files').splitlines()
                  if (Path(path).parts[0] in members or '/' not in path)
                  and (path.endswith('.rs') or Path(path).name in ('Cargo.toml', 'Cargo.lock')))


def source(tree):
    files = source_paths(tree)
    manifest = ''.join(f'{sha(tree / path)}  {path}\n' for path in files)
    return {'files': len(files), 'sha256sumManifestSha256': hashlib.sha256(manifest.encode()).hexdigest()}


def completed(stdout, stderr, workload, seconds):
    if FATAL.search(stdout.decode(errors='replace')) or FATAL.search(stderr.decode(errors='replace')):
        raise ValueError('Fatal firmware/emulator diagnostic')
    reports = [line for line in stderr.decode(errors='replace').splitlines() if line.startswith('[emu] stop:')]
    if len(reports) != 1 or not (match := STOP.fullmatch(reports[0])):
        raise ValueError('Expected exactly one well-formed completion report')
    values = match.groupdict()
    work = re.fullmatch(r'core0 (\d+) \+ core1 (\d+)' if workload['cores'] == 2 else r'(\d+)', values['work'])
    if work is None or values['reason'] != 'Halted' or float(values['seconds']) != seconds:
        raise ValueError('Incomplete run or incorrect per-core instruction format')
    cycles = int(values['cycles'])
    if not workload['cpuHz'] * seconds <= cycles < workload['cpuHz'] * seconds + 64:
        raise ValueError('Run did not stop at the requested native q64 deadline')
    if workload['marker'].encode() not in stdout:
        raise ValueError('Expected firmware boot marker absent')
    counts = [int(value) for value in work.groups()]
    if sum(counts) <= 0:
        raise ValueError('No guest work')
    return {'coreInstructions': counts, 'instructions': sum(counts), 'cycles': cycles,
            'guestSeconds': float(values['seconds']), 'stopReason': values['reason'],
            'exceptions': int(values['exceptions']), 'interrupts': int(values['interrupts'])}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline-bin', type=Path, required=True)
    parser.add_argument('--candidate-bin', type=Path, required=True)
    parser.add_argument('--baseline-source', type=Path, required=True)
    parser.add_argument('--candidate-source', type=Path, required=True)
    parser.add_argument('--baseline-revision', required=True)
    parser.add_argument('--candidate-revision', required=True)
    parser.add_argument('--firmware-dir', type=Path, required=True)
    parser.add_argument('--model-file', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--raw-output', type=Path, required=True, help='New ignored directory for raw captures')
    parser.add_argument('--seconds', type=int, default=2)
    parser.add_argument('--pairs', type=int, default=3)
    parser.add_argument('--include-p4', action='store_true', help='Also cover the P4 device_set with its pinned hello')
    parser.add_argument('--candidate-build-command', default='cargo build --release --bins',
                        help='Record the actual candidate build command; do not imply identical feature graphs')
    args = parser.parse_args()
    if args.pairs < 3 or args.seconds < 1:
        parser.error('At least three measured pairs and a positive guest duration are required')
    trees = {'baseline': args.baseline_source.resolve(), 'candidate': args.candidate_source.resolve()}
    bins = {'baseline': args.baseline_bin.resolve(), 'candidate': args.candidate_bin.resolve()}
    for arm, revision in [('baseline', args.baseline_revision), ('candidate', args.candidate_revision)]:
        git(trees[arm], 'rev-parse', '--verify', revision + '^{commit}')
        check = subprocess.run(['git', 'diff', '--quiet', revision, '--', *source_paths(trees[arm])], cwd=trees[arm])
        if check.returncode:
            parser.error(arm + ' source files differ from the supplied binary revision')
    fw, model = args.firmware_dir.resolve(), args.model_file.resolve()
    out, raw = args.output.resolve(), args.raw_output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    raw.mkdir(parents=True, exist_ok=False)
    labels = {str(fw): '$FIRMWARE', str(model): '$MODEL', str(raw): '$RAW', str(out): '$EVIDENCE',
              str(bins['baseline']): '$BASELINE_BIN', str(bins['candidate']): '$CANDIDATE_BIN',
              str(trees['baseline']): '$BASELINE_SOURCE', str(trees['candidate']): '$CANDIDATE_SOURCE'}

    def normalized(value):
        for path, label in sorted(labels.items(), key=lambda row: -len(row[0])):
            value = value.replace(path, label)
        return value

    def normalized_bytes(value):
        for path, label in sorted(labels.items(), key=lambda row: -len(row[0])):
            value = value.replace(path.encode(), label.encode())
        return value

    def write():
        dest = out / 'native-perf.json'
        temp = dest.with_suffix('.json.tmp')
        temp.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
        temp.replace(dest)

    inputs, workloads = {}, []
    definitions = [
            ('s3-hello', 'esp32sim', 'hello', 2, 240_000_000, 'Hello world!'),
            ('c3-hello', 'esp32sim-c3', 'c3-hello', 1, 160_000_000, 'Hello world!'),
            ('c6-hello', 'esp32sim-c6', 'c6-hello', 1, 160_000_000, 'Hello world!'),
            ('pocket-tank', 'esp32sim', 'pocket-tank', 2, 240_000_000, 'pocket-tank boot')]
    if args.include_p4:
        definitions.append(('p4-hello', 'esp32sim-p4', 'p4-hello', 2, 360_000_000, 'Hello world!'))
    binary_names = sorted({row[1] for row in definitions})
    for key, binary, manifest, cores, hz, marker in definitions:
        config = fw / (manifest + '.json')
        definition = json.loads(config.read_text())
        paths = [(option, fw / filename) for option, filename in definition['files'].items()]
        command = ['--boot', 'rom', '--no-dump', '--console', 'uart0', '--max-seconds', str(args.seconds),
                   '--flash-mb', str(definition['flash_mb'])]
        if binary == 'esp32sim':
            command += ['--board', definition['board'], '--psram-mb', str(definition['psram_mb'])]
        for option, path in paths:
            command += ['--' + option, str(path)]
        for offset, _ in definition.get('flash_at', {}).items():
            command += ['--flash-at', offset + '=' + str(model)]
            paths.append(('model', model))
        for path in [config, *(path for _, path in paths)]:
            inputs[str(path)] = {'path': normalized(str(path)), 'bytes': path.stat().st_size, 'sha256': sha(path)}
        workloads.append({'id': key, 'binary': binary, 'args': command, 'cores': cores, 'cpuHz': hz, 'marker': marker})
    report = {
        'schemaVersion': 1, 'harnessSha256': sha(Path(__file__)), 'complete': False, 'correctnessPassed': False,
        'revisions': {'baseline': args.baseline_revision, 'candidate': args.candidate_revision},
        'source': {arm: source(tree) for arm, tree in trees.items()},
        'buildCommands': {'baseline': 'cargo build --release --bins', 'candidate': args.candidate_build_command},
        'buildPolicy': 'Rust 1.100.0 stable; unmodified release profile; CARGO_BUILD_JOBS=2; CARGO_INCREMENTAL=0; shared target rebuilt sequentially from separate source trees, binaries copied before replacement.',
        'method': 'One excluded warm-up pair, then at least three measured pairs. Alternate AB/BA. Compare median child user CPU only; wall duration is not a performance metric.',
        'metric': 'resource.getrusage(RUSAGE_CHILDREN).ru_utime delta around the sole child emulator',
        'completionPolicy': 'Exit 0; exactly one Halted report; exact requested guest seconds; q64 deadline overshoot 0..63 cycles; firmware boot marker; no fatal diagnostic; exact per-core work, cycles, exceptions, interrupts and stdout SHA-256 across every arm including warm-up.',
        'scope': 'Required whole-change native bus regression check; related EX114 and EX094. No new execution optimization, isolated function cost, silicon timing, frame/audio equivalence or browser speed claim.',
        'conditions': {'os': platform.system(), 'architecture': platform.machine(), 'pythonVersion': platform.python_version(),
                       'logicalCpuCount': os.cpu_count(),
                       'affinityCpuCount': len(os.sched_getaffinity(0)) if hasattr(os, 'sched_getaffinity') else None,
                       'hostPolicy': 'Coordinator excludes concurrent workspace builds/tests/model rendering during measurement; unrelated shared-host load cannot be controlled.'},
        'privacy': 'Paths normalized to caller-supplied role labels before capture storage. No process inventory, hostname, login, device identity or credential collected. Full captures kept outside Git; original and retained stream hashes recorded; comparison uses original stdout bytes.',
        'runtimeEnvironmentPolicy': 'Remove UPDATE_GOLDENS, ESP_EMU_* and ESP32SIM_*; explicit ROM/firmware/board/memory arguments.',
        'binaryInputs': {arm: {name: {'bytes': (directory / name).stat().st_size, 'sha256': sha(directory / name)}
                              for name in binary_names} for arm, directory in bins.items()},
        'inputs': list(inputs.values()), 'seconds': args.seconds, 'measuredPairs': args.pairs,
        'workloads': [{**row, 'args': [normalized(part) for part in row['args']]} for row in workloads], 'runs': []}
    write()
    env = {key: value for key, value in os.environ.items()
           if key != 'UPDATE_GOLDENS' and not key.startswith(('ESP_EMU_', 'ESP32SIM_'))}
    reference = {}
    for workload in workloads:
        for pair in range(args.pairs + 1):
            for arm in (('baseline', 'candidate') if pair % 2 == 0 else ('candidate', 'baseline')):
                command = [str(bins[arm] / workload['binary']), *workload['args']]
                prefix = f"{workload['id']}-pair{pair:02d}-{arm}"
                load = list(os.getloadavg())
                before = resource.getrusage(resource.RUSAGE_CHILDREN)
                result = subprocess.run(command, cwd=trees['candidate'], env=env, capture_output=True)
                after = resource.getrusage(resource.RUSAGE_CHILDREN)
                stdout_file, stderr_file = raw / (prefix + '.stdout.log'), raw / (prefix + '.stderr.log')
                stdout_file.write_bytes(normalized_bytes(result.stdout))
                stderr_file.write_bytes(normalized_bytes(result.stderr))
                row = {'workload': workload['id'], 'pair': pair, 'warmup': pair == 0, 'arm': arm,
                       'loadAverageBefore': load,
                       'exitCode': result.returncode, 'userCpuSeconds': round(after.ru_utime - before.ru_utime, 6),
                       'systemCpuSeconds': round(after.ru_stime - before.ru_stime, 6),
                       'stdoutSha256': hashlib.sha256(result.stdout).hexdigest(),
                       'stderrSha256': hashlib.sha256(result.stderr).hexdigest(),
                       'retainedStdoutSha256': sha(stdout_file), 'retainedStderrSha256': sha(stderr_file),
                       'stdoutBytes': len(result.stdout), 'stderrBytes': len(result.stderr),
                       'rawStdout': '$RAW/' + stdout_file.name, 'rawStderr': '$RAW/' + stderr_file.name,
                       'validationPassed': False}
                report['runs'].append(row)
                write()
                try:
                    if result.returncode or row['userCpuSeconds'] <= 0:
                        raise ValueError('Nonzero process exit or nonpositive user CPU sample')
                    row.update(completed(result.stdout, result.stderr, workload, args.seconds))
                    signature = (tuple(row['coreInstructions']), row['cycles'], row['exceptions'],
                                 row['interrupts'], row['stdoutSha256'])
                    reference.setdefault(workload['id'], signature)
                    if signature != reference[workload['id']]:
                        raise ValueError('Guest work or console differs across arms')
                    row['validationPassed'] = True
                    write()
                except ValueError as error:
                    row['validationFailure'] = str(error)
                    report['validationFailure'] = {'run': prefix, 'reason': str(error)}
                    write()
                    raise SystemExit(f'REJECTED {prefix}: {error}') from error
                print(f"PASS {prefix}: user={row['userCpuSeconds']:.6f}s insns={row['instructions']}", flush=True)
    for arm, tree in trees.items():
        assert source(tree) == report['source'][arm], 'Source changed during comparison'
        for name, identity in report['binaryInputs'][arm].items():
            assert sha(bins[arm] / name) == identity['sha256'], 'Executable changed during comparison'
    for path, identity in inputs.items():
        assert sha(Path(path)) == identity['sha256'], 'Firmware input changed during comparison'
    report['summary'] = []
    for workload in workloads:
        rows = [row for row in report['runs'] if row['workload'] == workload['id'] and not row['warmup']]
        samples = {arm: [row['userCpuSeconds'] for row in rows if row['arm'] == arm] for arm in bins}
        medians = {arm: statistics.median(values) for arm, values in samples.items()}
        slower = sum(next(row['userCpuSeconds'] for row in rows if row['pair'] == pair and row['arm'] == 'candidate') >
                     next(row['userCpuSeconds'] for row in rows if row['pair'] == pair and row['arm'] == 'baseline')
                     for pair in range(1, args.pairs + 1))
        work = reference[workload['id']]
        report['summary'].append({'workload': workload['id'], 'userCpuSamples': samples,
                                  'medianUserCpuSeconds': medians, 'candidateChangePercent': 100 * (medians['candidate'] / medians['baseline'] - 1),
                                  'candidateSlowerPairs': slower, 'coreInstructions': list(work[0]),
                                  'instructions': sum(work[0]), 'cycles': work[1], 'exceptions': work[2],
                                  'interrupts': work[3], 'stdoutSha256': work[4], 'equalWorkAndConsole': True})
    report['complete'], report['correctnessPassed'] = True, True
    write()
    print(json.dumps(report['summary'], ensure_ascii=False), flush=True)


if __name__ == '__main__':
    main()
