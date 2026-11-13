// Dissection engine: chain link → 6LoWPAN → IP6 → UDP/ICMP6 → CoAP → LWM2M
// into a tree the UI can render. This is the equivalent of dpkt's `data`
// layer-chaining plus Wireshark-style protocol detail.
import { toHex } from './util.js';
import { IEEE802154, TYPE_DATA, TYPE_BEACON, TYPE_ACK, TYPE_CMD, IE_CSL, IE_ACK_NACK_TIME_CORRECTION, PIE_MLME, PIE_IETF, PIE_VENDOR, parsePayloadIEs, } from './ieee802154.js';
import { parseLowpan, DEFAULT_CONTEXT0 } from './lowpan.js';
import { IP6, IP6OptsHeader, IP6HopOptsHeader, IP6RoutingHeader, IP6FragmentHeader, ip6str, IP_PROTO_UDP as IPP_UDP, IP_PROTO_ICMP6 as IPP_ICMP6, IP_PROTO_TCP as IPP_TCP, } from './ip6.js';
import { UDP } from './udp.js';
import { TCP, tcpFlagsToStr } from './tcp.js';
import { IP, ip4str, IP_PROTO_ICMP as IPP_ICMP } from './ip.js';
import { ICMP, ICMPEcho } from './icmp.js';
import { CoAP } from './coap.js';
import { ICMP6_RPL_CONTROL } from './icmp6.js';
import { parseDnsLenient, DNS_A, DNS_AAAA, DNS_CNAME, DNS_PTR, DNS_TXT, DNS_SRV, DNS_MX, DNS_NS, DNS_SOA } from './dns.js';
import { Request as HttpRequest, Response as HttpResponse } from './http.js';
import { isNat64Address, extractIp4 } from './nat64.js';
import * as lwm2m from './lwm2m.js';
import { concat } from './packet.js';
import { cborDecodeSequence, cborDiag } from './cbor.js';
import { ip6PseudoChecksum } from './cksum.js';
import { dissectDtls, looksLikeDtls } from './dtls.js';
import { dissectMle, tmfPayloadNodes, MLE_PORT, TMF_PORT, MESHCOP_PORT } from './thread.js';
const COAP_TYPES = ['CON', 'NON', 'ACK', 'RST'];
const CF_NAMES = {
    0: 'text/plain', 40: 'application/link-format', 41: 'application/xml', 42: 'application/octet-stream', 47: 'application/exi',
    50: 'application/json', 60: 'application/cbor', 11542: 'LWM2M+TLV', 11543: 'LWM2M+JSON',
};
const errText = (e) => (e instanceof Error ? e.message : String(e));
const hex2 = (v) => v.toString(16).padStart(2, '0');
const hex4 = (v) => v.toString(16).padStart(4, '0');
/** A MAC address as `IEEE802154` keeps it (short addresses in frame order,
 *  little-endian), in network order. */
export function linkAddr(a) {
    if (!a)
        return undefined;
    return a.length === 2 ? Uint8Array.of(a[1], a[0]) : a;
}
/** A link-layer address in network order: an EUI-64 as 00:12:4b:..., a short address as 0xabcd. */
export function linkStr(a) {
    if (!a)
        return '(none)';
    return a.length === 8 ? [...a].map(hex2).join(':') : `0x${toHex(a)}`;
}
function prefixStr(p) {
    const g = [];
    for (let i = 0; i < 8; i += 2)
        g.push((p[i] << 8) | p[i + 1]);
    while (g.length && !g[g.length - 1])
        g.pop();
    return `${g.map((x) => x.toString(16)).join(':')}::/64`;
}
const SEC_LEVELS = ['None', 'MIC-32', 'MIC-64', 'MIC-128', 'ENC', 'ENC-MIC-32', 'ENC-MIC-64', 'ENC-MIC-128'];
const FRAME_TYPES = ['Beacon', 'Data', 'Ack', 'Cmd', 'Reserved', 'Multipurpose', 'Fragment', 'Extended'];
const VERSIONS = ['2003', '2006', '2015', 'reserved'];
const CMD_NAMES = {
    1: 'Association Request', 2: 'Association Response', 3: 'Disassociation Notification', 4: 'Data Request',
    5: 'PAN ID Conflict Notification', 6: 'Orphan Notification', 7: 'Beacon Request', 8: 'Coordinator Realignment', 9: 'GTS Request',
};
const HIE_NAMES = {
    0x00: 'Vendor Specific', 0x1a: 'CSL', 0x1b: 'RIT', 0x1c: 'DSME PAN Descriptor', 0x1d: 'Rendezvous Time',
    0x1e: 'ACK/NACK Time Correction', 0x1f: 'Group ACK', 0x20: 'LECIM Capabilities', 0x21: 'Simplified Superframe Specification',
    0x7e: 'Header Termination 1', 0x7f: 'Header Termination 2',
};
const PIE_NAMES = { 0x0: 'ESDU', 0x1: 'MLME', 0x2: 'Vendor Specific', 0x5: 'IETF', 0xf: 'Payload Termination' };
const MLME_SHORT = {
    0x1a: 'TSCH Synchronization', 0x1b: 'TSCH Slotframe and Link', 0x1c: 'TSCH Timeslot', 0x1d: 'Hopping Timing',
    0x1e: 'Enhanced Beacon Filter', 0x1f: 'MAC Metrics', 0x20: 'All MAC Metrics', 0x21: 'Coexistence Specification',
};
const MLME_LONG = { 0x8: 'Channel Hopping', 0x9: 'Channel Hopping' };
const le16 = (b, o) => b[o] | (b[o + 1] << 8);
function auxNode(aux) {
    const fields = [
        ['Security level', `${aux.level} (${SEC_LEVELS[aux.level]})`],
        ['Key ID mode', String(aux.keyIdMode)],
        ['Frame counter', aux.frameCounter == null ? 'suppressed' : String(aux.frameCounter)],
    ];
    if (aux.keySource)
        fields.push(['Key source', toHex(aux.keySource)]);
    if (aux.keyIndex != null)
        fields.push(['Key index', String(aux.keyIndex)]);
    return { label: 'Auxiliary security header', summary: auxSummary(aux), fields };
}
function auxSummary(aux) {
    return `${SEC_LEVELS[aux.level]}${aux.keyIndex != null ? `, key ${aux.keyIndex}` : ''}`;
}
/** One header IE, decoded where its content is defined. */
function headerIENode(ie) {
    const name = HIE_NAMES[ie.id] ?? `Header IE 0x${ie.id.toString(16)}`;
    const c = ie.content;
    const node = { label: name, fields: [] };
    if (ie.id === IE_CSL && c.length >= 4) {
        node.fields.push(['Phase', `${le16(c, 0)} (×10 symbols)`], ['Period', `${le16(c, 2)} (×10 symbols)`]);
        if (c.length >= 6)
            node.fields.push(['Rendezvous time', String(le16(c, 4))]);
        node.summary = `period ${le16(c, 2)}`;
    }
    else if (ie.id === IE_ACK_NACK_TIME_CORRECTION && c.length >= 2) {
        const v = le16(c, 0);
        const corr = (v & 0x800) ? (v & 0xfff) - 0x1000 : (v & 0xfff);
        node.fields.push(['Time correction', `${corr} µs`], ['NACK', String((v >> 15) & 1)]);
        node.summary = `${corr} µs${(v >> 15) & 1 ? ' NACK' : ''}`;
    }
    else if (ie.id === 0x1d && c.length >= 2) {
        node.fields.push(['Rendezvous time', String(le16(c, 0))]);
        if (c.length >= 4)
            node.fields.push(['Wake-up interval', String(le16(c, 2))]);
    }
    else if (ie.id === 0x00 && c.length >= 3) {
        node.fields.push(['OUI', toHex(c.subarray(0, 3))]);
        if (c.length > 3)
            node.raw = toHex(c.subarray(3));
    }
    else if (c.length)
        node.raw = toHex(c);
    if (!node.fields.length)
        delete node.fields;
    return node;
}
/** One payload IE; MLME IEs are opened into their nested IEs. */
function payloadIENode(pie) {
    const node = { label: `${PIE_NAMES[pie.group] ?? `Payload IE group 0x${pie.group.toString(16)}`} IE`, children: [] };
    const c = pie.content;
    if (pie.group === PIE_MLME) {
        let off = 0;
        while (off + 2 <= c.length) {
            const v = le16(c, off);
            const long = (v >> 15) & 1;
            const len = long ? v & 0x7ff : v & 0xff;
            const sub = long ? (v >> 11) & 0xf : (v >> 8) & 0x7f;
            const body = c.subarray(off + 2, off + 2 + len);
            node.children.push(mlmeNode(long, sub, body));
            off += 2 + len;
        }
    }
    else if (pie.group === PIE_IETF && c.length) {
        // IETF IE (RFC 8137): the first byte is the sub-ID; 0xc9 is 6P (RFC 8480).
        const sub = c[0];
        node.children.push(sub === 0xc9 ? sixPNode(c.subarray(1)) : { label: `IETF sub-IE 0x${sub.toString(16)}`, raw: toHex(c.subarray(1)) });
    }
    else if (pie.group === PIE_VENDOR && c.length >= 3) {
        node.fields = [['OUI', toHex(c.subarray(0, 3))]];
        if (c.length > 3)
            node.raw = toHex(c.subarray(3));
    }
    else if (c.length)
        node.raw = toHex(c);
    if (!node.children.length)
        delete node.children;
    return node;
}
function mlmeNode(long, sub, b) {
    const name = (long ? MLME_LONG[sub] : MLME_SHORT[sub]) ?? `MLME ${long ? 'long' : 'short'} sub-IE 0x${sub.toString(16)}`;
    const node = { label: name, fields: [] };
    if (!long && sub === 0x1a && b.length >= 6) {
        const asn = b[0] + b[1] * 2 ** 8 + b[2] * 2 ** 16 + b[3] * 2 ** 24 + b[4] * 2 ** 32;
        node.fields.push(['ASN', String(asn)], ['Join metric', String(b[5])]);
        node.summary = `ASN ${asn}, join metric ${b[5]}`;
    }
    else if (!long && sub === 0x1c && b.length >= 1) {
        node.fields.push(['Timeslot template', String(b[0])]);
    }
    else if (!long && sub === 0x1b && b.length >= 1) {
        let off = 1;
        node.fields.push(['Slotframes', String(b[0])]);
        node.children = [];
        for (let i = 0; i < b[0] && off + 4 <= b.length; i++) {
            const handle = b[off], size = le16(b, off + 1), links = b[off + 3];
            off += 4;
            const sf = { label: `Slotframe ${handle}`, summary: `${size} slots, ${links} links`, fields: [] };
            for (let j = 0; j < links && off + 5 <= b.length; j++, off += 5)
                sf.fields.push([`Link ${j}`, `timeslot ${le16(b, off)}, channel offset ${le16(b, off + 2)}, options 0x${b[off + 4].toString(16)}`]);
            node.children.push(sf);
        }
    }
    else if (long && (sub === 0x9 || sub === 0x8) && b.length >= 1) {
        node.fields.push(['Hopping sequence ID', String(b[0])]);
    }
    else if (b.length)
        node.raw = toHex(b);
    if (!node.fields.length)
        delete node.fields;
    return node;
}
const SIXP_TYPES = ['Request', 'Response', 'Confirmation'];
function sixPNode(b) {
    if (b.length < 4)
        return { label: '6P', error: 'short', raw: toHex(b) };
    const type = (b[0] >> 4) & 3;
    return {
        label: '6P (6top Protocol)', summary: `${SIXP_TYPES[type] ?? type} code ${b[1]} SFID ${b[2]} seq ${b[3]}`,
        fields: [['Version', String(b[0] & 0xf)], ['Type', SIXP_TYPES[type] ?? String(type)], ['Code', String(b[1])], ['SFID', String(b[2])], ['Sequence', String(b[3])]],
        raw: b.length > 4 ? toHex(b.subarray(4)) : undefined,
    };
}
/** An 802.15.4-2006 beacon's payload: superframe, GTS, pending addresses, and
 *  the beacon payload (a Thread beacon decoded). */
