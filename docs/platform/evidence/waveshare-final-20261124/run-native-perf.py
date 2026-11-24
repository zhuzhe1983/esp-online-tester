#!/usr/bin/env python3
"""Compare unchanged guest work/output and median child user CPU, alternating arms."""
import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import re
import resource
import statistics
import subprocess
import tempfile
import time

GUEST_SECONDS = 30
NATIVE_QUANTUM_CYCLES = 64
STOP_LINE = re.compile(
    r'^\[emu\] stop: (?P<reason>.+?) — (?P<work>.+?) insns in .+?s wall = .+? Minsn/s; '
    r'emulated (?P<seconds>\d+\.\d+)s \((?P<cycles>\d+) cycles\); '
    r'(?P<exceptions>\d+) exceptions, (?P<interrupts>\d+) interrupts$')
FATAL_MARKERS = [
    ('panic', r'\bpanic(?:ked|king)?\b'),
    ('guest-fatal', r'Guru Meditation|assert(?:ion)? failed|abort\(\)|stack overflow'),
    ('watchdog', r'\b[A-Z0-9_]*WDT[A-Z0-9_]*(?:RST|RESET)\b|Interrupt wdt timeout|task_wdt'),
    ('jit-failure', r'\bjit (?:failed|failure|error)\b'),
]


def completion(stdout, stderr, cores, cpu_hz):
    """Accept the known native CLI completion contract, never a mere success marker."""
    for stream, data in [('stdout', stdout), ('stderr', stderr)]:
        text = data.decode(errors='replace')
        for label, pattern in FATAL_MARKERS:
            if re.search(pattern, text, re.IGNORECASE):
                raise ValueError(f'{label} marker in {stream}')
    lines = [line for line in stderr.decode(errors='replace').splitlines() if line.startswith('[emu] stop:')]
    if len(lines) != 1:
        raise ValueError(f'Expected one final stop report, found {len(lines)}')
    match = STOP_LINE.fullmatch(lines[0])
    if not match:
        raise ValueError('Malformed final stop report')
    values = match.groupdict()
    if cores == 1:
        work = re.fullmatch(r'(\d+)', values['work'])
    elif cores == 2:
        work = re.fullmatch(r'core0 (\d+) \+ core1 (\d+)', values['work'])
    else:
        raise ValueError(f'Unsupported benchmark core count: {cores}')
    if not work:
        raise ValueError(f'Wrong per-core instruction report for {cores} cores')
    per_core = [int(value) for value in work.groups()]
    cycles = int(values['cycles'])
    target_cycles = cpu_hz * GUEST_SECONDS
    if values['reason'] != 'Halted':
        raise ValueError('Abnormal terminal stop: ' + values['reason'])
    if float(values['seconds']) != GUEST_SECONDS:
        raise ValueError('Run did not complete 30.000 guest seconds')
    # The native scheduler is q64. A final boundary may round past the limit;
    # do not accept a run below the deadline or more than one quantum beyond it.
    if not target_cycles <= cycles < target_cycles + NATIVE_QUANTUM_CYCLES:
        raise ValueError(f'Guest cycles outside the 30-second native boundary: {cycles}')
    if sum(per_core) <= 0:
        raise ValueError('No retired guest instructions')
    return {'stopReason': values['reason'], 'guestSeconds': float(values['seconds']),
            'cycles': cycles, 'cycleOverrun': cycles - target_cycles,
            'coreInstructions': per_core, 'instructions': sum(per_core),
            'exceptions': int(values['exceptions']), 'interrupts': int(values['interrupts']),
            'fatalMarkers': [], 'stopLine': lines[0]}


def work_signature(completed, stdout):
    return (tuple(completed['coreInstructions']), completed['cycles'], completed['exceptions'],
            completed['interrupts'], hashlib.sha256(stdout).hexdigest())


p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--baseline', required=True, type=Path)
p.add_argument('--candidate', required=True, type=Path)
p.add_argument('--output', required=True, type=Path)
p.add_argument('--pairs', type=int, default=3)
p.add_argument('--raw-output', type=Path, help='Private raw logs; default: a new ignored directory below the candidate target/')
a = p.parse_args()
assert a.pairs >= 3
base, repo, out = a.baseline.resolve(), a.candidate.resolve(), a.output.resolve()
out.mkdir(parents=True, exist_ok=True)
if a.raw_output:
    raw_out = a.raw_output.resolve()
    raw_out.mkdir(parents=True, exist_ok=False)
else:
    (repo / 'target').mkdir(exist_ok=True)
    raw_out = Path(tempfile.mkdtemp(prefix='native-perf-raw-', dir=repo / 'target'))
fw = repo / 'web/wasm/fw'

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def relative(text):
    return text.replace(str(base), '$BASELINE').replace(str(repo), '$REPO').replace(str(out), '$PERF_EVIDENCE')

def source(tree):
    paths = sorted(x for x in subprocess.check_output(['git', 'ls-files'], cwd=tree, text=True).splitlines()
                   if x.endswith('.rs') or Path(x).name in ('Cargo.toml', 'Cargo.lock'))
    lines = ''.join(f'{sha(tree / path)}  {path}\n' for path in paths)
    return {'files': len(paths), 'sha256sumManifestSha256': hashlib.sha256(lines.encode()).hexdigest()}

