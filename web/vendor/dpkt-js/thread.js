import { parseAuxSecurityHeader } from './ieee802154.js';
import { toHex } from './util.js';
export const MLE_PORT = 19788;
/** Thread Management Framework: CoAP. */
export const TMF_PORT = 61631;
/** MeshCoP commissioning (DTLS). */
export const MESHCOP_PORT = 49191;
export const MLE_COMMANDS = {
    0: 'Link Request', 1: 'Link Accept', 2: 'Link Accept and Request', 3: 'Link Reject', 4: 'Advertisement',
    5: 'Update', 6: 'Update Request', 7: 'Data Request', 8: 'Data Response', 9: 'Parent Request',
    10: 'Parent Response', 11: 'Child ID Request', 12: 'Child ID Response', 13: 'Child Update Request',
    14: 'Child Update Response', 15: 'Announce', 16: 'Discovery Request', 17: 'Discovery Response',
    18: 'Link Metrics Management Request', 19: 'Link Metrics Management Response', 20: 'Link Probe', 21: 'Time Sync',
};
export const MLE_TLVS = {
    0: 'Source Address', 1: 'Mode', 2: 'Timeout', 3: 'Challenge', 4: 'Response', 5: 'Link-layer Frame Counter',
    6: 'Link Quality', 7: 'Network Parameter', 8: 'MLE Frame Counter', 9: 'Route64', 10: 'Address16',
    11: 'Leader Data', 12: 'Network Data', 13: 'TLV Request', 14: 'Scan Mask', 15: 'Connectivity',
    16: 'Link Margin', 17: 'Status', 18: 'Version', 19: 'Address Registration', 20: 'Channel', 21: 'PAN ID',
    22: 'Active Timestamp', 23: 'Pending Timestamp', 24: 'Active Operational Dataset',
    25: 'Pending Operational Dataset', 26: 'Thread Discovery', 27: 'Supervision Interval',
    74: 'CSL Channel', 85: 'CSL Synchronized Timeout', 86: 'CSL Clock Accuracy',
    87: 'Link Metrics Query', 88: 'Link Metrics Management', 89: 'Link Metrics Report', 90: 'Link Probe',
};
export const MESHCOP_TLVS = {
    0: 'Channel', 1: 'PAN ID', 2: 'Extended PAN ID', 3: 'Network Name', 4: 'PSKc', 5: 'Network Key',
    6: 'Network Key Sequence', 7: 'Mesh-Local Prefix', 8: 'Steering Data', 9: 'Border Agent Locator',
    10: 'Commissioner ID', 11: 'Commissioner Session ID', 12: 'Security Policy', 13: 'Get', 14: 'Active Timestamp',
    15: 'Commissioner UDP Port', 16: 'State', 17: 'Joiner DTLS Encapsulation', 18: 'Joiner UDP Port',
    19: 'Joiner IID', 20: 'Joiner Router Locator', 21: 'Joiner Router KEK', 51: 'Pending Timestamp',
    52: 'Delay Timer', 53: 'Channel Mask', 128: 'Discovery Request', 129: 'Discovery Response',
};
const be16 = (b, o) => (b[o] << 8) | b[o + 1];
const be32 = (b, o) => ((b[o] << 24) >>> 0) + (b[o + 1] << 16) + (b[o + 2] << 8) + b[o + 3];
const hex16 = (v) => `0x${v.toString(16).padStart(4, '0')}`;
/** Dissect an MLE message.  `plaintext` is the decrypted command and TLVs
 *  of a secured message, when a key was known. */