function beaconNode(b) {
    if (b.length < 4)
        return { label: 'Beacon', error: 'short', raw: toHex(b) };
    const sf = le16(b, 0);
    const fields = [
        ['Beacon order', String(sf & 0xf)], ['Superframe order', String((sf >> 4) & 0xf)],
        ['PAN coordinator', String((sf >> 14) & 1)], ['Association permit', String((sf >> 15) & 1)],
    ];
    let off = 2;
    const gts = b[off++];
    const gtsCount = gts & 7;
    if (gtsCount)
        off += 1 + 3 * gtsCount;
    const pend = b[off++];
    const nShort = pend & 7, nExt = (pend >> 4) & 7;
    fields.push(['GTS descriptors', String(gtsCount)], ['Pending short/ext addresses', `${nShort}/${nExt}`]);
    off += 2 * nShort + 8 * nExt;
    const node = { label: 'Beacon', fields, children: [] };
    const payload = b.subarray(off);
    if (payload.length >= 26 && payload[0] === 3) {
        const name = new TextDecoder().decode(payload.subarray(2, 18)).replace(/\0+$/, '');
        node.children.push({
            label: 'Thread beacon', summary: `"${name}"`,
            fields: [['Protocol ID', '3 (Thread)'], ['Version', String(payload[1] >> 4)],
                ['Native commissioner', String((payload[1] >> 3) & 1)], ['Joining permitted', String(payload[1] & 1)],
                ['Network name', name], ['Extended PAN ID', toHex(payload.subarray(18, 26))]],
            raw: payload.length > 26 ? toHex(payload.subarray(26)) : undefined,
        });
    }
    else if (payload.length)
        node.children.push({ label: 'Beacon payload', raw: toHex(payload) });
    if (!node.children.length)
        delete node.children;
    return node;
}
function commandNode(b) {
    if (!b.length)
        return { label: 'Command', error: 'no command ID' };
    const id = b[0];
    const name = CMD_NAMES[id] ?? `0x${id.toString(16)}`;
    const node = { label: `Command: ${name}`, fields: [['Command ID', `0x${id.toString(16).padStart(2, '0')}`]] };
    if (id === 1 && b.length >= 2)
        node.fields.push(['Capability information', `0x${b[1].toString(16)}`]);
    if (id === 2 && b.length >= 4)
        node.fields.push(['Short address', `0x${le16(b, 1).toString(16)}`], ['Status', String(b[3])]);
    if (id === 3 && b.length >= 2)
        node.fields.push(['Reason', String(b[1])]);
    return node;
}
/** An enhanced beacon's TSCH Synchronization IE gives the ASN of its slot;
 *  its Timeslot IE, the slot's timing (802.15.4-2015 7.4.4.2, 7.4.4.4). */
function noteTschClock(ies, keys, ts) {
    let asn = -1, txOffset = 2120, slot = 10000;
    for (const ie of ies) {
        if (ie.group !== PIE_MLME)
            continue;
        const c = ie.content;
        for (let off = 0; off + 2 <= c.length;) {
            const v = le16(c, off);
            const long = (v >> 15) & 1;
            const len = long ? v & 0x7ff : v & 0xff;
            const sub = long ? -1 : (v >> 8) & 0x7f;
            const b = c.subarray(off + 2, off + 2 + len);
            if (sub === 0x1a && b.length >= 5)
                asn = b[0] + b[1] * 2 ** 8 + b[2] * 2 ** 16 + b[3] * 2 ** 24 + b[4] * 2 ** 32;
            if (sub === 0x1c && b.length >= 25) {
                txOffset = le16(b, 5);
                slot = le16(b, b.length - 2);
            }
            off += 2 + len;
        }
    }
    if (asn >= 0)
        keys.noteTschBeacon(asn, ts, txOffset, slot);
}
/** Dissect a raw IEEE 802.15.4 frame, without its FCS (the shape csim and
 *  slip-radio emit).  `opts` carries what reaches across frames: 6LoWPAN
 *  contexts and reassembly, and keys. */
