// Internet Protocol v4 — port of dpkt/ip.py (header, options, fragmentation
// bits, header + transport checksums). The NAT64 translation path
// (Contiki-NG ip64) produces/consumes these packets.
import { Packet, UnpackError, dataBytes, concat } from './packet.js';
import { inCksum, ip4PseudoChecksum } from './cksum.js';
export const IP_ADDR_LEN = 4;
export const IP_HDR_LEN = 20;
// Fragmentation flags (in _flags_offset)
export const IP_RF = 0x8000; // reserved
export const IP_DF = 0x4000; // don't fragment
export const IP_MF = 0x2000; // more fragments (not last frag)
export const IP_OFFMASK = 0x1fff; // mask for fragment offset
export const IP_TTL_DEFAULT = 64;
export const IP_TTL_MAX = 255;
// Protocol numbers (the subset the IoT/NAT64 path uses; ip6.ts has the v6 set)
export const IP_PROTO_ICMP = 1;
export const IP_PROTO_IGMP = 2;
export const IP_PROTO_IPIP = 4;
export const IP_PROTO_IP6 = 41;
export const IP_PROTO_GRE = 47;
export const IP_PROTO_SCTP = 132;
export const IP_PROTO_RAW = 255;
// TCP/UDP/ICMP6 come from ip6.ts (shared numbering); re-declared here so the
// module is self-contained for IPv4-only users.
const PROTO_TCP = 6;
const PROTO_UDP = 17;
export class IP extends Packet {
    static hdr = [
        ['_v_hl', 'B', (4 << 4) | (IP_HDR_LEN >> 2)],
        ['tos', 'B', 0],
        ['len', 'H', IP_HDR_LEN],
        ['id', 'H', 0],
        ['_flags_offset', 'H', 0],
        ['ttl', 'B', IP_TTL_DEFAULT],
        ['p', 'B', 0],
        ['sum', 'H', 0],
        ['src', '4s', new Uint8Array(4)],
        ['dst', '4s', new Uint8Array(4)],
    ];
    static bitFields = {
        _v_hl: [['v', 4], ['hl', 4]],
        _flags_offset: [['rf', 1], ['df', 1], ['mf', 1], ['offset', 13]],
    };
    constructor(buf, fields) {
        super(buf, fields);
        if (this.opts === undefined)
            this.opts = new Uint8Array(0);
        // Mirror dpkt: when built field-wise without an explicit len, size it.
        if (!buf && !(fields && 'len' in fields))
            this.len = this.byteLength();
    }
    byteLength() {
        return this.hdrLen + this.opts.length + dataBytes(this.data).length;
    }
    unpack(buf) {
        super.unpack(buf);
        const ol = ((this._v_hl & 0xf) << 2) - this.hdrLen;
        if (ol < 0)
            throw new UnpackError('invalid header length');
        this.opts = buf.subarray(this.hdrLen, this.hdrLen + ol);
        // len==0 happens with TCP segmentation offload captures
        this.data = this.len ? buf.subarray(this.hdrLen + ol, this.len) : buf.subarray(this.hdrLen + ol);
    }
    toBytes() {
        if (this.sum === 0) {
            this.len = this.byteLength();
            this.sum = inCksum(concat(this.packHdr(), this.opts));
            const d = this.data;
            // Fill zeroed TCP/UDP checksums for non-fragments wrapping a sub-packet.
            if ((this.p === PROTO_TCP || this.p === PROTO_UDP) &&
                (this._flags_offset & (IP_MF | IP_OFFMASK)) === 0 &&
                d && typeof d === 'object' && 'sum' in d && !d.sum) {
                const pb = dataBytes(d);
                d.sum = ip4PseudoChecksum(this.src, this.dst, this.p, pb);
                // RFC 768: a computed UDP checksum of 0 is transmitted as all ones.
                if (this.p === PROTO_UDP && d.sum === 0)
                    d.sum = 0xffff;
            }
        }
        return concat(this.packHdr(), this.opts, dataBytes(this.data));
    }
}
/** Dotted-quad textual form of a 4-byte IPv4 address. */
export function ip4str(addr) {
    return `${addr[0]}.${addr[1]}.${addr[2]}.${addr[3]}`;
}
/** Parse "a.b.c.d" into 4 bytes. */
export function ip4bytes(s) {
    const parts = s.split('.').map(Number);
    if (parts.length !== 4 || parts.some((n) => !Number.isInteger(n) || n < 0 || n > 255)) {
        throw new Error(`invalid IPv4 address: ${s}`);
    }
    return Uint8Array.from(parts);
}
//# sourceMappingURL=ip.js.map