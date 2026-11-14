// CBOR serial-radio driver — port of the drone-test `serialradio` protocol:
// SLIP framing → CBOR map payload → 2-byte little-endian CRC-16.
// Transport-agnostic (reuses RadioTransport from slipradio.ts).
import { SLIP, slipEncode, SLIP_END } from './slip.js';
import { concat } from './packet.js';
import { crc16Append, crc16Verify } from './crc16.js';
import { cborEncode, cborDecode } from './cbor.js';
// Command opcodes (host → node)
export const Command = {
    PING: 0, GET_PARAM: 1, SET_PARAM: 2, RSSI_SCAN_START: 3, RSSI_SCAN_STOP: 4,
    RX_ON: 5, RX_OFF: 6, FAST_SCAN_START: 7, FAST_SCAN_STOP: 8, JAM_START: 9, JAM_STOP: 10,
    TX_RAW_FRAME: 50,
};
// Event opcodes (node → host)
export const Event = {
    PARAM_RESPONSE: 51, RX_FRAME: 52, RSSI_SCAN_RESULT: 53, TX_RESPONSE: 54,
    HEARTBEAT: 55, FAST_SCAN_RESULT: 56, PONG: 100, ERROR: 255,
};
// radio_param_e (os/dev/radio.h)
export const RadioParam = {
    POWER_MODE: 0, CHANNEL: 1, PAN_ID: 2, SHORT_ADDR: 3, RX_MODE: 4, TX_MODE: 5,
    TXPOWER: 6, CCA_THRESHOLD: 7, RSSI: 8, LONG_ADDR: 12,
};
// CBOR map keys (single char, compact)
const K = {
    TYPE: 't', ID: 'i', PARAM: 'p', VALUE: 'v', FRAME: 'f', RSSI: 'r', LQI: 'l',
    CHANNEL: 'c', START_CH: 's', END_CH: 'e', DWELL: 'd', ERROR: 'x', VERSION: 'V',
    RSSI_ARRAY: 'R', SEQ: 'n',
};
export class SerialRadioCbor {
    transport;
    onRxFrame;
    onHeartbeat;
    onScanResult;
    onError;
    onText;
    rxbuf = new Uint8Array(0);
    nextId = 1;
    pending = new Map();
    timeoutMs;
    constructor(transport, opts = {}) {
        this.transport = transport;
        this.timeoutMs = opts.timeoutMs ?? 2000;
        transport.onData((d) => this.feed(d));
    }
    // ---- commands ----
    async ping() {
        const r = await this.send({ [K.TYPE]: Command.PING });
        return r?.[K.TYPE] === Event.PONG;
    }
    async getParam(param) {
        const r = await this.send({ [K.TYPE]: Command.GET_PARAM, [K.PARAM]: param });
        return r && r[K.TYPE] === Event.PARAM_RESPONSE ? r[K.VALUE] : null;
    }
    async setParam(param, value) {
        const r = await this.send({ [K.TYPE]: Command.SET_PARAM, [K.PARAM]: param, [K.VALUE]: value });
        return !!r && r[K.TYPE] !== Event.ERROR;
    }
    setChannel(channel) { return this.setParam(RadioParam.CHANNEL, channel); }
    rxOn() { return this.send({ [K.TYPE]: Command.RX_ON }); }
    rxOff() { return this.send({ [K.TYPE]: Command.RX_OFF }); }
    async sendFrame(data, channel) {
        const cmd = { [K.TYPE]: Command.TX_RAW_FRAME, [K.FRAME]: data };
        if (channel !== undefined)
            cmd[K.CHANNEL] = channel;
        const r = await this.send(cmd);
        return !!r && r[K.TYPE] === Event.TX_RESPONSE && r[K.VALUE] === 0;
    }
    startScan(startCh = 11, endCh = 26, dwellMs = 10) {
        return this.send({ [K.TYPE]: Command.RSSI_SCAN_START, [K.START_CH]: startCh, [K.END_CH]: endCh, [K.DWELL]: dwellMs });
    }
    /** Encode + frame a command and (optionally) await its matching response. */
    send(cmd, expectResponse = true) {
        const id = this.nextId++;
        if (this.nextId > 0xffff)
            this.nextId = 1;
        cmd[K.ID] = id;
        const framed = crc16Append(cborEncode(cmd));
        const p = expectResponse
            ? new Promise((resolve) => {
                const timer = setTimeout(() => { this.pending.delete(id); resolve(null); }, this.timeoutMs);
                this.pending.set(id, { resolve, timer });
            })
            : Promise.resolve(null);
        this.transport.write(slipEncode(framed));
        return p;
    }
    // ---- receive ----
    feed(chunk) {
        this.rxbuf = concat(this.rxbuf, chunk);
        let idx;
        while ((idx = this.rxbuf.indexOf(SLIP_END)) !== -1) {
            const seg = this.rxbuf.subarray(0, idx);
            this.rxbuf = this.rxbuf.subarray(idx + 1);
            if (seg.length)
                this.handleSegment(new SLIP(seg).data);
        }
    }
    handleSegment(payload) {
        // Non-CRC/CBOR debug text the firmware may emit between framed messages.
        if (!crc16Verify(payload)) {
            this.onText?.(safeText(payload));
            return;
        }
        let msg;
        try {
            msg = cborDecode(payload.subarray(0, payload.length - 2));
        }
        catch {
            return;
        }
        this.dispatch(msg);
    }
    dispatch(msg) {
        const id = msg[K.ID] ?? 0;
        const waiter = this.pending.get(id);
        if (id && waiter) {
            clearTimeout(waiter.timer);
            this.pending.delete(id);
            waiter.resolve(msg);
            return;
        }
        switch (msg[K.TYPE]) {
            case Event.RX_FRAME:
                this.onRxFrame?.({ data: msg[K.FRAME] ?? new Uint8Array(0), rssi: msg[K.RSSI] ?? 0, lqi: msg[K.LQI] ?? 0 });
                break;
            case Event.RSSI_SCAN_RESULT:
                this.onScanResult?.(msg[K.CHANNEL] ?? 0, msg[K.RSSI] ?? 0);
                break;
            case Event.HEARTBEAT:
                this.onHeartbeat?.(msg);
                break;
            case Event.ERROR:
                this.onError?.(msg[K.ERROR] ?? -1);
                break;
            default: break;
        }
    }
}
function safeText(b) {
    try {
        return new TextDecoder().decode(b).trim();
    }
    catch {
        return '';
    }
}
//# sourceMappingURL=serialradio_cbor.js.map