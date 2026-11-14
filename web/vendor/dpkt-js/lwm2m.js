// OMA LWM2M — port of dpkt/lwm2m.py. TLV + JSON wire formats with a
// format-agnostic data model (Resource / ObjectInstance).
import { Packet, NeedData } from './packet.js';
import { pack, unpack } from './struct.js';
import { concat } from './packet.js';
// TLV type IDs (top two bits of the type byte)
export const TLV_OBJECT_INSTANCE = 0x00;
export const TLV_RESOURCE_INSTANCE = 0x40;
export const TLV_MULTIPLE_RESOURCE = 0x80;
export const TLV_RESOURCE = 0xc0;
export const ContentFormat = {
    TEXT: 0, OPAQUE: 42, TLV: 11542, JSON: 11543, SENML_JSON: 110, SENML_CBOR: 112, LWM2M_CBOR: 11544,
};
const enc = new TextEncoder();
const dec = new TextDecoder();
// ----- data model -----
// JS has no float/int distinction (15.0 === 15, Number.isInteger(15.0) === true),
// but LWM2M TLV is not self-describing, so a FLOAT-typed resource must be encoded
// as an IEEE double even when its value is whole. Wrap such values with float().
export class LwFloat {
    value;
    constructor(value) {
        this.value = value;
    }
}
export function float(n) { return new LwFloat(n); }
export class Resource {
    id;
    value;
    instances;
    constructor(id, value = null, instances = null) {
        this.id = id;
        this.value = value;
        this.instances = instances;
    }
    get isMulti() { return this.instances !== null; }
}
export class ObjectInstance {
    id;
    resources;
    constructor(id = 0, resources = []) {
        this.id = id;
        this.resources = resources;
    }
}
// ----- TLV wire format -----
export class TLV extends Packet {
    constructor(buf, fields) {
        super(buf);
        this.type ??= 0;
        this.id ??= 0;
        this.value ??= new Uint8Array(0);
        if (fields) {
            if (fields.type !== undefined)
                this.type = fields.type;
            if (fields.id !== undefined)
                this.id = fields.id;
            if (fields.value !== undefined)
                this.value = fields.value;
        }
    }
    unpack(buf) {
        if (!buf.length)
            throw new NeedData('empty TLV');
        const tb = buf[0];
        this.type = tb & 0xc0;
        const idLen = (tb & 0x20) >> 5;
        const lenType = (tb & 0x18) >> 3;
        let valLen = tb & 0x07;
        let off = 1;
        if (idLen === 0) {
            this.id = buf[off];
            off += 1;
        }
        else {
            this.id = unpack('>H', buf, off)[0];
            off += 2;
        }
        if (lenType === 1) {
            valLen = buf[off];
            off += 1;
        }
        else if (lenType === 2) {
            valLen = unpack('>H', buf, off)[0];
            off += 2;
        }
        else if (lenType === 3) {
            valLen = (buf[off] << 16) | (buf[off + 1] << 8) | buf[off + 2];
            off += 3;
        }
        if (off + valLen > buf.length)
            throw new NeedData('short TLV value');
        this.value = buf.subarray(off, off + valLen);
        this.data = buf.subarray(off + valLen);
    }
    toBytes() {
        const idBit = this.id > 255 ? 0x20 : 0x00;
        const vl = this.value.length;
        let lenType;
        let lenBits;
        let lenField;
        if (vl < 8) {
            lenType = 0x00;
            lenBits = vl;
            lenField = new Uint8Array(0);
        }
        else if (vl < 256) {
            lenType = 0x08;
            lenBits = 0;
            lenField = pack('B', [vl]);
        }
        else if (vl < 65536) {
            lenType = 0x10;
            lenBits = 0;
            lenField = pack('>H', [vl]);
        }
        else {
            lenType = 0x18;
            lenBits = 0;
            lenField = pack('>I', [vl]).subarray(1);
        }
        const typeByte = this.type | idBit | lenType | lenBits;
        const idField = idBit ? pack('>H', [this.id]) : pack('B', [this.id]);
        return concat(pack('B', [typeByte]), idField, lenField, this.value);
    }
}
export class LwJSON extends Packet {
    constructor(buf, fields) {
        super(typeof buf === 'string' ? enc.encode(buf) : buf);
        this.bn ??= '';
        this.bt ??= null;
        this.entries ??= [];
        if (fields) {
            if (fields.bn !== undefined)
                this.bn = fields.bn;
            if (fields.bt !== undefined)
                this.bt = fields.bt;
            if (fields.entries !== undefined)
                this.entries = fields.entries;
        }
    }
    unpack(buf) {
        const obj = JSON.parse(dec.decode(buf));
        this.bn = obj.bn ?? '';
        this.bt = obj.bt ?? null;
        this.entries = obj.e ?? [];
        this.data = new Uint8Array(0);
    }
    toBytes() {
        const out = {};
        if (this.bn)
            out.bn = this.bn;
        if (this.bt !== null)
            out.bt = this.bt;
        out.e = this.entries;
        return enc.encode(JSON.stringify(out));
    }
}
// ----- dispatch -----
export function encode(obj, contentFormat, basePath = '') {
    if (contentFormat === ContentFormat.TLV)
        return encodeTlv(obj);
    if (contentFormat === ContentFormat.JSON)
        return encodeJson(obj, basePath);
    throw new Error(`LWM2M content format ${contentFormat} is not implemented`);
}
export function decode(buf, contentFormat, basePath = '') {
    if (contentFormat === ContentFormat.TLV)
        return decodeTlv(buf);
    if (contentFormat === ContentFormat.JSON)
        return decodeJson(buf, basePath);
    throw new Error(`LWM2M content format ${contentFormat} is not implemented`);
}
// ----- TLV codec -----
function encodeTlvValue(v) {
    if (v instanceof Uint8Array)
        return v;
    if (v instanceof LwFloat)
        return pack('>d', [v.value]);
    if (typeof v === 'boolean')
        return new Uint8Array([v ? 1 : 0]);
    if (typeof v === 'number' && Number.isInteger(v)) {
        if (v >= -128 && v <= 127)
            return pack('>b', [v]);
        if (v >= -32768 && v <= 32767)
            return pack('>h', [v]);
        if (v >= -2147483648 && v <= 2147483647)
            return pack('>i', [v]);
        return pack('>q', [BigInt(v)]);
    }
    if (typeof v === 'number')
        return pack('>d', [v]);
    if (typeof v === 'string')
        return enc.encode(v);
    throw new TypeError('cannot encode TLV value');
}
function resourceToTlv(res) {
    if (res.isMulti) {
        let body = new Uint8Array(0);
        for (const instId of [...res.instances.keys()].sort((a, b) => a - b)) {
            const inner = new TLV(undefined, { type: TLV_RESOURCE_INSTANCE, id: instId, value: encodeTlvValue(res.instances.get(instId)) });
            body = concat(body, inner.toBytes());
        }
        return new TLV(undefined, { type: TLV_MULTIPLE_RESOURCE, id: res.id, value: body });
    }
    return new TLV(undefined, { type: TLV_RESOURCE, id: res.id, value: encodeTlvValue(res.value) });
}
function encodeTlv(obj) {
    if (obj instanceof Resource)
        return resourceToTlv(obj).toBytes();
    if (obj instanceof ObjectInstance) {
        let body = new Uint8Array(0);
        for (const r of obj.resources)
            body = concat(body, resourceToTlv(r).toBytes());
        return new TLV(undefined, { type: TLV_OBJECT_INSTANCE, id: obj.id, value: body }).toBytes();
    }
    if (Array.isArray(obj))
        return concat(...obj.map(encodeTlv));
    throw new TypeError('cannot encode as TLV');
}
function tlvToModel(tlv) {
    if (tlv.type === TLV_RESOURCE || tlv.type === TLV_RESOURCE_INSTANCE)
        return new Resource(tlv.id, tlv.value);
    if (tlv.type === TLV_MULTIPLE_RESOURCE) {
        const instances = new Map();
        let buf = tlv.value;
        while (buf.length) {
            const inner = new TLV(buf);
            instances.set(inner.id, inner.value);
            buf = inner.data;
        }
        return new Resource(tlv.id, null, instances);
    }
    if (tlv.type === TLV_OBJECT_INSTANCE) {
        const children = [];
        let buf = tlv.value;
        while (buf.length) {
            const child = new TLV(buf);
            children.push(tlvToModel(child));
            buf = child.data;
        }
        return new ObjectInstance(tlv.id, children);
    }
    throw new Error(`unknown TLV type 0x${tlv.type.toString(16)}`);
}
function decodeTlv(buf) {
    const items = [];
    while (buf.length) {
        const tlv = new TLV(buf);
        items.push(tlvToModel(tlv));
        buf = tlv.data;
    }
    return items.length === 1 ? items[0] : items;
}
// ----- JSON codec -----
function entryForValue(name, value) {
    const e = { n: name };
    if (value instanceof LwFloat)
        e.v = value.value;
    else if (typeof value === 'boolean')
        e.bv = value;
    else if (typeof value === 'number')
        e.v = value;
    else if (typeof value === 'string')
        e.sv = value;
    else if (value instanceof Uint8Array)
        e.sv = btoaBytes(value);
    else
        throw new TypeError('cannot encode JSON value');
    return e;
}
function valueFromEntry(entry) {
    for (const k of ['sv', 'v', 'bv', 'ov'])
        if (k in entry && entry[k] !== undefined)
            return entry[k];
    return null;
}
function* jsonEntriesFor(obj, prefix = '') {
    if (obj instanceof Resource) {
        if (obj.isMulti) {
            for (const instId of [...obj.instances.keys()].sort((a, b) => a - b))
                yield entryForValue(`${prefix}${obj.id}/${instId}`, obj.instances.get(instId));
        }
        else
            yield entryForValue(`${prefix}${obj.id}`, obj.value);
    }
    else if (obj instanceof ObjectInstance) {
        for (const r of obj.resources)
            yield* jsonEntriesFor(r, prefix);
    }
    else if (Array.isArray(obj)) {
        for (const item of obj)
            yield* jsonEntriesFor(item, prefix);
    }
    else
        throw new TypeError('cannot encode as JSON');
}
function encodeJson(obj, basePath) {
    return new LwJSON(undefined, { bn: basePath, entries: [...jsonEntriesFor(obj)] }).toBytes();
}
function decodeJson(buf, basePath) {
    const j = new LwJSON(buf);
    const byRes = new Map();
    for (const e of j.entries) {
        const parts = e.n.split('/');
        if (parts.length === 1) {
            const id = parseInt(parts[0], 10);
            byRes.set(id, new Resource(id, valueFromEntry(e)));
        }
        else if (parts.length === 2) {
            const id = parseInt(parts[0], 10);
            const instId = parseInt(parts[1], 10);
            let r = byRes.get(id);
            if (!r || !r.isMulti) {
                r = new Resource(id, null, new Map());
                byRes.set(id, r);
            }
            r.instances.set(instId, valueFromEntry(e));
        }
        else
            throw new Error(`unexpected JSON entry name ${e.n}`);
    }
    const resources = [...byRes.keys()].sort((a, b) => a - b).map((id) => byRes.get(id));
    const bp = j.bn || basePath;
    const bpParts = bp.split('/').filter(Boolean);
    if (bpParts.length >= 2)
        return new ObjectInstance(parseInt(bpParts[1], 10), resources);
    if (resources.length === 1)
        return resources[0];
    return resources;
}
function btoaBytes(b) {
    let s = '';
    for (const x of b)
        s += String.fromCharCode(x);
    return typeof btoa !== 'undefined' ? btoa(s) : globalThis.Buffer.from(b).toString('base64');
}
//# sourceMappingURL=lwm2m.js.map