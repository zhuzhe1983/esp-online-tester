// ---- AES-128 (FIPS-197), encryption only (CCM uses no decryption) ----
const SBOX = (() => {
    const s = new Uint8Array(256);
    let p = 1, q = 1;
    do {
        p = p ^ ((p << 1) & 0xff) ^ (p & 0x80 ? 0x1b : 0);
        q ^= q << 1;
        q ^= q << 2;
        q ^= q << 4;
        q &= 0xff;
        if (q & 0x80)
            q ^= 0x09;
        const x = q ^ ((q << 1) | (q >> 7)) ^ ((q << 2) | (q >> 6)) ^ ((q << 3) | (q >> 5)) ^ ((q << 4) | (q >> 4));
        s[p] = (x ^ 0x63) & 0xff;
    } while (p !== 1);
    s[0] = 0x63;
    return s;
})();
const xtime = (b) => ((b << 1) ^ (b & 0x80 ? 0x1b : 0)) & 0xff;
/** An expanded AES-128 key. */
export class Aes128 {
    rk;
    constructor(key) {
        if (key.length !== 16)
            throw new Error('AES-128 needs a 16-byte key');
        const rk = new Uint8Array(176);
        rk.set(key);
        let rcon = 1;
        for (let i = 16; i < 176; i += 4) {
            let a = rk[i - 4], b = rk[i - 3], c = rk[i - 2], d = rk[i - 1];
            if (i % 16 === 0) {
                [a, b, c, d] = [SBOX[b] ^ rcon, SBOX[c], SBOX[d], SBOX[a]];
                rcon = xtime(rcon);
            }
            rk[i] = rk[i - 16] ^ a;
            rk[i + 1] = rk[i - 15] ^ b;
            rk[i + 2] = rk[i - 14] ^ c;
            rk[i + 3] = rk[i - 13] ^ d;
        }
        this.rk = rk;
    }
    encryptBlock(input) {
        const s = new Uint8Array(16);
        for (let i = 0; i < 16; i++)
            s[i] = input[i] ^ this.rk[i];
        const t = new Uint8Array(16);
        for (let r = 1; r <= 10; r++) {
            // SubBytes + ShiftRows
            for (let c = 0; c < 4; c++)
                for (let row = 0; row < 4; row++)
                    t[c * 4 + row] = SBOX[s[((c + row) % 4) * 4 + row]];
            if (r < 10) {
                // MixColumns
                for (let c = 0; c < 4; c++) {
                    const a0 = t[c * 4], a1 = t[c * 4 + 1], a2 = t[c * 4 + 2], a3 = t[c * 4 + 3];
                    const all = a0 ^ a1 ^ a2 ^ a3;
                    s[c * 4] = a0 ^ all ^ xtime(a0 ^ a1);
                    s[c * 4 + 1] = a1 ^ all ^ xtime(a1 ^ a2);
                    s[c * 4 + 2] = a2 ^ all ^ xtime(a2 ^ a3);
                    s[c * 4 + 3] = a3 ^ all ^ xtime(a3 ^ a0);
                }
            }
            else
                s.set(t);
            for (let i = 0; i < 16; i++)
                s[i] ^= this.rk[r * 16 + i];
        }
        return s;
    }
}
// ---- CCM* (RFC 3610 with L = 2, and M = 0 allowed) ----
/** Decrypt and verify CCM* with a 13-byte nonce.  Returns the plaintext, or
 *  null if the MIC does not match.  With `micLen` 0 nothing is verified. */
