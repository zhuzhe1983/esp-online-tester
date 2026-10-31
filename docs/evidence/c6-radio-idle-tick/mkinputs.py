"""Write inputs.json: SHA-256 of every fixture input used here and of the four release binaries per arm.
usage: mkinputs.py <worktree> <starter-dir> <bins-dir> <out.json>"""
import sys, json, hashlib
from pathlib import Path
tree, starter, bins, out = map(Path, sys.argv[1:5])
h = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
fw = tree / 'web/wasm/fw'
files = {}
for n in ('c6-hello', 'c6-contiki', 'c6-energy-scan', 'c3-hello', 'p4-hello'):
    j = json.loads((fw / f'{n}.json').read_text())
    for k in ('rom', 'bootloader', 'ptable', 'app'):
        f = j['files'].get(k)
        if f: files[f] = h(fw / f)
man = json.loads((starter / 'manifest.json').read_text())
for a in man['artifacts']: assert h(starter / a['filename']) == a['sha256'], a
res = {'fetched_by': 'tools/fetch-demo-assets.sh --no-linux', 'firmware': files,
       'public_display_starter': {'board': 'waveshare-c6-lcd147', 'flash_mb': 8, 'artifacts': man['artifacts']},
       'binaries': {arm: {b.name: h(b) for b in sorted((bins / arm).iterdir())} for arm in ('base', 'cand')},
       'build': 'cargo +1.99.0 build --release --bins', 'base_commit': '2e36224d', 'candidate_code_commit': '69e441e65efb96c8c42df9dabb8870107cc5e670'}
out.write_text(json.dumps(res, indent=1) + '\n')
print(json.dumps(res['binaries'], indent=1))
