// Domain Name System — port of dpkt/dns.py (name compression, questions,
// resource records with per-type rdata). mDNS (Contiki resolv.c) uses the
// same wire format on UDP port 5353.
import { Packet, UnpackError, NeedData, concat } from './packet.js';
import { pack, unpack } from './struct.js';
import { toHex } from './util.js';
export const DNS_Q = 0;
export const DNS_R = 1;
// Opcodes
export const DNS_QUERY = 0;
export const DNS_IQUERY = 1;
export const DNS_STATUS = 2;
export const DNS_NOTIFY = 4;
export const DNS_UPDATE = 5;
// Flags
export const DNS_CD = 0x0010; // checking disabled
export const DNS_AD = 0x0020; // authenticated data
export const DNS_Z = 0x0040; // unused
export const DNS_RA = 0x0080; // recursion available
export const DNS_RD = 0x0100; // recursion desired
export const DNS_TC = 0x0200; // truncated
export const DNS_AA = 0x0400; // authoritative answer
export const DNS_QR = 0x8000; // response (query / response)
// Response codes
export const DNS_RCODE_NOERR = 0;
export const DNS_RCODE_FORMERR = 1;
export const DNS_RCODE_SERVFAIL = 2;
export const DNS_RCODE_NXDOMAIN = 3;
export const DNS_RCODE_NOTIMP = 4;
export const DNS_RCODE_REFUSED = 5;
// RR types
export const DNS_A = 1;
export const DNS_NS = 2;
export const DNS_CNAME = 5;
export const DNS_SOA = 6;
export const DNS_NULL = 10;
export const DNS_PTR = 12;
export const DNS_HINFO = 13;
export const DNS_MX = 15;
export const DNS_TXT = 16;
export const DNS_AAAA = 28;
export const DNS_SRV = 33;
export const DNS_OPT = 41;
// RR classes
export const DNS_IN = 1;
export const DNS_CHAOS = 3;
export const DNS_HESIOD = 4;
export const DNS_ANY = 255;
const utf8enc = new TextEncoder();
const utf8dec = new TextDecoder();
/** Pack a domain name with RFC 1035 label compression. */
export function packName(name, off, labelPtrs) {
    const labels = name ? name.split('.') : [];
    labels.push('');
    const parts = [];
    let len = 0;
    for (let i = 0; i < labels.length; i++) {
        const key = labels.slice(i).join('.').toUpperCase();
        const ptr = labelPtrs.get(key);
        if (ptr === undefined) {
            if (key.length > 1) {
                const p = off + len;
                if (p < 0xc000)
                    labelPtrs.set(key, p);
            }
            const lb = utf8enc.encode(labels[i]);
            parts.push(Uint8Array.of(lb.length), lb);
            len += 1 + lb.length;
        }
        else {
            parts.push(pack('>H', [0xc000 | ptr]));
            break;
        }
    }
    return concat(...parts);
}
/** Unpack a (possibly compressed) domain name. Returns [name, newOffset]. */
export function unpackName(buf, off) {
    const name = [];
    let savedOff = 0;
    let startOff = off;
    let nameLength = 0;
    for (;;) {
        if (off >= buf.length)
            throw new NeedData();
        const n = buf[off];
        if (n === 0) {
            off += 1;
            break;
        }
        if ((n & 0xc0) === 0xc0) {
            if (off + 2 > buf.length)
                throw new NeedData();
            const ptr = ((buf[off] << 8) | buf[off + 1]) & 0x3fff;
            if (ptr >= startOff)
                throw new UnpackError('Invalid label compression pointer');
            off += 2;
            if (!savedOff)
                savedOff = off;
            startOff = off = ptr;
        }
        else if ((n & 0xc0) === 0x00) {
            off += 1;
            if (off + n > buf.length)
                throw new NeedData();
            name.push(buf.subarray(off, off + n));
            nameLength += n + 1;
            if (nameLength > 255)
                throw new UnpackError('name longer than 255 bytes');
            off += n;
        }
        else {
            throw new UnpackError(`Invalid label length ${n.toString(16)}`);
        }
    }
    if (!savedOff)
        savedOff = off;
    return [name.map((l) => utf8dec.decode(l)).join('.'), savedOff];
}
/** DNS question. */
export class DNSQ {
    name = '';
    type = DNS_A;
    cls = DNS_IN;
    constructor(fields) { Object.assign(this, fields); }
}
/** DNS resource record. */
/** Set while parseDnsLenient parses. */
let keepUnmodelled = false;
export class DNSRR {
    /** Read by parseDnsLenient as a type this module has no model for. */
    unmodelled = false;
    name = '';
    type = DNS_A;
    cls = DNS_IN;
    ttl = 0;
    rlen = 4;
    rdata = new Uint8Array(0);
    constructor(fields) { Object.assign(this, fields); }
    packRdata(off, labelPtrs) {
        if (this.rdata && this.rdata.length)
            return this.rdata;
        switch (this.type) {
            case DNS_A: return this.ip;
            case DNS_NS: return packName(this.nsname, off, labelPtrs);
            case DNS_CNAME: return packName(this.cname, off, labelPtrs);
            case DNS_PTR: return packName(this.ptrname, off, labelPtrs);
            case DNS_SOA: {
                const m = packName(this.mname, off, labelPtrs);
                const r = packName(this.rname, off + m.length, labelPtrs);
                return concat(m, r, pack('>IIIII', [this.serial, this.refresh, this.retry, this.expire, this.minimum]));
            }
            case DNS_MX: return concat(pack('>H', [this.preference]), packName(this.mxname, off + 2, labelPtrs));
            case DNS_TXT:
            case DNS_HINFO:
                return concat(...(this.text ?? []).map((x) => {
                    const b = typeof x === 'string' ? utf8enc.encode(x) : x;
                    return concat(Uint8Array.of(b.length), b);
                }));
            case DNS_AAAA: return this.ip6;
            case DNS_SRV:
                return concat(pack('>HHH', [this.priority, this.weight, this.port]), packName(this.srvname, off + 6, labelPtrs));
            case DNS_OPT: return new Uint8Array(0); // RFC 6891 pseudo-RR
            default:
                // A type read by parseDnsLenient round-trips as its rdata.
                if (this.unmodelled)
                    return this.rdata;
                throw new UnpackError(`RR type ${this.type} is not supported`);
        }
    }
    unpackRdata(buf, off) {
        switch (this.type) {
            case DNS_A:
                this.ip = this.rdata;
                break;
            case DNS_NS:
                [this.nsname] = unpackName(buf, off);
                break;
            case DNS_CNAME:
                [this.cname] = unpackName(buf, off);
                break;
            case DNS_PTR:
                [this.ptrname] = unpackName(buf, off);
                break;
            case DNS_SOA: {
                let o = off;
                [this.mname, o] = unpackName(buf, o);
                [this.rname, o] = unpackName(buf, o);
                if (o + 20 > buf.length)
                    throw new NeedData();
                [this.serial, this.refresh, this.retry, this.expire, this.minimum] =
                    unpack('>IIIII', buf, o);
                break;
            }
            case DNS_MX:
                if (this.rdata.length < 2)
                    throw new NeedData();
                this.preference = unpack('>H', this.rdata, 0)[0];
                [this.mxname] = unpackName(buf, off + 2);
                break;
            case DNS_TXT:
            case DNS_HINFO: {
                this.text = [];
                let b = this.rdata;
                while (b.length) {
                    const n = b[0];
                    this.text.push(utf8dec.decode(b.subarray(1, 1 + n)));
                    b = b.subarray(1 + n);
                }
                break;
            }
            case DNS_AAAA:
                this.ip6 = this.rdata;
                break;
            case DNS_NULL:
                this.null = toHex(this.rdata);
                break;
            case DNS_SRV:
                if (this.rdata.length < 6)
                    throw new NeedData();
                [this.priority, this.weight, this.port] = unpack('>HHH', this.rdata, 0);
                [this.srvname] = unpackName(buf, off + 6);
                break;
            case DNS_OPT: break; // RFC 6891: OPT carries no DNS data
            default:
                if (!keepUnmodelled)
                    throw new UnpackError(`RR type ${this.type} is not supported`);
                this.unmodelled = true; // kept as rdata: KEY and SIG in an SRP update, ANY to delete an RRset
                break;
        }
    }
}
export class DNS extends Packet {
    static hdr = [
        ['id', 'H', 0],
        ['op', 'H', DNS_RD], // recursive query
        ['_qd_cnt', 'H', 0],
        ['_an_cnt', 'H', 0],
        ['_ns_cnt', 'H', 0],
        ['_ar_cnt', 'H', 0],
    ];
    static Q = DNSQ;
    static RR = DNSRR;
    constructor(buf, fields) {
        super(buf, fields);
        if (this.qd === undefined)
            this.qd = [];
        if (this.an === undefined)
            this.an = [];
        if (this.ns === undefined)
            this.ns = [];
        if (this.ar === undefined)
            this.ar = [];
    }
    get qr() { return (this.op & DNS_QR) === DNS_QR ? 1 : 0; }
    set qr(v) { if (v)
        this.op |= DNS_QR;
    else
        this.op &= ~DNS_QR; }
    get opcode() { return (this.op >> 11) & 0xf; }
    set opcode(v) { this.op = (this.op & ~0x7800) | ((v & 0xf) << 11); }
    get aa() { return (this.op & DNS_AA) === DNS_AA ? 1 : 0; }
    set aa(v) { if (v)
        this.op |= DNS_AA;
    else
        this.op &= ~DNS_AA; }
    get tc() { return (this.op & DNS_TC) === DNS_TC ? 1 : 0; }
    set tc(v) { if (v)
        this.op |= DNS_TC;
    else
        this.op &= ~DNS_TC; }
    get rd() { return (this.op & DNS_RD) === DNS_RD ? 1 : 0; }
    set rd(v) { if (v)
        this.op |= DNS_RD;
    else
        this.op &= ~DNS_RD; }
    get ra() { return (this.op & DNS_RA) === DNS_RA ? 1 : 0; }
    set ra(v) { if (v)
        this.op |= DNS_RA;
    else
        this.op &= ~DNS_RA; }
    get zero() { return (this.op & DNS_Z) === DNS_Z ? 1 : 0; }
    set zero(v) { if (v)
        this.op |= DNS_Z;
    else
        this.op &= ~DNS_Z; }
    get rcode() { return this.op & 0xf; }
    set rcode(v) { this.op = (this.op & ~0xf) | (v & 0xf); }
    unpackQ(buf, off) {
        const q = new DNSQ();
        [q.name, off] = unpackName(buf, off);
        if (off + 4 > buf.length)
            throw new NeedData();
        [q.type, q.cls] = unpack('>HH', buf, off);
        return [q, off + 4];
    }
    unpackRR(buf, off) {
        const rr = new DNSRR();
        [rr.name, off] = unpackName(buf, off);
        if (off + 10 > buf.length)
            throw new NeedData();
        let rdlen;
        [rr.type, rr.cls, rr.ttl, rdlen] = unpack('>HHIH', buf, off);
        off += 10;
        rr.rdata = buf.subarray(off, off + rdlen);
        rr.rlen = rdlen;
        rr.unpackRdata(buf, off);
        return [rr, off + rdlen];
    }
    unpack(buf) {
        super.unpack(buf);
        let off = this.hdrLen;
        this.qd = [];
        for (let i = 0; i < this._qd_cnt; i++) {
            const [q, o] = this.unpackQ(buf, off);
            this.qd.push(q);
            off = o;
        }
        for (const x of ['an', 'ns', 'ar']) {
            const cnt = this[`_${x}_cnt`];
            this[x] = [];
            for (let i = 0; i < cnt; i++) {
                const [rr, o] = this.unpackRR(buf, off);
                this[x].push(rr);
                off = o;
            }
        }
        this.data = new Uint8Array(0);
    }
    toBytes() {
        const labelPtrs = new Map();
        let buf = pack('>HHHHHH', [this.id, this.op, this.qd.length, this.an.length, this.ns.length, this.ar.length]);
        for (const q of this.qd) {
            buf = concat(buf, packName(q.name, buf.length, labelPtrs), pack('>HH', [q.type, q.cls]));
        }
        for (const x of ['an', 'ns', 'ar']) {
            for (const rr of this[x]) {
                const name = packName(rr.name, buf.length, labelPtrs);
                const rdata = rr.packRdata(buf.length + name.length + 10, labelPtrs);
                buf = concat(buf, name, pack('>HHIH', [rr.type, rr.cls, rr.ttl, rdata.length]), rdata);
            }
        }
        return buf;
    }
    byteLength() { return this.toBytes().length; }
}
/** Parse a DNS message, keeping RR types this module does not model as
 *  their rdata (dpkt's DNS, and `new DNS()`, refuse them).  DNS UPDATE
 *  needs it: SRP sends KEY and SIG records, and ANY to delete an RRset. */
export function parseDnsLenient(buf) {
    keepUnmodelled = true;
    try {
        return new DNS(buf);
    }
    finally {
        keepUnmodelled = false;
    }
}
//# sourceMappingURL=dns.js.map