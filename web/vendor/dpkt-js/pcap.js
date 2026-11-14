// pcap (libpcap file format) reader and writer, and a per-link-type entry
// into the dissector.  Reads both byte orders and both timestamp resolutions
// (microseconds, magic a1b2c3d4; nanoseconds, a1b23c4d — what Cooja-NG
// writes).  pcapng is not handled.
import { crc16Data } from './crc16.js';
import { concat } from './packet.js';
import { toHex } from './util.js';
import { IP6 } from './ip6.js';
import { dissect802154, dissectIp4, dissectIp6 } from './dissect.js';
import { LowpanReassembler, DEFAULT_CONTEXT0, parsePrefix } from './lowpan.js';
import { KeyStore } from './security.js';
export const LINKTYPE_ETHERNET = 1;
export const LINKTYPE_RAW = 101;
export const LINKTYPE_IPV4 = 228;
export const LINKTYPE_IPV6 = 229;
/** IEEE 802.15.4, the frame followed by its 2-byte FCS. */
export const LINKTYPE_IEEE802_15_4_WITHFCS = 195;
/** IEEE 802.15.4, no FCS. */
export const LINKTYPE_IEEE802_15_4_NOFCS = 230;
const MAGIC_US = 0xa1b2c3d4;
const MAGIC_NS = 0xa1b23c4d;
export class PcapError extends Error {
}
export class PcapReader {
    buf;
    linktype;
    snaplen;
    nanos;
    versionMajor;
    versionMinor;
    view;
    little;
    constructor(buf) {
        this.buf = buf;
        if (buf.length < 24)
            throw new PcapError('pcap: shorter than its 24-byte header');
        this.view = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
        const le = this.view.getUint32(0, true);
        const be = this.view.getUint32(0, false);
        if (le === MAGIC_US || le === MAGIC_NS)
            this.little = true;
        else if (be === MAGIC_US || be === MAGIC_NS)
            this.little = false;
        else
            throw new PcapError(`pcap: bad magic 0x${le.toString(16)} (pcapng is not supported)`);
        this.nanos = (this.little ? le : be) === MAGIC_NS;
        this.versionMajor = this.view.getUint16(4, this.little);
        this.versionMinor = this.view.getUint16(6, this.little);
        this.snaplen = this.view.getUint32(16, this.little);
        this.linktype = this.view.getUint32(20, this.little) & 0x0fffffff;
    }
    *[Symbol.iterator]() {
        let off = 24;
        while (off + 16 <= this.buf.length) {
            const tsSec = this.view.getUint32(off, this.little);
            const tsFrac = this.view.getUint32(off + 4, this.little);
            const capLen = this.view.getUint32(off + 8, this.little);
            const origLen = this.view.getUint32(off + 12, this.little);
            off += 16;
            if (off + capLen > this.buf.length)
                throw new PcapError(`pcap: record at ${off - 16} runs past the end of the file`);
            yield {
                ts: tsSec + tsFrac / (this.nanos ? 1e9 : 1e6),
                tsSec, tsFrac,
                data: this.buf.subarray(off, off + capLen),
                origLen,
            };
            off += capLen;
        }
    }
    /** Every record, as an array. */
    packets() {
        return [...this];
    }
}
export class PcapWriter {
    linktype;
    opts;
    parts = [];
    constructor(linktype, opts = {}) {
        this.linktype = linktype;
        this.opts = opts;
        const h = new Uint8Array(24);
        const v = new DataView(h.buffer);
        v.setUint32(0, opts.nanos ? MAGIC_NS : MAGIC_US, true);
        v.setUint16(4, 2, true);
        v.setUint16(6, 4, true);
        v.setUint32(16, opts.snaplen ?? 65535, true);
        v.setUint32(20, linktype, true);
        this.parts.push(h);
    }
    /** Append a record; `tsFrac` is in the file's resolution (µs, or ns with `nanos`). */
    write(data, tsSec = 0, tsFrac = 0) {
        const r = new Uint8Array(16);
        const v = new DataView(r.buffer);
        v.setUint32(0, tsSec, true);
        v.setUint32(4, tsFrac, true);
        v.setUint32(8, data.length, true);
        v.setUint32(12, data.length, true);
        this.parts.push(r, data);
    }
    toBytes() {
        return concat(...this.parts);
    }
}
/** The 802.15.4 FCS of `frame` (CRC-16, initial value 0), as it goes on air. */
export function ieee802154Fcs(frame) {
    return crc16Data(frame);
}
/** Dissect one captured frame by the file's link type.  Stateless unless
 *  `opts` brings state (a reassembler, keys): see `Dissector` for a capture. */
