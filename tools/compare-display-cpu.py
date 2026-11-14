#!/usr/bin/env python3
"""Alternating native CPU regression check: three hello demos and Pocket Tank.

Build both revisions with `cargo build --release --bins`. Input directories and
binary directories are explicit. Retains hashes and counts, never serial text.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import resource
import statistics
import subprocess
import time


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ['baseline', 'candidate', 'firmware', 'roms', 'output']:
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--rounds', type=int, default=3)
    parser.add_argument('--seconds', type=int, default=30,
                        help='guest seconds per run; use a pre-reset window when reset semantics intentionally differ')
    parser.add_argument('--workload', action='append', choices=['hello', 'c3-hello', 'c6-hello', 'pocket-tank', 's31-hello'])
    parser.add_argument('--s31-firmware', type=Path, help='directory containing the pinned S31 hello images')
    args = parser.parse_args()
    if args.rounds < 3:
        parser.error('at least three alternating rounds are required')
    if args.seconds < 1:
        parser.error('guest seconds must be positive')
    if args.workload and 's31-hello' in args.workload and not args.s31_firmware:
        parser.error('--workload s31-hello requires --s31-firmware')
    receipt = {'method': 'RUSAGE_CHILDREN user CPU delta around one subprocess; alternating AB/BA rounds',
               'guestSeconds': args.seconds, 'rounds': args.rounds, 'workloads': {}}
    for name, binary in [('hello', 'esp32sim'), ('c3-hello', 'esp32sim-c3'),
                         ('c6-hello', 'esp32sim-c6'), ('pocket-tank', 'esp32sim'), ('s31-hello', 'esp32sim-s31')]:
        if args.workload and name not in args.workload:
            continue
        if name == 's31-hello':
            if not args.s31_firmware:
                continue
            manifest = {'flash_mb': 2, 'psram_mb': 0, 'files': {
                'rom': str((args.roms / 'esp32s31_rev0_rom.elf').resolve()),
                **{key: str((args.s31_firmware / file).resolve()) for key, file in [
                    ('bootloader', 's31-hello-bootloader.bin'), ('ptable', 's31-hello-ptable.bin'),
                    ('app', 's31-hello_world.bin')]}}}
        else:
            manifest = json.loads((args.firmware / (name + '.json')).read_text())
        files = {key: (args.roms if key == 'rom' else args.firmware) / value
                 for key, value in manifest['files'].items()}
        argv = ['--boot', 'rom', '--flash-mb', str(manifest['flash_mb'])]
        if manifest['psram_mb']:
            argv += ['--psram-mb', str(manifest['psram_mb'])]
        if binary == 'esp32sim':
            argv += ['--board', manifest['board']]
        for key, path in files.items():
            argv += ['--' + key, str(path)]
        for offset, file in manifest.get('flash_at', {}).items():
            files[offset] = args.firmware / file
            argv += ['--flash-at', offset + '=' + str(files[offset])]
        argv += ['--max-seconds', str(args.seconds), '--no-dump']
        row = {'inputs': {key: sha(path) for key, path in files.items()},
               'binarySha256': {label: sha(getattr(args, label) / binary) for label in ['baseline', 'candidate']},
               'samples': []}
        for round_index in range(args.rounds):
            order = ['baseline', 'candidate'] if round_index % 2 == 0 else ['candidate', 'baseline']
            for label in order:
                before = resource.getrusage(resource.RUSAGE_CHILDREN)
                start = time.perf_counter()
                result = subprocess.run([str(getattr(args, label) / binary), *argv], capture_output=True)
                wall = time.perf_counter() - start
                after = resource.getrusage(resource.RUSAGE_CHILDREN)
                stderr = result.stderr.decode(errors='replace')
                match = re.search(r'\[emu\] stop: (.*?) — (.*?) insns .*?emulated ([\d.]+)s \((\d+) cycles\); (\d+) exceptions, (\d+) interrupts', stderr)
                if result.returncode or not match or match[1] != 'Halted' or match[3] != f'{args.seconds:.3f}':
                    raise RuntimeError(f'{name}/{label}: unexpected stop; exit={result.returncode}')
                counters = {'stop': match[1], 'instructions': match[2], 'guestSeconds': match[3],
                            'cycles': int(match[4]), 'exceptions': int(match[5]), 'interrupts': int(match[6]),
                            'stdoutSha256': hashlib.sha256(result.stdout).hexdigest()}
                sample = {'round': round_index + 1, 'label': label, 'userSeconds': after.ru_utime - before.ru_utime,
                          'systemSeconds': after.ru_stime - before.ru_stime, 'wallSeconds': wall, **counters}
                row['samples'].append(sample)
                print(name, round_index + 1, label, round(sample['userSeconds'], 6), flush=True)
        invariant = ['stop', 'instructions', 'guestSeconds', 'cycles', 'exceptions', 'interrupts', 'stdoutSha256']
        row['identicalWorkAndOutput'] = all(len({str(sample[key]) for sample in row['samples']}) == 1 for key in invariant)
        row['medianUserSeconds'] = {label: statistics.median(s['userSeconds'] for s in row['samples'] if s['label'] == label)
                                    for label in ['baseline', 'candidate']}
        row['candidateToBaselineRatio'] = row['medianUserSeconds']['candidate'] / row['medianUserSeconds']['baseline']
        receipt['workloads'][name] = row
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(receipt, indent=2) + '\n')
        if not row['identicalWorkAndOutput']:
            raise RuntimeError(f'{name}: guest work or output differs')


if __name__ == '__main__':
    main()
