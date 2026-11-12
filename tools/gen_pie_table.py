#!/usr/bin/env python3
"""Generate xtensa-lx7/src/pie_table.rs from the ESP32-S3 TRM chapter-1 instruction layouts (pie/trm.json,
produced by parsing the TRM text: one entry per 1.8.N section with the 'Instruction Word' bit layout)."""
import json, re, sys
src, dst = sys.argv[1], sys.argv[2]
trm = json.load(open(src))
ROLES = set()
def role_of(name, distinct_selectors=False):
    n = name.lower()
    if n.startswith('imm'): r = 'Imm'
    elif n.startswith('sel'): r = n.capitalize() if distinct_selectors else 'Sel'
    elif n.startswith('upd'): r = 'Upd'
    elif n.startswith('sar'): r = 'Sar'
    else: r = n[0].upper() + n[1:]
    ROLES.add(r); return r
def imm_spec(name):   # (signed, scale)
    n = name.lower()
    return {'imm1': (False, 1), 'imm2': (False, 2), 'imm4': (True, 4), 'imm8': (True, 8), 'imm16': (True, 16), 'imm16f': (True, 16), 'imm': (True, 16)}.get(n, (False, 1))
def kind(mn):
    m = mn[3:] if mn.startswith('ee.') else mn
    mode = lambda s: {'ip': 'Mode::Ip', 'xp': 'Mode::Xp', 'incp': 'Mode::Incp', None: 'Mode::None', '': 'Mode::None'}[s]
    r = lambda p: re.fullmatch(p, m)
    if x := r(r'vld\.128\.(ip|xp)'): return f'Kind::Vld128({mode(x[1])})'
    if x := r(r'vld\.(l|h)\.64\.(ip|xp)'): return f'Kind::Vld64 {{ high: {str(x[1]=="h").lower()}, mode: {mode(x[2])} }}'
    if x := r(r'ld\.128\.usar\.(ip|xp)'): return f'Kind::LdUsar({mode(x[1])})'
    if x := r(r'vldbc\.(8|16|32)(?:\.(ip|xp))?'): return f'Kind::Ldbc {{ w: {x[1]}, mode: {mode(x[2])} }}'
    if r(r'vldhbc\.16\.incp'): return 'Kind::Ldhbc16'
    if r(r'ldxq\.32'): return 'Kind::Ldxq32'
    if x := r(r'ldqa\.(s8|s16|u8|u16)\.128\.(ip|xp)'): return f'Kind::Ldqa {{ w: {x[1][1:]}, signed: {str(x[1][0]=="s").lower()}, mode: {mode(x[2])} }}'
    if r(r'ld\.accx\.ip'): return 'Kind::LdAccx'
    if r(r'st\.accx\.ip'): return 'Kind::StAccx'
    if x := r(r'(ld|st)\.qacc_(h|l)\.(h\.32|l\.128)\.ip'): return f'Kind::{"Ld" if x[1]=="ld" else "St"}Qacc {{ h: {str(x[2]=="h").lower()}, high32: {str(x[3]=="h.32").lower()} }}'
    if r(r'ld\.ua_state\.ip'): return 'Kind::LdUa'
    if r(r'st\.ua_state\.ip'): return 'Kind::StUa'
    if x := r(r'ldf\.(64|128)\.(ip|xp)'): return f'Kind::Ldf {{ n: {int(x[1])//32}, mode: {mode(x[2])} }}'
    if x := r(r'stf\.(64|128)\.(ip|xp)'): return f'Kind::Stf {{ n: {int(x[1])//32}, mode: {mode(x[2])} }}'
    if x := r(r'vst\.128\.(ip|xp)'): return f'Kind::Vst128({mode(x[1])})'
    if x := r(r'vst\.(l|h)\.64\.(ip|xp)'): return f'Kind::Vst64 {{ high: {str(x[1]=="h").lower()}, mode: {mode(x[2])} }}'
    if r(r'movi\.32\.a'): return 'Kind::MoviA'
    if r(r'movi\.32\.q'): return 'Kind::MoviQ'
    if r(r'zero\.q'): return 'Kind::ZeroQ'
    if r(r'zero\.qacc'): return 'Kind::ZeroQacc'
    if r(r'zero\.accx'): return 'Kind::ZeroAccx'
    if x := r(r'mov\.(s8|s16|u8|u16)\.qacc'): return f'Kind::MovQacc {{ w: {x[1][1:]}, signed: {str(x[1][0]=="s").lower()} }}'
    if x := r(r'(andq|orq|xorq|notq)'): return f'Kind::{x[1].capitalize()}'
    if r(r'vsl\.32'): return 'Kind::Vsl32'
    if r(r'vsr\.32'): return 'Kind::Vsr32'
    if r(r'slcxxp\.2q'): return 'Kind::Slcxxp'
    if r(r'slci\.2q'): return 'Kind::Slci'
    if r(r'srci\.2q'): return 'Kind::Srci'
    if r(r'srcxxp\.2q'): return 'Kind::Srcxxp'
    if x := r(r'src\.q(\.qup)?'): return f'Kind::SrcQ {{ qup: {str(bool(x[1])).lower()}, ld: Mode::None }}'
    if x := r(r'src\.q\.ld\.(ip|xp)'): return f'Kind::SrcQ {{ qup: false, ld: {mode(x[1])} }}'
    if x := r(r'srcmb\.(s8|s16)\.qacc'): return f'Kind::Srcmb {{ w: {x[1][1:]} }}'
    if r(r'srs\.accx'): return 'Kind::SrsAccx'
    if x := r(r'(vadds|vsubs|vmax|vmin)\.(s8|s16|s32)(?:\.(ld|st)\.incp)?'):
        op = {'vadds': 'ArithOp::Adds', 'vsubs': 'ArithOp::Subs', 'vmax': 'ArithOp::Max', 'vmin': 'ArithOp::Min'}[x[1]]
        return f'Kind::Arith {{ op: {op}, w: {x[2][1:]}, ld: {str(x[3]=="ld").lower()}, st: {str(x[3]=="st").lower()} }}'
    if x := r(r'vmul\.(s8|s16|u8|u16)(?:\.(ld|st)\.incp)?'):
        return f'Kind::Arith {{ op: ArithOp::Mul {{ signed: {str(x[1][0]=="s").lower()} }}, w: {x[1][1:]}, ld: {str(x[2]=="ld").lower()}, st: {str(x[2]=="st").lower()} }}'
    if x := r(r'vcmp\.(eq|lt|gt)\.(s8|s16|s32)'): return f'Kind::Vcmp {{ cmp: Cmp::{x[1].capitalize()}, w: {x[2][1:]} }}'
    if x := r(r'vrelu\.(s8|s16)'): return f'Kind::Vrelu {{ w: {x[1][1:]} }}'
    if x := r(r'vprelu\.(s8|s16)'): return f'Kind::Vprelu {{ w: {x[1][1:]} }}'
    if x := r(r'vzip\.(8|16|32)'): return f'Kind::Vzip {{ w: {x[1]} }}'
    if x := r(r'vunzip\.(8|16|32)'): return f'Kind::Vunzip {{ w: {x[1]} }}'
    if x := r(r'vmulas\.(s8|u8|s16|u16)\.(accx|qacc)(?:\.ld\.(ip|xp)|\.(ldbc)\.incp)?(\.qup)?'):
        ld = 'LdKind::Ldbc' if x[4] else {'ip': 'LdKind::Ip', 'xp': 'LdKind::Xp', None: 'LdKind::None'}[x[3]]
        return f'Kind::Vmulas {{ signed: {str(x[1][0]=="s").lower()}, w: {x[1][1:]}, accx: {str(x[2]=="accx").lower()}, ld: {ld}, qup: {str(bool(x[5])).lower()} }}'
    if x := r(r'vsmulas\.(s8|s16)\.qacc(\.ld\.incp)?'): return f'Kind::Vsmulas {{ w: {x[1][1:]}, ld: {str(bool(x[2])).lower()} }}'
    if x := r(r'fft\.cmul\.s16\.(ld|st)\.xp'): return f'Kind::Cmul {{ store: {str(x[1]=="st").lower()} }}'
    if r(r'ld\.qr'): return 'Kind::LdQr'
    if r(r'st\.qr'): return 'Kind::StQr'
    if r(r'mv\.qr'): return 'Kind::MvQr'
    return 'Kind::Unimpl'
