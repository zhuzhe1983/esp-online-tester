import { toHex } from './util.js';
const CONTENT_TYPES = {
    20: 'Change Cipher Spec', 21: 'Alert', 22: 'Handshake', 23: 'Application Data', 24: 'Heartbeat', 25: 'ACK',
};
const VERSIONS = { 0xfeff: 'DTLS 1.0', 0xfefd: 'DTLS 1.2', 0xfefc: 'DTLS 1.3' };
const HANDSHAKE = {
    0: 'Hello Request', 1: 'Client Hello', 2: 'Server Hello', 3: 'Hello Verify Request', 4: 'New Session Ticket',
    11: 'Certificate', 12: 'Server Key Exchange', 13: 'Certificate Request', 14: 'Server Hello Done',
    15: 'Certificate Verify', 16: 'Client Key Exchange', 20: 'Finished',
};
const ALERT_LEVELS = { 1: 'warning', 2: 'fatal' };
const be16 = (b, o) => (b[o] << 8) | b[o + 1];
const be24 = (b, o) => (b[o] << 16) | (b[o + 1] << 8) | b[o + 2];
/** Whether a UDP payload starts with a well-formed DTLS record. */
export function looksLikeDtls(b) {
    return b.length >= 13 && b[0] >= 20 && b[0] <= 25 && b[1] === 0xfe && (b[2] === 0xff || b[2] === 0xfd || b[2] === 0xfc) &&
        13 + be16(b, 11) <= b.length;
}
export function dissectDtls(b) {
    const node = { label: 'DTLS', children: [] };
    const names = [];
    let off = 0;
    while (off + 13 <= b.length) {
        const ct = b[off], ver = be16(b, off + 1), epoch = be16(b, off + 3);
        const seq = be16(b, off + 5) * 2 ** 32 + ((b[off + 7] << 24) >>> 0) + (b[off + 8] << 16) + (b[off + 9] << 8) + b[off + 10];
        const len = be16(b, off + 11);
        if (off + 13 + len > b.length) {
            node.children.push({ label: 'Record', error: `length ${len} runs past the datagram`, raw: toHex(b.subarray(off)) });
            break;
        }
        const body = b.subarray(off + 13, off + 13 + len);
        const ctName = CONTENT_TYPES[ct] ?? `content type ${ct}`;
        const rec = {
            label: `${ctName}`,
            fields: [['Version', VERSIONS[ver] ?? `0x${ver.toString(16)}`], ['Epoch', String(epoch)], ['Sequence', String(seq)], ['Length', String(len)]],
            children: [],
        };
        let name = ctName;
        if (epoch > 0 && ct !== 20) {
            name = ct === 23 ? 'Application Data' : `Encrypted ${ctName}`;
            rec.summary = `${len} bytes, encrypted`;
            rec.raw = toHex(body);
        }
        else if (ct === 22) {
            const msgs = [];
            let h = 0;
            while (h + 12 <= body.length) {
                const t = body[h], mlen = be24(body, h + 1), fragOff = be24(body, h + 6), fragLen = be24(body, h + 9);
                const hs = HANDSHAKE[t] ?? `handshake type ${t}`;
                msgs.push(hs);
                rec.children.push({
                    label: hs,
                    fields: [['Length', String(mlen)], ['Message sequence', String(be16(body, h + 4))],
                        ['Fragment', fragOff || fragLen !== mlen ? `${fragOff}+${fragLen} of ${mlen}` : 'whole']],
                    raw: toHex(body.subarray(h + 12, h + 12 + fragLen)),
                });
                h += 12 + fragLen;
            }
            name = msgs.join(', ') || ctName;
            rec.summary = name;
        }
        else if (ct === 21 && body.length >= 2) {
            rec.summary = `${ALERT_LEVELS[body[0]] ?? body[0]}, description ${body[1]}`;
        }
        else if (body.length)
            rec.raw = toHex(body);
        if (!rec.children.length)
            delete rec.children;
        names.push(name);
        node.children.push(rec);
        off += 13 + len;
    }
    node.summary = names.join(', ');
    return node;
}
//# sourceMappingURL=dtls.js.map