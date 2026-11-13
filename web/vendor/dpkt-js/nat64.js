// NAT64 / DNS64 — RFC 6052 (IPv4-embedded IPv6 addresses), RFC 6146 (NAT64
// packet translation) and RFC 6147 (DNS64 AAAA synthesis). This mirrors what
// Contiki-NG's ip64 module does on the border router: v6-only devices reach
// the v4 internet through a translated prefix (default 64:ff9b::/96).
import { UnpackError } from './packet.js';
import { inCksum, ip4PseudoChecksum, ip6PseudoChecksum } from './cksum.js';
import { IP, IP_PROTO_ICMP } from './ip.js';
import { IP6, IP_PROTO_TCP, IP_PROTO_UDP, IP_PROTO_ICMP6 } from './ip6.js';
import { ICMP_ECHO, ICMP_ECHOREPLY } from './icmp.js';
import { ICMP6_ECHO_REQUEST, ICMP6_ECHO_REPLY } from './icmp6.js';
import { DNS_A, DNS_AAAA } from './dns.js';
/** The NAT64 well-known prefix 64:ff9b::/96 (RFC 6052 §2.1). */
export const NAT64_WELL_KNOWN_PREFIX = Uint8Array.from([0x00, 0x64, 0xff, 0x9b, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
const V4_BYTE_POS = {
    // prefix length -> the four byte positions carrying the v4 address
    // (bits 64..71, the "u" byte, must stay zero — RFC 6052 §2.2)
    32: [4, 5, 6, 7],
    40: [5, 6, 7, 9],
    48: [6, 7, 9, 10],
    56: [7, 9, 10, 11],
    64: [9, 10, 11, 12],
    96: [12, 13, 14, 15],
};
/** Embed a 4-byte IPv4 address in an IPv6 address under `prefix` (RFC 6052). */
export function synthesizeIp6(ip4, prefix = NAT64_WELL_KNOWN_PREFIX, prefixLen = 96) {
    const pos = V4_BYTE_POS[prefixLen];
    if (!pos)
        throw new UnpackError(`unsupported NAT64 prefix length: ${prefixLen}`);
    const out = new Uint8Array(16);
    out.set(prefix.subarray(0, prefixLen / 8));
    pos.forEach((p, i) => { out[p] = ip4[i]; });
    return out;
}
/** Extract the embedded IPv4 address from an RFC 6052 IPv6 address. */
export function extractIp4(ip6, prefixLen = 96) {
    const pos = V4_BYTE_POS[prefixLen];
    if (!pos)
        throw new UnpackError(`unsupported NAT64 prefix length: ${prefixLen}`);
    return Uint8Array.from(pos, (p) => ip6[p]);
}
/** True if `addr` is inside the given NAT64 prefix. */
export function isNat64Address(addr, prefix = NAT64_WELL_KNOWN_PREFIX, prefixLen = 96) {
    const n = prefixLen / 8;
    for (let i = 0; i < n; i++)
        if (addr[i] !== prefix[i])
            return false;
    return true;
}
// ----- DNS64 (RFC 6147) — how Contiki's ip64-dns64 rewrites resolv traffic -----
/** Rewrite an AAAA query so it can be forwarded to a v4-only resolver. */
export function dns64QueryToV4(dns) {
    for (const q of dns.qd)
        if (q.type === DNS_AAAA)
            q.type = DNS_A;
    return dns;
}
/** Rewrite an A response into the AAAA response the v6-only device expects:
 *  question and answers become AAAA with RFC 6052-synthesized addresses. */
export function dns64ResponseToV6(dns, prefix = NAT64_WELL_KNOWN_PREFIX, prefixLen = 96) {
    for (const q of dns.qd)
        if (q.type === DNS_A)
            q.type = DNS_AAAA;
    const synth = (rrs) => {
        for (const rr of rrs) {
            if (rr.type !== DNS_A)
                continue;
            rr.type = DNS_AAAA;
            rr.ip6 = synthesizeIp6(rr.ip ?? rr.rdata, prefix, prefixLen);
            rr.rdata = rr.ip6;
            rr.rlen = 16;
            delete rr.ip;
        }
    };
    synth(dns.an);
    synth(dns.ar);
    return dns;
}
// ----- NAT64 packet translation (RFC 6146) -----
/** Rewrite the transport checksum inside raw payload bytes (returns a copy). */
function fixChecksum(payload, proto, pseudoSum) {
    const out = payload.slice();
    const off = proto === IP_PROTO_TCP ? 16 : 6; // sum offset in TCP / UDP header
    out[off] = 0;
    out[off + 1] = 0;
    let sum = pseudoSum(out);
    if (proto === IP_PROTO_UDP && sum === 0)
        sum = 0xffff; // RFC 768
    out[off] = sum >> 8;
    out[off + 1] = sum & 0xff;
    return out;
}
/**
 * Translate an outbound IPv6 packet (dst inside the NAT64 prefix) into the
 * IPv4 packet the translator emits. `srcV4` is the translator's own v4
 * address (Contiki ip64 uses a single address + port mapping state).
 * Supports UDP, TCP and ICMPv6 echo — the same set as Contiki-NG ip64.
 */
export function translate6to4(pkt, srcV4, prefixLen = 96) {
    const proto = pkt.p ?? pkt.nxt;
    const dst4 = extractIp4(pkt.dst, prefixLen);
    const payload = pkt.data;
    if (!(payload instanceof Uint8Array))
        throw new UnpackError('translate6to4 expects raw payload bytes');
    let p4;
    let body;
    if (proto === IP_PROTO_UDP || proto === IP_PROTO_TCP) {
        p4 = proto;
        body = fixChecksum(payload, proto, (b) => ip4PseudoChecksum(srcV4, dst4, proto, b));
    }
    else if (proto === IP_PROTO_ICMP6) {
        const type = payload[0];
        if (type !== ICMP6_ECHO_REQUEST && type !== ICMP6_ECHO_REPLY) {
            throw new UnpackError(`NAT64: unsupported ICMPv6 type ${type}`);
        }
        p4 = IP_PROTO_ICMP;
        body = payload.slice();
        body[0] = type === ICMP6_ECHO_REQUEST ? ICMP_ECHO : ICMP_ECHOREPLY;
        body[2] = 0;
        body[3] = 0;
        const sum = inCksum(body); // ICMPv4: no pseudo-header
        body[2] = sum >> 8;
        body[3] = sum & 0xff;
    }
    else {
        throw new UnpackError(`NAT64: unsupported protocol ${proto}`);
    }
    const ip = new IP(undefined, {
        ttl: pkt.hlim, p: p4, src: srcV4.slice(), dst: dst4,
    });
    ip.data = body;
    ip.len = ip.byteLength();
    ip.sum = 0; // filled by toBytes()
    return ip;
}
/**
 * Translate an inbound IPv4 packet back into IPv6 for the device behind the
 * translator: src becomes the RFC 6052-synthesized address of the v4 sender,
 * dst is the device's real v6 address (from the NAT's mapping state).
 */
export function translate4to6(pkt, dstV6, prefix = NAT64_WELL_KNOWN_PREFIX, prefixLen = 96) {
    const src6 = synthesizeIp6(pkt.src, prefix, prefixLen);
    const payload = pkt.data;
    if (!(payload instanceof Uint8Array))
        throw new UnpackError('translate4to6 expects raw payload bytes');
    let nxt;
    let body;
    if (pkt.p === IP_PROTO_UDP || pkt.p === IP_PROTO_TCP) {
        nxt = pkt.p;
        body = fixChecksum(payload, pkt.p, (b) => ip6PseudoChecksum(src6, dstV6, nxt, b));
    }
    else if (pkt.p === IP_PROTO_ICMP) {
        const type = payload[0];
        if (type !== ICMP_ECHO && type !== ICMP_ECHOREPLY) {
            throw new UnpackError(`NAT64: unsupported ICMPv4 type ${type}`);
        }
        nxt = IP_PROTO_ICMP6;
        body = payload.slice();
        body[0] = type === ICMP_ECHO ? ICMP6_ECHO_REQUEST : ICMP6_ECHO_REPLY;
        body[2] = 0;
        body[3] = 0;
        const sum = ip6PseudoChecksum(src6, dstV6, IP_PROTO_ICMP6, body);
        body[2] = sum >> 8;
        body[3] = sum & 0xff;
    }
    else {
        throw new UnpackError(`NAT64: unsupported protocol ${pkt.p}`);
    }
    const ip6 = new IP6(undefined, {
        hlim: pkt.ttl, nxt, plen: body.length, src: src6, dst: dstV6.slice(),
    });
    ip6.data = body;
    return ip6;
}
//# sourceMappingURL=nat64.js.map