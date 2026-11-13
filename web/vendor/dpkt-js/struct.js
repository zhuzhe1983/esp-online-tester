// Minimal port of Python's `struct` module, covering the format characters
// dpkt actually uses. Operates on Uint8Array via DataView.
//
// Supported byte-order prefixes: '>' / '!' (big), '<' (little), '=' / '@' (native -> little here).
// Supported type chars (standard sizes, since dpkt always prefixes a byte order):
//   x(pad) b/B(int8/uint8) h/H(int16/uint16) i/I/l/L(int32/uint32) q/Q(int64/uint64, BigInt) s(bytes)
// A leading integer count repeats numeric types or sets the byte length for 's'.
const SIZES = {
    x: 1, b: 1, B: 1, h: 2, H: 2,
    i: 4, I: 4, l: 4, L: 4, q: 8, Q: 8, f: 4, d: 8, s: 1,
};
function isByteOrder(c) {
    return c === '>' || c === '<' || c === '!' || c === '=' || c === '@';
}
function parse(fmt) {
    let little = false; // default big-endian ('>')
    let i = 0;
    if (fmt.length && isByteOrder(fmt[0])) {
        little = fmt[0] === '<' || fmt[0] === '=' || fmt[0] === '@';
        i = 1;
    }
    const tokens = [];
    let num = '';
    for (; i < fmt.length; i++) {
        const c = fmt[i];
        if (c >= '0' && c <= '9') {
            num += c;
            continue;
        }
        if (!(c in SIZES))
            throw new Error(`struct: unsupported format char '${c}' in '${fmt}'`);
        const count = num === '' ? 1 : parseInt(num, 10);
        num = '';
        tokens.push({ type: c, count });
    }
    return { little, tokens };
}
export function calcsize(fmt) {
    const { tokens } = parse(fmt);
    let n = 0;
    for (const t of tokens) {
        n += t.type === 's' || t.type === 'x' ? t.count : SIZES[t.type] * t.count;
    }
    return n;
}
export function unpack(fmt, buf, offset = 0) {
    const { little, tokens } = parse(fmt);
    const dv = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
    const out = [];
    let off = offset;
    for (const t of tokens) {
        if (t.type === 'x') {
            off += t.count;
            continue;
        }
        if (t.type === 's') {
            out.push(buf.subarray(off, off + t.count));
            off += t.count;
            continue;
        }
        for (let r = 0; r < t.count; r++) {
            switch (t.type) {
                case 'b':
                    out.push(dv.getInt8(off));
                    off += 1;
                    break;
                case 'B':
                    out.push(dv.getUint8(off));
                    off += 1;
                    break;
                case 'h':
                    out.push(dv.getInt16(off, little));
                    off += 2;
                    break;
                case 'H':
                    out.push(dv.getUint16(off, little));
                    off += 2;
                    break;
                case 'i':
                case 'l':
                    out.push(dv.getInt32(off, little));
                    off += 4;
                    break;
                case 'I':
                case 'L':
                    out.push(dv.getUint32(off, little));
                    off += 4;
                    break;
                case 'q':
                    out.push(dv.getBigInt64(off, little));
                    off += 8;
                    break;
                case 'Q':
                    out.push(dv.getBigUint64(off, little));
                    off += 8;
                    break;
                case 'f':
                    out.push(dv.getFloat32(off, little));
                    off += 4;
                    break;
                case 'd':
                    out.push(dv.getFloat64(off, little));
                    off += 8;
                    break;
            }
        }
    }
    return out;
}
export function pack(fmt, values) {
    const { little, tokens } = parse(fmt);
    const buf = new Uint8Array(calcsize(fmt));
    const dv = new DataView(buf.buffer);
    let off = 0;
    let vi = 0;
    for (const t of tokens) {
        if (t.type === 'x') {
            off += t.count;
            continue;
        }
        if (t.type === 's') {
            const v = values[vi++];
            const src = v ?? new Uint8Array(0);
            buf.set(src.subarray(0, t.count), off);
            off += t.count;
            continue;
        }
        for (let r = 0; r < t.count; r++) {
            const v = values[vi++];
            switch (t.type) {
                case 'b':
                    dv.setInt8(off, Number(v));
                    off += 1;
                    break;
                case 'B':
                    dv.setUint8(off, Number(v));
                    off += 1;
                    break;
                case 'h':
                    dv.setInt16(off, Number(v), little);
                    off += 2;
                    break;
                case 'H':
                    dv.setUint16(off, Number(v), little);
                    off += 2;
                    break;
                case 'i':
                case 'l':
                    dv.setInt32(off, Number(v), little);
                    off += 4;
                    break;
                case 'I':
                case 'L':
                    dv.setUint32(off, Number(v) >>> 0, little);
                    off += 4;
                    break;
                case 'q':
                    dv.setBigInt64(off, BigInt(v), little);
                    off += 8;
                    break;
                case 'Q':
                    dv.setBigUint64(off, BigInt(v), little);
                    off += 8;
                    break;
                case 'f':
                    dv.setFloat32(off, Number(v), little);
                    off += 4;
                    break;
                case 'd':
                    dv.setFloat64(off, Number(v), little);
                    off += 8;
                    break;
            }
        }
    }
    return buf;
}
//# sourceMappingURL=struct.js.map