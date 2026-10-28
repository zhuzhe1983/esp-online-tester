#!/usr/bin/env python3
"""Compare every function present in two disassemblies after normalising addresses.

Usage: asmcmp.py MAIN.dis BRANCH.dis [skip-regex] [function names to print diffs for...]

Normalised away: instruction addresses, branch/call target addresses and +offsets, `nop`
alignment padding, `udf` traps after no-return calls, and the page offsets that pair with an
`adrp` (constant-pool / data addresses). Call targets may carry different linker-chosen names
(identical-code folding picks one alias, LTO adds `(.N)` suffixes); two call lines match when
the targets' bodies have the same shape (their own call names replaced). Everything else
(opcodes, registers, stack and field offsets, immediates, instruction order) must match.
"""
import re, sys, collections, difflib

def funcs(path):
    out = collections.defaultdict(list)
    cur = None
    adrp_regs = set()
    for l in open(path):
        if l.endswith(":\n") and not l.startswith(" ") and "<" in l:
            cur = l[l.index("<") + 1:l.rindex(">")]
            adrp_regs = set()
            out[cur].append([])
            continue
        if cur is None or not l.strip() or ":" not in l:
            continue
        ins = re.sub(r"^\s*[0-9a-f]+:\s*", "", l).strip()
        if "<" not in ins:
            ins = re.sub(r"\s*;.*$", "", ins)  # `; =imm` comments; symbol names may contain `;`
        op = ins.split("\t")[0]
        args = ins.split("\t")[1] if "\t" in ins else ""
        if op in ("nop", "udf"):
            continue
        if op == "adrp":
            adrp_regs.add(args.split(",")[0])
            ins = "adrp " + args.split(",")[0]
        else:
            src = re.findall(r"\[(x\d+)|, (x\d+), #", args)
            regs = {r for t in src for r in t if r}
            if regs & adrp_regs:
                ins = re.sub(r"#-?0x[0-9a-f]+|#\d+", "#PAGEOFF", ins)
            adrp_regs.discard(args.split(",")[0].strip())
        ins = re.sub(r"0x[0-9a-f]+ <(.*?)(\+0x[0-9a-f]+)?>$", r"<\1>", ins)
        ins = re.sub(r"<(GCC_except_table|ltmp|l_|dyld_stub_binder)\w*>", "<DATA>", ins)
        out[cur][-1].append(ins)
    return {n: [i for body in v for i in body] for n, v in out.items()}

TGT = re.compile(r"<(.*)>$")

def shape(fs, n):
    return tuple(TGT.sub("<FN>", i) for i in fs.get(n, ()))

def same(fa, fb, name):
    x, y = fa[name], fb[name]
    if len(x) != len(y):
        return False
    for i, j in zip(x, y):
        if i == j:
            continue
        mi, mj = TGT.search(i), TGT.search(j)
        if not (mi and mj) or TGT.sub("", i) != TGT.sub("", j):
            return False
        ti, tj = mi.group(1), mj.group(1)
        if ti == name or tj == name or not shape(fa, ti) or shape(fa, ti) != shape(fb, tj):
            return False
    return True

a, b = funcs(sys.argv[1]), funcs(sys.argv[2])
skip = re.compile(sys.argv[3]) if len(sys.argv) > 3 and sys.argv[3] else None
ident = diff = 0
for name in sorted(set(a) & set(b)):
    if skip and skip.search(name):
        continue
    if same(a, b, name):
        ident += 1
    else:
        diff += 1
        print(f"DIFF {len(a[name]):6d} {len(b[name]):6d}  {name[:200]}")
print(f"identical={ident} different={diff} only_main={len(set(a)-set(b))} only_branch={len(set(b)-set(a))}")
for name in sys.argv[4:]:
    d = [l for l in difflib.unified_diff(a[name], b[name], lineterm="", n=0) if l[:1] in "+-" and not l.startswith(("+++", "---"))]
    print("==", name, len(d)); print("\n".join(d[:30]))