export function dissect802154(buf, opts = {}) {
    try {
        const mac = new IEEE802154(buf);
        const v2 = mac.version === 2;
        const typeName = v2 && mac.type === TYPE_BEACON ? 'Enhanced Beacon' : v2 && mac.type === TYPE_ACK ? 'Enh-Ack' : (FRAME_TYPES[mac.type] ?? String(mac.type));
        const seq = mac.seq == null ? 'suppressed' : String(mac.seq);
        const src = linkAddr(mac.src_addr), dst = linkAddr(mac.dst_addr);
        const node = {
            label: 'IEEE 802.15.4',
            summary: `${typeName}${mac.seq == null ? '' : ` seq=${mac.seq}`}${src || dst ? ` ${linkStr(src)} → ${linkStr(dst)}` : ''}`,
            fields: [
                ['Frame type', typeName],
                ['Frame version', `${mac.version} (${VERSIONS[mac.version]})`],
                ['Sequence', seq],
                ['Dst PAN', mac.dst_pan != null ? `0x${hex4(mac.dst_pan)}` : '(none)'],
                ['Dst addr', linkStr(dst)],
                ['Src addr', linkStr(src)],
                ['Ack req', String(mac.ar)],
                ['Frame pending', String(mac.frame_pending)],
            ],
            children: [],
        };
        if (mac.src_pan != null && mac.src_pan !== mac.dst_pan)
            node.fields.splice(5, 0, ['Src PAN', `0x${hex4(mac.src_pan)}`]);
        if (src && src.length === 8)
            opts.keys?.learn(src);
        let payload = mac.data;
        let payloadIEs = mac.payload_ies;
        const aux = mac.aux;
        if (aux) {
            // What protects the payload, never the key.  An encrypting level
            // leaves only ciphertext (payload IEs included).
            node.fields.push(['Security', auxSummary(aux)]);
            node.children.push(auxNode(aux));
        }
        if (mac.header_ies.length)
            node.children.push({ label: 'Header IEs', summary: mac.header_ies.map((ie) => HIE_NAMES[ie.id] ?? `0x${ie.id.toString(16)}`).join(', '),
                children: mac.header_ies.map(headerIENode) });
        if (aux) {
            const mic = aux.micLength ? payload.subarray(payload.length - aux.micLength) : new Uint8Array(0);
            payload = payload.subarray(0, payload.length - aux.micLength);
            if (aux.level >= 4) {
                // 2006 command frames keep their command ID in the clear (authenticated only).
                const clearCmd = mac.type === TYPE_CMD && !v2 && payload.length ? 1 : 0;
                const headerLen = buf.length - mac.data.length;
                const dec = opts.keys
                    ? opts.keys.decryptMac(aux, buf.subarray(0, headerLen + clearCmd), payload.subarray(clearCmd), mic, src, mac.src_pan, opts.ts)
                    : null;
                if (!dec) {
                    node.children.push({ label: 'Encrypted payload', summary: `${payload.length} bytes`, raw: toHex(payload),
                        fields: aux.micLength ? [['MIC', toHex(mic)]] : [] });
                    opts.keys?.noteDestination(dst);
                    return node;
                }
                node.fields.push(['Decrypted', `${dec.key}, MIC ${toHex(mic)} verified${src && src.length === 2 ? `, source ${linkStr(dec.source)}` : ''}`]);
                payload = clearCmd ? concat(payload.subarray(0, 1), dec.plaintext) : dec.plaintext;
                if (mac.payloadIEsFollow) {
                    const r = parsePayloadIEs(payload, 0);
                    payloadIEs = r.ies;
                    payload = payload.subarray(r.off);
                }
            }
            else if (aux.micLength)
                node.fields.push(['MIC', toHex(mic)]);
        }
        opts.keys?.noteDestination(dst);
        if (opts.keys && opts.ts !== undefined)
            noteTschClock(payloadIEs, opts.keys, opts.ts);
        if (payloadIEs.length)
            node.children.push({ label: 'Payload IEs', summary: payloadIEs.map((ie) => PIE_NAMES[ie.group] ?? `0x${ie.group.toString(16)}`).join(', '),
                children: payloadIEs.map(payloadIENode) });
        if (mac.type === TYPE_DATA && payload.length) {
            // 00xxxxxx is "not a LoWPAN frame" (RFC 4944 5.1): raw payload, as
            // Contiki-NG's nullnet sends it.
            if (payload[0] < 0x40)
                node.children.push({ label: 'Payload', summary: `${payload.length} bytes, not 6LoWPAN`, raw: toHex(payload) });
            else
                node.children.push(dissect6lowpan(payload, src, dst, opts));
        }
        else if (mac.type === TYPE_DATA && !payload.length && !payloadIEs.length) {
            node.summary += ' (no payload)';
        }
        else if (mac.type === TYPE_BEACON && !v2) {
            node.children.push(beaconNode(payload));
        }
        else if (mac.type === TYPE_CMD) {
            node.children.push(commandNode(payload));
        }
        else if (payload.length) {
            node.children.push({ label: 'Payload', summary: `${payload.length} bytes`, raw: toHex(payload) });
        }
        if (!node.children.length)
            delete node.children;
        return node;
    }
    catch (e) {
        return { label: 'IEEE 802.15.4', error: errText(e), raw: toHex(buf) };
    }
}
export function dissect6lowpan(buf, srcMac, dstMac, opts = {}) {
    const contexts = opts.contexts ?? { 0: DEFAULT_CONTEXT0 };
    let f;
    try {
        f = parseLowpan(buf, srcMac, dstMac, contexts);
    }
    catch (e) {
        return { label: '6LoWPAN', error: errText(e), raw: toHex(buf) };
    }
    const node = { label: '6LoWPAN', children: [] };
    const parts = [];
    if (f.mesh) {
        const m = f.mesh;
        parts.push('mesh');
        node.children.push({
            label: 'Mesh header', summary: `${linkStr(m.originator)} → ${linkStr(m.final)}, ${m.hopsLeft} hops left`,
            fields: [['Originator', linkStr(m.originator)], ['Final destination', linkStr(m.final)], ['Hops left', String(m.hopsLeft)]],
        });
    }
    if (f.bc0 !== undefined) {
        parts.push('broadcast');
        node.children.push({ label: 'Broadcast header', summary: `sequence ${f.bc0}` });
    }
    if (f.frag) {
        const fr = f.frag;
        parts.push(fr.first ? 'first fragment' : `fragment @${fr.offset}`);
        const fields = [['Datagram size', String(fr.size)], ['Datagram tag', `0x${hex4(fr.tag)}`]];
        if (!fr.first)
            fields.push(['Datagram offset', `${fr.offset} (${fr.offset / 8} × 8)`]);
        node.children.push({
            label: fr.first ? 'Fragment header (first)' : 'Fragment header',
            summary: `datagram ${fr.size} bytes, tag 0x${hex4(fr.tag)}${fr.first ? '' : `, offset ${fr.offset}`}`, fields,
        });
    }
    if (f.unknownDispatch !== undefined) {
        // Not 6LoWPAN after all: an application writing straight to the MAC.
        const d = dataNode(buf);
        if (d.summary?.startsWith('"'))
            return { label: 'Payload', summary: `${d.summary}, not 6LoWPAN`, raw: d.raw };
        node.summary = `unknown dispatch 0x${hex2(f.unknownDispatch)}`;
        node.children.push({ label: 'Payload', summary: `${buf.length} bytes, not a 6LoWPAN dispatch`, raw: toHex(buf) });
        return node;
    }
    let hint;
    if (f.iphc) {
        const fields = [...f.iphc.fields];
        if (f.iphc.nhc.length)
            fields.push(['NHC', f.iphc.nhc.join(', ')]);
        for (const id of f.iphc.contextsUsed)
            fields.push([`Context ${id}`, f.iphc.contextsMissing.includes(id) ? 'unknown (its prefix is shown as zeros)' : prefixStr(contexts[id])]);
        node.children.push({ label: 'IPHC header', summary: f.iphc.nhc.length ? `NHC ${f.iphc.nhc.join(', ')}` : undefined, fields });
        parts.push(f.iphc.nhc.length ? `IPHC + ${f.iphc.nhc.join(', ')}` : 'IPHC');
        if (f.iphc.contextsUsed.length)
            hint = `are the 6LoWPAN contexts right? (${f.iphc.contextsUsed.map((id) => `${id}: ${contexts[id] ? prefixStr(contexts[id]) : 'unknown'}`).join(', ')})`;
    }
    else if (f.dispatch === 0x41)
        parts.push('uncompressed IPv6');
    const reasm = f.frag && opts.reassembler ? opts.reassembler.add(f, opts.frame ?? 0) : null;
    if (f.ip6)
        node.children.push(dissectIp6Bytes(f.ip6, { partial: !!f.frag, hint }, opts));
    else if (f.fragData)
        node.children.push({ label: 'Fragment data', summary: `${f.fragData.length} bytes at offset ${f.frag.offset} of ${f.frag.size}`, raw: toHex(f.fragData) });
    if (reasm && 'retransmissionOf' in reasm) {
        node.fields = [['Retransmission', `of a fragment of the datagram reassembled in frame ${reasm.retransmissionOf}`]];
        parts.push('retransmitted');
    }
    else if (reasm) {
        node.children.push({
            label: 'Reassembled IPv6 datagram', summary: `${reasm.datagram.length} bytes from frames ${reasm.frames.join(', ')}`,
            children: [dissectIp6Bytes(reasm.datagram, { hint: reassemblyHint(opts, reasm.datagram) }, opts)],
        });
    }
    node.summary = parts.join(', ');
    return node;
}
/** The first fragment's context hint can't be carried to the last; name the
 *  default when the reassembled addresses are not link-local. */
