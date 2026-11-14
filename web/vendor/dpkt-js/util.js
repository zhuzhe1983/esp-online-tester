// Small byte helpers (browser + node safe; no Buffer dependency).
export function fromHex(hex) {
    const clean = hex.replace(/\s+/g, '');
    const out = new Uint8Array(clean.length / 2);
    for (let i = 0; i < out.length; i++)
        out[i] = parseInt(clean.substr(i * 2, 2), 16);
    return out;
}
export function toHex(buf) {
    return Array.from(buf, (b) => b.toString(16).padStart(2, '0')).join('');
}
export function bytesEqual(a, b) {
    if (a.length !== b.length)
        return false;
    for (let i = 0; i < a.length; i++)
        if (a[i] !== b[i])
            return false;
    return true;
}
const ASCII = (b) => (b >= 0x20 && b < 0x7f ? String.fromCharCode(b) : '.');
export function hexdump(buf, width = 16) {
    const lines = [];
    for (let i = 0; i < buf.length; i += width) {
        const slice = buf.subarray(i, i + width);
        const hex = Array.from(slice, (b) => b.toString(16).padStart(2, '0')).join(' ');
        const ascii = Array.from(slice, ASCII).join('');
        lines.push(`  ${i.toString().padStart(4, '0')}:  ${hex.padEnd(width * 3)} ${ascii}`);
    }
    return lines.join('\n');
}
export function decodeUtf8(buf) {
    return new TextDecoder().decode(buf);
}
//# sourceMappingURL=util.js.map