workloads = []
for key, binary, manifest, cores, cpu_hz in [
        ('s3-hello', 'esp32sim', 'hello', 2, 240_000_000), ('c3-hello', 'esp32sim-c3', 'c3-hello', 1, 160_000_000),
        ('c6-hello', 'esp32sim-c6', 'c6-hello', 1, 160_000_000), ('pocket-tank', 'esp32sim', 'pocket-tank', 2, 240_000_000)]:
    definition = json.loads((fw / (manifest + '.json')).read_text())
    command = ['--boot', 'rom', '--no-dump', '--console', 'uart0', '--max-seconds', '30',
               '--flash-mb', str(definition['flash_mb'])]
    if binary == 'esp32sim':
        command += ['--board', definition['board'], '--psram-mb', str(definition['psram_mb'])]
    inputs = []
    for option, filename in definition['files'].items():
        path = fw / filename
        command += ['--' + option, str(path)]
        inputs.append({'path': str(path.relative_to(repo)), 'bytes': path.stat().st_size, 'sha256': sha(path)})
    for offset, filename in definition.get('flash_at', {}).items():
        path = fw / filename
        command += ['--flash-at', offset + '=' + str(path)]
        inputs.append({'path': str(path.relative_to(repo)), 'bytes': path.stat().st_size, 'sha256': sha(path)})
    workloads.append({'id': key, 'binary': binary, 'args': command, 'inputs': inputs,
                      'cores': cores, 'cpuHz': cpu_hz, 'targetCycles': cpu_hz * GUEST_SECONDS})

report = {
    'schemaVersion': 1, 'harnessSha256': sha(Path(__file__)),
    'baselineCommit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=base, text=True).strip(),
    'candidateCommitAtStart': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip(),
    'baselineSource': source(base), 'candidateSource': source(repo),
    'buildCommandBothArms': 'cargo build --release --bins',
    'buildPolicy': 'Rust 1.100.0; unmodified Cargo.toml release profile; CARGO_BUILD_JOBS=2; CARGO_INCREMENTAL=0.',
    'method': 'One excluded warm-up pair then three or more measured pairs per workload. Alternate AB/BA order. Median child process user CPU; each arm executes 30 guest seconds.',
    'metric': 'resource.getrusage(RUSAGE_CHILDREN).ru_utime difference around the sole child process',
    'completionPolicy': {'stopReason': 'Halted', 'guestSeconds': GUEST_SECONDS,
                         'cycleOverrunAllowed': [0, NATIVE_QUANTUM_CYCLES - 1],
                         'source': ['cli/src/lib.rs:prepare/report', 'esp-soc/src/machine.rs:QUANTUM/run',
                                    'esp32s3/src/periph.rs:CPU_HZ', 'esp32c3/src/periph.rs:CPU_HZ', 'esp32c6/src/periph.rs:CPU_HZ'],
                         'comparison': 'Exact per-core instructions, cycles, exception/interrupt totals and stdout bytes across all arms, including warm-up.',
                         'stderr': 'Every byte retained before validation; all stdout/stderr scanned for fatal markers. Host wall/rate and code-cache diagnostics are not required to match.',
                         'fatalPatterns': FATAL_MARKERS},
    'rawRetention': 'Original stdout/stderr are written to the private raw directory before validation; public stderr changes only repository paths and records the original SHA-256.',
    'rawOutputDirectory': '$REPO/' + str(raw_out.relative_to(repo)) if raw_out.is_relative_to(repo) else '$RAW_OUTPUT',
    'mechanismContext': ['EX114 counted inactive-device/tick shortcuts', 'EX094 reject invalid benchmark comparisons'],
    'scope': 'Whole adaptation source regression versus the fixed patch-chain base; no individual speed optimization or silicon timing claim.',
    'conditions': 'Linux x86_64 shared execution host. Coordinator waits for its builds/tests/model rendering to finish; unrelated host activity is not controlled.',
    'workloads': [{**w, 'args': [relative(x) for x in w['args']]} for w in workloads],
    'binaryInputs': {arm: {name: {'bytes': (tree/'target/release'/name).stat().st_size, 'sha256': sha(tree/'target/release'/name)}
                          for name in ('esp32sim', 'esp32sim-c3', 'esp32sim-c6')}
                     for arm, tree in [('baseline', base), ('candidate', repo)]},
    'runs': [], 'complete': False, 'correctnessPassed': False,
}
dest = out / 'native-perf.json'

def save():
    temp = dest.with_suffix('.json.tmp')
    temp.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    temp.replace(dest)

env = dict(os.environ)
for key in list(env):
    if key == 'UPDATE_GOLDENS' or key.startswith(('ESP_EMU_', 'ESP32SIM_')):
        env.pop(key)
