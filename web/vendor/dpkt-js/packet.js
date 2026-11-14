// Port of dpkt.dpkt.Packet — the declarative binary-struct framework.
//
// Python uses a metaclass to turn `__hdr__` / `__bit_fields__` into slots,
// packed format strings and bit-field properties. JS has no metaclass, so we
// run an equivalent one-time `register()` per subclass that:
//   - builds the combined header format string + defaults
//   - defines getter/setter properties for each bit field on the prototype
//
// Subclasses declare static `hdr` and (optionally) static `bitFields`.
import { calcsize, pack, unpack } from './struct.js';
export class DpktError extends Error {
}
export class UnpackError extends DpktError {
}
export class NeedData extends UnpackError {
}
export class PackError extends DpktError {
}
function register(cls) {
    if (cls._registered)
        return;
    cls._registered = true;
    const bo = cls.byteOrder ?? '>';
    const hdr = cls.hdr;
    if (hdr) {
        cls._hdrFields = hdr.map((f) => f[0]);
        cls._hdrFmt = bo + hdr.map((f) => f[1]).join('');
        cls._hdrLen = calcsize(cls._hdrFmt);
        cls._hdrDefaults = {};
        for (const [name, , def] of hdr)
            cls._hdrDefaults[name] = def;
    }
    const bitFields = cls.bitFields;
    if (bitFields && hdr) {
        const proto = cls.prototype;
        for (const [phName, phFmt] of hdr) {
            const specs = bitFields[phName];
            if (!specs)
                continue;
            const bitsTotal = calcsize(bo + phFmt) * 8;
            const declared = specs.reduce((a, s) => a + s[1], 0);
            if (declared !== bitsTotal) {
                throw new Error(`bit field count for ${phName} (${declared}) != struct size (${bitsTotal})`);
            }
            let used = 0;
            for (const [bfName, bfSize] of specs) {
                const shift = bitsTotal - used - bfSize;
                const maxVal = 2 ** bfSize;
                const lowDiv = 2 ** shift;
                const highDiv = 2 ** (shift + bfSize);
                used += bfSize;
                if (bfName.startsWith('_'))
                    continue; // hidden/reserved
                // Arithmetic (not 32-bit bitwise) so it stays correct for 32-bit
                // placeholders like IPv6's _v_fc_flow where bit 31 may be set.
                Object.defineProperty(proto, bfName, {
                    configurable: true,
                    get() {
                        return Math.floor(this[phName] / lowDiv) % maxVal;
                    },
                    set(v) {
                        if (v >= maxVal)
                            throw new PackError(`value ${v} too large for field ${bfName}`);
                        const cur = this[phName];
                        const low = cur % lowDiv;
                        const high = Math.floor(cur / highDiv) * highDiv;
                        this[phName] = high + v * lowDiv + low;
                    },
                });
            }
        }
    }
}
export class Packet {
    static hdr;
    static byteOrder;
    static bitFields;
    data = new Uint8Array(0);
    constructor(buf, fields) {
        const cls = this.constructor;
        register(cls);
        if (buf) {
            this.unpack(buf);
        }
        else {
            if (cls._hdrFields) {
                for (const k of cls._hdrFields)
                    this[k] = cls._hdrDefaults[k];
            }
            if (fields)
                for (const k of Object.keys(fields))
                    this[k] = fields[k];
        }
    }
    cls() {
        return this.constructor;
    }
    get hdrLen() {
        return this.cls()._hdrLen ?? 0;
    }
    unpack(buf) {
        const c = this.cls();
        if (c._hdrFmt && c._hdrFields) {
            if (buf.length < (c._hdrLen ?? 0)) {
                throw new NeedData(`got ${buf.length}, need ${c._hdrLen}`);
            }
            const vals = unpack(c._hdrFmt, buf, 0);
            c._hdrFields.forEach((k, i) => { this[k] = vals[i]; });
            this.data = buf.subarray(c._hdrLen);
        }
        else {
            this.data = buf;
        }
    }
    packHdr() {
        const c = this.cls();
        if (!c._hdrFmt || !c._hdrFields)
            return new Uint8Array(0);
        return pack(c._hdrFmt, c._hdrFields.map((k) => this[k]));
    }
    toBytes() {
        return concat(this.packHdr(), dataBytes(this.data));
    }
    /** Total serialized byte length. (Not a getter named `length`, which would
     *  collide with subclasses like IPv6 ext headers that store a `.length`.) */
    byteLength() {
        return this.hdrLen + dataBytes(this.data).length;
    }
}
export function dataBytes(d) {
    return d instanceof Packet ? d.toBytes() : d;
}
export function concat(...parts) {
    const total = parts.reduce((a, p) => a + p.length, 0);
    const out = new Uint8Array(total);
    let off = 0;
    for (const p of parts) {
        out.set(p, off);
        off += p.length;
    }
    return out;
}
//# sourceMappingURL=packet.js.map