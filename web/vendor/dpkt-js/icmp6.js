// ICMPv6 + RPL (RFC 4443, RFC 6550) — port of dpkt/icmp6.py (incl. RPL additions).
import { Packet, NeedData, UnpackError, concat, dataBytes } from './packet.js';
import { pack } from './struct.js';
import { IP6 } from './ip6.js';
// Error / informational message types
export const ICMP6_DST_UNREACH = 1;
export const ICMP6_PACKET_TOO_BIG = 2;
export const ICMP6_TIME_EXCEEDED = 3;
export const ICMP6_PARAM_PROB = 4;
export const ICMP6_ECHO_REQUEST = 128;
export const ICMP6_ECHO_REPLY = 129;
// Neighbor Discovery (RFC 4861) — type values (payloads not modeled, as in dpkt)
export const ND_ROUTER_SOLICIT = 133;
export const ND_ROUTER_ADVERT = 134;
export const ND_NEIGHBOR_SOLICIT = 135;
export const ND_NEIGHBOR_ADVERT = 136;
export const ICMP6_RPL_CONTROL = 155;
export const RPL_DIS = 0x00;
export const RPL_DIO = 0x01;
export const RPL_DAO = 0x02;
export const RPL_DAO_ACK = 0x03;
export const RPL_OPT_PAD1 = 0;
export const RPL_OPT_PADN = 1;
export const RPL_OPT_DODAG_CONF = 4;
export const RPL_OPT_TARGET = 5;
export const RPL_OPT_TRANSIT_INFO = 6;
export const RPL_OPT_PREFIX_INFO = 8;
// ----- RPL options -----
export class RPLOption extends Packet {
    static hdr = [['type', 'B', 0], ['len', 'B', 0]];
    unpack(buf) {
        if (!buf.length)
            throw new NeedData();
        this.type = buf[0];
        if (this.type === RPL_OPT_PAD1) {
            this.len = 0;
            this.data = new Uint8Array(0);
            return;
        }
        if (this.constructor !== RPLOption) {
            super.unpack(buf);
            return;
        }
        if (buf.length < 2)
            throw new NeedData();
        this.len = buf[1];
        if (buf.length < 2 + this.len)
            throw new NeedData();
        this.data = buf.subarray(2, 2 + this.len);
    }
    toBytes() {
        if (this.type === RPL_OPT_PAD1)
            return new Uint8Array([0]);
        if (this.constructor !== RPLOption)
            return concat(this.packHdr(), dataBytes(this.data));
        return concat(pack('BB', [this.type, dataBytes(this.data).length]), dataBytes(this.data));
    }
}
export class RPLOptPadN extends RPLOption {
    static hdr = [['type', 'B', RPL_OPT_PADN], ['len', 'B', 0]];
}
export class RPLOptDODAGConf extends RPLOption {
    static hdr = [
        ['type', 'B', RPL_OPT_DODAG_CONF], ['len', 'B', 14], ['flags', 'B', 0],
        ['dio_int_min', 'B', 0], ['dio_int_doub', 'B', 0], ['dio_redun', 'B', 0],
        ['max_rank_inc', 'H', 0], ['min_hop_rank_inc', 'H', 0], ['ocp', 'H', 0],
        ['rsvd', 'B', 0], ['def_lifetime', 'B', 0], ['lifetime_unit', 'H', 0],
    ];
}
export class RPLOptPrefixInfo extends RPLOption {
    static hdr = [
        ['type', 'B', RPL_OPT_PREFIX_INFO], ['len', 'B', 30], ['prefix_len', 'B', 0],
        ['_l_a_r_flags', 'B', 0], ['valid_lifetime', 'I', 0], ['pref_lifetime', 'I', 0],
        ['rsvd', 'I', 0], ['prefix', '16s', new Uint8Array(16)],
    ];
    static bitFields = { _l_a_r_flags: [['l', 1], ['a', 1], ['r', 1], ['flags', 5]] };
}
export class RPLOptTarget extends RPLOption {
    static hdr = [
        ['type', 'B', RPL_OPT_TARGET], ['len', 'B', 0], ['flags', 'B', 0], ['prefix_len', 'B', 0],
    ];
    unpack(buf) { Packet.prototype.unpack.call(this, buf); this.prefix = this.data; this.data = new Uint8Array(0); }
    toBytes() { return concat(this.packHdr(), this.prefix ?? new Uint8Array(0)); }
}
export class RPLOptTransitInfo extends RPLOption {
    static hdr = [
        ['type', 'B', RPL_OPT_TRANSIT_INFO], ['len', 'B', 0], ['flags', 'B', 0],
        ['path_ctl', 'B', 0], ['path_seq', 'B', 0], ['path_lifetime', 'B', 0],
    ];
    unpack(buf) { Packet.prototype.unpack.call(this, buf); this.parent = this.data; this.data = new Uint8Array(0); }
    toBytes() { return concat(this.packHdr(), this.parent ?? new Uint8Array(0)); }
}
const RPL_OPT_SW = {
    [RPL_OPT_PADN]: RPLOptPadN,
    [RPL_OPT_DODAG_CONF]: RPLOptDODAGConf,
    [RPL_OPT_PREFIX_INFO]: RPLOptPrefixInfo,
    [RPL_OPT_TARGET]: RPLOptTarget,
    [RPL_OPT_TRANSIT_INFO]: RPLOptTransitInfo,
};
export function decodeRplOpts(buf) {
    const opts = [];
    let off = 0;
    while (off < buf.length) {
        const t = buf[off];
        if (t === RPL_OPT_PAD1) {
            opts.push(new RPLOption(buf.subarray(off, off + 1)));
            off += 1;
            continue;
        }
        if (buf.length - off < 2)
            break;
        const l = buf[off + 1];
        if (buf.length - off < 2 + l)
            break;
        const Cls = RPL_OPT_SW[t] ?? RPLOption;
        opts.push(new Cls(buf.subarray(off, off + 2 + l)));
        off += 2 + l;
    }
    return opts;
}
export function encodeRplOpts(opts) {
    return concat(...opts.map((o) => o.toBytes()));
}
// ----- RPL control messages -----
class RPLMessageWithOpts extends Packet {
    constructor(buf, fields) {
        super(buf, fields);
        if (this.opts === undefined)
            this.opts = [];
    }
}
export class RPLDIS extends RPLMessageWithOpts {
    static hdr = [['flags', 'B', 0], ['rsvd', 'B', 0]];
    unpack(buf) { super.unpack(buf); this.opts = decodeRplOpts(this.data); this.data = new Uint8Array(0); }
    toBytes() { return concat(this.packHdr(), encodeRplOpts(this.opts)); }
}
export class RPLDIO extends RPLMessageWithOpts {
    static hdr = [
        ['instance_id', 'B', 0], ['version', 'B', 0], ['rank', 'H', 0],
        ['_g_mop_prf', 'B', 0], ['dtsn', 'B', 0], ['flags', 'B', 0], ['rsvd', 'B', 0],
        ['dodagid', '16s', new Uint8Array(16)],
    ];
    static bitFields = { _g_mop_prf: [['g', 1], ['_rsvd', 1], ['mop', 3], ['prf', 3]] };
    unpack(buf) { super.unpack(buf); this.opts = decodeRplOpts(this.data); this.data = new Uint8Array(0); }
    toBytes() { return concat(this.packHdr(), encodeRplOpts(this.opts)); }
}
export class RPLDAO extends RPLMessageWithOpts {
    static hdr = [['instance_id', 'B', 0], ['_k_d_flags', 'B', 0], ['rsvd', 'B', 0], ['dao_sequence', 'B', 0]];
    static bitFields = { _k_d_flags: [['k', 1], ['d', 1], ['flags', 6]] };
    unpack(buf) {
        super.unpack(buf);
        let rest = this.data;
        if (this.d) {
            this.dodagid = rest.subarray(0, 16);
            rest = rest.subarray(16);
        }
        else
            this.dodagid = null;
        this.opts = decodeRplOpts(rest);
        this.data = new Uint8Array(0);
    }
    toBytes() {
        const parts = [this.packHdr()];
        if (this.d && this.dodagid)
            parts.push(this.dodagid);
        parts.push(encodeRplOpts(this.opts));
        return concat(...parts);
    }
}
export class RPLDAOACK extends RPLMessageWithOpts {
    static hdr = [['instance_id', 'B', 0], ['_d_flags', 'B', 0], ['dao_sequence', 'B', 0], ['status', 'B', 0]];
    static bitFields = { _d_flags: [['d', 1], ['flags', 7]] };
    unpack(buf) {
        super.unpack(buf);
        let rest = this.data;
        if (this.d) {
            this.dodagid = rest.subarray(0, 16);
            rest = rest.subarray(16);
        }
        else
            this.dodagid = null;
        this.opts = decodeRplOpts(rest);
        this.data = new Uint8Array(0);
    }
    toBytes() {
        const parts = [this.packHdr()];
        if (this.d && this.dodagid)
            parts.push(this.dodagid);
        parts.push(encodeRplOpts(this.opts));
        return concat(...parts);
    }
}
const RPL_SW = {
    [RPL_DIS]: RPLDIS, [RPL_DIO]: RPLDIO, [RPL_DAO]: RPLDAO, [RPL_DAO_ACK]: RPLDAOACK,
};
// ----- base ICMPv6 messages (Echo + error messages that embed an IP6 packet) -----
export class Echo extends Packet {
    static hdr = [['id', 'H', 0], ['seq', 'H', 0]];
}
export class ICMP6Error extends Packet {
    static hdr = [['pad', 'I', 0]];
    unpack(buf) {
        super.unpack(buf);
        this.ip6 = new IP6(this.data);
        this.data = this.ip6;
    }
}
export class Unreach extends ICMP6Error {
}
export class TooBig extends ICMP6Error {
    static hdr = [['mtu', 'I', 1232]];
}
export class TimeExceed extends ICMP6Error {
}
export class ParamProb extends ICMP6Error {
    static hdr = [['ptr', 'I', 0]];
}
const TYPE_SW = {
    [ICMP6_DST_UNREACH]: Unreach, [ICMP6_PACKET_TOO_BIG]: TooBig,
    [ICMP6_TIME_EXCEEDED]: TimeExceed, [ICMP6_PARAM_PROB]: ParamProb,
    [ICMP6_ECHO_REQUEST]: Echo, [ICMP6_ECHO_REPLY]: Echo,
};
export class ICMP6 extends Packet {
    static hdr = [['type', 'B', 0], ['code', 'B', 0], ['sum', 'H', 0]];
    unpack(buf) {
        super.unpack(buf);
        try {
            if (this.type === ICMP6_RPL_CONTROL) {
                const Cls = RPL_SW[this.code];
                if (!Cls)
                    return;
                this.data = new Cls(this.data);
                this[this.data.constructor.name.toLowerCase()] = this.data;
            }
            else if (TYPE_SW[this.type]) {
                this.data = new TYPE_SW[this.type](this.data);
                this[this.data.constructor.name.toLowerCase()] = this.data;
            }
        }
        catch (e) {
            if (!(e instanceof UnpackError))
                throw e;
        }
    }
}
//# sourceMappingURL=icmp6.js.map