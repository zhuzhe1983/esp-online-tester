#!/usr/bin/env python3
"""Write the host JPEG inputs of the classic JPEG fixture; standard library only.

Each file is a baseline (SOF0) single-component JPEG: unit quantisation, the
ITU-T T.81 Annex K.3 luminance Huffman tables, and one DC value per 8x8 block
with no AC coefficients (every block is flat). These are host inputs, not an
emulator encoder; the emulator delivers them byte for byte.

usage: make-jpeg.py OUTPUT_DIR   (prints name, size, length and SHA-256 as JSON)
"""
import hashlib
import json
from pathlib import Path
import sys

DC_BITS = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0]
DC_VALS = list(range(12))
AC_BITS = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7d]
AC_VALS = bytes.fromhex(
    '01020300041105122131410613516107227114328191a1082342b1c11552d1f0'
    '2433627282090a161718191a25262728292a3435363738393a434445464748494a'
    '535455565758595a636465666768696a737475767778797a838485868788898a'
    '92939495969798999aa2a3a4a5a6a7a8a9aab2b3b4b5b6b7b8b9bac2c3c4c5c6c7'
    'c8c9cad2d3d4d5d6d7d8d9dae1e2e3e4e5e6e7e8e9eaf1f2f3f4f5f6f7f8f9fa')
assert len(AC_VALS) == 162


def codes(bits, vals):
    table, code, k = {}, 0, 0
    for length, count in enumerate(bits, 1):
        for _ in range(count):
            table[vals[k]] = (code, length)
            code, k = code + 1, k + 1
        code <<= 1
    return table


DC, AC = codes(DC_BITS, DC_VALS), codes(AC_BITS, AC_VALS)


class Bits:
    def __init__(self):
        self.out, self.acc, self.n = bytearray(), 0, 0

    def put(self, value, length):
        for i in range(length - 1, -1, -1):
            self.acc = (self.acc << 1) | ((value >> i) & 1)
            self.n += 1
            if self.n == 8:
                self.out.append(self.acc)
                if self.acc == 0xff:
                    self.out.append(0)  # byte stuffing
                self.acc, self.n = 0, 0

    def flush(self):
        while self.n:
            self.put(1, 1)  # pad with one bits
        return bytes(self.out)


def segment(marker, body):
    return bytes([0xff, marker]) + (len(body) + 2).to_bytes(2, 'big') + body


def jpeg(w, h, level):
    """level(bx, by) -> 0..255 grey level of the 8x8 block at column bx, row by."""
    scan, prev = Bits(), 0
    for by in range((h + 7) // 8):
        for bx in range((w + 7) // 8):
            dc = (level(bx, by) - 128) * 8  # unit quantisation: pixel = dc / 8 + 128
            diff, prev = dc - prev, dc
            size = abs(diff).bit_length()
            scan.put(*DC[size])
            if size:
                scan.put(diff if diff > 0 else diff + (1 << size) - 1, size)
            scan.put(*AC[0x00])  # EOB: no AC coefficients
    return b''.join([
        b'\xff\xd8',
        segment(0xe0, b'JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00'),
        segment(0xdb, b'\x00' + bytes([1] * 64)),
        segment(0xc0, bytes([8]) + h.to_bytes(2, 'big') + w.to_bytes(2, 'big') + b'\x01\x01\x11\x00'),
        segment(0xc4, b'\x00' + bytes(DC_BITS) + bytes(DC_VALS)),
        segment(0xc4, b'\x10' + bytes(AC_BITS) + AC_VALS),
        segment(0xda, b'\x01\x01\x00\x00\x3f\x00'),
        scan.flush(),
        b'\xff\xd9',
    ])


IMAGES = {
    'qvga-a.jpg': (320, 240, lambda x, y: (x * 6 + y * 3) % 256),
    'qvga-b.jpg': (320, 240, lambda x, y: 255 - (x * 6 + y * 3) % 256),
    'vga-a.jpg': (640, 480, lambda x, y: ((x // 4) ^ (y // 4)) * 37 % 256),
    'vga-b.jpg': (640, 480, lambda x, y: (x * y) % 256),
}

if __name__ == '__main__':
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    rows = []
    for name, (w, h, level) in IMAGES.items():
        data = jpeg(w, h, level)
        (out / name).write_bytes(data)
        rows.append(dict(file=name, width=w, height=h, bytes=len(data), sha256=hashlib.sha256(data).hexdigest()))
    print(json.dumps(rows, indent=2))
