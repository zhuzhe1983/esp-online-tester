// Constrained Application Protocol (CoAP, RFC 7252) — port of dpkt/coap.py.
import { Packet, UnpackError, NeedData, dataBytes, concat } from './packet.js';
import { pack, unpack } from './struct.js';
// Message types
export const COAP_CON = 0;
export const COAP_NON = 1;
export const COAP_ACK = 2;
export const COAP_RST = 3;
// Methods
export const COAP_GET = 1;
export const COAP_POST = 2;
export const COAP_PUT = 3;
export const COAP_DELETE = 4;
// A few response codes (class.detail encoded as class*32 + detail)
export const COAP_CREATED = 65; // 2.01
export const COAP_DELETED = 66; // 2.02
export const COAP_CHANGED = 68; // 2.04
export const COAP_CONTENT = 69; // 2.05
export const COAP_NOT_FOUND = 132; // 4.04
// A few common option numbers
export const COAP_OPT_URI_HOST = 3;
export const COAP_OPT_LOCATION_PATH = 8;
export const COAP_OPT_URI_PATH = 11;
export const COAP_OPT_CONTENT_FORMAT = 12;
export const COAP_OPT_URI_QUERY = 15;
export const COAP_OPT_ACCEPT = 17;
// Content formats (incl. OMA LWM2M)
export const COAP_FORMAT_TEXT = 0;
export const COAP_FORMAT_LINK = 40;
export const COAP_FORMAT_JSON = 50;
export const COAP_FORMAT_LWM2M_TLV = 11542;
export const COAP_FORMAT_LWM2M_JSON = 11543;
export class CoAP extends Packet {
    static hdr = [
        ['_v_t_tkl', 'B', 0x40], // Ver(2) T(2) TKL(4); default Ver=1
        ['code', 'B', 0],
        ['id', 'H', 0],
    ];
    static bitFields = {
        _v_t_tkl: [['v', 2], ['t', 2], ['tkl', 4]],
    };
    constructor(buf, fields) {
        super(buf, fields);
        if (this.token === undefined)
            this.token = new Uint8Array(0);
        if (this.opts === undefined)
            this.opts = [];
    }
    unpack(buf) {
        super.unpack(buf);
        const hl = this.hdrLen;
        if (this.tkl > 8)
            throw new UnpackError(`invalid token length: ${this.tkl}`);
        if (buf.length < hl + this.tkl)
            throw new NeedData('short token');
        this.token = buf.subarray(hl, hl + this.tkl);
        let off = hl + this.tkl;
        this.opts = [];
        let lastOpt = 0;
        while (off < buf.length) {
            if (buf[off] === 0xff) {
                if (off + 1 === buf.length)
                    throw new UnpackError('payload marker with no payload');
                off += 1;
                break;
            }
            const dl = buf[off];
            off += 1;
            let delta = (dl & 0xf0) >> 4;
            let length = dl & 0x0f;
            if (delta === 13) {
                if (off >= buf.length)
                    throw new NeedData('short option delta (1 byte)');
                delta = buf[off] + 13;
                off += 1;
            }
            else if (delta === 14) {
                if (off + 2 > buf.length)
                    throw new NeedData('short option delta (2 bytes)');
                delta = unpack('>H', buf, off)[0] + 269;
                off += 2;
            }
            else if (delta === 15) {
                throw new UnpackError('invalid option delta: 15');
            }
            if (length === 13) {
                if (off >= buf.length)
                    throw new NeedData('short option length (1 byte)');
                length = buf[off] + 13;
                off += 1;
            }
            else if (length === 14) {
                if (off + 2 > buf.length)
                    throw new NeedData('short option length (2 bytes)');
                length = unpack('>H', buf, off)[0] + 269;
                off += 2;
            }
            else if (length === 15) {
                throw new UnpackError('invalid option length: 15');
            }
            if (off + length > buf.length)
                throw new NeedData('short option value');
            const optNum = lastOpt + delta;
            this.opts.push([optNum, buf.subarray(off, off + length)]);
            off += length;
            lastOpt = optNum;
        }
        this.data = buf.subarray(off);
    }
    packOpt(optNum, value, lastOpt) {
        const delta = optNum - lastOpt;
        const length = value.length;
        const dVal = delta < 13 ? delta : delta < 269 ? 13 : 14;
        const lVal = length < 13 ? length : length < 269 ? 13 : 14;
        const parts = [pack('B', [(dVal << 4) | lVal])];
        if (dVal === 13)
            parts.push(pack('B', [delta - 13]));
        else if (dVal === 14)
            parts.push(pack('>H', [delta - 269]));
        if (lVal === 13)
            parts.push(pack('B', [length - 13]));
        else if (lVal === 14)
            parts.push(pack('>H', [length - 269]));
        parts.push(value);
        return concat(...parts);
    }
    sortedOpts() {
        return [...this.opts].sort((a, b) => a[0] - b[0]);
    }
    toBytes() {
        this.tkl = this.token.length;
        const parts = [this.packHdr(), this.token];
        let lastOpt = 0;
        for (const [num, val] of this.sortedOpts()) {
            parts.push(this.packOpt(num, val, lastOpt));
            lastOpt = num;
        }
        const payload = dataBytes(this.data);
        if (payload.length) {
            parts.push(new Uint8Array([0xff]));
            parts.push(payload);
        }
        return concat(...parts);
    }
    get codeClass() { return this.code >> 5; }
    get codeDetail() { return this.code & 0x1f; }
}
//# sourceMappingURL=coap.js.map