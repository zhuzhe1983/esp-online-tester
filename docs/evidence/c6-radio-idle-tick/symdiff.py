"""For each differing esp32sim-c6 symbol (codegen-classes.json 'code'), print a unified diff of the
normalized instruction lists (cg_lib.load), so same-length symbols can be checked to differ only in
radio field offsets. usage: symdiff.py [substr]"""
import sys, json, difflib, re
from pathlib import Path
sub = sys.argv[1] if len(sys.argv) > 1 else ''
sys.argv = sys.argv[:1]
import cg_lib
D = Path(__file__).parent
sb, sc = cg_lib.load(D / 'bins/base/esp32sim-c6'), cg_lib.load(D / 'bins/cand/esp32sim-c6')
cls = json.load(open(D / 'codegen-classes.json'))['esp32sim-c6']
for k, v in cls.items():
    if v['class'] != 'code' or sub not in k: continue
    a, c = sb.get(k, []), sc.get(k, [])
    d = [l for l in difflib.unified_diff(a, c, lineterm='', n=0) if not l.startswith(('---', '+++'))]
    off = all(re.sub(r'#(0x[0-9a-f]+|\d+)', '#N', x) == re.sub(r'#(0x[0-9a-f]+|\d+)', '#N', y) for x, y in zip(a, c)) and len(a) == len(c)
    print(f'== {k[-70:]} {len(a)} -> {len(c)} same-shape-immediates-only={off} diff-lines={len(d)}')
    if not off: print('\n'.join(d[:200]))
