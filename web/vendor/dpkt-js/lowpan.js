// 6LoWPAN for the dissector (RFC 4944, RFC 6282): the dispatch headers
// (mesh, broadcast, fragment), IPHC and NHC decompressed back into the
// IPv6 datagram's own bytes, with contexts, and fragment reassembly.
//
// Rebuilding the real bytes (not a model of them) is what lets the layers
// above check their checksums: a wrong context, a byte-swapped short
// address or a misplaced fragment shows up as a bad UDP/ICMPv6 checksum.
/** fe80::/64 */
const LL_PREFIX = Uint8Array.of(0xfe, 0x80, 0, 0, 0, 0, 0, 0);
/** Contiki-NG's default prefix (UIP_DS6_DEFAULT_PREFIX, fd00::/64): context 0
 *  unless one is given or learned. */
export const DEFAULT_CONTEXT0 = Uint8Array.of(0xfd, 0, 0, 0, 0, 0, 0, 0);
export const NHC_EID_PROTO = [0, 43, 44, 60, 135, -1, -1, 41];
export const NHC_EID_NAMES = ['Hop-by-Hop Options', 'Routing', 'Fragment', 'Destination Options', 'Mobility', 'EID 5', 'EID 6', 'IPv6'];
/** The IID a link-layer address maps to (RFC 4944 6, RFC 6282 3.2.2):
 *  an EUI-64 with its U/L bit flipped, or 0000:00ff:fe00:XXXX.  Addresses
 *  are in network order (most significant byte first). */
export function iidFromLink(a) {
    if (a && a.length === 8) {
        const iid = Uint8Array.from(a);
        iid[0] ^= 0x02;
        return iid;
    }
    if (a && a.length === 2)
        return Uint8Array.of(0, 0, 0, 0xff, 0xfe, 0, a[0], a[1]);
    return new Uint8Array(8);
}
/** Parse a 6LoWPAN frame (the 802.15.4 payload).  `linkSrc`/`linkDst` are the
 *  MAC addresses in network order (the mesh header's replace them). */
export function parseLowpan(buf, linkSrc, linkDst, contexts = { 0: DEFAULT_CONTEXT0 }) {
    const f = { linkSrc, linkDst };
    let off = 0;
    for (;;) {
        if (off >= buf.length)
            throw new Error('6LoWPAN: no dispatch');
        const b = buf[off];
        if ((b & 0xc0) === 0x80) {
            // Mesh header: 10 V F HopsLeft, then originator and final addresses.
            const v = (b >> 5) & 1, fl = (b >> 4) & 1;
            let hops = b & 0xf;
            off++;
            if (hops === 0xf)
                hops = buf[off++];
            const ol = v ? 2 : 8, fl2 = fl ? 2 : 8;
            if (off + ol + fl2 > buf.length)
                throw new Error('mesh header truncated');
            const originator = buf.slice(off, off + ol);
            off += ol;
            const final = buf.slice(off, off + fl2);
            off += fl2;
            f.mesh = { hopsLeft: hops, originator, final };
            f.linkSrc = originator;
            f.linkDst = final;
            continue;
        }
        if (b === 0x50) {
            if (off + 2 > buf.length)
                throw new Error('BC0 truncated');
            f.bc0 = buf[off + 1];
            off += 2;
            continue;
        }
        if ((b & 0xf8) === 0xc0 || (b & 0xf8) === 0xe0) {
            const first = (b & 0xf8) === 0xc0;
            const hl = first ? 4 : 5;
            if (off + hl > buf.length)
                throw new Error('fragment header truncated');
            f.frag = {
                first, size: ((b & 7) << 8) | buf[off + 1], tag: (buf[off + 2] << 8) | buf[off + 3],
                offset: first ? 0 : buf[off + 4] * 8,
            };
            off += hl;
            if (!first) {
                f.fragData = buf.subarray(off);
                return f;
            }
            continue;
        }
        if (b === 0x41) {
            f.dispatch = b;
            f.ip6 = buf.slice(off + 1);
            return f;
        }
        if ((b & 0xe0) === 0x60) {
            f.dispatch = b;
            const out = [];
            const fix = [];
            const info = { fields: [], nhc: [], contextsUsed: [], contextsMissing: [] };
            off = iphc(buf, off, f.linkSrc, f.linkDst, contexts, out, fix, info);
            f.iphc = info;
            const payload = buf.subarray(off);
            const total = f.frag ? f.frag.size : out.length + payload.length;
            const ip6 = new Uint8Array(out.length + payload.length);
            ip6.set(out);
            ip6.set(payload, out.length);
            // Lengths the compression elided: IPv6 payload lengths, UDP lengths.
            for (const [pos, base] of fix) {
                const v = total - base;
                ip6[pos] = v >> 8;
                ip6[pos + 1] = v & 0xff;
            }
            f.ip6 = ip6;
            return f;
        }
        f.unknownDispatch = b;
        return f;
    }
}
/** Decompress one IPHC header (and the NHC chain after it) into `out`.
 *  Returns the offset in `buf` after the compressed headers. */
