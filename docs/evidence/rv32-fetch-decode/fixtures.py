"""C3/C6/P4 fixture command lines shared by the EX249 scripts. ESP32SIM_TREE: the checkout whose
web/wasm/fw holds the fetched assets (default: current directory); C6_STARTER: a directory with
the public display starter's four images and manifest.json (offsets and SHA-256, inputs.json);
FIXTURES: space-separated fixture names (default: the four C6 ones)."""
from pathlib import Path
import json, hashlib, os
TREE = Path(os.environ.get('ESP32SIM_TREE', '.')).resolve()
FW = TREE / 'web/wasm/fw'
STARTER = Path(os.environ.get('C6_STARTER', 'starter'))

def fixture_args(name):
    if name == 'public-display-starter':
        a = ['--boot', 'rom', '--rom', str(FW / 'esp32c6_rev0_rom.elf'), '--no-dump', '--flash-mb', '8', '--board', 'waveshare-c6-lcd147']
        man = json.loads((STARTER / 'manifest.json').read_text())
        for f in man['artifacts']:
            p = STARTER / f['filename']
            assert hashlib.sha256(p.read_bytes()).hexdigest() == f['sha256']
            a += ['--flash-at', f"{f['offset']:x}={p}"]
        return a
    j = json.loads((FW / f'{name}.json').read_text())
    board = j['board']
    a = ['--boot', 'rom', '--rom', str(FW / j['files'].get('rom', 'esp32c6_rev0_rom.elf')), '--no-dump']
    a += ['--flash-mb', str(j['flash_mb']), '--board', 'none' if board in ('esp32c6', 'esp32c3', 'esp32p4') else board]
    for k in ('bootloader', 'ptable', 'app'):
        a += [f'--{k}', str(FW / j['files'][k])]
    return a

FIXTURES = os.environ.get('FIXTURES', 'c6-hello c6-contiki c6-energy-scan public-display-starter').split()