export function dissectMle(b, plaintext) {
    if (!b.length)
        return { label: 'MLE', error: 'empty' };
    if (b[0] === 255) {
        const cmd = mleCommandNode(b.subarray(1));
        return { label: 'MLE', summary: cmd.label, fields: [['Security suite', '255 (none)']], children: [cmd] };
    }
    if (b[0] !== 0)
        return { label: 'MLE', error: `unknown security suite ${b[0]}`, raw: toHex(b) };
    const aux = parseAuxSecurityHeader(b.subarray(1));
    if (!aux)
        return { label: 'MLE', error: 'security header truncated', raw: toHex(b) };
    const body = b.subarray(1 + aux.length, b.length - aux.micLength);
    const mic = b.subarray(b.length - aux.micLength);
    const node = {
        label: 'MLE',
        fields: [['Security suite', '0 (802.15.4 security)'], ['Frame counter', String(aux.frameCounter)],
            ['Key', aux.keySource ? `sequence ${be32(aux.keySource, 0)}, index ${aux.keyIndex}` : `index ${aux.keyIndex}`],
            ['MIC', toHex(mic)]],
        children: [],
    };
    if (plaintext) {
        const cmd = mleCommandNode(plaintext);
        node.summary = cmd.label;
        node.children.push(cmd);
    }
    else {
        node.summary = `secured, ${body.length} bytes`;
        node.children.push({ label: 'Encrypted MLE command', summary: `${body.length} bytes`, raw: toHex(body) });
    }
    return node;
}
/** The command byte and its TLVs. */
export function mleCommandNode(b) {
    if (!b.length)
        return { label: 'MLE command', error: 'missing' };
    const name = MLE_COMMANDS[b[0]] ?? `Command ${b[0]}`;
    const node = { label: name, children: tlvNodes(b.subarray(1), MLE_TLVS, mleTlvValue) };
    return node;
}
function tlvNodes(b, names, decode) {
    const out = [];
    let off = 0;
    while (off + 2 <= b.length) {
        const t = b[off];
        let len = b[off + 1];
        let hdr = 2;
        if (len === 0xff && off + 4 <= b.length) {
            len = be16(b, off + 2);
            hdr = 4;
        } // MeshCoP extended length
        if (off + hdr + len > b.length) {
            out.push({ label: names[t] ?? `TLV ${t}`, error: `length ${len} runs past the message`, raw: toHex(b.subarray(off)) });
            break;
        }
        const v = b.subarray(off + hdr, off + hdr + len);
        const node = { label: names[t] ?? `TLV ${t}` };
        let d = null;
        try {
            d = decode(t, v);
        }
        catch {
            d = null;
        }
        if (d)
            Object.assign(node, d);
        if (!d || (!d.summary && !d.fields && !d.children))
            node.raw = toHex(v);
        out.push(node);
        off += hdr + len;
    }
    if (off < b.length)
        out.push({ label: 'Trailing bytes', raw: toHex(b.subarray(off)) });
    return out;
}
function timestamp(v) {
    const secs = v[0] * 2 ** 40 + v[1] * 2 ** 32 + be32(v, 2);
    const ticks = be16(v, 6) >> 1;
    return `${secs} s + ${ticks}/32768${v[7] & 1 ? ', authoritative' : ''}`;
}
function mleTlvValue(t, v) {
    switch (t) {
        case 0:
        case 10: return { summary: hex16(be16(v, 0)) };
        case 1: {
            const m = v[0];
            const flags = [m & 0x08 ? 'rx-on-when-idle' : 'sleepy', m & 0x02 ? 'full Thread device' : 'minimal', m & 0x01 ? 'full network data' : 'stable network data'];
            return { summary: `0x${m.toString(16).padStart(2, '0')} (${flags.join(', ')})` };
        }
        case 2:
        case 5:
        case 8: return { summary: String(be32(v, 0)) };
        case 3:
        case 4: return { summary: toHex(v) };
        case 9: {
            const mask = v.subarray(1, 9);
            let n = 0;
            for (const x of mask)
                for (let i = 0; i < 8; i++)
                    if (x & (1 << i))
                        n++;
            return { summary: `${n} router${n === 1 ? '' : 's'}, ID sequence ${v[0]}`, fields: [['Router mask', toHex(mask)], ['Route data', toHex(v.subarray(9))]] };
        }
        case 11: return {
            summary: `partition 0x${be32(v, 0).toString(16)}, leader router ${v[7]}`,
            fields: [['Partition ID', `0x${be32(v, 0).toString(16)}`], ['Weighting', String(v[4])], ['Data version', String(v[5])],
                ['Stable data version', String(v[6])], ['Leader router ID', String(v[7])]],
        };
        case 13: return { summary: [...v].map((x) => MLE_TLVS[x] ?? String(x)).join(', ') };
        case 14: return { summary: [v[0] & 0x80 ? 'routers' : '', v[0] & 0x40 ? 'end devices' : ''].filter(Boolean).join(' and ') || 'none' };
        case 15: return {
            fields: [['Parent priority', String(((v[0] >> 6) & 3) - (v[0] & 0x80 ? 4 : 0))], ['Link quality 3/2/1', `${v[1]}/${v[2]}/${v[3]}`],
                ['Leader cost', String(v[4])], ['ID sequence', String(v[5])], ['Active routers', String(v[6])]],
        };
        case 16: return { summary: `${v[0]} dB` };
        case 17: return { summary: String(v[0]) };
        case 18: return { summary: String(be16(v, 0)) };
        case 20: return { summary: `page ${v[0]}, channel ${be16(v, 1)}` };
        case 21: return { summary: hex16(be16(v, 0)) };
        case 22:
        case 23: return { summary: timestamp(v) };
        case 24:
        case 25:
        case 26: return { children: tlvNodes(v, MESHCOP_TLVS, meshcopTlvValue) };
        case 27: return { summary: `${be16(v, 0)} s` };
        default: return null;
    }
}
function meshcopTlvValue(t, v) {
    switch (t) {
        case 0: return { summary: `page ${v[0]}, channel ${be16(v, 1)}` };
        case 1: return { summary: hex16(be16(v, 0)) };
        case 2:
        case 4:
        case 5:
        case 8: return { summary: toHex(v) };
        case 3:
        case 10: return { summary: `"${new TextDecoder().decode(v)}"` };
        case 7: {
            const g = [];
            for (let i = 0; i < 8; i += 2)
                g.push(be16(v, i).toString(16));
            return { summary: `${g.join(':')}::/64` };
        }
        case 9:
        case 15:
        case 18: return { summary: String(be16(v, 0)) };
        case 14:
        case 51: return { summary: timestamp(v) };
        case 128: return { summary: `version ${v[0] >> 4}${v[0] & 0x08 ? ', joiner' : ''}` };
        case 129: return { summary: `version ${v[0] >> 4}${v[0] & 0x08 ? ', native commissioner' : ''}` };
        default: return null;
    }
}
/** Thread's network-layer TLVs, which TMF messages (/a/..., /n/...) carry. */
export const TMF_TLVS = {
    0: 'Target EID', 1: 'MAC Extended Address', 2: 'RLOC16', 3: 'ML-EID', 4: 'Status', 6: 'Last Transaction Time',
    7: 'Router Mask', 8: 'ND Option', 9: 'ND Data', 10: 'Thread Network Data', 11: 'Timeout', 12: 'Network Name',
    14: 'IPv6 Addresses', 15: 'Commissioner Session ID', 16: 'Extended PAN ID', 17: 'Joiner Router Locator',
};
/** The TMF URIs, by what they do. */
export const TMF_URIS = {
    '/a/aq': 'Address Query', '/a/an': 'Address Notification', '/a/ae': 'Address Error', '/a/as': 'Address Solicit',
    '/a/ar': 'Address Release', '/a/sd': 'Server Data', '/a/sl': 'Anycast Locate', '/n/dn': 'Network Data',
    '/n/mr': 'MLR', '/c/la': 'Leader Get', '/c/ls': 'Leader Set', '/c/lp': 'Leader Petition', '/c/lk': 'Leader Keep-Alive',
    '/c/ag': 'Active Get', '/c/as': 'Active Set', '/c/pg': 'Pending Get', '/c/ps': 'Pending Set',
    '/c/rx': 'Relay RX', '/c/tx': 'Relay TX', '/c/ur': 'Udp Proxy', '/d/dg': 'Diagnostic Get', '/d/dr': 'Diagnostic Reset',
};
/** Decode a TMF message's payload by its URI; null for one it doesn't know. */
export function tmfPayloadNodes(uri, b) {
    if (uri.startsWith('/c/'))
        return tlvNodes(b, MESHCOP_TLVS, meshcopTlvValue);
    if (uri.startsWith('/a/') || uri.startsWith('/n/') || uri === '')
        return tlvNodes(b, TMF_TLVS, tmfTlvValue);
    return null;
}
function ip6(v) {
    const g = [];
    for (let i = 0; i < v.length; i += 2)
        g.push(be16(v, i).toString(16));
    return g.join(':').replace(/(^|:)0(:0)+(:|$)/, '::');
}
function tmfTlvValue(t, v) {
    switch (t) {
        case 0: return { summary: v.length === 16 ? ip6(v) : toHex(v) };
        case 1:
        case 3: return { summary: [...v].map((x) => x.toString(16).padStart(2, '0')).join(':') };
        case 2: return { summary: hex16(be16(v, 0)) };
        case 4: return { summary: `${v[0]}${v[0] === 0 ? ' (success)' : v[0] === 1 ? ' (no address available)' : ''}` };
        case 6:
        case 11: return { summary: `${be32(v, 0)} s` };
        case 7: {
            let n = 0;
            for (const x of v.subarray(1))
                for (let i = 0; i < 8; i++)
                    if (x & (1 << i))
                        n++;
            return { summary: `${n} router${n === 1 ? '' : 's'}, ID sequence ${v[0]}` };
        }
        case 14: {
            const a = [];
            for (let i = 0; i + 16 <= v.length; i += 16)
                a.push(ip6(v.subarray(i, i + 16)));
            return { summary: a.join(', ') };
        }
        default: return null;
    }
}
//# sourceMappingURL=thread.js.map