report['runtimeEnvironmentPolicy'] = 'Remove UPDATE_GOLDENS, ESP_EMU_* and ESP32SIM_* overrides. ROM, firmware, board and memory are explicit CLI arguments.'
save()
reference = {}
for workload in workloads:
    for pair in range(a.pairs + 1):
        order = [('baseline', base), ('candidate', repo)] if pair % 2 == 0 else [('candidate', repo), ('baseline', base)]
        for arm, tree in order:
            command = [str(tree / 'target/release' / workload['binary']), *workload['args']]
            prefix = f"{workload['id']}-pair{pair:02d}-{arm}"
            started = dt.datetime.now(dt.timezone.utc).isoformat()
            usage_before = resource.getrusage(resource.RUSAGE_CHILDREN)
            before = time.monotonic()
            result = subprocess.run(command, cwd=repo, env=env, capture_output=True)
            wall = time.monotonic() - before
            usage_after = resource.getrusage(resource.RUSAGE_CHILDREN)
            stdout = result.stdout
            stderr_raw = result.stderr.decode(errors='replace')
            # Persist both original streams before parsing or checking success. Failed
            # arms remain inspectable; raw stderr can contain private local paths.
            (raw_out / (prefix + '.stdout.log')).write_bytes(stdout)
            (raw_out / (prefix + '.stderr.log')).write_bytes(result.stderr)
            stdout_path = out / (prefix + '.stdout.log')
            stderr_path = out / (prefix + '.stderr.log')
            stdout_path.write_bytes(stdout)
            stderr_path.write_text(relative(stderr_raw))
            row = {'workload': workload['id'], 'pair': pair, 'warmup': pair == 0, 'arm': arm,
                   'startedAt': started, 'exitCode': result.returncode, 'command': [relative(x) for x in command],
                   'userCpuSeconds': round(usage_after.ru_utime - usage_before.ru_utime, 6),
                   'systemCpuSeconds': round(usage_after.ru_stime - usage_before.ru_stime, 6),
                   'wallSeconds': round(wall, 6),
                   'stdout': stdout_path.name, 'stderr': stderr_path.name,
                   'stdoutSha256': sha(stdout_path), 'stderrSha256': sha(stderr_path),
                   'originalStderrSha256': hashlib.sha256(result.stderr).hexdigest(),
                   'originalStderrBytes': len(result.stderr), 'validationPassed': False}
            report['runs'].append(row)
            save()
            try:
                if result.returncode != 0:
                    raise ValueError(f'Child exited with status {result.returncode}')
                if row['userCpuSeconds'] <= 0:
                    raise ValueError('Nonpositive measured child user CPU time')
                completed = completion(stdout, result.stderr, workload['cores'], workload['cpuHz'])
                row.update(completed)
                row['stopLine'] = relative(completed['stopLine'])
                marker = b'Hello world!' if workload['id'].endswith('hello') else b'tok/s'
                if marker not in stdout:
                    raise ValueError('Expected firmware output marker is absent')
                signature = work_signature(completed, stdout)
                reference.setdefault(workload['id'], signature)
                if reference[workload['id']] != signature:
                    raise ValueError('Unequal per-core guest work, cycles, exceptions/interrupts or console')
                row['validationPassed'] = True
                save()
            except ValueError as error:
                row['validationFailure'] = str(error)
                report['validationFailure'] = {'run': prefix, 'reason': str(error)}
                save()
                raise SystemExit(f'REJECTED {prefix}: {error}') from error
            print(f"PASS {prefix}: user={row['userCpuSeconds']:.6f}s wall={wall:.3f}s instructions={row['instructions']}", flush=True)

assert source(base) == report['baselineSource']
assert source(repo) == report['candidateSource']
for arm, tree in [('baseline', base), ('candidate', repo)]:
    for name, identity in report['binaryInputs'][arm].items():
        assert sha(tree / 'target/release' / name) == identity['sha256'], 'Executable changed during comparison'
for workload in workloads:
    for identity in workload['inputs']:
        assert sha(repo / identity['path']) == identity['sha256'], 'Firmware input changed during comparison'
report['summary'] = []
for workload in workloads:
    rows = [row for row in report['runs'] if row['workload'] == workload['id'] and not row['warmup']]
    samples = {arm: [row['userCpuSeconds'] for row in rows if row['arm'] == arm] for arm in ('baseline', 'candidate')}
    medians = {arm: statistics.median(values) for arm, values in samples.items()}
    report['summary'].append({'workload': workload['id'], 'measuredPairs': a.pairs, 'userCpuSamples': samples,
                              'medianUserCpuSeconds': medians,
                              'candidateChangePercent': 100 * (medians['candidate'] / medians['baseline'] - 1),
                              'coreInstructions': list(reference[workload['id']][0]),
                              'instructions': sum(reference[workload['id']][0]), 'cycles': reference[workload['id']][1],
                              'exceptions': reference[workload['id']][2], 'interrupts': reference[workload['id']][3],
                              'stdoutSha256': reference[workload['id']][4],
                              'equalWorkAndConsole': True})
report['complete'] = True
report['correctnessPassed'] = True
save()
print(json.dumps(report['summary'], ensure_ascii=False), flush=True)
