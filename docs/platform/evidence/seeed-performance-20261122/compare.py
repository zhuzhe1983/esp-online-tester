#!/usr/bin/env python3
"""Prepare, then explicitly run the repository's quiet native performance gate."""
import argparse
import datetime
import hashlib
import json
import pathlib
import re
import shutil
import statistics
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--base', required=True, type=pathlib.Path)
parser.add_argument('--candidate', required=True, type=pathlib.Path)
parser.add_argument('--root', required=True, type=pathlib.Path)
parser.add_argument('--rom-dir', required=True, type=pathlib.Path)
parser.add_argument('--output', required=True, type=pathlib.Path)
parser.add_argument('--pairs', type=int, default=3)
parser.add_argument('--seconds', type=int, default=30)
parser.add_argument('--pause-file', type=pathlib.Path, help='Create this file to stop before the next pair; an active pair completes.')
parser.add_argument('--workloads', nargs='+', choices=['hello', 'c3-hello', 'c6-hello', 'pocket-tank'], default=['hello', 'c3-hello', 'c6-hello', 'pocket-tank'])
parser.add_argument('--baseline-commit', default='573f5a6c1bcc6dbf0388383d178a3935ec8ddcce')
parser.add_argument('--baseline-sha256', default='22954f6f7ef68a1d12e76a07ed3494c25e9cc2c231781ea912d479af8f3a87be')
parser.add_argument('--candidate-sha256', default='84e13293e0524d108f4c08817fe09b0352fc525c8e134cecb29a81c7b80ef7b1')
parser.add_argument('--run', action='store_true', help='Start timed runs only after the quiet-window signal.')
args = parser.parse_args()
if args.pairs < 3:
    parser.error('Use at least three alternating pairs.')
if args.seconds < 1:
    parser.error('The guest window must be positive.')

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat(timespec='seconds').replace('+00:00', 'Z')

def write(name, data):
    (args.output / name).write_text(json.dumps(data, ensure_ascii=False, indent=2) + '\n')

args.output.mkdir(parents=True, exist_ok=True)
if (args.output / 'results.json').exists() or (args.output / 'results.partial.json').exists():
    raise SystemExit('Preserve the previous result; choose a new output directory.')
arms = {'main': args.base.resolve(), 'candidate': args.candidate.resolve()}
expected = {'main': args.baseline_sha256, 'candidate': args.candidate_sha256}
timer = '/usr/bin/time' if pathlib.Path('/usr/bin/time').is_file() else shutil.which('bash')
if not timer:
    raise SystemExit('No POSIX time implementation found.')
timer_kind = 'external-time-p' if timer == '/usr/bin/time' else 'bash-builtin-time-p'
for arm, binary in arms.items():
    if sha(binary) != expected[arm]:
        raise SystemExit(f'{arm} binary SHA changed; revalidate its build before timing.')

firmware = args.root.resolve() / 'web/wasm/fw'
cases = {}
for name, chip in [('hello', 's3'), ('c3-hello', 'c3'), ('c6-hello', 'c6'), ('pocket-tank', 's3')]:
    if name not in args.workloads:
        continue
    cfg = json.loads((firmware / f'{name}.json').read_text())
    options = ['--chip', chip, '--board', cfg['board'] if name == 'pocket-tank' else 'none',
               '--boot', 'rom', '--max-seconds', str(args.seconds), '--no-dump',
               '--flash-mb', str(cfg['flash_mb'])]
    if chip == 's3':
        options += ['--psram-mb', str(cfg['psram_mb'])]
    inputs = {}
    for kind, rel in cfg['files'].items():
        path = (args.rom_dir / pathlib.Path(rel).name).resolve() if kind == 'rom' else firmware / rel
        options += ['--' + kind, str(path)]
        inputs[kind] = {'path': str(path), 'bytes': path.stat().st_size, 'sha256': sha(path)}
    for offset, rel in cfg.get('flash_at', {}).items():
        path = firmware / rel
        options += ['--flash-at', offset + '=' + str(path)]
        inputs['flash_at_' + offset] = {'path': str(path), 'bytes': path.stat().st_size, 'sha256': sha(path)}
    cases[name] = {'arguments': options, 'inputs': inputs}

result = {
    'schema': 1, 'preparedAt': now(), 'status': 'prepared-not-run',
    'baselineCommit': args.baseline_commit,
    'candidateRevision': subprocess.check_output(['git', '-C', str(args.root), 'rev-parse', 'HEAD'], text=True).strip(),
    'method': 'Unchanged release profiles; alternating main/candidate pairs for each workload with identical explicit guest windows; median POSIX time -p user; exact stdout bytes and per-core instruction totals must match every run. Raw logs remain local.',
    'timer': {'kind': timer_kind, 'path': timer, 'note': 'Both arms use the same timer. Bash builtin time -p is the fallback when /usr/bin/time is absent; the initial missing-executable attempt started no emulator and is retained separately.'},
    'pairs': args.pairs, 'guestSeconds': args.seconds,
    'binaries': {arm: {'path': str(binary), 'sha256': expected[arm]} for arm, binary in arms.items()},
    'workloads': cases,
}
write('prepared.json', result)
if not args.run:
    print(f'Prepared {len(cases)} workloads and {args.pairs * 2 * len(cases)} runs; no emulator or timer started. {args.output / "prepared.json"}', flush=True)
    raise SystemExit(0)