function reassemblyHint(opts, d) {
    const ll = (o) => d[o] === 0xfe && (d[o + 1] & 0xc0) === 0x80;
    if (d.length < 40 || (ll(8) && (ll(24) || d[24] === 0xff)))
        return undefined;
    const c = opts.contexts ?? { 0: DEFAULT_CONTEXT0 };
    return `are the 6LoWPAN contexts right? (${Object.keys(c).map((k) => `${k}: ${prefixStr(c[+k])}`).join(', ')})`;
}
const IP_PROTO_NAMES = {
    0: 'Hop-by-Hop Options', 6: 'TCP', 17: 'UDP', 41: 'IPv6', 43: 'Routing', 44: 'Fragment', 50: 'ESP', 51: 'AH',
    58: 'ICMPv6', 59: 'No Next Header', 60: 'Destination Options',
};
/** Dissect IPv6 bytes, as a frame or a reassembly produced them. */
export function dissectIp6Bytes(b, o = {}, opts = {}) {
    let p;
    try {
        p = new IP6(b);
    }
    catch (e) {
        return { label: 'IPv6', error: errText(e), raw: toHex(b) };
    }
    return dissectIp6(p, o, opts);
}
export function dissectIp6(p, o = {}, opts = {}) {
    const proto = p.p ?? p.nxt; // transport proto after the ext-header chain
    const node = {
        label: 'IPv6',
        summary: `${ip6str(p.src)} → ${ip6str(p.dst)}`,
        fields: [['Source', ip6str(p.src)], ['Destination', ip6str(p.dst)],
            ['Next header', `${p.nxt}${IP_PROTO_NAMES[p.nxt] ? ` (${IP_PROTO_NAMES[p.nxt]})` : ''}`],
            ['Hop limit', String(p.hlim)], ['Payload length', String(p.plen)]],
        children: [],
    };
    if (p.fc)
        node.fields.push(['Traffic class', `0x${hex2(p.fc)}`]);
    if (p.flow)
        node.fields.push(['Flow label', `0x${p.flow.toString(16)}`]);
    let finalDst = p.dst;
    let extLen = 0;
    for (const ext of p.all_extension_headers ?? []) {
        extLen += ext.length;
        const [n, dst] = extNode(ext, p);
        if (dst)
            finalDst = dst;
        node.children.push(n);
    }
    // Flag RFC 6052 NAT64-synthesized addresses (Contiki ip64 traffic).
    if (isNat64Address(p.dst))
        node.fields.push(['NAT64', `dst embeds ${ip4str(extractIp4(p.dst))}`]);
    if (isNat64Address(p.src))
        node.fields.push(['NAT64', `src embeds ${ip4str(extractIp4(p.src))}`]);
    const payload = p.data;
    if (!(payload instanceof Uint8Array) || !payload.length)
        return node;
    const expected = p.plen ? p.plen - extLen : payload.length;
    const partial = !!o.partial && payload.length < expected;
    if (partial)
        node.fields.push(['In this fragment', `${payload.length} of ${expected} payload bytes`]);
    let t;
    if (proto === IPP_UDP)
        t = dissectUdp(payload, { headerOnly: partial, src: p.src, dst: finalDst }, opts);
    else if (proto === IPP_TCP)
        t = partial ? { label: 'TCP', summary: 'header in this fragment' } : dissectTcp(payload);
    else if (proto === IPP_ICMP6)
        t = dissectIcmp6(payload, { headerOnly: partial }, opts);
    else if (proto === 41)
        t = dissectIp6Bytes(payload, { partial }, opts);
    else if (proto === 59)
        t = { label: 'No next header' };
    else
        t = { label: `Protocol ${proto}`, raw: toHex(payload) };
    // Checksums, over the pseudo-header with the final destination (RFC 8200 8.1).
    if (!partial && (proto === IPP_UDP || proto === IPP_ICMP6 || proto === IPP_TCP) && payload.length >= expected && !t.error) {
        const sumAt = proto === IPP_TCP ? 16 : proto === IPP_UDP ? 6 : 2;
        const stored = (payload[sumAt] << 8) | payload[sumAt + 1];
        const field = t.fields?.find((f) => f[0] === 'Checksum');
        if (proto === IPP_UDP && stored === 0) {
            if (field)
                field[1] = '0x0000 (none)';
        }
        else {
            const ok = ip6PseudoChecksum(p.src, finalDst, proto, payload.subarray(0, expected)) === 0;
            if (field)
                field[1] = `0x${hex4(stored)} (${ok ? 'correct' : 'incorrect'})`;
            if (!ok)
                t.error = `bad checksum${o.hint ? ` — ${o.hint}` : ''}`;
        }
    }
    node.children.push(t);
    return node;
}
/** One extension header; returns the final destination a routing header names. */
function extNode(ext, p) {
    if (ext instanceof IP6OptsHeader) {
        const name = ext instanceof IP6HopOptsHeader ? 'Hop-by-Hop Options' : 'Destination Options';
        const children = ext.options.map((opt) => {
            if (opt.type === 0x63 || opt.type === 0x23) {
                // RPL Option (RFC 6553; 0x23 since RFC 9008): O R F flags, instance, sender rank.
                const d = opt.data;
                const flags = [d[0] & 0x80 ? 'down' : 'up', d[0] & 0x40 ? 'rank error' : '', d[0] & 0x20 ? 'forwarding error' : ''].filter(Boolean);
                return {
                    label: 'RPL Option', summary: `instance ${d[1]}, sender rank ${(d[2] << 8) | d[3]}, ${flags.join(', ')}`,
                    fields: [['Option type', `0x${hex2(opt.type)}`], ['Flags', flags.join(', ')], ['RPL instance', String(d[1])], ['Sender rank', String((d[2] << 8) | d[3])]],
                };
            }
            if (opt.type === 0x6d || opt.type === 0x4d) {
                // MPL Option (RFC 7731): seed ID length S, M flag, V flag, sequence, seed ID.
                const d = opt.data;
                const sl = [0, 2, 8, 16][d[0] >> 6];
                const seed = d.subarray(2, 2 + sl);
                const seedStr = sl === 0 ? 'the IPv6 source' : sl === 16 ? ip6str(seed) : `0x${toHex(seed)}`;
                return {
                    label: 'MPL Option', summary: `sequence ${d[1]}, seed ${seedStr}`,
                    fields: [['Seed ID', seedStr], ['Sequence', String(d[1])], ['Largest sequence (M)', String((d[0] >> 5) & 1)]],
                };
            }
            if (opt.type === 0x0c) {
                // Contiki-NG's ROLL-TM (Trickle multicast) option: a 2-byte seed ID
                // when short seeds are used (else the IPv6 source), M, a 15-bit sequence.
                const d = opt.data;
                const short = d.length >= 4;
                const f = short ? 2 : 0;
                const seq = ((d[f] & 0x7f) << 8) | d[f + 1];
                return {
                    label: 'Trickle Multicast Option', summary: `sequence ${seq}, seed ${short ? `0x${toHex(d.subarray(0, 2))}` : 'the IPv6 source'}`,
                    fields: [['Seed ID', short ? `0x${toHex(d.subarray(0, 2))}` : 'the IPv6 source'], ['M', String(d[f] >> 7)], ['Sequence', String(seq)]],
                };
            }
            if (opt.type === 0x05) {
                const v = (opt.data[0] << 8) | opt.data[1];
                return { label: 'Router Alert', summary: v === 0 ? 'MLD' : String(v) };
            }
            return { label: `Option 0x${hex2(opt.type)}`, raw: toHex(opt.data) };
        });
        return [{ label: name, summary: children.map((c) => c.label).join(', '), children }, undefined];
    }
    if (ext instanceof IP6RoutingHeader) {
        if (ext.type === 3 && ext.bytes) {
            // RPL Source Route Header (RFC 6554): addresses with CmprI/CmprE
            // leading bytes elided (taken from the IPv6 destination).
            const b = ext.bytes;
            const cmprI = b[4] >> 4, cmprE = b[4] & 0xf, pad = b[5] >> 4;
            const body = b.subarray(8, b.length - pad);
            const n = body.length >= 16 - cmprE ? Math.floor((body.length - (16 - cmprE)) / (16 - cmprI)) + 1 : 0;
            const addrs = [];
            let off = 0;
            for (let i = 0; i < n; i++) {
                const elided = i < n - 1 ? cmprI : cmprE;
                const a = new Uint8Array(16);
                a.set(p.dst.subarray(0, elided));
                a.set(body.subarray(off, off + 16 - elided), elided);
                off += 16 - elided;
                addrs.push(a);
            }
            return [{
                    label: 'Routing Header (RPL source route)',
                    summary: n ? `${ext.segs_left} of ${n} segments left` : 'no addresses: the destination is the next hop',
                    fields: [['Segments left', String(ext.segs_left)], ['CmprI / CmprE / Pad', `${cmprI} / ${cmprE} / ${pad}`],
                        ...addrs.map((a, i) => [`Address ${i + 1}`, ip6str(a)])],
                }, ext.segs_left && addrs.length ? addrs[addrs.length - 1] : undefined];
        }
        return [{
                label: `Routing Header (type ${ext.type})`, summary: `${ext.segs_left} segments left`,
                fields: ext.addresses.map((a, i) => [`Address ${i + 1}`, ip6str(a)]),
            }, ext.segs_left && ext.addresses.length ? ext.addresses[ext.addresses.length - 1] : undefined];
    }
    if (ext instanceof IP6FragmentHeader) {
        return [{ label: 'Fragment Header', fields: [['Offset', String(ext.frag_off ?? '')], ['Identification', String(ext.id ?? '')]] }, undefined];
    }
    return [{ label: `Extension header ${ext.nxt ?? ''}`.trim() }, undefined];
}
/** Dissect an IPv4 packet (the far side of a NAT64 translator). */
export function dissectIp4(p) {
    try {
        const ip = p instanceof IP ? p : new IP(p);
        const node = {
            label: 'IPv4',
            summary: `${ip4str(ip.src)} → ${ip4str(ip.dst)} proto=${ip.p} ttl=${ip.ttl}`,
            fields: [['Source', ip4str(ip.src)], ['Destination', ip4str(ip.dst)], ['Protocol', String(ip.p)], ['TTL', String(ip.ttl)], ['Length', String(ip.len)], ['Flags', `${ip.df ? 'DF' : ''}${ip.mf ? ' MF' : ''}`.trim() || '(none)']],
            children: [],
        };
        const payload = ip.data;
        if (!(payload instanceof Uint8Array) || !payload.length)
            return node;
        if (ip.p === IPP_UDP)
            node.children.push(dissectUdp(payload));
        else if (ip.p === IPP_TCP)
            node.children.push(dissectTcp(payload));
        else if (ip.p === IPP_ICMP)
            node.children.push(dissectIcmp4(payload));
        else
            node.children.push({ label: `proto ${ip.p}`, raw: toHex(payload) });
        return node;
    }
    catch (e) {
        return { label: 'IPv4', error: String(e), raw: toHex(p instanceof IP ? p.toBytes() : p) };
    }
}
export function dissectIcmp4(buf) {
    const icmp = new ICMP(buf);
    const node = {
        label: 'ICMP',
        summary: icmp.type === 8 ? 'Echo request' : icmp.type === 0 ? 'Echo reply' : `type=${icmp.type} code=${icmp.code}`,
        fields: [['Type', String(icmp.type)], ['Code', String(icmp.code)], ['Checksum', `0x${icmp.sum.toString(16)}`]],
    };
    if (icmp.data instanceof ICMPEcho) {
        node.children = [{ label: 'Echo', fields: [['id', String(icmp.data.id)], ['seq', String(icmp.data.seq)]] }];
    }
    return node;
}
export function dissectUdp(buf, o = {}, opts = {}) {
    let u;
    try {
        u = new UDP(buf);
    }
    catch (e) {
        return { label: 'UDP', error: errText(e), raw: toHex(buf) };
    }
    const node = {
        label: 'UDP',
        summary: `${u.sport} → ${u.dport} len=${u.ulen}`,
        fields: [['Src port', String(u.sport)], ['Dst port', String(u.dport)], ['Length', String(u.ulen)], ['Checksum', `0x${u.sum.toString(16)}`]],
        children: [],
    };
    const payload = u.data.subarray(0, Math.max(0, u.ulen - 8));
    if (o.headerOnly || !payload.length)
        return node;
    const port = (p) => u.sport === p || u.dport === p;
    let c;
    if (port(53) || port(5353))
        c = dissectDns(payload);
    else if (SRP_PORTS(u.sport) || SRP_PORTS(u.dport)) {
        c = dissectDns(payload);
        if (!c.error)
            c.label = 'DNS (SRP)';
        else
            c = dataNode(payload);
    }
    else if (port(MLE_PORT))
        c = dissectMle(payload, opts.keys && o.src && o.dst ? opts.keys.decryptMle(payload, o.src, o.dst) ?? undefined : undefined);
    else if (DTLS_PORTS.has(u.dport) || DTLS_PORTS.has(u.sport) || looksLikeDtls(payload))
        c = dissectDtls(payload);
    else if (COAP_PORTS.has(u.dport) || COAP_PORTS.has(u.sport)) {
        c = dissectCoap(payload, port(TMF_PORT) ? tmfPayloadNodes : undefined, opts.coapRequests);
        if (port(TMF_PORT))
            c.label = 'CoAP (Thread TMF)';
    }
    else {
        // Elsewhere, CoAP only if it parses cleanly (version bits alone match
        // a quarter of all payloads: "hello" begins 0x68).
        const cc = (payload[0] >> 6) === 1 ? dissectCoap(payload, undefined, opts.coapRequests) : null;
        c = cc && !cc.error ? cc : dataNode(payload);
    }
    node.children.push(c);
    return node;
}
/** OpenThread's SRP server ports (OPENTHREAD_CONFIG_SRP_SERVER_UDP_PORT_MIN..MAX). */
const SRP_PORTS = (p) => p >= 53535 && p <= 53554;
/** CoAP, and Thread's management (TMF) port, which carries CoAP. */
const COAP_PORTS = new Set([5683, TMF_PORT]);
/** CoAP over DTLS, Thread commissioning. */
const DTLS_PORTS = new Set([5684, MESHCOP_PORT]);
/** Application bytes; text is shown as text. */
function dataNode(b) {
    let printable = 0;
    for (const x of b)
        if ((x >= 0x20 && x < 0x7f) || x === 9 || x === 10 || x === 13)
            printable++;
    // Text, allowing trailing NUL padding (a C string sent with its buffer).
    let end = b.length;
    while (end > 1 && b[end - 1] === 0)
        end--;
    const text = printable === end && end > 0;
    if (!text)
        return { label: 'Data', summary: `${b.length} bytes`, raw: toHex(b) };
    const s = new TextDecoder().decode(b.subarray(0, end));
    return { label: 'Data', summary: JSON.stringify(s.length > 60 ? `${s.slice(0, 60)}…` : s), raw: toHex(b) };
}
const HTTP_PORTS = new Set([80, 8080, 8000]);
export function dissectTcp(buf) {
    try {
        const t = new TCP(buf);
        const node = {
            label: 'TCP',
            summary: `${t.sport} → ${t.dport} [${tcpFlagsToStr(t.flags) || '-'}] seq=${t.seq}${t.flags & 0x10 ? ` ack=${t.ack}` : ''} win=${t.win}`,
            fields: [['Src port', String(t.sport)], ['Dst port', String(t.dport)], ['Seq', String(t.seq)], ['Ack', String(t.ack)], ['Flags', tcpFlagsToStr(t.flags) || '(none)'], ['Window', String(t.win)], ['Checksum', `0x${t.sum.toString(16)}`]],
            children: [],
        };
        const payload = t.data;
        if (payload.length) {
            if (HTTP_PORTS.has(t.sport) || HTTP_PORTS.has(t.dport) || looksLikeHttp(payload)) {
                node.children.push(dissectHttp(payload, HTTP_PORTS.has(t.dport)));
            }
            else
                node.children.push({ label: 'data', raw: toHex(payload) });
        }
        return node;
    }
    catch (e) {
        return { label: 'TCP', error: String(e), raw: toHex(buf) };
    }
}
function looksLikeHttp(payload) {
    const head = new TextDecoder('ascii').decode(payload.subarray(0, 8));
    return head.startsWith('HTTP/') || /^(GET|PUT|POST|HEAD|DELETE) /.test(head);
}
export function dissectHttp(buf, isRequest) {
    const head = new TextDecoder('ascii').decode(buf.subarray(0, 8));
    const request = isRequest ?? !head.startsWith('HTTP/');
    try {
        const msg = request ? new HttpRequest(buf) : new HttpResponse(buf);
        const node = {
            label: 'HTTP',
            summary: request
                ? `${msg.method} ${msg.uri} HTTP/${msg.version}`
                : `HTTP/${msg.version} ${msg.status} ${msg.reason}`,
            fields: Object.entries(msg.headers).map(([k, v]) => [k, Array.isArray(v) ? v.join(', ') : v]),
        };
        if (msg.body.length)
            node.children = [{ label: `Body (${msg.body.length} bytes)`, raw: toHex(msg.body) }];
        return node;
    }
    catch (e) {
        return { label: 'HTTP', error: String(e), raw: toHex(buf) };
    }
}
const DNS_TYPE_NAMES = {
    [DNS_A]: 'A', [DNS_NS]: 'NS', [DNS_CNAME]: 'CNAME', [DNS_SOA]: 'SOA', [DNS_PTR]: 'PTR',
    [DNS_MX]: 'MX', [DNS_TXT]: 'TXT', [DNS_AAAA]: 'AAAA', [DNS_SRV]: 'SRV', 41: 'OPT', 24: 'SIG', 25: 'KEY', 255: 'ANY',
};
function dnsRRNode(rr) {
    const t = DNS_TYPE_NAMES[rr.type] ?? String(rr.type);
    let val = '';
    if (rr.type === DNS_A && rr.ip)
        val = ip4str(rr.ip);
    else if (rr.type === DNS_AAAA && rr.ip6)
        val = ip6str(rr.ip6);
    else if (rr.type === DNS_CNAME)
        val = rr.cname ?? '';
    else if (rr.type === DNS_PTR)
        val = rr.ptrname ?? '';
    else if (rr.type === DNS_NS)
        val = rr.nsname ?? '';
    else if (rr.type === DNS_MX)
        val = `${rr.preference} ${rr.mxname}`;
    else if (rr.type === DNS_SRV)
        val = `${rr.priority} ${rr.weight} ${rr.port} ${rr.srvname}`;
    else if (rr.type === DNS_TXT)
        val = (rr.text ?? []).join(' ');
    else if (rr.type === 41) {
        // EDNS(0) options; an SRP update carries its leases in Update Lease (2).
        const o = [];
        const b = rr.rdata ?? new Uint8Array(0);
        for (let off = 0; off + 4 <= b.length;) {
            const code = (b[off] << 8) | b[off + 1], len = (b[off + 2] << 8) | b[off + 3];
            const v = b.subarray(off + 4, off + 4 + len);
            const u32 = (i) => ((v[i] << 24) >>> 0) + (v[i + 1] << 16) + (v[i + 2] << 8) + v[i + 3];
            o.push(code === 2 && len >= 4 ? `update lease ${u32(0)} s${len >= 8 ? `, key lease ${u32(4)} s` : ''}` : `option ${code} (${len} bytes)`);
            off += 4 + len;
        }
        return { label: 'OPT (EDNS)', summary: o.join(', ') || `UDP payload size ${rr.cls}` };
    }
    else if (rr.rlen === 0)
        val = rr.cls === 255 ? 'delete RRset' : rr.cls === 254 ? 'must not exist' : '(empty)';
    else
        val = `${rr.rlen} bytes`;
    const cls = rr.cls === 255 ? ' ANY' : rr.cls === 254 ? ' NONE' : '';
    return { label: `${rr.name || '<root>'} ${t}${cls}`, summary: `${val} (ttl ${rr.ttl})` };
}
export function dissectDns(buf) {
    try {
        const d = parseDnsLenient(buf);
        // UPDATE (RFC 2136) names its sections differently; SRP (RFC 9665) is one.
        const update = d.opcode === 5;
        const node = {
            label: 'DNS',
            summary: update
                ? `${d.qr ? `UPDATE response, rcode ${d.rcode}` : `UPDATE ${d.qd[0]?.name ?? ''}, ${d.ns.length} records`} id=0x${d.id.toString(16)}`
                : `${d.qr ? 'response' : 'query'} id=0x${d.id.toString(16)}${d.qd[0] ? ` ${d.qd[0].name} ${DNS_TYPE_NAMES[d.qd[0].type] ?? d.qd[0].type}` : ''}${d.qr ? ` an=${d.an.length}` : ''}`,
            fields: [['ID', `0x${d.id.toString(16)}`], ['QR', d.qr ? 'response' : 'query'], ['Opcode', `${d.opcode}${update ? ' (UPDATE)' : d.opcode === 0 ? ' (QUERY)' : ''}`], ['RCODE', String(d.rcode)]],
            children: [],
        };
        if (d.qd.length)
            node.children.push({ label: update ? 'Zone' : 'Questions', children: d.qd.map((q) => ({ label: q.name, summary: DNS_TYPE_NAMES[q.type] ?? String(q.type) })) });
        if (d.an.length)
            node.children.push({ label: update ? 'Prerequisites' : 'Answers', children: d.an.map(dnsRRNode) });
        if (d.ns.length)
            node.children.push({ label: update ? 'Updates' : 'Authority', children: d.ns.map(dnsRRNode) });
        if (d.ar.length)
            node.children.push({ label: 'Additional', children: d.ar.map(dnsRRNode) });
        return node;
    }
    catch (e) {
        return { label: 'DNS', error: String(e), raw: toHex(buf) };
    }
}
const COAP_OPTIONS = {
    1: 'If-Match', 3: 'Uri-Host', 4: 'ETag', 5: 'If-None-Match', 6: 'Observe', 7: 'Uri-Port', 8: 'Location-Path',
    11: 'Uri-Path', 12: 'Content-Format', 14: 'Max-Age', 15: 'Uri-Query', 17: 'Accept', 20: 'Location-Query',
    23: 'Block2', 27: 'Block1', 28: 'Size2', 35: 'Proxy-Uri', 39: 'Proxy-Scheme', 60: 'Size1',
};
const COAP_METHODS = ['', 'GET', 'POST', 'PUT', 'DELETE', 'FETCH', 'PATCH', 'iPATCH'];
const COAP_RESPONSES = {
    201: 'Created', 202: 'Deleted', 203: 'Valid', 204: 'Changed', 205: 'Content', 231: 'Continue',
    400: 'Bad Request', 401: 'Unauthorized', 402: 'Bad Option', 403: 'Forbidden', 404: 'Not Found',
    405: 'Method Not Allowed', 406: 'Not Acceptable', 408: 'Request Entity Incomplete', 412: 'Precondition Failed',
    413: 'Request Entity Too Large', 415: 'Unsupported Content-Format', 500: 'Internal Server Error',
    501: 'Not Implemented', 502: 'Bad Gateway', 503: 'Service Unavailable', 504: 'Gateway Timeout',
};
const optUint = (v) => v.reduce((a, b) => a * 256 + b, 0);
/** EDHOC (RFC 9528) messages as CoAP carries them: CBOR sequences.  A request
 *  starting with `true` is message_1; otherwise a request carries C_R and
 *  message_3, a response message_2 (or message_4, or an error). */
