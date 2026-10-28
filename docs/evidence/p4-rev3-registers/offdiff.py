# offdiff.py V1DIR V3DIR HEADER...: per register offset, the field layout (bit position, width) in v1 vs v3, names ignored.
import re, sys, os
line_reg = re.compile(r"^#define\s+([A-Z0-9_]+_REG)\s+\(\s*[A-Z0-9_]+\s*\+\s*(0x[0-9a-fA-F]+)\s*\)")
line_v = re.compile(r"^#define\s+([A-Z0-9_]+)_V\s+\(?\s*(0x[0-9a-fA-F]+)U?\s*\)?")
line_s = re.compile(r"^#define\s+([A-Z0-9_]+)_S\s+\(?\s*(\d+)\s*\)?")
def parse(p):
    regs = {}; cur = None; pend = {}
    for l in open(p):
        m = line_reg.match(l)
        if m: cur = int(m.group(2), 16); regs.setdefault(cur, {"name": m.group(1), "f": set()}); continue
        if cur is None: continue
        m = line_v.match(l)
        if m: pend[m.group(1)] = [int(m.group(2), 16), None]; continue
        m = line_s.match(l)
        if m and m.group(1) in pend:
            v = pend.pop(m.group(1))[0]; s = int(m.group(2)); w = v.bit_length()
            regs[cur]["f"].add((s, w))
    return regs
v1, v3 = sys.argv[1], sys.argv[2]
for h in sys.argv[3:]:
    a = parse(os.path.join(v1, h)); b = parse(os.path.join(v3, h))
    changed = [o for o in a if o in b and a[o]["f"] != b[o]["f"]]
    gone = [o for o in a if o not in b]; new = [o for o in b if o not in a]
    print(f"## {h}: offsets {len(a)}->{len(b)}; layout changed {len(changed)}, gone {len(gone)}, new {len(new)}")
    for o in sorted(changed):
        x, y = a[o]["f"], b[o]["f"]
        print(f"   {o:#06x} {b[o]['name']}: only v1 {sorted(x - y)}; only v3 {sorted(y - x)}")
    for o in sorted(gone): print(f"   GONE {o:#06x} {a[o]['name']}")
    for o in sorted(new): print(f"   NEW  {o:#06x} {b[o]['name']}")