export function ccmStarDecrypt(aes, nonce, a, c, mic) {
    const M = mic.length;
    const ctr = (i) => {
        const blk = new Uint8Array(16);
        blk[0] = 1;
        blk.set(nonce, 1);
        blk[14] = i >> 8;
        blk[15] = i & 0xff;
        return aes.encryptBlock(blk);
    };
    const m = new Uint8Array(c.length);
    for (let i = 0; i * 16 < c.length; i++) {
        const s = ctr(i + 1);
        for (let j = 0; j < 16 && i * 16 + j < c.length; j++)
            m[i * 16 + j] = c[i * 16 + j] ^ s[j];
    }
    if (!M)
        return m;
    // CBC-MAC over B0, then the length-prefixed a, then m, each zero-padded to blocks.
    const b0 = new Uint8Array(16);
    b0[0] = (a.length ? 0x40 : 0) | (((M - 2) / 2) << 3) | 1;
    b0.set(nonce, 1);
    b0[14] = m.length >> 8;
    b0[15] = m.length & 0xff;
    let x = aes.encryptBlock(b0);
    const absorb = (data) => {
        for (let i = 0; i < data.length; i += 16) {
            const blk = new Uint8Array(16);
            blk.set(data.subarray(i, i + 16));
            for (let j = 0; j < 16; j++)
                blk[j] ^= x[j];
            x = aes.encryptBlock(blk);
        }
    };
    if (a.length) {
        const la = new Uint8Array(2 + a.length);
        la[0] = a.length >> 8;
        la[1] = a.length & 0xff;
        la.set(a, 2);
        absorb(la);
    }
    absorb(m);
    const s0 = ctr(0);
    for (let j = 0; j < M; j++)
        if ((x[j] ^ s0[j]) !== mic[j])
            return null;
    return m;
}
// ---- SHA-256 and HMAC (FIPS 180-4, RFC 2104) ----
const K256 = new Uint32Array([
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
]);
export function sha256(msg) {
    const h = new Uint32Array([0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19]);
    const len = msg.length;
    const padded = new Uint8Array(((len + 9 + 63) >> 6) << 6);
    padded.set(msg);
    padded[len] = 0x80;
    const bits = len * 8;
    const dv = new DataView(padded.buffer);
    dv.setUint32(padded.length - 8, Math.floor(bits / 2 ** 32));
    dv.setUint32(padded.length - 4, bits >>> 0);
    const w = new Uint32Array(64);
    const rotr = (x, n) => (x >>> n) | (x << (32 - n));
    for (let off = 0; off < padded.length; off += 64) {
        for (let i = 0; i < 16; i++)
            w[i] = dv.getUint32(off + i * 4);
        for (let i = 16; i < 64; i++) {
            const s0 = rotr(w[i - 15], 7) ^ rotr(w[i - 15], 18) ^ (w[i - 15] >>> 3);
            const s1 = rotr(w[i - 2], 17) ^ rotr(w[i - 2], 19) ^ (w[i - 2] >>> 10);
            w[i] = (w[i - 16] + s0 + w[i - 7] + s1) >>> 0;
        }
        let [a, b, c, d, e, f, g, hh] = h;
        for (let i = 0; i < 64; i++) {
            const t1 = (hh + (rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25)) + ((e & f) ^ (~e & g)) + K256[i] + w[i]) >>> 0;
            const t2 = ((rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22)) + ((a & b) ^ (a & c) ^ (b & c))) >>> 0;
            hh = g;
            g = f;
            f = e;
            e = (d + t1) >>> 0;
            d = c;
            c = b;
            b = a;
            a = (t1 + t2) >>> 0;
        }
        h[0] += a;
        h[1] += b;
        h[2] += c;
        h[3] += d;
        h[4] += e;
        h[5] += f;
        h[6] += g;
        h[7] += hh;
    }
    const out = new Uint8Array(32);
    const odv = new DataView(out.buffer);
    for (let i = 0; i < 8; i++)
        odv.setUint32(i * 4, h[i]);
    return out;
}
export function hmacSha256(key, msg) {
    const k = new Uint8Array(64);
    k.set(key.length > 64 ? sha256(key) : key);
    const inner = new Uint8Array(64 + msg.length);
    const outer = new Uint8Array(64 + 32);
    for (let i = 0; i < 64; i++) {
        inner[i] = k[i] ^ 0x36;
        outer[i] = k[i] ^ 0x5c;
    }
    inner.set(msg, 64);
    outer.set(sha256(inner), 64);
    return sha256(outer);
}
// ---- Keys ----
const THREAD = new TextEncoder().encode('Thread');
/** Thread's well-known key for key ID mode 2 frames (key source 0xffffffff,
 *  index 0xff): MLE Discovery and Announce, which a device sends before it
 *  has the network key (OpenThread's Mac::sMode2Key). */
export const THREAD_MODE2_KEY = Uint8Array.of(0x78, 0x58, 0x16, 0x86, 0xfd, 0xb4, 0x58, 0x0f, 0xb0, 0x92, 0x54, 0x6a, 0xec, 0xbd, 0x15, 0x66);
const MODE2 = new Aes128(THREAD_MODE2_KEY);
/** ...and the extended address its nonce uses in place of the sender's (OpenThread's Mac::sMode2ExtAddress). */
export const THREAD_MODE2_EXT_ADDRESS = Uint8Array.of(0x35, 0x06, 0xfe, 0xb8, 0x23, 0xd4, 0x87, 0x12);
/** Thread's MAC and MLE keys for one key sequence (Thread spec 7.1.4):
 *  HMAC-SHA256(network key, sequence || "Thread"); MLE key first, MAC key second. */
