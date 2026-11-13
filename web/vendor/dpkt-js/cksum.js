// Internet checksum (RFC 1071) + IPv6 transport pseudo-header checksum.
// Byte-compatible with dpkt's in_cksum (e.g. in_cksum(0x00..0x0e) === 51150).
import { pack } from './struct.js';
import { concat } from './packet.js';
export const IP_PROTO_TCP = 6;
export const IP_PROTO_UDP = 17;
export const IP_PROTO_ICMP6 = 58;
/** Standard Internet checksum over a byte buffer. Returns the 16-bit value to
 *  store big-endian in a `sum` field. */
export function inCksum(buf) {
    let sum = 0;
    for (let i = 0; i + 1 < buf.length; i += 2)
        sum += (buf[i] << 8) | buf[i + 1];
    if (buf.length & 1)
        sum += buf[buf.length - 1] << 8;
    while (sum >>> 16)
        sum = (sum & 0xffff) + (sum >>> 16);
    return (~sum) & 0xffff;
}
/** Checksum over an IPv6 pseudo-header + upper-layer bytes (TCP/UDP/ICMPv6). */
export function ip6PseudoChecksum(src, dst, proto, payload) {
    // struct '>16s16sxBH' : src, dst, pad, proto, length
    const pseudo = pack('>16s16sxBH', [src, dst, proto, payload.length]);
    return inCksum(concat(pseudo, payload));
}
/** Convenience: ICMPv6 checksum (proto 58). */
export function icmp6Checksum(src, dst, icmpBytes) {
    return ip6PseudoChecksum(src, dst, IP_PROTO_ICMP6, icmpBytes);
}
/** Checksum over an IPv4 pseudo-header + upper-layer bytes (TCP/UDP). */
export function ip4PseudoChecksum(src, dst, proto, payload) {
    // struct '>4s4sxBH' : src, dst, pad, proto, length
    const pseudo = pack('>4s4sxBH', [src, dst, proto, payload.length]);
    return inCksum(concat(pseudo, payload));
}
//# sourceMappingURL=cksum.js.map