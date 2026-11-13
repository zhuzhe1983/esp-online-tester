// Serial Line IP (SLIP) framing — port of dpkt/slip.py.
import { Packet, concat, dataBytes } from './packet.js';
export const SLIP_END = 0xc0;
export const SLIP_ESC = 0xdb;
export const SLIP_ESC_END = 0xdc;
export const SLIP_ESC_ESC = 0xdd;
export class SLIP extends Packet {
    unpack(buf) {
        const res = [];
        let i = 0;
        while (i < buf.length) {
            const b = buf[i];
            if (b === SLIP_ESC && i + 1 < buf.length) {
                const n = buf[i + 1];
                if (n === SLIP_ESC_END) {
                    res.push(SLIP_END);
                    i += 2;
                    continue;
                }
                if (n === SLIP_ESC_ESC) {
                    res.push(SLIP_ESC);
                    i += 2;
                    continue;
                }
            }
            res.push(b);
            i += 1;
        }
        this.data = Uint8Array.from(res);
    }
    toBytes() {
        const res = [];
        for (const b of dataBytes(this.data)) {
            if (b === SLIP_END)
                res.push(SLIP_ESC, SLIP_ESC_END);
            else if (b === SLIP_ESC)
                res.push(SLIP_ESC, SLIP_ESC_ESC);
            else
                res.push(b);
        }
        return Uint8Array.from(res);
    }
}
/** Split a stream on END markers, returning each frame's decoded payload. */
export function slipDecode(buf) {
    const packets = [];
    let start = 0;
    for (let i = 0; i <= buf.length; i++) {
        if (i === buf.length || buf[i] === SLIP_END) {
            if (i > start)
                packets.push(new SLIP(buf.subarray(start, i)).data);
            start = i + 1;
        }
    }
    return packets;
}
/** Wrap a payload in END markers, escaping the body. */
export function slipEncode(buf) {
    const body = new SLIP(undefined, { data: buf }).toBytes();
    return concat(new Uint8Array([SLIP_END]), body, new Uint8Array([SLIP_END]));
}
//# sourceMappingURL=slip.js.map