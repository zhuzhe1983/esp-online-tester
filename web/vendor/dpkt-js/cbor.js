// Minimal CBOR (RFC 8949) encoder/decoder — just the major types the
// serialradio protocol uses: uint(0), negint(1), byte string(2), text string(3),
// array(4), map(5), and simple true/false/null(7). Not a general CBOR library.
import { concat } from './packet.js';
const enc = new TextEncoder();
const dec = new TextDecoder();
function head(major, n) {
    const mt = major << 5;
    if (n < 24)
        return Uint8Array.of(mt | n);
    if (n < 0x100)
        return Uint8Array.of(mt | 24, n);
    if (n < 0x10000)
        return Uint8Array.of(mt | 25, (n >> 8) & 0xff, n & 0xff);
    // 32-bit
    return Uint8Array.of(mt | 26, (n >>> 24) & 0xff, (n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff);
}
export function cborEncode(v) {
    if (typeof v === 'number') {
        if (!Number.isInteger(v))
            throw new Error('cbor: only integer numbers supported');
        return v >= 0 ? head(0, v) : head(1, -1 - v);
    }
    if (typeof v === 'boolean')
        return Uint8Array.of(0xe0 | (v ? 21 : 20));
    if (v === null)
        return Uint8Array.of(0xf6);
    if (v instanceof Uint8Array)
        return concat(head(2, v.length), v);
    if (typeof v === 'string') {
        const b = enc.encode(v);
        return concat(head(3, b.length), b);
    }
    if (Array.isArray(v))
        return concat(head(4, v.length), ...v.map(cborEncode));
    // map (plain object), string keys
    const keys = Object.keys(v);
    return concat(head(5, keys.length), ...keys.flatMap((k) => [cborEncode(k), cborEncode(v[k])]));
}
function readHead(r) {
    const ib = r.buf[r.off++];
    const major = ib >> 5;
    const ai = ib & 0x1f;
    let n;
    if (ai < 24)
        n = ai;
    else if (ai === 24)
        n = r.buf[r.off++];
    else if (ai === 25) {
        n = (r.buf[r.off] << 8) | r.buf[r.off + 1];
        r.off += 2;
    }
    else if (ai === 26) {
        n = ((r.buf[r.off] << 24) | (r.buf[r.off + 1] << 16) | (r.buf[r.off + 2] << 8) | r.buf[r.off + 3]) >>> 0;
        r.off += 4;
    }
    else
        throw new Error(`cbor: unsupported additional info ${ai}`);
    return { major, n };
}
function readValue(r) {
    const { major, n } = readHead(r);
    switch (major) {
        case 0: return n; // uint
        case 1: return -1 - n; // negint
        case 2: {
            const b = r.buf.subarray(r.off, r.off + n);
            r.off += n;
            return b;
        } // byte string
        case 3: {
            const b = r.buf.subarray(r.off, r.off + n);
            r.off += n;
            return dec.decode(b);
        } // text
        case 4: {
            const a = [];
            for (let i = 0; i < n; i++)
                a.push(readValue(r));
            return a;
        }
        case 5: {
            const o = {};
            for (let i = 0; i < n; i++) {
                const k = readValue(r);
                o[k] = readValue(r);
            }
            return o;
        }
        case 7: return n === 21 ? true : n === 20 ? false : null;
        default: throw new Error(`cbor: unsupported major type ${major}`);
    }
}
export function cborDecode(buf) {
    return readValue({ buf, off: 0 });
}
/** A CBOR sequence (RFC 8742): items one after another. */
export function cborDecodeSequence(buf) {
    const r = { buf, off: 0 };
    const out = [];
    while (r.off < buf.length) {
        out.push(readValue(r));
        if (r.off > buf.length)
            throw new Error('cbor: item runs past the end');
    }
    return out;
}
/** CBOR diagnostic notation (RFC 8949 8), for display. */
export function cborDiag(v) {
    if (v instanceof Uint8Array)
        return `h'${[...v].map((x) => x.toString(16).padStart(2, '0')).join('')}'`;
    if (Array.isArray(v))
        return `[${v.map(cborDiag).join(', ')}]`;
    if (v && typeof v === 'object')
        return `{${Object.entries(v).map(([k, x]) => `${JSON.stringify(k)}: ${cborDiag(x)}`).join(', ')}}`;
    return JSON.stringify(v);
}
//# sourceMappingURL=cbor.js.map