// 6LoWPAN (RFC 4944, RFC 6282) — port of dpkt/sixlowpan.py,
// itself based on Contiki-NG's sicslowpan.c.
import { Packet, NeedData, concat, dataBytes } from './packet.js';
import { pack, unpack } from './struct.js';
import { IP6 } from './ip6.js';
export const DISPATCH_IPV6 = 0x41;
export const DISPATCH_IPHC = 0x60;
export const DISPATCH_FRAG1 = 0xc0;
export const DISPATCH_FRAGN = 0xe0;
export const NHC_EXT_HDR_MASK = 0xf0;
export const NHC_EXT_HDR_ID = 0xe0;
export const NHC_UDP_MASK = 0xf8;
export const NHC_UDP_ID = 0xf0;
export const NHC_UDP_CHECKSUM_C = 0x04;
export const PROTO_HBHO = 0;
export const PROTO_ROUTING = 43;
export const PROTO_FRAGMENT = 44;
export const PROTO_ICMP6 = 58;
export const PROTO_DESTO = 60;
export const PROTO_UDP = 17;
const NHC_EID_TO_PROTO = { 0: PROTO_HBHO, 1: PROTO_ROUTING, 2: PROTO_FRAGMENT, 3: PROTO_DESTO };
const LL_PREFIX = concat(new Uint8Array([0xfe, 0x80]), new Uint8Array(6));
const u16be = (buf, off) => unpack('>H', buf, off)[0];
export class SixLoWPAN extends Packet {
    constructor(buf, fields) {
        super(buf, fields);
        this.dispatch ??= 0;
    }
    unpack(buf) {
        if (!buf.length)
            throw new NeedData();
        const b0 = buf[0];
        this.dispatch = b0;
        if (b0 === DISPATCH_IPV6) {
            this.ip6 = new IP6(buf.subarray(1));
            this.data = this.ip6;
        }
        else if ((b0 & 0xe0) === DISPATCH_IPHC) {
            this.iphc = new SixLoWPAN_IPHC(buf);
            this.data = this.iphc;
        }
        else if ((b0 & 0xf8) === DISPATCH_FRAG1 || (b0 & 0xf8) === DISPATCH_FRAGN) {
            this.frag = new SixLoWPAN_Frag(buf);
            this.data = this.frag;
        }
        else {
            this.data = buf;
        }
    }
    decompress(srcMac, dstMac) {
        if (this.data instanceof SixLoWPAN_IPHC)
            return this.data.decompress(srcMac, dstMac);
        if (this.dispatch === DISPATCH_IPV6)
            return this.ip6 ?? null;
        return null;
    }
    toBytes() {
        if (this.iphc)
            return this.iphc.toBytes();
        if (this.dispatch === DISPATCH_IPV6)
            return concat(new Uint8Array([DISPATCH_IPV6]), dataBytes(this.data));
        return dataBytes(this.data);
    }
}
export class SixLoWPAN_IPHC extends Packet {
    static hdr = [['iphc', 'H', 0]];
    static byteOrder = '>';
    static bitFields = {
        iphc: [
            ['_dispatch', 3], ['tf', 2], ['nh', 1], ['hlim', 2], ['cid', 1],
            ['sac', 1], ['sam', 2], ['m', 1], ['dac', 1], ['dam', 2],
        ],
    };
    unpack(buf) {
        super.unpack(buf);
        this._raw = buf;
        let off = 2;
        if (this.cid)
            off += 1;
        if (this.tf === 0)
            off += 4;
        else if (this.tf === 1)
            off += 3;
        else if (this.tf === 2)
            off += 1;
        if (this.nh === 0)
            off += 1;
        if (this.hlim === 0)
            off += 1;
        if (!this.sac) {
            if (this.sam === 0)
                off += 16;
            else if (this.sam === 1)
                off += 8;
            else if (this.sam === 2)
                off += 2;
        }
        else {
            if (this.sam === 1)
                off += 8;
            else if (this.sam === 2)
                off += 2;
        }
        if (!this.dac) {
            if (!this.m) {
                if (this.dam === 0)
                    off += 16;
                else if (this.dam === 1)
                    off += 8;
                else if (this.dam === 2)
                    off += 2;
            }
            else {
                if (this.dam === 0)
                    off += 16;
                else if (this.dam === 1)
                    off += 6;
                else if (this.dam === 2)
                    off += 4;
                else if (this.dam === 3)
                    off += 1;
            }
        }
        else if (!this.m) {
            if (this.dam === 1)
                off += 8;
            else if (this.dam === 2)
                off += 2;
        }
        this.data = buf.subarray(off);
    }
    decompress(srcMac, dstMac) {
        const p = new IP6();
        const buf = this._raw.subarray(2);
        let off = 0;
        if (this.cid)
            off += 1;
        // Traffic class & flow label
        if (this.tf === 0) {
            const tmp = buf[off];
            const tc = ((tmp >> 2) & 0x3c) | (tmp & 0x03);
            const fl = ((buf[off + 1] & 0x0f) << 16) | (buf[off + 2] << 8) | buf[off + 3];
            p.v_tc_fl = (6 * 2 ** 28) + (tc << 20) + fl;
            off += 4;
        }
        else if (this.tf === 1) {
            const tmp = buf[off];
            const ecn = (tmp >> 6) & 0x03;
            const fl = ((tmp & 0x0f) << 16) | (buf[off + 1] << 8) | buf[off + 2];
            p.v_tc_fl = (6 * 2 ** 28) + (ecn << 20) + fl;
            off += 3;
        }
        else if (this.tf === 2) {
            const tmp = buf[off];
            const tc = ((tmp >> 2) & 0x3c) | (tmp & 0x03);
            p.v_tc_fl = (6 * 2 ** 28) + (tc << 20);
            off += 1;
        }
        else {
            p.v_tc_fl = 6 * 2 ** 28;
        }
        // Next header
        if (this.nh === 0) {
            p.nxt = buf[off];
            off += 1;
        }
        else {
            p.nxt = 0;
        }
        // Hop limit
        const hlimVals = { 1: 1, 2: 64, 3: 255 };
        if (this.hlim in hlimVals)
            p.hlim = hlimVals[this.hlim];
        else {
            p.hlim = buf[off];
            off += 1;
        }
        // Source address
        if (this.sac) {
            if (this.sam === 0)
                p.src = new Uint8Array(16);
            else if (this.sam === 1) {
                p.src = concat(LL_PREFIX, buf.subarray(off, off + 8));
                off += 8;
            }
            else if (this.sam === 2) {
                p.src = concat(LL_PREFIX, new Uint8Array([0, 0, 0, 0xff, 0xfe, 0]), buf.subarray(off, off + 2));
                off += 2;
            }
            else if (this.sam === 3)
                p.src = deriveIp6Addr(LL_PREFIX, srcMac);
        }
        else {
            if (this.sam === 0) {
                p.src = buf.subarray(off, off + 16);
                off += 16;
            }
            else if (this.sam === 1) {
                p.src = concat(LL_PREFIX, buf.subarray(off, off + 8));
                off += 8;
            }
            else if (this.sam === 2) {
                p.src = concat(LL_PREFIX, new Uint8Array([0, 0, 0, 0xff, 0xfe, 0]), buf.subarray(off, off + 2));
                off += 2;
            }
            else if (this.sam === 3)
                p.src = deriveIp6Addr(LL_PREFIX, srcMac);
        }
        // Destination address
        if (this.m) {
            if (!this.dac) {
                if (this.dam === 0) {
                    p.dst = buf.subarray(off, off + 16);
                    off += 16;
                }
                else if (this.dam === 1) {
                    const sc = buf[off];
                    p.dst = concat(new Uint8Array([0xff, sc]), new Uint8Array(9), buf.subarray(off + 1, off + 6));
                    off += 6;
                }
                else if (this.dam === 2) {
                    const sc = buf[off];
                    p.dst = concat(new Uint8Array([0xff, sc]), new Uint8Array(11), buf.subarray(off + 1, off + 4));
                    off += 4;
                }
                else if (this.dam === 3) {
                    p.dst = concat(new Uint8Array([0xff, 0x02]), new Uint8Array(13), buf.subarray(off, off + 1));
                    off += 1;
                }
            }
            else {
                p.dst = buf.subarray(off, off + 16);
                off += 16;
            }
        }
        else {
            if (!this.dac) {
                if (this.dam === 0) {
                    p.dst = buf.subarray(off, off + 16);
                    off += 16;
                }
                else if (this.dam === 1) {
                    p.dst = concat(LL_PREFIX, buf.subarray(off, off + 8));
                    off += 8;
                }
                else if (this.dam === 2) {
                    p.dst = concat(LL_PREFIX, new Uint8Array([0, 0, 0, 0xff, 0xfe, 0]), buf.subarray(off, off + 2));
                    off += 2;
                }
                else if (this.dam === 3)
                    p.dst = deriveIp6Addr(LL_PREFIX, dstMac);
            }
            else {
                if (this.dam === 0) {
                    p.dst = buf.subarray(off, off + 16);
                    off += 16;
                }
                else if (this.dam === 1) {
                    p.dst = concat(LL_PREFIX, buf.subarray(off, off + 8));
                    off += 8;
                }
                else if (this.dam === 2) {
                    p.dst = concat(LL_PREFIX, new Uint8Array([0, 0, 0, 0xff, 0xfe, 0]), buf.subarray(off, off + 2));
                    off += 2;
                }
                else if (this.dam === 3)
                    p.dst = deriveIp6Addr(LL_PREFIX, dstMac);
            }
        }
        // NHC
        if (this.nh === 1) {
            off = this.decompressNhc(p, buf, off);
            const data = p.data;
            if (!(data instanceof Uint8Array) || data.length > 0) {
                p.plen = data instanceof Uint8Array ? data.length : 0;
                return p;
            }
        }
        p.data = buf.subarray(off);
        p.plen = p.data.length;
        return p;
    }
    decompressNhc(p, buf, off) {
        while (off < buf.length) {
            const nhc = buf[off];
            if ((nhc & NHC_EXT_HDR_MASK) === NHC_EXT_HDR_ID) {
                const eid = (nhc >> 1) & 0x07;
                const nhFlag = nhc & 0x01;
                off += 1;
                p.nxt = NHC_EID_TO_PROTO[eid] ?? 0;
                if (!nhFlag) {
                    p.nxt = buf[off];
                    off += 1;
                }
                const extLen = buf[off];
                off += 1;
                off += extLen;
                if (!nhFlag)
                    break;
            }
            else if ((nhc & NHC_UDP_MASK) === NHC_UDP_ID) {
                p.nxt = PROTO_UDP;
                off = this.decompressNhcUdp(p, buf, off);
                break;
            }
            else
                break;
        }
        return off;
    }
    decompressNhcUdp(p, buf, off) {
        const nhc = buf[off];
        off += 1;
        const portMode = nhc & 0x03;
        const csumCompressed = nhc & NHC_UDP_CHECKSUM_C;
        let sport;
        let dport;
        if (portMode === 0) {
            sport = u16be(buf, off);
            off += 2;
            dport = u16be(buf, off);
            off += 2;
        }
        else if (portMode === 1) {
            sport = u16be(buf, off);
            off += 2;
            dport = 0xf000 + buf[off];
            off += 1;
        }
        else if (portMode === 2) {
            sport = 0xf000 + buf[off];
            off += 1;
            dport = u16be(buf, off);
            off += 2;
        }
        else {
            const both = buf[off];
            off += 1;
            sport = 0xf0b0 + ((both >> 4) & 0x0f);
            dport = 0xf0b0 + (both & 0x0f);
        }
        let checksum = 0;
        if (!csumCompressed) {
            checksum = u16be(buf, off);
            off += 2;
        }
        const payload = buf.subarray(off);
        const udpHdr = pack('>HHHH', [sport, dport, 8 + payload.length, checksum]);
        p.data = concat(udpHdr, payload);
        p.plen = p.data.length;
        return buf.length;
    }
    toBytes() {
        this.iphc = (this.iphc & 0x1fff) | 0x6000;
        return concat(this.packHdr(), dataBytes(this.data));
    }
    compress(p, srcMac, dstMac) {
        this.iphc = 0x6000;
        let res = new Uint8Array(0);
        const tc = p.v_tc_fl !== undefined ? Math.floor(p.v_tc_fl / 2 ** 20) & 0xff : 0;
        const fl = p.v_tc_fl !== undefined ? p.v_tc_fl & 0xfffff : 0;
        if (tc === 0 && fl === 0)
            this.tf = 3;
        else if (fl === 0) {
            this.tf = 2;
            const dscp = (tc >> 2) & 0x3f;
            const ecn = tc & 0x03;
            res = concat(res, new Uint8Array([(dscp << 2) | ecn]));
        }
        else {
            this.tf = 0;
            const dscp = (tc >> 2) & 0x3f;
            const ecn = tc & 0x03;
            res = concat(res, new Uint8Array([(ecn << 6) | dscp]), pack('>I', [fl]).subarray(1));
        }
        this.nh = 0;
        res = concat(res, new Uint8Array([p.nxt]));
        if (p.hlim === 1)
            this.hlim = 1;
        else if (p.hlim === 64)
            this.hlim = 2;
        else if (p.hlim === 255)
            this.hlim = 3;
        else {
            this.hlim = 0;
            res = concat(res, new Uint8Array([p.hlim]));
        }
        // Source
        if (bytesAllZero(p.src)) {
            this.sac = 1;
            this.sam = 0;
        }
        else if (eq(p.src.subarray(0, 8), LL_PREFIX)) {
            this.sac = 0;
            if (srcMac && eq(p.src, deriveIp6Addr(LL_PREFIX, srcMac)))
                this.sam = 3;
            else if (is16bitCompressible(p.src)) {
                this.sam = 2;
                res = concat(res, p.src.subarray(14, 16));
            }
            else {
                this.sam = 1;
                res = concat(res, p.src.subarray(8, 16));
            }
        }
        else {
            this.sac = 0;
            this.sam = 0;
            res = concat(res, p.src);
        }
        // Destination
        if (p.dst[0] === 0xff) {
            this.m = 1;
            this.dac = 0;
            if (isMcast8(p.dst)) {
                this.dam = 3;
                res = concat(res, p.dst.subarray(15, 16));
            }
            else if (isMcast32(p.dst)) {
                this.dam = 2;
                res = concat(res, new Uint8Array([p.dst[1]]), p.dst.subarray(13, 16));
            }
            else if (isMcast48(p.dst)) {
                this.dam = 1;
                res = concat(res, new Uint8Array([p.dst[1]]), p.dst.subarray(11, 16));
            }
            else {
                this.dam = 0;
                res = concat(res, p.dst);
            }
        }
        else {
            this.m = 0;
            this.dac = 0;
            if (eq(p.dst.subarray(0, 8), LL_PREFIX)) {
                if (dstMac && eq(p.dst, deriveIp6Addr(LL_PREFIX, dstMac)))
                    this.dam = 3;
                else if (is16bitCompressible(p.dst)) {
                    this.dam = 2;
                    res = concat(res, p.dst.subarray(14, 16));
                }
                else {
                    this.dam = 1;
                    res = concat(res, p.dst.subarray(8, 16));
                }
            }
            else {
                this.dam = 0;
                res = concat(res, p.dst);
            }
        }
        this.data = concat(res, dataBytes(p.data));
        return this.data;
    }
}
export class SixLoWPAN_Frag extends Packet {
    static hdr = [['size_tag', 'H', 0]];
    static byteOrder = '>';
    static bitFields = { size_tag: [['_dispatch', 5], ['datagram_size', 11]] };
    unpack(buf) {
        super.unpack(buf);
        let off = 2;
        if (off + 2 > buf.length)
            throw new NeedData();
        this.datagram_tag = u16be(buf, off);
        off += 2;
        if ((buf[0] & 0xf8) === DISPATCH_FRAGN) {
            if (off >= buf.length)
                throw new NeedData();
            this.datagram_offset = buf[off];
            off += 1;
        }
        else
            this.datagram_offset = 0;
        this.data = buf.subarray(off);
    }
}
export function deriveIp6Addr(prefix, mac) {
    if (!mac)
        return concat(prefix, new Uint8Array(8));
    if (mac.length === 8) {
        const iid = Uint8Array.from(mac);
        iid[0] ^= 0x02;
        return concat(prefix, iid);
    }
    if (mac.length === 2)
        return concat(prefix, new Uint8Array([0, 0, 0, 0xff, 0xfe, 0]), mac);
    return concat(prefix, new Uint8Array(8));
}
function eq(a, b) {
    if (a.length !== b.length)
        return false;
    for (let i = 0; i < a.length; i++)
        if (a[i] !== b[i])
            return false;
    return true;
}
function bytesAllZero(a) { return a.length === 16 && a.every((x) => x === 0); }
function is16bitCompressible(a) {
    return eq(a.subarray(8, 14), new Uint8Array([0, 0, 0, 0xff, 0xfe, 0]));
}
function isMcast8(a) {
    return a[1] === 0x02 && a.subarray(2, 14).every((x) => x === 0) && a[14] === 0x00;
}
function isMcast32(a) {
    return a.subarray(2, 12).every((x) => x === 0) && a[12] === 0x00;
}
function isMcast48(a) {
    return a.subarray(2, 10).every((x) => x === 0) && a[10] === 0x00;
}
//# sourceMappingURL=sixlowpan.js.map