result.update(status='running', startedAt=now())
try:
    for name, case in cases.items():
        case['runs'] = []
        reference = None
        for pair in range(args.pairs):
            if args.pause_file and args.pause_file.exists():
                result.update(status='paused-between-pairs', pausedAt=now(), pauseReason='Caller-created pause file')
                write('results.partial.json', result)
                print('Paused before the next pair; completed measurements retained.', flush=True)
                raise SystemExit(0)
            # Keep main/candidate alternating. No concurrent emulator or warmup is hidden.
            for arm in arms:
                if sha(arms[arm]) != expected[arm]:
                    raise RuntimeError(f'{arm} binary changed during the quiet window')
                prefix = args.output / f'{name}-{pair + 1}-{arm}'
                print(f'Running {name} pair {pair + 1}/{args.pairs} {arm}', flush=True)
                with prefix.with_suffix('.stdout').open('wb') as stdout:
                    if timer_kind == 'external-time-p':
                        with prefix.with_suffix('.stderr').open('wb') as stderr:
                            proc = subprocess.run([timer, '-p', '-o', str(prefix.with_suffix('.time')),
                                                   str(arms[arm]), *case['arguments']], stdout=stdout, stderr=stderr)
                    else:
                        # File paths and emulator arguments remain positional parameters, never shell code.
                        shell = 'perf_time_file="$1"; perf_stderr_file="$2"; shift 2; { time -p "$@" 2>"$perf_stderr_file"; } 2>"$perf_time_file"'
                        proc = subprocess.run([timer, '-c', shell, 'seeed-perf-time',
                                               str(prefix.with_suffix('.time')), str(prefix.with_suffix('.stderr')),
                                               str(arms[arm]), *case['arguments']], stdout=stdout, stderr=subprocess.PIPE)
                        if proc.stderr:
                            with prefix.with_suffix('.stderr').open('ab') as stderr:
                                stderr.write(proc.stderr)
                timing = prefix.with_suffix('.time').read_text()
                stderr = prefix.with_suffix('.stderr').read_text(errors='replace')
                stop = re.search(r'^\[emu\] stop: (.+?) — (.+?) insns in .*?; emulated ([0-9.]+)s \((\d+) cycles\); (\d+) exceptions, (\d+) interrupts$', stderr, re.M)
                if proc.returncode != 0 or not stop:
                    raise RuntimeError(f'{name}/{arm}: exit {proc.returncode} or missing stop report; see local logs')
                reason, counters, seconds, cycles, exceptions, interrupts = stop.groups()
                per_core = [int(value) for value in re.findall(r'core\d+ (\d+)', counters)]
                if not per_core:
                    per_core = [int(counters)]
                correct = {'stdoutSha256': sha(prefix.with_suffix('.stdout')),
                           'stdoutBytes': prefix.with_suffix('.stdout').stat().st_size,
                           'stopReason': reason, 'perCoreInstructions': per_core,
                           'totalInstructions': sum(per_core), 'guestSeconds': float(seconds),
                           'cycles': int(cycles), 'exceptions': int(exceptions), 'interrupts': int(interrupts)}
                if float(seconds) != args.seconds:
                    raise RuntimeError(f'{name}/{arm}: stopped before the {args.seconds}-second guest limit')
                sample = {'pair': pair + 1, 'arm': arm, **correct,
                          **{kind + 'Seconds': float(re.search(r'^' + kind + r' ([0-9.]+)$', timing, re.M)[1]) for kind in ['real', 'user', 'sys']}}
                case['runs'].append(sample)
                write('results.partial.json', result)
                if reference is None:
                    reference = correct
                if correct != reference:
                    raise RuntimeError(f'{name}/{arm}: exact console/instruction/cycle/interrupt comparison failed')
                if sha(arms[arm]) != expected[arm]:
                    raise RuntimeError(f'{arm} binary changed during a run')
                print(f'Completed {name} {arm}: user={sample["userSeconds"]:.2f}s, instructions={sample["totalInstructions"]}, console={sample["stdoutSha256"]}', flush=True)
        case['medianUserSeconds'] = {arm: statistics.median(r['userSeconds'] for r in case['runs'] if r['arm'] == arm) for arm in arms}
        med = case['medianUserSeconds']
        case['candidateDeltaPercent'] = (med['candidate'] / med['main'] - 1) * 100
        case['exactExecutionMatch'] = True
        write('results.partial.json', result)
    for case in cases.values():
        for item in case['inputs'].values():
            if sha(pathlib.Path(item['path'])) != item['sha256']:
                raise RuntimeError('A firmware input changed during timing')
    result.update(status='complete', completedAt=now())
    write('results.json', result)
    print(json.dumps({name: {'medianUserSeconds': c['medianUserSeconds'], 'candidateDeltaPercent': c['candidateDeltaPercent'], 'exactExecutionMatch': c['exactExecutionMatch']} for name, c in cases.items()}, indent=2), flush=True)
except Exception as error:
    result.update(status='failed', failedAt=now(), error=str(error))
    write('results.partial.json', result)
    raise
