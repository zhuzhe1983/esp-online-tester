// IEEE 802.15.4 MAC layer — port of dpkt/ieee802154.py.
import { Packet, NeedData, concat, dataBytes } from './packet.js';
import { pack, unpack } from './struct.js';
export const TYPE_BEACON = 0;
export const TYPE_DATA = 1;
export const TYPE_ACK = 2;
export const TYPE_CMD = 3;
export const ADDR_MODE_NONE = 0;
export const ADDR_MODE_RSVD = 1;
export const ADDR_MODE_SHORT = 2;
export const ADDR_MODE_EXT = 3;
function rev(a) {
    const out = new Uint8Array(a.length);
    for (let i = 0; i < a.length; i++)
        out[i] = a[a.length - 1 - i];
    return out;
}
/** Header IE element IDs (802.15.4-2015 table 7-7, and 802.15.4e). */
export const IE_CSL = 0x1a;
export const IE_RIT = 0x1b;
export const IE_RENDEZVOUS_TIME = 0x1d;
export const IE_ACK_NACK_TIME_CORRECTION = 0x1e;
export const IE_HT1 = 0x7e; // header termination, payload IEs follow
export const IE_HT2 = 0x7f; // header termination, payload follows
/** Payload IE group IDs. */
export const PIE_ESDU = 0x0;
export const PIE_MLME = 0x1;
export const PIE_VENDOR = 0x2;
export const PIE_IETF = 0x5;
export const PIE_TERMINATION = 0xf;
/** Parse an auxiliary security header; null if it is cut short. */
export function parseAuxSecurityHeader(b) {
    if (b.length < 1)
        return null;
    const ctl = b[0];
    const level = ctl & 7, keyIdMode = (ctl >> 3) & 3, fcSuppressed = (ctl >> 5) & 1;
    let off = 1;
    let frameCounter = null;
    if (!fcSuppressed) {
        if (b.length < off + 4)
            return null;
        frameCounter = (b[off] | (b[off + 1] << 8) | (b[off + 2] << 16) | (b[off + 3] << 24)) >>> 0;
        off += 4;
        if ((ctl >> 6) & 1)
            off += 1; // 2015: a 5-byte frame counter (the fifth byte is not kept)
    }
    let keySource = null, keyIndex = null;
    if (keyIdMode) {
        const srcLen = [0, 0, 4, 8][keyIdMode];
        if (b.length < off + srcLen + 1)
            return null;
        if (srcLen)
            keySource = b.subarray(off, off + srcLen);
        keyIndex = b[off + srcLen];
        off += srcLen + 1;
    }
    return { level, keyIdMode, frameCounter, keySource, keyIndex, length: off, micLength: [0, 4, 8, 16][level & 3] };
}
/** Payload IEs from `off` up to a termination IE or the first non-IE. */
export function parsePayloadIEs(buf, off, end = buf.length) {
    const ies = [];
    while (off + 2 <= end) {
        const v = buf[off] | (buf[off + 1] << 8);
        if (!(v & 0x8000))
            break;
        const len = v & 0x7ff, group = (v >> 11) & 0xf;
        if (off + 2 + len > end)
            throw new NeedData('payload IE');
        ies.push({ group, content: buf.subarray(off + 2, off + 2 + len) });
        off += 2 + len;
        if (group === PIE_TERMINATION)
            break;
    }
    return { ies, off };
}
/** Which PAN IDs a frame carries ([dst, src]): 802.15.4-2006 7.2.1.5, and the
 *  2015 table 7-2 for frame version 2. */