function edhocNode(b, request) {
    let items;
    try {
        items = cborDecodeSequence(b);
    }
    catch (e) {
        return { label: 'EDHOC', error: errText(e), raw: toHex(b) };
    }
    const size = (v) => (v instanceof Uint8Array ? `${v.length} bytes` : cborDiag(v));
    let summary;
    const fields = [];
    if (request && items[0] === true) {
        const [, method, suites, gx, ci] = items;
        summary = 'message_1';
        fields.push(['Method', cborDiag(method)], ['Cipher suites', cborDiag(suites)], ['G_X', size(gx)], ['C_I', cborDiag(ci)]);
        if (items.length > 5)
            fields.push(['EAD_1', `${items.length - 5} item(s)`]);
    }
    else if (request) {
        summary = 'message_3';
        fields.push(['C_R', cborDiag(items[0])], ['CIPHERTEXT_3', size(items[1])]);
    }
    else if (items.length === 1 && items[0] instanceof Uint8Array) {
        summary = 'message_2 (or message_4)';
        fields.push(['G_Y_CIPHERTEXT_2', size(items[0])]);
    }
    else if (typeof items[0] === 'number') {
        summary = 'error';
        fields.push(['ERR_CODE', String(items[0])], ['ERR_INFO', cborDiag(items[1] ?? null)]);
    }
    else
        summary = `${items.length} CBOR items`;
    return { label: 'EDHOC', summary, fields, children: [{ label: 'CBOR sequence', summary: items.map(cborDiag).join(', ').slice(0, 120) }] };
}
/** CoAP (RFC 7252).  `payloadNodes` decodes the payload for a protocol that
 *  defines it by URI (Thread's TMF). */