export function threadKeys(networkKey, sequence) {
    const msg = new Uint8Array(4 + THREAD.length);
    new DataView(msg.buffer).setUint32(0, sequence >>> 0);
    msg.set(THREAD, 4);
    const h = hmacSha256(networkKey, msg);
    return { mle: h.subarray(0, 16), mac: h.subarray(16, 32) };
}
export function keyBytes(k) {
    if (k instanceof Uint8Array)
        return k;
    const hex = k.replace(/[\s:]/g, '');
    if (!/^[0-9a-fA-F]{32}$/.test(hex))
        throw new Error('a key is 16 bytes of hex');
    return Uint8Array.from(hex.match(/../g).map((x) => parseInt(x, 16)));
}
/** Keys anyone may know, tried after the ones given: 6TiSCH's K1 for
 *  enhanced beacons (RFC 8180 "6TiSCH minimal15"), Contiki-NG's defaults for
 *  TSCH data (TSCH_SECURITY_K2) and for CSMA link-layer security
 *  (CSMA_LLSEC_DEFAULT_KEY0), and the all-zero key. */
export const WELL_KNOWN_802154_KEYS = [
    { key: new TextEncoder().encode('6TiSCH minimal15'), name: '6TiSCH K1 (RFC 8180)', index: 1 },
    { key: Uint8Array.of(0xde, 0xad, 0xbe, 0xef, 0xfa, 0xce, 0xca, 0xfe, 0xde, 0xad, 0xbe, 0xef, 0xfa, 0xce, 0xca, 0xfe), name: "Contiki-NG's default TSCH K2", index: 2 },
    { key: Uint8Array.from({ length: 16 }, (_, i) => 0x10 + i), name: "Contiki-NG's default CSMA LLSEC key" },
    { key: new Uint8Array(16), name: 'all-zero key' },
];
const WELL_KNOWN = WELL_KNOWN_802154_KEYS.map((k) => ({ aes: new Aes128(k.key), name: k.name, index: k.index }));
/** The 802.15.4 CCM* nonce: extended source address, frame counter, level. */
function nonce(ext, counter, level) {
    const n = new Uint8Array(13);
    n.set(ext);
    new DataView(n.buffer).setUint32(8, counter >>> 0);
    n[12] = level;
    return n;
}
/** TSCH's nonce (802.15.4e): extended source address and the 5-byte ASN. */
function asnNonce(ext, asn) {
    const n = new Uint8Array(13);
    n.set(ext);
    n[8] = Math.floor(asn / 2 ** 32) & 0xff;
    new DataView(n.buffer).setUint32(9, asn >>> 0);
    return n;
}
/** The keys a capture is read with, and what it has learned along the way:
 *  extended addresses, which short-addressed frames' nonces need. */