export function panIdPresence(version, dstMode, srcMode, comp) {
    const dst = dstMode !== ADDR_MODE_NONE, src = srcMode !== ADDR_MODE_NONE;
    if (version < 2)
        return [dst, src && !(comp && dst)];
    if (!dst && !src)
        return [!!comp, false];
    if (dst && !src)
        return [!comp, false];
    if (!dst && src)
        return [false, !comp];
    if (dstMode === ADDR_MODE_EXT && srcMode === ADDR_MODE_EXT)
        return [!comp, false];
    return [true, !comp];
}
export class IEEE802154 extends Packet {
    static hdr = [
        ['fcf', 'H', 0],
        ['seq', 'B', 0],
    ];
    static byteOrder = '<';
    // Bit fields defined MSB(bit15)..LSB(bit0) of the 16-bit FCF.
    static bitFields = {
        fcf: [
            ['src_mode', 2], // 15-14
            ['version', 2], // 13-12
            ['dst_mode', 2], // 11-10
            ['ie_present', 1], // 9   (2015: header/payload IEs follow the addressing)
            ['seq_suppr', 1], // 8   (2015: no sequence number)
            ['_rsvd', 1], // 7
            ['pan_id_comp', 1], // 6
            ['ar', 1], // 5
            ['frame_pending', 1], // 4
            ['security', 1], // 3
            ['type', 3], // 2-0
        ],
    };
    constructor(buf, fields) {
        super(buf, fields);
        this.dst_pan ??= null;
        this.dst_addr ??= null;
        this.src_pan ??= null;
        this.src_addr ??= null;
        this.aux ??= null;
        this.header_ies ??= [];
        this.payload_ies ??= [];
    }
    unpack(buf) {
        this.aux = null;
        this.header_ies = [];
        this.payload_ies = [];
        // A 2015 frame with its sequence number suppressed can be 2 bytes long.
        super.unpack(buf.length === 2 ? Uint8Array.of(buf[0], buf[1], 0) : buf);
        let off = this.hdrLen;
        if (this.version === 2 && this.seq_suppr) {
            this.seq = null; // no sequence number in the frame
            off -= 1;
        }
        const [dstPan, srcPan] = panIdPresence(this.version, this.dst_mode, this.src_mode, this.pan_id_comp);
        if (this.dst_mode !== ADDR_MODE_NONE || dstPan) {
            if (dstPan) {
                if (off + 2 > buf.length)
                    throw new NeedData();
                this.dst_pan = unpack('<H', buf, off)[0];
                off += 2;
            }
        }
        if (this.dst_mode !== ADDR_MODE_NONE) {
            const len = this.dst_mode === ADDR_MODE_SHORT ? 2 : 8;
            if (off + len > buf.length)
                throw new NeedData();
            const raw = buf.subarray(off, off + len);
            this.dst_addr = len === 8 ? rev(raw) : raw;
            off += len;
        }
        if (this.src_mode !== ADDR_MODE_NONE) {
            if (srcPan) {
                if (off + 2 > buf.length)
                    throw new NeedData();
                this.src_pan = unpack('<H', buf, off)[0];
                off += 2;
            }
            else {
                this.src_pan = this.dst_pan;
            }
            const len = this.src_mode === ADDR_MODE_SHORT ? 2 : 8;
            if (off + len > buf.length)
                throw new NeedData();
            const raw = buf.subarray(off, off + len);
            this.src_addr = len === 8 ? rev(raw) : raw;
            off += len;
        }
        if (this.security) {
            this.aux = parseAuxSecurityHeader(buf.subarray(off));
            if (!this.aux)
                throw new NeedData('auxiliary security header');
            off += this.aux.length;
        }
        // IEs (802.15.4-2015, 7.4): header IEs up to a termination IE; payload
        // IEs after HT1, readable only if the payload is not encrypted.
        let payloadIEs = false;
        // IEs end where the MIC begins: a secured frame with no payload has no
        // termination IE, its header IEs run up to the MIC.
        const end = buf.length - (this.aux ? this.aux.micLength : 0);
        if (this.ie_present) {
            while (off + 2 <= end) {
                const v = buf[off] | (buf[off + 1] << 8);
                if (v & 0x8000)
                    break; // not a header IE
                const len = v & 0x7f, id = (v >> 7) & 0xff;
                if (off + 2 + len > end)
                    throw new NeedData('header IE');
                this.header_ies.push({ id, content: buf.subarray(off + 2, off + 2 + len) });
                off += 2 + len;
                if (id === IE_HT1) {
                    payloadIEs = true;
                    break;
                }
                if (id === IE_HT2)
                    break;
            }
            // An EB or an Enh-ACK may carry payload IEs with no HT1 (no header IEs).
            if (!payloadIEs && off + 2 <= end && (buf[off + 1] & 0x80) && this.header_ies.length === 0)
                payloadIEs = true;
        }
        this.payloadIEsFollow = payloadIEs;
        if (payloadIEs && !(this.aux && this.aux.level >= 4)) {
            const r = parsePayloadIEs(buf, off, end);
            this.payload_ies = r.ies;
            off = r.off;
        }
        this.data = buf.subarray(off);
    }
    toBytes() {
        const parts = [this.packHdr()];
        if (this.dst_mode !== ADDR_MODE_NONE) {
            parts.push(pack('<H', [this.dst_pan ?? 0]));
            const a = this.dst_addr;
            parts.push(a.length === 8 ? rev(a) : a);
        }
        if (this.src_mode !== ADDR_MODE_NONE) {
            const panCompressed = this.dst_mode !== ADDR_MODE_NONE && this.pan_id_comp;
            if (!panCompressed)
                parts.push(pack('<H', [this.src_pan ?? 0]));
            const a = this.src_addr;
            parts.push(a.length === 8 ? rev(a) : a);
        }
        parts.push(dataBytes(this.data));
        return concat(...parts);
    }
}
//# sourceMappingURL=ieee802154.js.map