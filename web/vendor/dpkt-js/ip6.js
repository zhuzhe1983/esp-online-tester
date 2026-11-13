// Internet Protocol v6 — port of dpkt/ip6.py, including the extension-header
// chain and the RPL source-routing header (RH3).
import { Packet, concat, dataBytes } from './packet.js';
import { ip6PseudoChecksum } from './cksum.js';
export const IP_PROTO_HOPOPTS = 0;
export const IP_PROTO_TCP = 6;
export const IP_PROTO_UDP = 17;
export const IP_PROTO_ROUTING = 43;
export const IP_PROTO_FRAGMENT = 44;
export const IP_PROTO_ESP = 50;
export const IP_PROTO_AH = 51;
export const IP_PROTO_ICMP6 = 58;
export const IP_PROTO_DSTOPTS = 60;
// ----- extension headers -----
export class IP6ExtensionHeader extends Packet {
}
export class IP6OptsHeader extends IP6ExtensionHeader {
    static hdr = [['nxt', 'B', 0], ['len', 'B', 0]];
    unpack(buf) {
        super.unpack(buf);
        this.length = (this.len + 1) * 8;
        const options = [];
        const body = buf; // includes the 2-byte header
        let index = 2;
        while (index < this.length) {
            const optType = body[index];
            if (optType === 0) {
                index += 1;
                continue;
            } // PAD1
            const optLen = body[index + 1];
            if (optType === 1) {
                index += optLen + 2;
                continue;
            } // PADN
            options.push({ type: optType, opt_length: optLen, data: body.subarray(index + 2, index + 2 + optLen) });
            index += optLen + 2;
        }
        this.options = options;
        this.data = buf.subarray(2, this.length);
    }
}
export class IP6HopOptsHeader extends IP6OptsHeader {
}
export class IP6DstOptsHeader extends IP6OptsHeader {
}
export class IP6RoutingHeader extends IP6ExtensionHeader {
    static hdr = [
        ['nxt', 'B', 0], ['len', 'B', 0], ['type', 'B', 0], ['segs_left', 'B', 0],
        ['_rsvd_sl_bits', 'I', 0],
    ];
    static bitFields = { _rsvd_sl_bits: [['_rsvd', 8], ['sl_bits', 24]] };
    unpack(buf) {
        super.unpack(buf);
        this.bytes = buf.subarray(0, this.len * 8 + 8);
        const HDR = 8;
        const ADDR = 16;
        const num = Math.floor(this.len / 2);
        const body = buf.subarray(HDR, HDR + num * ADDR);
        const addresses = [];
        for (let i = 0; i < num; i++)
            addresses.push(body.subarray(i * ADDR, i * ADDR + ADDR));
        this.data = body;
        this.addresses = addresses;
        this.length = this.len * 8 + 8;
    }
    toBytes() {
        // segs_left/len follow the address list unless caller set them explicitly.
        const addrs = this.addresses ?? [];
        if (!this.len)
            this.len = addrs.length * 2;
        return concat(this.packHdr(), ...addrs);
    }
}
export class IP6FragmentHeader extends IP6ExtensionHeader {
    static hdr = [['nxt', 'B', 0], ['_resv', 'B', 0], ['_frag_off_resv_m', 'H', 0], ['id', 'I', 0]];
    static bitFields = { _frag_off_resv_m: [['frag_off', 13], ['_resv', 2], ['m_flag', 1]] };
    unpack(buf) { super.unpack(buf); this.length = this.hdrLen; this.data = new Uint8Array(0); }
}
export class IP6AHHeader extends IP6ExtensionHeader {
    static hdr = [['nxt', 'B', 0], ['len', 'B', 0], ['_resv', 'H', 0], ['spi', 'I', 0], ['seq', 'I', 0]];
    unpack(buf) { super.unpack(buf); this.length = (this.len + 2) * 4; this.auth_data = this.data.subarray(0, (this.len - 1) * 4); }
}
export class IP6ESPHeader extends IP6ExtensionHeader {
    static hdr = [['spi', 'I', 0], ['seq', 'I', 0]];
    unpack(buf) { super.unpack(buf); this.length = this.hdrLen + this.data.length; }
}
export const EXT_HDRS = new Set([IP_PROTO_HOPOPTS, IP_PROTO_ROUTING, IP_PROTO_FRAGMENT, IP_PROTO_AH, IP_PROTO_ESP, IP_PROTO_DSTOPTS]);
const EXT_HDRS_CLS = {
    [IP_PROTO_HOPOPTS]: IP6HopOptsHeader,
    [IP_PROTO_ROUTING]: IP6RoutingHeader,
    [IP_PROTO_FRAGMENT]: IP6FragmentHeader,
    [IP_PROTO_ESP]: IP6ESPHeader,
    [IP_PROTO_AH]: IP6AHHeader,
    [IP_PROTO_DSTOPTS]: IP6DstOptsHeader,
};
// ----- IP6 -----
export class IP6 extends Packet {
    static hdr = [
        ['_v_fc_flow', 'I', 0x60000000],
        ['plen', 'H', 0],
        ['nxt', 'B', 0],
        ['hlim', 'B', 0],
        ['src', '16s', new Uint8Array(16)],
        ['dst', '16s', new Uint8Array(16)],
    ];
    static bitFields = { _v_fc_flow: [['v', 4], ['fc', 8], ['flow', 20]] };
    unpack(buf) {
        super.unpack(buf);
        this.extension_hdrs = {};
        this.all_extension_headers = [];
        let payload = this.plen ? this.data.subarray(0, this.plen) : this.data;
        let next = this.nxt;
        while (EXT_HDRS.has(next)) {
            const Cls = EXT_HDRS_CLS[next];
            const ext = new Cls(payload);
            this.extension_hdrs[next] = ext;
            this.all_extension_headers.push(ext);
            payload = payload.subarray(ext.length);
            next = ext.nxt;
            if (next === undefined)
                break;
        }
        this.p = next;
        this.data = payload;
    }
    /** (finalProto, serialized-ext-headers) — mirrors dpkt headers_str(). */
    headersStr() {
        if (this.all_extension_headers && this.all_extension_headers.length) {
            const nxt = this.all_extension_headers[this.all_extension_headers.length - 1].nxt;
            return [nxt, concat(...this.all_extension_headers.map((e) => e.toBytes()))];
        }
        return [this.nxt, new Uint8Array(0)];
    }
    toBytes() {
        const [p, hdrStr] = this.headersStr();
        this.p = p;
        const d = this.data;
        // Fill in TCP/UDP/ICMPv6 checksum when wrapping a sub-packet with sum===0.
        if ((p === IP_PROTO_TCP || p === IP_PROTO_UDP || p === IP_PROTO_ICMP6) &&
            d && typeof d === 'object' && 'sum' in d && !d.sum) {
            const pb = dataBytes(d);
            d.sum = ip6PseudoChecksum(this.src, this.dst, p, pb);
        }
        return concat(this.packHdr(), hdrStr, dataBytes(this.data));
    }
}
const HEX = (b) => b.toString(16).padStart(2, '0');
/** The textual form of a 16-byte IPv6 address (RFC 5952: lowercase, the
 *  longest run of two or more zero groups folded to ::). */
export function ip6str(addr) {
    const g = [];
    for (let i = 0; i < 16; i += 2)
        g.push((addr[i] << 8) | addr[i + 1]);
    let best = -1, bestLen = 1;
    for (let i = 0; i < 8;) {
        if (g[i]) {
            i++;
            continue;
        }
        let j = i;
        while (j < 8 && !g[j])
            j++;
        if (j - i > bestLen) {
            best = i;
            bestLen = j - i;
        }
        i = j;
    }
    const hexs = g.map((x) => x.toString(16));
    if (best < 0)
        return hexs.join(':');
    return `${hexs.slice(0, best).join(':')}::${hexs.slice(best + bestLen).join(':')}`;
}
export { HEX };
//# sourceMappingURL=ip6.js.map