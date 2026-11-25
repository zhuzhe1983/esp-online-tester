#!/usr/bin/env python3
"""Compile the owned SHTC3 source/tests with the actual I2cDevice declaration.

No Cargo, controller, CPU, board, guest or copied protocol engine is involved.
The only extracted text is the unchanged public trait; the sensor is included
directly by path. Temporary compiler outputs stay in the requested directory.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out-dir', type=Path, default=Path('target/shtc3-model-check'))
    parser.add_argument('--receipt', type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    trait_source = root / 'esp-periph/src/i2c.rs'
    sensor = root / 'esp-periph/src/i2c/shtc3.rs'
    text = trait_source.read_text()
    start = text.index('pub trait I2cDevice {')
    end = text.index('\npub const INT_END_DETECT:', start)
    trait = text[start:end]
    directory = args.out_dir.resolve()
    directory.mkdir(parents=True, exist_ok=True)
    harness = directory / 'shtc3.rs'
    harness.write_text('pub mod i2c {\n' + trait + '\n#[path=' + json.dumps(str(sensor)) + ']\npub mod shtc3;\n}\n')
    binary = directory / 'shtc3-tests'
    version = subprocess.check_output(['rustc', '--version'], text=True).strip()
    subprocess.run(['rustc', '--edition=2021', '--test', '-D', 'warnings', '--crate-name', 'shtc3_contract', str(harness), '-o', str(binary)], check=True)
    result = subprocess.run([str(binary), '--test-threads=1'], check=True, capture_output=True, text=True)
    print(result.stdout, end='')
    if result.stderr:
        print(result.stderr, end='')
    receipt = {
        'kind': 'isolated-owned-sensor-unit-tests', 'rustc': version,
        'sensorSha256': hashlib.sha256(sensor.read_bytes()).hexdigest(),
        'i2cTraitSha256': hashlib.sha256(trait.encode()).hexdigest(),
        'harness': 'includes actual SHTC3 file and extracts current unchanged I2cDevice declaration',
        'compileWarningsDenied': True, 'result': 'pass',
        'testSummary': next(line for line in result.stdout.splitlines() if line.startswith('test result:')),
        'controllerExecuted': False, 'factoryFirmwareExecuted': False,
    }
    if args.receipt:
        args.receipt.write_text(json.dumps(receipt, indent=2) + '\n')


if __name__ == '__main__':
    main()
