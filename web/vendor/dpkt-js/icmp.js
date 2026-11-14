// Internet Control Message Protocol (v4) — minimal port of dpkt/icmp.py:
// the subset NAT64 translation needs (echo request/reply + unreach header).
import { Packet, dataBytes, concat } from './packet.js';
import { inCksum } from './cksum.js';
export const ICMP_ECHOREPLY = 0; // echo reply
export const ICMP_UNREACH = 3; // dest unreachable, codes below
export const ICMP_ECHO = 8; // echo service
export const ICMP_TIMEXCEED = 11; // time exceeded, code:
export const ICMP_UNREACH_NET = 0;
export const ICMP_UNREACH_HOST = 1;
export const ICMP_UNREACH_PROTO = 2;
export const ICMP_UNREACH_PORT = 3;
export class ICMPEcho extends Packet {
    static hdr = [['id', 'H', 0], ['seq', 'H', 0]];
}
export class ICMP extends Packet {
    static hdr = [
        ['type', 'B', 8],
        ['code', 'B', 0],
        ['sum', 'H', 0],
    ];
    unpack(buf) {
        super.unpack(buf);
        if (this.type === ICMP_ECHO || this.type === ICMP_ECHOREPLY) {
            this.data = new ICMPEcho(this.data);
        }
    }
    toBytes() {
        let out = concat(this.packHdr(), dataBytes(this.data));
        if (this.sum === 0) {
            // ICMPv4 checksum covers the message only (no pseudo-header).
            this.sum = inCksum(out);
            out = concat(this.packHdr(), dataBytes(this.data));
        }
        return out;
    }
}
//# sourceMappingURL=icmp.js.map