out = ['// GENERATED by tools/gen_pie_table.py from the ESP32-S3 TRM (chapter 1, Instruction Word layouts). Do not edit.',
       'use crate::pie::*;', '', 'pub static OPS: &[PieInsn] = &[']
n = 0
for name in sorted(trm):
    v = trm[name]
    if not v.get('width_ok'): continue
    ln = v['len']; nbits = 8 * ln
    mask = 0; value = 0
    fields = {}
    for f in v['fields']:
        if 'const' in f:
            w = len(f['const']); mask |= ((1 << w) - 1) << f['wpos']; value |= int(f['const'], 2) << f['wpos']
        else:
            fields.setdefault(f['name'], []).append((f['hi'], f['lo'], f['wpos']))
    syn = v['syntax'].splitlines()[0]
    m = re.match(r'\S+\s*(.*)', syn); toks = [t.strip() for t in m.group(1).split(',')] if m and m.group(1).strip() else []
    order = []; used = set()
    prev = None
    for t in toks:
        if t in fields: order.append(t); used.add(t)
        elif re.fullmatch(r'-?\d+(\.\.-?\d+)?', t) or t == 'imm':
            if prev == 'imm' and t != 'imm': continue
            cand = [k for k in fields if k not in used and k.lower().startswith(('imm', 'sel', 'sar', 'upd'))]
            if cand: order.append(cand[0]); used.add(cand[0])
        prev = t
    for k in fields:
        if k not in used: order.append(k)
    mn = name.lower()
    distinct_selectors = len([k for k in fields if k.lower().startswith('sel')]) > 1
    fl = []
    for k in order:
        pieces = ', '.join(f'({hi}, {lo}, {wp})' for hi, lo, wp in fields[k])
        role = role_of(k, distinct_selectors)
        signed, scale = imm_spec(k) if role == 'Imm' else (False, 1)
        fl.append(f'Field {{ role: Role::{role}, pieces: &[{pieces}], signed: {str(signed).lower()}, scale: {scale} }}')
    out.append(f'    PieInsn {{ name: "{mn}", len: {ln}, mask: 0x{mask:x}, value: 0x{value:x}, fields: &[{", ".join(fl)}], kind: {kind(mn)} }},')
    n += 1
out.append('];')
out.insert(2, '#[derive(Clone, Copy, PartialEq, Eq, Debug)]\npub enum Role { ' + ', '.join(sorted(ROLES)) + ' }\n')
open(dst, 'w').write('\n'.join(out) + '\n')
print(n, 'entries written')