export function dissectCoap(buf, payloadNodes, requests) {
    try {
        const c = new CoAP(buf);
        let cf;
        const path = [];
        const query = [];
        const optChildren = c.opts.map(([num, val]) => {
            if (num === 12)
                cf = optUint(val);
            if (num === 11)
                path.push(new TextDecoder().decode(val));
            if (num === 15)
                query.push(new TextDecoder().decode(val));
            const name = COAP_OPTIONS[num] ?? `Option ${num}`;
            let text;
            if (num === 11 || num === 15 || num === 3 || num === 8 || num === 20 || num === 35 || num === 39)
                text = new TextDecoder().decode(val);
            else if (num === 23 || num === 27) {
                const v = optUint(val);
                text = `block ${v >> 4}, ${16 << (v & 7)} bytes${v & 8 ? ', more' : ''}`;
            }
            else if (num === 12 || num === 17)
                text = CF_NAMES[optUint(val)] ?? String(optUint(val));
            else if (num === 6 || num === 7 || num === 14 || num === 28 || num === 60)
                text = String(optUint(val));
            else
                text = val.length ? `0x${toHex(val)}` : '(empty)';
            return { label: name, summary: text };
        });
        const code = c.codeClass * 100 + c.codeDetail;
        const codeStr = `${c.codeClass}.${String(c.codeDetail).padStart(2, '0')}`;
        const name = c.codeClass === 0 && c.codeDetail === 0 ? 'Empty'
            : c.codeClass === 0 ? (COAP_METHODS[c.codeDetail] ?? '') : (COAP_RESPONSES[code] ?? '');
        const what = c.codeClass === 0 ? (name || codeStr) : `${codeStr}${name ? ` ${name}` : ''}`;
        const uri = path.length ? `/${path.join('/')}${query.length ? `?${query.join('&')}` : ''}` : '';
        const node = {
            label: 'CoAP',
            summary: `${COAP_TYPES[c.t]} ${what}${uri ? ` ${uri}` : ''} mid=0x${c.id.toString(16)}`,
            fields: [['Type', COAP_TYPES[c.t]], ['Code', name ? `${codeStr} (${name})` : codeStr], ['Message ID', String(c.id)], ['Token', c.tkl ? `0x${toHex(c.token)}` : '(none)']],
            children: optChildren.length ? [{ label: 'Options', summary: optChildren.map((o) => o.label).join(', '), children: optChildren }] : [],
        };
        if (uri)
            node.fields.push(['URI', uri]);
        // A response names no URI: its token says which request it answers,
        // or for a piggybacked ACK without one, the Message ID.
        let target = uri;
        if (requests) {
            const keys = [...(c.tkl ? [`t${toHex(c.token)}`] : []), ...(c.t === 0 || c.t === 2 ? [`m${c.id}`] : [])];
            if (c.codeClass === 0 && uri)
                for (const k of keys)
                    requests.set(k, uri);
            else if (c.codeClass >= 2) {
                const k = keys.find((x) => requests.has(x) && (x[0] === 't' || c.t === 2));
                if (k) {
                    target = requests.get(k);
                    node.fields.push(['In response to', target]);
                }
            }
        }
        const block = c.opts.find(([n]) => n === 23 || n === 27);
        const payload = c.data;
        if (payload.length) {
            const decoded = payloadNodes ? payloadNodes(target, payload) : null;
            if (decoded)
                node.children.push({ label: 'Payload', summary: `${payload.length} bytes`, children: decoded });
            else if (target === '/.well-known/edhoc' || cf === 64 || cf === 65) {
                const e = edhocNode(payload, c.codeClass === 0);
                // One block of a block-wise transfer is not a whole CBOR sequence.
                if (e.error && block) {
                    const v = optUint(block[1]);
                    node.children.push({ label: 'EDHOC', summary: `block ${v >> 4}${v & 8 ? ', more to follow' : ', last'} of a block-wise message`, raw: toHex(payload) });
                }
                else
                    node.children.push(e);
            }
            else if (cf === lwm2m.ContentFormat.TLV || cf === lwm2m.ContentFormat.JSON)
                node.children.push(dissectLwm2m(payload, cf));
            else if (cf === 0 || cf === 50) {
                const text = new TextDecoder().decode(payload);
                node.children.push({ label: `Payload (${CF_NAMES[cf]})`, summary: JSON.stringify(text.length > 60 ? `${text.slice(0, 60)}…` : text), raw: toHex(payload) });
            }
            else if (cf == null) {
                const d = dataNode(payload);
                node.children.push({ label: 'Payload', summary: d.summary, raw: d.raw });
            }
            else
                node.children.push({ label: `Payload (${CF_NAMES[cf] ?? cf})`, raw: toHex(payload) });
        }
        if (!node.children.length)
            delete node.children;
        return node;
    }
    catch (e) {
        return { label: 'CoAP', error: errText(e), raw: toHex(buf) };
    }
}
const ICMP6_NAMES = {
    1: 'Destination Unreachable', 2: 'Packet Too Big', 3: 'Time Exceeded', 4: 'Parameter Problem',
    128: 'Echo Request', 129: 'Echo Reply', 130: 'Multicast Listener Query', 131: 'Multicast Listener Report',
    132: 'Multicast Listener Done', 133: 'Router Solicitation', 134: 'Router Advertisement',
    135: 'Neighbor Solicitation', 136: 'Neighbor Advertisement', 137: 'Redirect', 143: 'Multicast Listener Report v2',
    155: 'RPL Control', 157: 'Duplicate Address Request', 158: 'Duplicate Address Confirmation', 159: 'MPL Control',
    200: 'Private experimentation (ROLL-TM)',
};
const RPL_NAMES = {
    0: 'DIS', 1: 'DIO', 2: 'DAO', 3: 'DAO-ACK', 4: 'DCO', 5: 'DCO-ACK', 0x0a: 'CC',
    0x80: 'Secure DIS', 0x81: 'Secure DIO', 0x82: 'Secure DAO', 0x83: 'Secure DAO-ACK',
};
const RPL_MOP = ['no downward routes', 'non-storing', 'storing without multicast', 'storing with multicast'];
const RPL_OPTS = {
    0: 'Pad1', 1: 'PadN', 2: 'DAG Metric Container', 3: 'Route Information', 4: 'DODAG Configuration', 5: 'RPL Target',
    6: 'Transit Information', 7: 'Solicited Information', 8: 'Prefix Information', 9: 'RPL Target Descriptor',
};
const ND_OPTS = {
    1: 'Source Link-Layer Address', 2: 'Target Link-Layer Address', 3: 'Prefix Information', 4: 'Redirected Header',
    5: 'MTU', 25: 'Recursive DNS Server', 31: 'DNS Search List', 33: 'Address Registration',
    34: '6LoWPAN Context', 35: 'Authoritative Border Router',
};
const ARO_STATUS = ['success', 'duplicate address', 'neighbor cache full'];
const MLD_RECORDS = { 1: 'is include', 2: 'is exclude', 3: 'to include', 4: 'to exclude', 5: 'allow new sources', 6: 'block old sources' };
const be16 = (b, o) => (b[o] << 8) | b[o + 1];
const be32 = (b, o) => ((b[o] << 24) >>> 0) + (b[o + 1] << 16) + (b[o + 2] << 8) + b[o + 3];
const addr16 = (b, o) => (b.length >= o + 16 ? ip6str(b.subarray(o, o + 16)) : '(truncated)');
const lifetime = (v) => (v === 0xffffffff ? 'infinite' : `${v} s`);
const prefixOf = (b, len) => {
    const a = new Uint8Array(16);
    a.set(b.subarray(0, Math.min(16, Math.ceil(len / 8))));
    return `${ip6str(a)}/${len}`;
};
/** ICMPv6 (RFC 4443) with Neighbor Discovery (RFC 4861, RFC 6775) and RPL
 *  (RFC 6550).  With `headerOnly`, only the type is named (a first fragment). */