export class KeyStore {
    threadKey;
    macKeys = [];
    derived = new Map();
    extAddrs = new Map();
    /** Short address (with PAN) → the extended address it last decrypted with. */
    shortToExt = new Map();
    tsch = null;
    /** The previous frame's destination: the likely sender of an ACK, which names none. */
    lastDst = null;
    constructor(opts = {}) {
        if (opts.threadNetworkKey)
            this.threadKey = keyBytes(opts.threadNetworkKey);
        (opts.macKeys ?? []).forEach((k, i) => this.macKeys.push({ aes: new Aes128(keyBytes(k)), name: `key ${i + 1}` }));
    }
    /** Whether no key was given (the well-known mode 2 key is always tried). */
    get empty() {
        return !this.threadKey && !this.macKeys.length;
    }
    /** Remember an extended address seen as a frame's source (network order). */
    learn(ext) {
        if (ext.length === 8)
            this.extAddrs.set(ext.join(','), Uint8Array.from(ext));
    }
    /** A frame's destination, kept for the ACK that may follow. */
    noteDestination(dst) {
        this.lastDst = dst && dst.length === 8 ? dst : null;
    }
    /** An enhanced beacon's ASN, sent at `ts` seconds: TSCH frames' nonces use
     *  the ASN, which the capture's timestamps then give for every later frame.
     *  `txOffset` and `slot` are in microseconds (the timeslot template's). */
    noteTschBeacon(asn, ts, txOffset = 2120, slot = 10000) {
        this.tsch = { asn, slotStart: ts - txOffset / 1e6, slot: slot / 1e6 };
    }
    thread(seq) {
        if (!this.threadKey)
            return null;
        let k = this.derived.get(seq);
        if (!k) {
            const t = threadKeys(this.threadKey, seq);
            k = { mle: new Aes128(t.mle), mac: new Aes128(t.mac) };
            this.derived.set(seq, k);
        }
        return k;
    }
    /** Decrypt a secured MAC frame's payload.  `a` is the authenticated part
     *  (the MAC header through the auxiliary security header and any header IEs),
     *  `c` the ciphertext, `mic` the MIC; `src` the source address in network
     *  order (short or extended), `pan` the source PAN. */
    decryptMac(aux, a, c, mic, src, pan, ts) {
        if (!mic.length)
            return null; // nothing to verify a guess against
        const keys = [];
        if (aux.keyIdMode === 1 && aux.keyIndex != null) {
            // Thread: key index = (sequence mod 128) + 1.
            const k = this.thread((aux.keyIndex - 1) & 0x7f);
            if (k)
                keys.push({ aes: k.mac, name: `Thread MAC key, sequence ${(aux.keyIndex - 1) & 0x7f}` });
        }
        else if (aux.keyIdMode === 2 && aux.keySource) {
            const seq = new DataView(aux.keySource.buffer, aux.keySource.byteOffset).getUint32(0);
            if (seq === 0xffffffff && aux.keyIndex === 0xff)
                keys.push({ aes: MODE2, name: 'Thread key ID mode 2 (well-known)', ext: THREAD_MODE2_EXT_ADDRESS });
            else {
                const k = this.thread(seq);
                if (k)
                    keys.push({ aes: k.mac, name: `Thread MAC key, sequence ${seq}` });
            }
        }
        keys.push(...this.macKeys);
        for (const w of WELL_KNOWN)
            if (w.index === undefined || w.index === aux.keyIndex || aux.keyIdMode === 0)
                keys.push(w);
        let sources;
        const shortKey = src && src.length === 2 ? `${pan ?? ''}/${src.join(',')}` : '';
        if (src && src.length === 8)
            sources = [src];
        else {
            const known = shortKey ? this.shortToExt.get(shortKey) : undefined;
            // No source address (an Enh-ACK): the previous frame's destination sent it.
            sources = [...(known ? [known] : []), ...(!src && this.lastDst ? [this.lastDst] : []), ...this.extAddrs.values()];
        }
        // Nonces: the frame counter's, and in TSCH the ASN's, which the slot the
        // frame was sent in gives (the neighbouring slots too, for clock drift).
        const counters = [];
        if (aux.frameCounter != null)
            counters.push((ext) => nonce(ext, aux.frameCounter, aux.level));
        if (this.tsch && ts !== undefined) {
            const base = this.tsch.asn + Math.floor((ts - this.tsch.slotStart) / this.tsch.slot);
            for (const asn of [base, base - 1, base + 1])
                if (asn >= 0)
                    counters.push((ext) => asnNonce(ext, asn));
        }
        for (const k of keys) {
            for (const ext of k.ext ? [k.ext] : sources) {
                for (const n of counters) {
                    const p = ccmStarDecrypt(k.aes, n(ext), a, c, mic);
                    if (p) {
                        if (shortKey && !k.ext)
                            this.shortToExt.set(shortKey, ext);
                        return { plaintext: p, key: k.name, source: ext };
                    }
                }
            }
        }
        return null;
    }
    /** Decrypt a secured MLE message (UDP 19788, security suite 0).  The nonce's
     *  address is the IPv6 source's IID; the IPv6 addresses are authenticated. */
    decryptMle(payload, src, dst) {
        if (payload[0] !== 0 || payload.length < 11)
            return null;
        const ctl = payload[1];
        const level = ctl & 7, keyIdMode = (ctl >> 3) & 3;
        if (keyIdMode !== 2 || payload.length < 1 + 10 + 4)
            return null;
        const auxLen = 10; // control, frame counter, 4-byte key source, key index
        const counter = new DataView(payload.buffer, payload.byteOffset + 2).getUint32(0, true);
        const seq = new DataView(payload.buffer, payload.byteOffset + 6).getUint32(0);
        const k = this.thread(seq);
        if (!k)
            return null;
        const micLen = [0, 4, 8, 16][level & 3];
        const ext = src.slice(8, 16);
        ext[0] ^= 0x02;
        const a = new Uint8Array(32 + auxLen);
        a.set(src);
        a.set(dst, 16);
        a.set(payload.subarray(1, 1 + auxLen), 32);
        const c = payload.subarray(1 + auxLen, payload.length - micLen);
        const mic = payload.subarray(payload.length - micLen);
        const p = ccmStarDecrypt(k.mle, nonce(ext, counter, level), a, c, mic);
        if (p)
            this.learn(ext);
        return p;
    }
}
//# sourceMappingURL=security.js.map