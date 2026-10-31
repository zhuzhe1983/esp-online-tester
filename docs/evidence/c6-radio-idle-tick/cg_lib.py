"""Whole-binary codegen comparison (alignment nops dropped), base vs candidate: one objdump per binary, split per symbol,
normalized as in codegen.py (addresses dropped, branch targets as symbol names or self-relative,
adrp pages and their page offsets masked, import stubs unnamed). Reports, per binary, how many
symbols differ and which, and separately every differing symbol that belongs to an Xtensa chip
crate (esp32, esp32s3, xtensa_lx7) or is a generic instantiated for one.
usage: codegen_all.py <base-dir> <cand-dir> <out.json>"""
import subprocess, sys, re, json
from pathlib import Path
base, cand, out = None, None, None
XT = re.compile(r'(_5esp32[0-9A-Z_]|7esp32s3|10xtensa_lx7)')
def load(b):
    t = subprocess.run(['objdump', '-d', '--no-show-raw-insn', str(b)], capture_output=True, text=True).stdout
    syms, cur, paged = {}, None, set()
    for l in t.splitlines():
        m = re.match(r'^[0-9a-f]+ <(.+)>:$', l)
        if m: cur = m.group(1); syms[cur] = []; paged = set(); continue
        if cur is None or not re.match(r'^\s*[0-9a-f]+:\s', l): continue
        ins = re.sub(r';.*$', '', l.split(':', 1)[1].strip()).strip()
        m = re.match(r'adrp\s+(\w+),', ins)
        if m: paged.add(m.group(1)); syms[cur].append(f'adrp {m.group(1)}, PAGE'); continue
        m = re.match(r'(add|ldr\w*|str\w*)\s+(\w+),\s*(?:\[)?(\w+),\s*#(0x[0-9a-f]+|\d+)', ins)
        if m and m.group(3) in paged: ins = re.sub(r'#(0x[0-9a-f]+|\d+)', '#PAGEOFF', ins, count=1)
        dst = re.match(r'\w+\s+(\w+)', ins)
        if dst and not (m and m.group(3) in paged and m.group(1) == 'add' and m.group(2) == m.group(3)): paged.discard(dst.group(1))
        c = cur
        ins = re.sub(r'0x([0-9a-f]+) <([^>+]+)(\+0x[0-9a-f]+)?>', lambda m: f'<{"self" if m.group(2) == c else m.group(2)}{m.group(3) or ""}>', ins)
        ins = re.sub(r'<dyld_stub_binder\+0x[0-9a-f]+>', '<import-stub>', ins)
        ins = re.sub(r'<(GCC_except_table|ltmp|l_)[^>]*>', '<data>', ins)
        if ins != "nop": syms[cur].append(canon(ins))
    keys = [canon(k) for k in syms]
    print(b.name, 'canonical-name collisions', len(keys) - len(set(keys)), file=sys.stderr)
    return {canon(k): v for k, v in syms.items()}
# impl-block ordinals in v0 mangling (Ms5_, Xsj_) shift when a crate gains an impl block; they name
# the same item, so compare names with the ordinal dropped
def canon(s): return re.sub(r'(?<=[MX])s[0-9a-z]*_', 's_', s)