export function dissectIcmp6(buf, o = {}, opts = {}) {
    if (buf.length < 4)
        return { label: 'ICMPv6', error: 'shorter than its header', raw: toHex(buf) };
    const type = buf[0], code = buf[1];
    const name = type === ICMP6_RPL_CONTROL ? `RPL ${RPL_NAMES[code] ?? `code ${code}`}` : (ICMP6_NAMES[type] ?? `type ${type}`);
    const node = {
        label: 'ICMPv6', summary: name,
        fields: [['Type', `${type} (${ICMP6_NAMES[type] ?? 'unknown'})`], ['Code', String(code)], ['Checksum', `0x${hex4(be16(buf, 2))}`]],
        children: [],
    };
    if (o.headerOnly)
        return node;
    const b = buf.subarray(4);
    try {
        switch (type) {
            case 1:
            case 2:
            case 3:
            case 4: {
                if (type === 2)
                    node.fields.push(['MTU', String(be32(b, 0))]);
                if (type === 4)
                    node.fields.push(['Pointer', String(be32(b, 0))]);
                node.children.push({ label: 'Invoking packet', children: [dissectIp6Bytes(b.subarray(4), { partial: true }, opts)] });
                break;
            }
            case 128:
            case 129: {
                node.summary = `${name}, id ${be16(b, 0)}, seq ${be16(b, 2)}`;
                node.fields.push(['Identifier', String(be16(b, 0))], ['Sequence', String(be16(b, 2))], ['Data', `${b.length - 4} bytes`]);
                break;
            }
            case 133:
                ndOptions(node, b.subarray(4));
                break;
            case 134: {
                node.fields.push(['Cur hop limit', String(b[0])], ['Flags', `${b[1] & 0x80 ? 'M ' : ''}${b[1] & 0x40 ? 'O' : ''}`.trim() || '(none)'], ['Router lifetime', `${be16(b, 2)} s`], ['Reachable time', `${be32(b, 4)} ms`], ['Retrans timer', `${be32(b, 8)} ms`]);
                ndOptions(node, b.subarray(12));
                break;
            }
            case 135: {
                node.summary = `${name} for ${addr16(b, 4)}`;
                node.fields.push(['Target', addr16(b, 4)]);
                ndOptions(node, b.subarray(20));
                break;
            }
            case 136: {
                const flags = [b[0] & 0x80 ? 'router' : '', b[0] & 0x40 ? 'solicited' : '', b[0] & 0x20 ? 'override' : ''].filter(Boolean).join(', ');
                node.summary = `${name}: ${addr16(b, 4)}${flags ? ` (${flags})` : ''}`;
                node.fields.push(['Flags', flags || '(none)'], ['Target', addr16(b, 4)]);
                ndOptions(node, b.subarray(20));
                break;
            }
            case 137: {
                node.fields.push(['Target', addr16(b, 4)], ['Destination', addr16(b, 20)]);
                ndOptions(node, b.subarray(36));
                break;
            }
            case 157:
            case 158: {
                node.fields.push(['Status', `${b[0]}${ARO_STATUS[b[0]] ? ` (${ARO_STATUS[b[0]]})` : ''}`], ['Registration lifetime', `${be16(b, 2)} min`], ['EUI-64', linkStr(b.subarray(4, 12))], ['Registered address', addr16(b, 12)]);
                break;
            }
            case 130:
            case 131:
            case 132: {
                node.summary = `${name}${type !== 130 || b.length >= 20 ? ` ${addr16(b, 4)}` : ''}`;
                node.fields.push(['Maximum response delay', `${be16(b, 0)} ms`], ['Multicast address', addr16(b, 4)]);
                break;
            }
            case 143: {
                // MLDv2 report (RFC 3810): records of a type, sources and a group.
                const n = be16(b, 2);
                const groups = [];
                let off = 4;
                for (let i = 0; i < n && off + 20 <= b.length; i++) {
                    const rtype = b[off], aux = b[off + 1] * 4, nsrc = be16(b, off + 2);
                    const group = addr16(b, off + 4);
                    groups.push(group);
                    node.children.push({ label: 'Multicast address record', summary: `${MLD_RECORDS[rtype] ?? `type ${rtype}`} ${group}`,
                        fields: nsrc ? [['Sources', String(nsrc)]] : undefined });
                    off += 20 + 16 * nsrc + aux;
                }
                node.summary = `${name}: ${groups.join(', ')}`;
                break;
            }
            case 159: {
                // MPL Control Message (RFC 7731 10.2): seed info entries.
                let off = 0;
                while (off + 2 <= b.length) {
                    const min = b[off], bmLen = b[off + 1] >> 2, sl = [0, 2, 8, 16][b[off + 1] & 3];
                    const seed = b.subarray(off + 2, off + 2 + sl);
                    const seedStr = sl === 0 ? 'the IPv6 source' : sl === 16 ? ip6str(seed) : `0x${toHex(seed)}`;
                    node.children.push({ label: 'MPL Seed Info', summary: `seed ${seedStr}, from sequence ${min}, ${bmLen}-byte bitmap`,
                        raw: toHex(b.subarray(off + 2 + sl, off + 2 + sl + bmLen)) });
                    off += 2 + sl + bmLen;
                }
                node.summary = `MPL Control, ${node.children.length} seed${node.children.length === 1 ? '' : 's'}`;
                break;
            }
            case 200: {
                // Contiki-NG's ROLL-TM Trickle ICMP: sequence lists, one per seed.
                let off = 0;
                while (off + 2 <= b.length) {
                    const flags = b[off], n = b[off + 1];
                    const sl = flags & 0x80 ? 2 : 16;
                    const seed = b.subarray(off + 2, off + 2 + sl);
                    const seqs = [];
                    for (let i = 0; i < n; i++)
                        seqs.push(be16(b, off + 2 + sl + 2 * i));
                    node.children.push({ label: 'Sequence List', summary: `seed ${sl === 16 ? ip6str(seed) : `0x${toHex(seed)}`}, M ${(flags >> 6) & 1}: ${seqs.join(', ')}` });
                    off += 2 + sl + 2 * n;
                }
                node.summary = 'Trickle Multicast (ROLL-TM)';
                break;
            }
            case ICMP6_RPL_CONTROL:
                node.children.push(rplNode(code, b));
                break;
            default: if (b.length)
                node.children.push({ label: 'Body', raw: toHex(b) });
        }
    }
    catch (e) {
        node.error = errText(e);
    }
    if (!node.children.length)
        delete node.children;
    return node;
}
function ndOptions(node, b) {
    let off = 0;
    while (off + 2 <= b.length) {
        const t = b[off], len = b[off + 1] * 8;
        if (!len || off + len > b.length) {
            node.children.push({ label: 'Option', error: `bad length ${len}`, raw: toHex(b.subarray(off)) });
            return;
        }
        const v = b.subarray(off + 2, off + len);
        const n = { label: ND_OPTS[t] ?? `Option ${t}`, fields: [] };
        if (t === 1 || t === 2) {
            // 802.15.4 (RFC 4944 8): an EUI-64, or a short address padded out.
            const a = len === 16 ? v.subarray(0, 8) : len === 8 && v[2] === 0 && v[3] === 0 && v[4] === 0 && v[5] === 0 ? v.subarray(0, 2) : v.subarray(0, 6);
            n.summary = a.length === 6 ? [...a].map(hex2).join(':') : linkStr(a);
        }
        else if (t === 3 && v.length >= 30) {
            n.summary = prefixOf(v.subarray(14), v[0]);
            n.fields.push(['Flags', `${v[1] & 0x80 ? 'on-link ' : ''}${v[1] & 0x40 ? 'autonomous' : ''}`.trim() || '(none)'], ['Valid lifetime', lifetime(be32(v, 2))], ['Preferred lifetime', lifetime(be32(v, 6))]);
        }
        else if (t === 5)
            n.summary = String(be32(v, 2));
        else if (t === 33 && v.length >= 14) {
            n.summary = `${ARO_STATUS[v[0]] ?? `status ${v[0]}`}, ${be16(v, 4)} min, ${linkStr(v.subarray(6, 14))}`;
        }
        else if (t === 34 && v.length >= 6) {
            n.summary = `context ${v[1] & 0xf}${v[1] & 0x10 ? '' : ' (not for compression)'}: ${prefixOf(v.subarray(6), v[0])}, ${be16(v, 4)} min`;
        }
        else if (t === 35 && v.length >= 22) {
            n.summary = `${addr16(v, 6)}, version ${(be16(v, 2) << 16) | be16(v, 0)}`;
        }
        else
            n.raw = toHex(v);
        if (!n.fields.length)
            delete n.fields;
        node.children.push(n);
        off += len;
    }
}
function rplNode(code, b) {
    const name = RPL_NAMES[code] ?? `code ${code}`;
    const node = { label: `RPL ${name}`, fields: [], children: [] };
    let off = 0;
    switch (code) {
        case 0:
            off = 2;
            break;
        case 1: {
            const g = b[4] >> 7, mop = (b[4] >> 3) & 7, prf = b[4] & 7;
            node.summary = `instance ${b[0]}, version ${b[1]}, rank ${be16(b, 2)}, DODAG ${addr16(b, 8)}`;
            node.fields.push(['RPL instance', String(b[0])], ['Version', String(b[1])], ['Rank', String(be16(b, 2))], ['Grounded', g ? 'yes' : 'no'], ['Mode of operation', `${mop} (${RPL_MOP[mop] ?? 'reserved'})`], ['Preference', String(prf)], ['DTSN', String(b[5])], ['DODAG ID', addr16(b, 8)]);
            off = 24;
            break;
        }
        case 2: {
            const k = b[1] >> 7, d = (b[1] >> 6) & 1;
            node.summary = `instance ${b[0]}, sequence ${b[3]}${k ? ', ACK requested' : ''}`;
            node.fields.push(['RPL instance', String(b[0])], ['ACK requested', k ? 'yes' : 'no'], ['DAO sequence', String(b[3])]);
            off = 4;
            if (d) {
                node.fields.push(['DODAG ID', addr16(b, 4)]);
                off += 16;
            }
            break;
        }
        case 3: {
            const d = b[1] >> 7;
            node.summary = `instance ${b[0]}, sequence ${b[2]}, status ${b[3]}`;
            node.fields.push(['RPL instance', String(b[0])], ['DAO sequence', String(b[2])], ['Status', String(b[3])]);
            off = 4;
            if (d) {
                node.fields.push(['DODAG ID', addr16(b, 4)]);
                off += 16;
            }
            break;
        }
        default:
            node.raw = toHex(b);
            return node;
    }
    while (off < b.length) {
        const t = b[off];
        if (t === 0) {
            off++;
            continue;
        }
        if (off + 2 > b.length)
            break;
        const len = b[off + 1];
        const v = b.subarray(off + 2, off + 2 + len);
        off += 2 + len;
        if (t === 1)
            continue;
        const o = { label: RPL_OPTS[t] ?? `Option ${t}`, fields: [] };
        if (t === 4 && v.length >= 14) {
            o.fields.push(['DIO interval doublings', String(v[1])], ['DIO interval min', String(v[2])], ['DIO redundancy', String(v[3])], ['Max rank increase', String(be16(v, 4))], ['Min hop rank increase', String(be16(v, 6))], ['Objective function', `${be16(v, 8)}${be16(v, 8) === 0 ? ' (OF0)' : be16(v, 8) === 1 ? ' (MRHOF)' : ''}`], ['Default lifetime', `${v[11]} × ${be16(v, 12)} s`]);
            o.summary = `OCP ${be16(v, 8)}, min hop rank increase ${be16(v, 6)}`;
        }
        else if (t === 5 && v.length >= 2) {
            o.summary = prefixOf(v.subarray(2), v[1]);
        }
        else if (t === 6 && v.length >= 4) {
            o.summary = `path sequence ${v[2]}, lifetime ${v[3]}${v.length >= 20 ? `, parent ${addr16(v, 4)}` : ''}`;
        }
        else if (t === 8 && v.length >= 30) {
            o.summary = prefixOf(v.subarray(14), v[0]);
            o.fields.push(['Flags', `${v[1] & 0x80 ? 'on-link ' : ''}${v[1] & 0x40 ? 'autonomous ' : ''}${v[1] & 0x20 ? 'router address' : ''}`.trim() || '(none)'], ['Valid lifetime', lifetime(be32(v, 2))], ['Preferred lifetime', lifetime(be32(v, 6))]);
        }
        else if (v.length)
            o.raw = toHex(v);
        if (!o.fields.length)
            delete o.fields;
        node.children.push(o);
    }
    if (!node.fields.length)
        delete node.fields;
    if (!node.children.length)
        delete node.children;
    return node;
}
export function dissectLwm2m(buf, cf) {
    try {
        const model = lwm2m.decode(buf, cf);
        const node = { label: `LWM2M (${cf === lwm2m.ContentFormat.TLV ? 'TLV' : 'JSON'})`, children: [] };
        const items = Array.isArray(model) ? model : [model];
        for (const item of items)
            node.children.push(modelNode(item));
        return node;
    }
    catch (e) {
        return { label: 'LWM2M', error: String(e), raw: toHex(buf) };
    }
}
function modelNode(m) {
    if (m instanceof lwm2m.ObjectInstance) {
        return { label: `ObjectInstance ${m.id}`, children: m.resources.map(modelNode) };
    }
    if (m.isMulti) {
        return { label: `Resource ${m.id} (multi)`, fields: [...m.instances.entries()].map(([k, v]) => [`[${k}]`, fmtVal(v)]) };
    }
    return { label: `Resource ${m.id}`, summary: fmtVal(m.value) };
}
function fmtVal(v) {
    if (v instanceof Uint8Array) {
        const s = new TextDecoder().decode(v);
        return /^[\x20-\x7e]*$/.test(s) ? `"${s}"` : `0x${toHex(v)}`;
    }
    return String(v);
}
//# sourceMappingURL=dissect.js.map