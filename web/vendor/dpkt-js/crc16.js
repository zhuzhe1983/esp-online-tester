// CRC-16 CCITT — byte-exact port of Contiki-NG's os/lib/crc16.c, as used by the
// drone-test `serialradio` framing (CBOR payload + 2-byte little-endian CRC).
import { concat } from './packet.js';
export function crc16Add(byte, crc) {
    crc &= 0xffff;
    byte &= 0xff;
    crc ^= byte;
    crc = ((crc >> 8) | (crc << 8)) & 0xffff;
    crc ^= ((crc & 0xff00) << 4) & 0xffff;
    crc ^= (crc >> 8) >> 4;
    crc ^= ((crc & 0xff00) >> 5) & 0xffff;
    return crc & 0xffff;
}
export function crc16Data(data, initial = 0) {
    let crc = initial & 0xffff;
    for (const b of data)
        crc = crc16Add(b, crc);
    return crc;
}
/** Append the CRC-16 little-endian (2 bytes) to a payload. */
export function crc16Append(data) {
    const crc = crc16Data(data);
    return concat(data, Uint8Array.of(crc & 0xff, (crc >> 8) & 0xff));
}
/** Verify a payload that has a 2-byte little-endian CRC appended. */
export function crc16Verify(data) {
    if (data.length < 2)
        return false;
    const payload = data.subarray(0, data.length - 2);
    const rx = data[data.length - 2] | (data[data.length - 1] << 8);
    return rx === crc16Data(payload);
}
//# sourceMappingURL=crc16.js.map