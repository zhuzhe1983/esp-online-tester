// slip-radio driver — the command protocol a Contiki-NG slip-radio speaks over
// SLIP, ported from examples/rpl_br.py. Transport-agnostic: give it anything
// that can write bytes and deliver received bytes (Node serialport, WebSerial,
// a TCP socket, or the in-memory transport used in tests).
import { SLIP, slipEncode, SLIP_END } from './slip.js';
import { IEEE802154 } from './ieee802154.js';
import { concat } from './packet.js';
import { pack } from './struct.js';
const enc = new TextEncoder();
const dec = new TextDecoder();
export class SlipRadio {
    transport;
    /** EUI-64 of the radio, once `?M` has been answered. */
    mac = null;
    // event hooks
    onMac;
    onFrame;
    onText;
    onTxReport;
    rxbuf = new Uint8Array(0);
    seq = 0;
    constructor(transport) {
        this.transport = transport;
        transport.onData((d) => this.feed(d));
    }
    // ---- commands to the radio (host → radio) ----
    /** Ask the radio for its MAC/EUI-64 (`?M`). */
    requestMac() { this.send(enc.encode('?M')); }
    /** Set the radio channel (`!V` + RADIO_PARAM_CHANNEL(0) + channel). */
    setChannel(channel = 26) { this.send(concat(enc.encode('!V'), pack('>HH', [0, channel]))); }
    /** Transmit a raw IEEE 802.15.4 frame (`!S` + seq + attr_count(0) + frame). */
    sendFrame(frame) {
        const bytes = frame instanceof IEEE802154 ? frame.toBytes() : frame;
        const cmd = concat(enc.encode('!S'), Uint8Array.of(this.seq & 0xff, 0x00), bytes);
        this.seq = (this.seq + 1) & 0xff;
        this.send(cmd);
    }
    send(payload) { this.transport.write(slipEncode(payload)); }
    // ---- receive path (radio → host), SLIP-framed ----
    feed(chunk) {
        this.rxbuf = concat(this.rxbuf, chunk);
        let idx;
        while ((idx = this.rxbuf.indexOf(SLIP_END)) !== -1) {
            const segment = this.rxbuf.subarray(0, idx);
            this.rxbuf = this.rxbuf.subarray(idx + 1);
            if (segment.length)
                this.dispatch(new SLIP(segment).data);
        }
    }
    dispatch(p) {
        if (!p.length)
            return;
        const c = p[0];
        if (c === 0x21) { // '!' radio command
            if (p[1] === 0x4d && p.length >= 10) { // '!M' + 8-byte MAC
                this.mac = p.subarray(2, 10);
                this.onMac?.(this.mac);
            }
            else if (p[1] === 0x52) { // '!R' TX report
                this.onTxReport?.(p[3] ?? -1, p[4] ?? 0);
            }
        }
        else if (c === 0x45) { // 'E' error text
            this.onText?.('ERROR ' + dec.decode(p).trim());
        }
        else if (c === 0x23) { // '#' debug text
            this.onText?.(dec.decode(p.subarray(1)).trim());
        }
        else if (p.length >= 5) { // a received 802.15.4 frame
            try {
                this.onFrame?.(new IEEE802154(p), p);
            }
            catch { /* malformed */ }
        }
    }
}
// ---- a simple in-memory transport (for tests / a simulated radio) ----
export class MemoryTransport {
    cb = null;
    /** Set by the paired peer: where our writes go. */
    peer = null;
    write(data) { this.peer?.(data); }
    onData(cb) { this.cb = cb; }
    /** Deliver bytes into this transport (called by the peer). */
    deliver(data) { this.cb?.(data); }
    /** Wire two MemoryTransports together (host <-> radio). */
    static pair() {
        const a = new MemoryTransport();
        const b = new MemoryTransport();
        a.peer = (d) => b.deliver(d);
        b.peer = (d) => a.deliver(d);
        return [a, b];
    }
}
//# sourceMappingURL=slipradio.js.map