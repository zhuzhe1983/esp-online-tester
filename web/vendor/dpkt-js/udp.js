// User Datagram Protocol — port of dpkt/udp.py (header).
import { Packet } from './packet.js';
export const UDP_HDR_LEN = 8;
export class UDP extends Packet {
    static hdr = [
        ['sport', 'H', 0xdead],
        ['dport', 'H', 0],
        ['ulen', 'H', 8],
        ['sum', 'H', 0],
    ];
}
//# sourceMappingURL=udp.js.map