function iphc(buf, off, ls, ld, ctx, out, fix, info) {
    const need = (n) => { if (off + n > buf.length)
        throw new Error('IPHC truncated'); };
    need(2);
    const b0 = buf[off], b1 = buf[off + 1];
    off += 2;
    const tf = (b0 >> 3) & 3, nh = (b0 >> 2) & 1, hlimMode = b0 & 3;
    const cid = (b1 >> 7) & 1, sac = (b1 >> 6) & 1, sam = (b1 >> 4) & 3, m = (b1 >> 3) & 1, dac = (b1 >> 2) & 1, dam = b1 & 3;
    let sci = 0, dci = 0;
    if (cid) {
        need(1);
        sci = buf[off] >> 4;
        dci = buf[off] & 0xf;
        off++;
    }
    let ecn = 0, dscp = 0, fl = 0;
    if (tf === 0) {
        need(4);
        ecn = buf[off] >> 6;
        dscp = buf[off] & 0x3f;
        fl = ((buf[off + 1] & 0xf) << 16) | (buf[off + 2] << 8) | buf[off + 3];
        off += 4;
    }
    else if (tf === 1) {
        need(3);
        ecn = buf[off] >> 6;
        fl = ((buf[off] & 0xf) << 16) | (buf[off + 1] << 8) | buf[off + 2];
        off += 3;
    }
    else if (tf === 2) {
        need(1);
        ecn = buf[off] >> 6;
        dscp = buf[off] & 0x3f;
        off += 1;
    }
    const tc = (dscp << 2) | ecn;
    let nxt = 0;
    if (!nh) {
        need(1);
        nxt = buf[off++];
    }
    let hlim = [0, 1, 64, 255][hlimMode];
    if (hlimMode === 0) {
        need(1);
        hlim = buf[off++];
    }
    const prefix = (id) => {
        if (!info.contextsUsed.includes(id))
            info.contextsUsed.push(id);
        const p = ctx[id];
        if (!p) {
            if (!info.contextsMissing.includes(id))
                info.contextsMissing.push(id);
            return new Uint8Array(8);
        }
        return p.subarray(0, 8);
    };
    const inline = (n) => { need(n); const r = buf.subarray(off, off + n); off += n; return r; };
    const unicast = (ac, am, ci, link) => {
        const a = new Uint8Array(16);
        if (am === 0) {
            if (ac)
                return a; // SAC=1 SAM=00: the unspecified address
            a.set(inline(16));
            return a;
        }
        a.set(ac ? prefix(ci) : LL_PREFIX);
        if (am === 1)
            a.set(inline(8), 8);
        else if (am === 2) {
            a.set([0, 0, 0, 0xff, 0xfe, 0], 8);
            a.set(inline(2), 14);
        }
        else
            a.set(iidFromLink(link), 8);
        return a;
    };
    const src = unicast(sac, sam, sci, ls);
    let dst;
    if (!m) {
        if (dac && dam === 0)
            throw new Error('IPHC: DAC=1 DAM=00 is reserved');
        dst = unicast(dac, dam, dci, ld);
    }
    else {
        dst = new Uint8Array(16);
        dst[0] = 0xff;
        if (!dac) {
            if (dam === 0)
                dst.set(inline(16));
            else if (dam === 1) {
                const x = inline(6);
                dst[1] = x[0];
                dst.set(x.subarray(1), 11);
            }
            else if (dam === 2) {
                const x = inline(4);
                dst[1] = x[0];
                dst.set(x.subarray(1), 13);
            }
            else {
                dst[1] = 0x02;
                dst[15] = inline(1)[0];
            }
        }
        else {
            if (dam !== 0)
                throw new Error('IPHC: M=1 DAC=1 with DAM≠00 is reserved');
            // ffXX:XXLL:PPPP:PPPP:PPPP:PPPP:XXXX:XXXX (RFC 3306), P from the context.
            const x = inline(6);
            dst[1] = x[0];
            dst[2] = x[1];
            dst[3] = 64;
            dst.set(prefix(dci), 4);
            dst.set(x.subarray(2), 12);
        }
    }
    info.fields.push(['Traffic class', tf === 3 ? 'elided (0)' : `0x${tc.toString(16).padStart(2, '0')}${tf === 1 ? ' (ECN only)' : ''}`], ['Flow label', tf === 0 || tf === 1 ? `0x${fl.toString(16)}` : 'elided (0)'], ['Next header', nh ? 'compressed (NHC)' : `inline (${nxt})`], ['Hop limit', hlimMode ? `${hlim} (compressed)` : `${hlim} (inline)`], ['Source', addrMode(sac, sam, sci, false)], ['Destination', addrMode(dac, dam, dci, !!m)]);
    const start = out.length;
    out.push(0x60 | (tc >> 4), ((tc & 0xf) << 4) | (fl >> 16), (fl >> 8) & 0xff, fl & 0xff, 0, 0, nxt, hlim, ...src, ...dst);
    fix.push([start + 4, start + 40]);
    if (nh)
        off = nhc(buf, off, start + 6, ls, ld, ctx, out, fix, info);
    return off;
}
function addrMode(ac, am, ci, mcast) {
    if (mcast)
        return ac ? `multicast, 48 bits inline, prefix from context ${ci}` : ['multicast, inline', 'multicast, 48 bits inline', 'multicast, 32 bits inline', 'multicast ff02::XX, 8 bits inline'][am];
    if (ac && am === 0)
        return 'unspecified (::)';
    const where = ac ? `context ${ci}` : 'link-local';
    return [`inline`, `${where} + 64 bits inline`, `${where} + 16 bits inline`, `${where} + from the link address`][am];
}
/** The NHC chain: extension headers, UDP, an encapsulated IPv6 header. */
function nhc(buf, off, nxtPos, ls, ld, ctx, out, fix, info) {
    const need = (n) => { if (off + n > buf.length)
        throw new Error('NHC truncated'); };
    for (;;) {
        need(1);
        const b = buf[off];
        if ((b & 0xf0) === 0xe0) {
            const eid = (b >> 1) & 7, nh = b & 1;
            off++;
            info.nhc.push(NHC_EID_NAMES[eid]);
            if (eid === 7) {
                // IPv6-in-IPv6: the inner header is IPHC again, links the same.
                out[nxtPos] = 41;
                return iphc(buf, off, ls, ld, ctx, out, fix, info);
            }
            if (NHC_EID_PROTO[eid] < 0)
                throw new Error(`NHC: reserved EID ${eid}`);
            out[nxtPos] = NHC_EID_PROTO[eid];
            let next = 0;
            if (!nh) {
                need(1);
                next = buf[off++];
            }
            need(1);
            const len = buf[off++];
            need(len);
            const start = out.length;
            out.push(next, 0, ...buf.subarray(off, off + len));
            off += len;
            if (eid === 0 || eid === 3) {
                // Options headers may have had their trailing padding elided.
                const pad = (8 - ((out.length - start) % 8)) % 8;
                if (pad === 1)
                    out.push(0);
                else if (pad > 1)
                    out.push(1, pad - 2, ...new Array(pad - 2).fill(0));
            }
            if (eid !== 2)
                out[start + 1] = (out.length - start) / 8 - 1;
            if (!nh)
                return off;
            nxtPos = start;
            continue;
        }
        if ((b & 0xf8) === 0xf0) {
            off++;
            info.nhc.push('UDP');
            out[nxtPos] = 17;
            const pm = b & 3;
            let sp, dp;
            if (pm === 0) {
                need(4);
                sp = (buf[off] << 8) | buf[off + 1];
                dp = (buf[off + 2] << 8) | buf[off + 3];
                off += 4;
            }
            else if (pm === 1) {
                need(3);
                sp = (buf[off] << 8) | buf[off + 1];
                dp = 0xf000 | buf[off + 2];
                off += 3;
            }
            else if (pm === 2) {
                need(3);
                sp = 0xf000 | buf[off];
                dp = (buf[off + 1] << 8) | buf[off + 2];
                off += 3;
            }
            else {
                need(1);
                sp = 0xf0b0 | (buf[off] >> 4);
                dp = 0xf0b0 | (buf[off] & 0xf);
                off += 1;
            }
            let ck = 0;
            if (!(b & 4)) {
                need(2);
                ck = (buf[off] << 8) | buf[off + 1];
                off += 2;
            }
            else
                info.fields.push(['UDP checksum', 'elided']);
            const start = out.length;
            out.push(sp >> 8, sp & 0xff, dp >> 8, dp & 0xff, 0, 0, ck >> 8, ck & 0xff);
            fix.push([start + 4, start]);
            return off;
        }
        throw new Error(`NHC: unknown header 0x${b.toString(16)}`);
    }
}
/** Reassembles fragmented datagrams (RFC 4944 5.3), keyed by the link
 *  addresses, the size and the tag. */
