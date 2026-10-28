#!/usr/bin/env python3
"""Extract one final linked hot-path pair, without source/debug metadata."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import re
import subprocess

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--base', type=Path, required=True)
p.add_argument('--head', type=Path, required=True)
p.add_argument('--out', type=Path, required=True)
a = p.parse_args()
a.out.mkdir(parents=True, exist_ok=True)
names = ['tick', 'fetch', 'read8', 'read16', 'read32', 'store']
collected, inputs = {}, {}
for label, exe in [('base', a.base), ('head', a.head)]:
    inputs[label] = hashlib.sha256(exe.read_bytes()).hexdigest()
    text = subprocess.check_output(['xcrun', 'llvm-objdump', '--disassemble', '--demangle', str(exe)], text=True)
    funcs, key = {}, None
    for line in text.splitlines():
        m = re.match(r'^([0-9a-f]+) <(.+)>:$', line)
        if m:
            key = m[2]
            funcs[key] = []
        elif key and re.match(r'^\s*[0-9a-f]+:', line):
            funcs[key].append(line)
    selected = {}
    for name in names:
        key = '<esp32::bus::SocBus>::store' if name == 'store' else '<esp32::bus::SocBus as emu_core::bus::Bus>::' + name
        selected[name] = funcs[key]
        assert selected[name], key
    collected[label] = selected
    # Keep operands/targets, but omit redundant opcode bytes and print addresses
    # relative to each function. The linked binaries are identified by SHA-256.
    def compact(lines):
        start = int(lines[0].split(':')[0], 16)
        out = []
        for line in lines:
            m = re.match(r'^\s*([0-9a-f]+):\s*[0-9a-f]+\s+(.*)$', line)
            assert m, line
            out.append(f'+0x{int(m[1], 16) - start:x}: {m[2]}')
        return '\n'.join(out)
    payload = '\n\n'.join(name + '\n' + compact(selected[name]) for name in names) + '\n'
    (a.out / (label + '-hotpaths.disasm.gz')).write_bytes(gzip.compress(payload.encode(), mtime=0))
summary = []
for name in names:
    b, h = collected['base'][name], collected['head'][name]
    def mnemonics(lines):
        return [re.sub(r'^\s*[0-9a-f]+:\s*[0-9a-f]+\s+', '', line).split()[0] for line in lines]
    summary.append(dict(function=name, base_instructions=len(b), head_instructions=len(h), same_mnemonic_sequence=mnemonics(b) == mnemonics(h)))
(a.out / 'disassembly.json').write_text(json.dumps(dict(binary_sha256=inputs, functions=summary), indent=2) + '\n')
print(json.dumps(summary, indent=2))
