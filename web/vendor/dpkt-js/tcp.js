// Transmission Control Protocol — port of dpkt/tcp.py.
import { Packet, UnpackError, dataBytes, concat } from './packet.js';
// TCP control flags
export const TH_FIN = 0x01; // end of data
export const TH_SYN = 0x02; // synchronize sequence numbers
export const TH_RST = 0x04; // reset connection
export const TH_PUSH = 0x08; // push
export const TH_ACK = 0x10; // acknowledgment number set
export const TH_URG = 0x20; // urgent pointer set
export const TH_ECE = 0x40; // ECN echo, RFC 3168
export const TH_CWR = 0x80; // congestion window reduced
export const TH_NS = 0x100; // nonce sum, RFC 3540
export const TCP_PORT_MAX = 65535;
export const TCP_WIN_MAX = 65535;
const FLAG_NAMES = [
    [TH_FIN, 'FIN'], [TH_SYN, 'SYN'], [TH_RST, 'RST'], [TH_PUSH, 'PUSH'],
    [TH_ACK, 'ACK'], [TH_URG, 'URG'], [TH_ECE, 'ECE'], [TH_CWR, 'CWR'], [TH_NS, 'NS'],
];
export function tcpFlagsToStr(val) {
    return FLAG_NAMES.filter(([f]) => val & f).map(([, n]) => n).join(',');
}
export class TCP extends Packet {
    static hdr = [
        ['sport', 'H', 0xdead],
        ['dport', 'H', 0],
        ['seq', 'I', 0xdeadbeef],
        ['ack', 'I', 0],
        ['_off_flags', 'H', (5 << 12) | TH_SYN],
        ['win', 'H', TCP_WIN_MAX],
        ['sum', 'H', 0],
        ['urp', 'H', 0],
    ];
    static bitFields = {
        _off_flags: [['off', 4], ['_rsv', 3], ['flags', 9]],
    };
    constructor(buf, fields) {
        super(buf, fields);
        if (this.opts === undefined)
            this.opts = new Uint8Array(0);
    }
    byteLength() {
        return this.hdrLen + this.opts.length + dataBytes(this.data).length;
    }
    unpack(buf) {
        super.unpack(buf);
        const ol = ((this._off_flags >> 12) << 2) - this.hdrLen;
        if (ol < 0)
            throw new UnpackError('invalid header length');
        this.opts = buf.subarray(this.hdrLen, this.hdrLen + ol);
        this.data = buf.subarray(this.hdrLen + ol);
    }
    toBytes() {
        return concat(this.packHdr(), this.opts, dataBytes(this.data));
    }
}
// Options (opt_type) - http://www.iana.org/assignments/tcp-parameters
export const TCP_OPT_EOL = 0; // end of option list
export const TCP_OPT_NOP = 1; // no operation
export const TCP_OPT_MSS = 2; // maximum segment size
export const TCP_OPT_WSCALE = 3; // window scale factor, RFC 1072
export const TCP_OPT_SACKOK = 4; // SACK permitted, RFC 2018
export const TCP_OPT_SACK = 5; // SACK, RFC 2018
export const TCP_OPT_TIMESTAMP = 8; // timestamp, RFC 1323
export const TCP_OPT_MD5 = 19; // MD5 signature, RFC 2385
export const TCP_OPT_MAX = 27;
/** Parse a TCP option buffer into a list of [option, data] tuples
 *  (null marks a truncated/malformed trailing option, like dpkt). */
export function parseOpts(buf) {
    const opts = [];
    let off = 0;
    while (off < buf.length) {
        const o = buf[off];
        if (o > TCP_OPT_NOP) {
            if (off + 1 >= buf.length) {
                opts.push(null);
                break;
            }
            const l = Math.max(2, buf[off + 1]);
            opts.push([o, buf.subarray(off + 2, off + l)]);
            off += l;
        }
        else {
            opts.push([o, new Uint8Array(0)]);
            off += 1;
        }
    }
    return opts;
}
//# sourceMappingURL=tcp.js.map