export function dissectFrame(linktype, data, opts = {}) {
    switch (linktype) {
        case LINKTYPE_IEEE802_15_4_WITHFCS: {
            if (data.length < 2)
                return { label: 'IEEE 802.15.4', error: 'shorter than its FCS', raw: toHex(data) };
            const frame = data.subarray(0, data.length - 2);
            const stored = data[data.length - 2] | (data[data.length - 1] << 8);
            const ok = ieee802154Fcs(frame) === stored;
            const node = dissect802154(frame, opts);
            (node.fields ??= []).push(['FCS', `0x${stored.toString(16).padStart(4, '0')} (${ok ? 'correct' : 'incorrect'})`]);
            if (!ok)
                node.error ??= 'bad FCS';
            return node;
        }
        case LINKTYPE_IEEE802_15_4_NOFCS:
            return dissect802154(data, opts);
        case LINKTYPE_RAW:
        case LINKTYPE_IPV4:
        case LINKTYPE_IPV6:
            return dissectIp(data);
        case LINKTYPE_ETHERNET: {
            if (data.length < 14)
                return { label: 'Ethernet', error: 'shorter than its header', raw: toHex(data) };
            const type = (data[12] << 8) | data[13];
            const node = {
                label: 'Ethernet',
                summary: `${mac(data.subarray(6, 12))} → ${mac(data.subarray(0, 6))}`,
                fields: [['Dst', mac(data.subarray(0, 6))], ['Src', mac(data.subarray(6, 12))], ['Type', `0x${type.toString(16).padStart(4, '0')}`]],
                children: [],
            };
            if (type === 0x0800 || type === 0x86dd)
                node.children.push(dissectIp(data.subarray(14)));
            return node;
        }
        default:
            return { label: `Link type ${linktype}`, error: 'not dissected', raw: toHex(data) };
    }
}
/** Dissects a capture frame by frame, keeping what reaches across frames:
 *  6LoWPAN fragments (reassembled in the frame that completes them),
 *  contexts, and the addresses decryption needs. */
export class Dissector {
    linktype;
    contexts = {};
    /** Where context 0 came from: "given", "learned from ...", or "default". */
    context0Source = 'default (Contiki-NG fd00::/64)';
    reassembler = new LowpanReassembler();
    coapRequests = new Map();
    keys;
    frame = 0;
    constructor(linktype, opts = {}) {
        this.linktype = linktype;
        for (const [k, v] of Object.entries(opts.contexts ?? {}))
            this.contexts[+k] = typeof v === 'string' ? parsePrefix(v) : v;
        if (this.contexts[0])
            this.context0Source = 'given';
        else
            this.contexts[0] = DEFAULT_CONTEXT0;
        this.keys = new KeyStore({ threadNetworkKey: opts.threadNetworkKey, macKeys: opts.macKeys });
    }
    /** Look through a capture for context 0 when none was given: a 6LoWPAN
     *  Context option (6CO), else an RPL DIO's prefix (PIO) or DODAG ID. */
    learn(frames) {
        if (this.context0Source === 'given')
            return;
        let dodag = null;
        for (const data of frames) {
            const t = dissectFrame(this.linktype, data, { contexts: this.contexts });
            const found = findPrefix(t);
            if (found?.kind === 'pio' || found?.kind === '6co') {
                this.contexts[0] = found.prefix;
                this.context0Source = `learned from ${found.kind === '6co' ? 'a 6LoWPAN Context option' : 'an RPL DIO prefix'}`;
                return;
            }
            if (found?.kind === 'dodag' && !dodag)
                dodag = found.prefix;
        }
        if (dodag) {
            this.contexts[0] = dodag;
            this.context0Source = 'learned from an RPL DODAG ID';
        }
    }
    /** Dissect the next frame.  `ts` is its capture time in seconds, which
     *  TSCH decryption needs (the nonce holds the slot's ASN). */
    dissect(data, ts) {
        this.frame++;
        return dissectFrame(this.linktype, data, {
            contexts: this.contexts, reassembler: this.reassembler, frame: this.frame, keys: this.keys, ts, coapRequests: this.coapRequests,
        });
    }
    /** Datagrams still waiting for fragments (at the end of a capture: never completed). */
    get pendingFragments() {
        return this.reassembler.pending;
    }
}
/** A prefix for context 0 in a dissected frame, if it carries one. */
function findPrefix(n) {
    const fieldOf = (x, k) => x.fields?.find((f) => f[0] === k)?.[1];
    if (n.label === '6LoWPAN Context' && n.summary && /^context 0[: ]/.test(n.summary)) {
        const m = /: ([0-9a-f:]+)\/(\d+)/.exec(n.summary);
        if (m)
            return { kind: '6co', prefix: parsePrefix(m[1]) };
    }
    if (n.label === 'Prefix Information' && n.summary) {
        const m = /^([0-9a-f:]+)\/64$/.exec(n.summary);
        if (m && !m[1].startsWith('fe80'))
            return { kind: 'pio', prefix: parsePrefix(m[1]) };
    }
    if (n.label === 'RPL DIO') {
        for (const c of n.children ?? []) {
            const r = findPrefix(c);
            if (r)
                return r;
        }
        const id = fieldOf(n, 'DODAG ID');
        if (id && !id.startsWith('fe80'))
            return { kind: 'dodag', prefix: parsePrefix(id) };
    }
    for (const c of n.children ?? []) {
        const r = findPrefix(c);
        if (r)
            return r;
    }
    return null;
}
function dissectIp(data) {
    const version = data.length ? data[0] >> 4 : 0;
    if (version === 4)
        return dissectIp4(data);
    if (version === 6) {
        try {
            return dissectIp6(new IP6(data));
        }
        catch (e) {
            return { label: 'IPv6', error: String(e), raw: toHex(data) };
        }
    }
    return { label: 'IP', error: `version ${version}`, raw: toHex(data) };
}
function mac(b) {
    return [...b].map((x) => x.toString(16).padStart(2, '0')).join(':');
}
//# sourceMappingURL=pcap.js.map