#!/usr/bin/env python3
"""Run the external S31 tests with caller-built fixtures and the pinned public ROM."""
import argparse
import hashlib
import os
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--fixtures', type=Path, required=True)
parser.add_argument('--rom', type=Path, required=True)
args = parser.parse_args()
rom = args.rom.resolve()
if hashlib.sha256(rom.read_bytes()).hexdigest() != 'a52ad7513deb656a910a5740125f1cce2c7941f11ce57213b7b43aea93d5ab87':
    parser.error('expected the public 20260528 esp32s31_rev0_rom.elf; see docs/esp32s31.md')
fixtures = args.fixtures.resolve()
env = os.environ.copy()
for elf in fixtures.glob('*.elf'):
    key = {'probe': 'UART', 'irq-probe': 'UART_IRQ'}.get(elf.stem, elf.stem.removesuffix('-probe').replace('-', '_').upper())
    env[f'S31_{key}_ELF'] = str(elf)
env.update(S31_ROM_ELF=str(rom), S31_FLASH_WORKER_BIN=str(fixtures / 'flash-worker.bin'),
           S31_BOOT_IMAGE=str(fixtures / 'spi0-observation.bin'))
for required in ('probe.elf', 'irq-probe.elf', 'crypto-clock-probe.elf', 'flash-worker.bin', 'spi0-observation.bin'):
    if not (fixtures / required).is_file():
        parser.error(f'missing {required}; build examples/s31-bringup with S31_BUILD_ROM_IMAGE=1')
subprocess.run(['cargo', 'test', '--release', '-p', 'esp32s31', '--test', 'uart_fixture', '--',
                '--include-ignored', '--nocapture', '--test-threads=1'],
               cwd=Path(__file__).resolve().parents[1], env=env, check=True)
