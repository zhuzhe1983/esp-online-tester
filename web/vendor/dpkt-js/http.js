// Hypertext Transfer Protocol — port of dpkt/http.py.
// Text-based, so it works over a small byte reader instead of the struct codec.
import { UnpackError, NeedData, concat } from './packet.js';
const ascii = new TextDecoder('ascii');
const utf8 = new TextDecoder();
const enc = new TextEncoder();
/** Sequential reader over a Uint8Array (the BytesIO of the Python original). */
class BufReader {
    buf;
    off;
    constructor(buf, off = 0) {
        this.buf = buf;
        this.off = off;
    }
    /** Read up to and including the next '\n' (or the rest of the buffer). */
    readline() {
        const i = this.buf.indexOf(0x0a, this.off);
        const end = i === -1 ? this.buf.length : i + 1;
        const line = this.buf.subarray(this.off, end);
        this.off = end;
        return line;
    }
    read(n) {
        const end = n === undefined ? this.buf.length : Math.min(this.off + n, this.buf.length);
        const out = this.buf.subarray(this.off, end);
        this.off = end;
        return out;
    }
}
const strip = (s) => s.replace(/^[\s\0]+|[\s\0]+$/g, '');
/** Parse HTTP headers from a reader; duplicate keys collect into an array. */
export function parseHeaders(f) {
    const d = {};
    for (;;) {
        const line = strip(ascii.decode(f.readline()));
        if (!line)
            break;
        const idx = line.indexOf(':');
        const rawKey = idx === -1 ? line : line.slice(0, idx);
        if (rawKey.trim().split(/\s+/).length !== 1)
            throw new UnpackError(`invalid header: '${line}'`);
        const k = rawKey.toLowerCase();
        const v = idx === -1 ? '' : line.slice(idx + 1).replace(/^\s+/, '');
        if (k in d) {
            if (!Array.isArray(d[k]))
                d[k] = [d[k]];
            d[k].push(v);
        }
        else {
            d[k] = v;
        }
    }
    return d;
}
/** Parse an HTTP body given already-parsed headers (chunked / content-length). */
export function parseBody(f, headers) {
    const te = headers['transfer-encoding'];
    if (typeof te === 'string' && te.toLowerCase() === 'chunked') {
        const chunks = [];
        let foundEnd = false;
        for (;;) {
            const szTok = strip(ascii.decode(f.readline())).split(/\s+/, 1)[0];
            if (!szTok)
                throw new UnpackError('missing chunk size');
            if (!/^[0-9a-fA-F]+$/.test(szTok))
                throw new UnpackError('invalid chunk size');
            const n = parseInt(szTok, 16);
            if (n === 0)
                foundEnd = true;
            const buf = f.read(n);
            if (strip(ascii.decode(f.readline())))
                break;
            if (n && buf.length === n)
                chunks.push(buf);
            else
                break; // short read: buffer ended mid-chunk
        }
        if (!foundEnd)
            throw new NeedData('premature end of chunked body');
        return concat(...chunks);
    }
    if ('content-length' in headers) {
        const n = parseInt(headers['content-length'], 10);
        const body = f.read(n);
        if (body.length !== n)
            throw new NeedData(`short body (missing ${n - body.length} bytes)`);
        return body;
    }
    if ('content-type' in headers)
        return f.read();
    // XXX - need to handle HTTP/0.9
    return new Uint8Array(0);
}
/** HTTP headers + body. */
export class Message {
    headers = {};
    body = new Uint8Array(0);
    data = new Uint8Array(0);
    constructor(buf, fields) {
        if (buf)
            this.unpack(buf);
        else if (fields)
            Object.assign(this, fields);
    }
    unpack(buf, isBodyAllowed = true) {
        const f = new BufReader(buf);
        this.headers = parseHeaders(f);
        this.body = isBodyAllowed ? parseBody(f, this.headers) : new Uint8Array(0);
        this.data = f.read(); // whatever follows this message
    }
    packHdr() {
        return Object.entries(this.headers)
            .flatMap(([k, v]) => (Array.isArray(v) ? v : [v]).map((x) => `${k}: ${x}\r\n`))
            .join('');
    }
    startLine() { return ''; }
    toString() {
        return `${this.startLine()}${this.packHdr()}\r\n${utf8.decode(this.body)}`;
    }
    toBytes() {
        return concat(enc.encode(`${this.startLine()}${this.packHdr()}\r\n`), this.body);
    }
    byteLength() { return this.toBytes().length; }
}
const METHODS = new Set([
    'GET', 'PUT', 'ICY',
    'COPY', 'HEAD', 'LOCK', 'MOVE', 'POLL', 'POST',
    'BCOPY', 'BMOVE', 'MKCOL', 'TRACE', 'LABEL', 'MERGE',
    'DELETE', 'SEARCH', 'UNLOCK', 'REPORT', 'UPDATE', 'NOTIFY',
    'BDELETE', 'CONNECT', 'OPTIONS', 'CHECKIN',
    'PROPFIND', 'CHECKOUT', 'CCM_POST',
    'SUBSCRIBE', 'PROPPATCH', 'BPROPFIND',
    'BPROPPATCH', 'UNCHECKOUT', 'MKACTIVITY',
    'MKWORKSPACE', 'UNSUBSCRIBE', 'RPC_CONNECT',
    'VERSION-CONTROL', 'BASELINE-CONTROL',
]);
const PROTO = 'HTTP';
export class Request extends Message {
    method = 'GET';
    uri = '/';
    version = '1.0';
    constructor(buf, fields) {
        super();
        if (buf)
            this.unpack(buf);
        else if (fields)
            Object.assign(this, fields);
    }
    unpack(buf) {
        const f = new BufReader(buf);
        const line = ascii.decode(f.readline());
        const l = strip(line).split(/\s+/);
        if (l.length < 2 || !l[0])
            throw new UnpackError(`invalid request: '${line}'`);
        if (!METHODS.has(l[0]))
            throw new UnpackError(`invalid http method: '${l[0]}'`);
        if (l.length === 2) {
            this.version = '0.9'; // HTTP/0.9 has no version in the request line
        }
        else {
            if (!l[2].startsWith(PROTO))
                throw new UnpackError(`invalid http version: '${l[2]}'`);
            this.version = l[2].slice(PROTO.length + 1);
        }
        this.method = l[0];
        this.uri = l[1];
        super.unpack(f.read());
    }
    startLine() {
        return `${this.method} ${this.uri} ${PROTO}/${this.version}\r\n`;
    }
}
export class Response extends Message {
    version = '1.0';
    status = '200';
    reason = 'OK';
    constructor(buf, fields) {
        super();
        if (buf)
            this.unpack(buf);
        else if (fields)
            Object.assign(this, fields);
    }
    unpack(buf) {
        const f = new BufReader(buf);
        const line = ascii.decode(f.readline());
        const l = strip(line).split(/\s+/);
        if (l.length < 2 || !l[0].startsWith(PROTO) || !/^\d+$/.test(l[1])) {
            throw new UnpackError(`invalid response: '${strip(line)}'`);
        }
        this.version = l[0].slice(PROTO.length + 1);
        this.status = l[1];
        // reason is everything after the status code (may contain spaces / be empty)
        const m = strip(line).match(/^\S+\s+\S+\s+(.*)$/);
        this.reason = m ? m[1] : '';
        // RFC 2616 sec 4.3: 1xx, 204 and 304 responses must not carry a body.
        const st = parseInt(this.status, 10);
        const isBodyAllowed = st >= 200 && st !== 204 && st !== 304;
        super.unpack(f.read(), isBodyAllowed);
    }
    startLine() {
        return `${PROTO}/${this.version} ${this.status} ${this.reason}\r\n`;
    }
}
//# sourceMappingURL=http.js.map