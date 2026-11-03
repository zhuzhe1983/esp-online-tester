#!/usr/bin/env python3
"""Run the unchanged public ST driver against the Rust I2C model; no downloads."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument('source', type=Path, help='local m5stack/M5StampFly source checkout/archive')
args = parser.parse_args()
here = Path(__file__).resolve().parent
root = here.parents[3]
source = args.source / 'lib/vl53l3c'
files = sorted(p for p in source.iterdir() if p.suffix in ('.c', '.h'))
digest = hashlib.sha256()
for path in files:
    digest.update(path.name.encode() + b'\0' + path.read_bytes() + b'\0')
expected = '5fe2ab98f3879e6de905bf89a7d18233a8d23386c8202f6e25354d3c597fb49f'
if digest.hexdigest() != expected:
    raise SystemExit('Source mismatch: require M5StampFly 2b96752c72c465c63b260996046c71c7675f81b2 unmodified lib/vl53l3c')
module = root / 'esp-periph/src/i2c/vl53l3cx.rs'
with tempfile.TemporaryDirectory(prefix='vl53l3cx-reference-') as scratch:
    work = Path(scratch)
    wrapper = (here / 'wrapper.rs.in').read_text().replace('@MODULE_PATH@', str(module).replace('\\', '\\\\').replace('"', '\\"'))
    (work / 'wrapper.rs').write_text(wrapper)
    subprocess.run(['rustc', '--edition=2021', '--test', str(work / 'wrapper.rs'), '-o', str(work / 'unit-tests')], check=True)
    subprocess.run([str(work / 'unit-tests')], check=True)
    subprocess.run(['rustc', '--edition=2021', '--crate-type', 'staticlib', str(work / 'wrapper.rs'), '-o', str(work / 'libmodel.a')], check=True)
    # ESP32 platform I2C/init and optional logging are replaced by the harness. Every
    # sensor algorithm, API, register encoder and NVM implementation stays unchanged.
    excluded = {'vl53lx_platform.c', 'vl53lx_platform_init.c', 'vl53lx_platform_log.c'}
    algorithms = [str(p) for p in files if p.suffix == '.c' and p.name not in excluded]
    subprocess.run(['cc', '-O0', '-g', '-DSTDINT_H', '-I' + str(source), str(here / 'driver.c'), *algorithms,
                    str(work / 'libmodel.a'), '-lm', '-o', str(work / 'driver')], check=True)
    result = subprocess.run([str(work / 'driver')], capture_output=True, text=True)
    print(result.stdout, end='')
    if result.returncode:
        raise SystemExit(result.stderr)
print(json.dumps({'sourceCommit': '2b96752c72c465c63b260996046c71c7675f81b2',
                  'sourceFileCount': len(files), 'sourceAggregateSha256': expected,
                  'moduleSha256': hashlib.sha256(module.read_bytes()).hexdigest(),
                  'result': 'pass'}, indent=2))
