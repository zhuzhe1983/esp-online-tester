"""Hot-path disassembly, base vs candidate: every symbol matching the selectors in each binary,
normalized (addresses dropped, branch targets as function-relative offsets or callee names,
literal-pool adrp/add pages masked), compared exactly. Writes codegen.json.
usage: codegen.py <base-dir> <cand-dir> <out.json>"""
import subprocess, sys, re, json, hashlib
from pathlib import Path
base, cand, out = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
SEL = [r'run_unmodeled', r'run_until_cycle', r'after_round', r'idle_budget', r'13next_deadline', r'19idle_deadline_slack',
       r'SocBus.*3Bus4tick', r'SocBus.*3Bus5fetch', r'11refresh_irq', r'12step_outcome', r'step_blocks', r'vq_close_round', r'6decode', r'4core.*3run',
       r'17optional_deadline', r'Device4tick', r'8Dispatch13tick_optional', r'20deliver_board_events', r'10TimerGroup', r'8Systimer']
def syms(b):
    out = subprocess.run(['nm', '--defined-only', str(b)], capture_output=True, text=True).stdout
    return sorted({l.split()[-1] for l in out.splitlines() if l.split()[1].lower() == 't' and any(re.search(s, l) for s in SEL) and 'RecordingBus' not in l})
def dis(b, s):
    t = subprocess.run(['objdump', '-d', '--no-show-raw-insn', f'--disassemble-symbols={s}', str(b)], capture_output=True, text=True).stdout
    lines = [l for l in t.splitlines() if re.match(r'^\s*[0-9a-f]+:\s', l)]
    if not lines: return []
    norm, paged = [], set()
    for l in lines:
        ins = l.split(':', 1)[1].strip()
        ins = re.sub(r';.*$', '', ins).strip()
        m = re.match(r'adrp\s+(\w+),', ins)
        if m: paged.add(m.group(1)); norm.append(f'adrp {m.group(1)}, PAGE'); continue
        # An omitted immediate on a page-based load/store is the zero page offset.
        z = re.match(r'(?:ldr\w*|str\w*)\s+\w+,\s*\[(\w+)\]$', ins)
        if z and z.group(1) in paged: ins = ins[:-1] + ', #0]'
        # data placement: the page offset applied to an adrp'd register is layout, not code
        m = re.match(r'(add|ldr\w*|str\w*)\s+(\w+),\s*(?:\[)?(\w+),\s*#(0x[0-9a-f]+|\d+)', ins)
        if m and m.group(3) in paged: ins = re.sub(r'#(0x[0-9a-f]+|\d+)', '#PAGEOFF', ins, count=1)
        dst = re.match(r'\w+\s+(\w+)', ins)
        if dst and not (m and m.group(3) in paged and m.group(1) == 'add' and m.group(2) == m.group(3)): paged.discard(dst.group(1))
        ins = re.sub(r'0x([0-9a-f]+) <([^>+]+)(\+0x[0-9a-f]+)?>', lambda m: f'<{"self" if m.group(2) == s else m.group(2)}{m.group(3) or ""}>', ins)
        # calls into the import stubs (memcpy, memset, ...) carry no name in the listing
        ins = re.sub(r'<dyld_stub_binder\+0x[0-9a-f]+>', '<import-stub>', ins)
        norm.append(ins)
    return norm
res = {}
for b in sorted(p.name for p in base.iterdir()):
    sb, sc = syms(base / b), syms(cand / b)
    rows = []
    for s in sorted(set(sb) | set(sc)):
        db = dis(base / b, s) if s in sb else None; dc = dis(cand / b, s) if s in sc else None
        rows.append({'symbol': s, 'base_insns': len(db) if db is not None else None, 'cand_insns': len(dc) if dc is not None else None,
                     'identical': db == dc, 'base_sha256': hashlib.sha256('\n'.join(db).encode()).hexdigest() if db is not None else None,
                     'cand_sha256': hashlib.sha256('\n'.join(dc).encode()).hexdigest() if dc is not None else None})
    res[b] = rows
    diff = [r for r in rows if not r['identical']]
    print(b, f'{len(rows) - len(diff)}/{len(rows)} identical')
    for r in diff: print('   differs:', r['symbol'][:110], r['base_insns'], '->', r['cand_insns'])
out.write_text(json.dumps(res, indent=1) + '\n')