export class LowpanReassembler {
    waiting = new Map();
    /** Datagrams already completed, by key: a later fragment of one is a
     *  retransmission (its ACK was lost), not the start of another. */
    done = new Map();
    /** Datagrams with fragments still missing. */
    get pending() {
        return this.waiting.size;
    }
    /** Add a fragment's bytes; returns the whole datagram and the frames it
     *  came from once every byte is in. */
    add(f, frame) {
        if (!f.frag)
            return null;
        const data = f.frag.first ? f.ip6 : f.fragData;
        if (!data)
            return null;
        const key = `${hex(f.linkSrc)}>${hex(f.linkDst)}:${f.frag.size}:${f.frag.tag}`;
        const doneAt = this.done.get(key);
        if (doneAt !== undefined && !this.waiting.has(key)) {
            if (!f.frag.first)
                return { retransmissionOf: doneAt };
            this.done.delete(key);
        }
        let e = this.waiting.get(key);
        if (!e || (f.frag.first && e.got)) {
            // A new FRAG1 for a key in progress restarts it (a retransmission of the datagram).
            e = { buf: new Uint8Array(f.frag.size), have: new Uint8Array(f.frag.size), got: 0, frames: [] };
            this.waiting.set(key, e);
        }
        const at = f.frag.offset;
        const n = Math.min(data.length, f.frag.size - at);
        if (n <= 0)
            return null;
        e.buf.set(data.subarray(0, n), at);
        for (let i = at; i < at + n; i++)
            if (!e.have[i]) {
                e.have[i] = 1;
                e.got++;
            }
        if (!e.frames.includes(frame))
            e.frames.push(frame);
        if (e.got < f.frag.size)
            return null;
        this.waiting.delete(key);
        this.done.set(key, frame);
        return { datagram: e.buf, frames: e.frames };
    }
}
function hex(a) {
    return a ? [...a].map((x) => x.toString(16).padStart(2, '0')).join('') : '-';
}
/** Parse "fd00::/64" or "fdde:ad00:beef:0::" into an 8-byte context prefix. */
export function parsePrefix(s) {
    const text = s.trim().replace(/\/\d+$/, '');
    const [head, tail] = text.includes('::') ? text.split('::') : [text, ''];
    const h = head ? head.split(':') : [];
    const t = tail ? tail.split(':') : [];
    const groups = [...h, ...new Array(Math.max(0, 8 - h.length - t.length)).fill('0'), ...t];
    if (groups.length !== 8 || groups.some((g) => !/^[0-9a-fA-F]{1,4}$/.test(g)))
        throw new Error(`not an IPv6 prefix: ${s}`);
    const out = new Uint8Array(8);
    for (let i = 0; i < 4; i++) {
        const v = parseInt(groups[i], 16);
        out[2 * i] = v >> 8;
        out[2 * i + 1] = v & 0xff;
    }
    return out;
}
//# sourceMappingURL=lowpan.js.map