#!/usr/bin/env python3
"""Reproduce original S2 SRAM probes without downloading a compiler or firmware.

The emitted Xtensa integer instructions use the S2 GPIO/APB map and real
software SPI edges. No host framebuffer writes or ROM stubs are involved.
Encodings use the same repository assembler convention as lilygo-s3-probe.mjs.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct

IRAM = 0x40021000
POOL = 1024
ENTRY = IRAM + POOL
DRAM = 0x3FFB0100
MARKER = 0x3FFFF000
GPIO = 0x3F404000
MUX = 0x3F409004
UART = 0x3F400000


class Assembler:
    def __init__(self):
        self.words = []
        self.labels = {}
        self.fixups = []
        self.literals = {}

    def emit(self, word):
        self.words.append(word & 0xFFFFFF)

    def label(self, name):
        assert name not in self.labels
        self.labels[name] = ENTRY + len(self.words) * 3

    def ref(self, kind, name, **args):
        self.fixups.append((len(self.words), kind, name, args))
        self.emit(0)

    def li(self, register, value):
        value &= 0xFFFFFFFF
        if value not in self.literals:
            self.literals[value] = len(self.literals) * 4
        self.ref("literal", value, register=register)

    def rri8(self, r, s, t, value):
        self.emit(2 | (t << 4) | (s << 8) | (r << 12) | ((value & 255) << 16))

    def load(self, value, base, offset=0):
        assert offset % 4 == 0 and 0 <= offset <= 1020
        self.rri8(2, base, value, offset // 4)

    def store(self, value, base, offset=0):
        assert offset % 4 == 0 and 0 <= offset <= 1020
        self.rri8(6, base, value, offset // 4)

    def addi(self, target, source, value):
        assert -128 <= value <= 127
        self.rri8(12, source, target, value)

    def bitop(self, target, a, b, op):
        self.emit((b << 4) | (a << 8) | (target << 12) | (op << 20))

    def extui(self, target, source, shift, width):
        assert 0 <= shift <= 31 and 1 <= width <= 16
        self.emit((source << 4) | ((shift & 15) << 8) | (target << 12)
                  | ((4 | (shift >> 4)) << 16) | ((width - 1) << 20))

    def slli(self, target, source, shift):
        encoded = 32 - shift
        assert 1 <= shift <= 31
        self.emit(((encoded & 15) << 4) | (source << 8) | (target << 12)
                  | (1 << 16) | ((encoded >> 4) << 20))

    def jump(self, name):
        self.ref("jump", name)

    def bz(self, name, register, nonzero=False):
        self.ref("zero", name, register=register, nonzero=nonzero)

    def beq(self, name, a, b):
        self.ref("equal", name, a=a, b=b)

    def write(self, address, value):
        self.li(3, address)
        self.li(2, value)
        self.store(2, 3)

    def finish(self):
        assert len(self.literals) * 4 <= POOL
        for at, kind, name, args in self.fixups:
            pc = ENTRY + at * 3
            if kind == "literal":
                offset = IRAM + self.literals[name] - ((pc + 3) & ~3)
                assert offset % 4 == 0 and -262144 <= offset <= -4
                word = 1 | (args["register"] << 4) | (((offset >> 2) & 65535) << 8)
            else:
                offset = self.labels[name] - (pc + 4)
                if kind == "jump":
                    assert -131072 <= offset <= 131071
                    word = 6 | ((offset & 0x3FFFF) << 6)
                elif kind == "equal":
                    assert -128 <= offset <= 127
                    word = 7 | (args["b"] << 4) | (args["a"] << 8) | (1 << 12) | ((offset & 255) << 16)
                else:
                    assert -2048 <= offset <= 2047
                    word = 0x16 | (int(args["nonzero"]) << 6) | (args["register"] << 8) | ((offset & 4095) << 12)
            self.words[at] = word & 0xFFFFFF
        data = bytearray((POOL + len(self.words) * 3 + 3) & ~3)
        for value, offset in self.literals.items():
            struct.pack_into("<I", data, offset, value)
        for index, word in enumerate(self.words):
            data[POOL + index * 3:POOL + index * 3 + 3] = word.to_bytes(3, "little")
        return data


def probe(screen):
    a = Assembler()
    serial_byte = 0

    def uart(text):
        for value in text.encode("ascii"):
            a.li(7, value)
            a.store(7, 13)

    def spi_byte(register, dc):
        # a12=GPIO, a15=LED latch. a8..a11 remain live across byte loops.
        nonlocal serial_byte
        label = "bit-" + str(serial_byte)
        serial_byte += 1
        a.addi(4, register, 0)
        a.li(5, 8)
        a.li(6, 0x42 | (int(dc) << 5))  # BL33/RST38 high, CS34/CLK36 low
        a.bitop(6, 6, 15, 2)
        a.label(label)
        a.extui(7, 4, 7, 1)
        a.slli(7, 7, 3)  # MOSI35 is bit3 in OUT1
        a.bitop(3, 6, 7, 2)
        a.store(3, 12, 0x10)
        a.addi(3, 3, 16)  # rising CLK36 edge
        a.store(3, 12, 0x10)
        a.slli(4, 4, 1)
        a.addi(5, 5, -1)
        a.bz(label, 5, True)

    def command(value, data=()):
        a.li(4, value)
        spi_byte(4, False)
        for value in data:
            a.li(4, value)
            spi_byte(4, True)

    # Function1=GPIO; actual S2 IO_MUX offsets, input enable and pullup on BOOT0.
    a.write(MUX, (1 << 12) | (1 << 9) | (1 << 8))
    if screen:
        for pin in range(33, 40):
            a.write(MUX + 4 * pin, (1 << 12) | (1 << 9))
            a.write(GPIO + 0x554 + 4 * pin, 256)
        a.write(GPIO + 0x10, 0x46)
        a.write(GPIO + 0x2C, 0xFE)
    a.li(12, GPIO)
    a.li(13, UART)
    a.li(15, 0)
    if screen:
        command(0x11)
        command(0x29)
        command(0x3A, [0x55])
        command(0x36, [0x08])
        command(0x21)  # official panel init enables optical inversion
    uart("S2 SRAM READY\n")
    a.li(11, 0xFFFFFFFF)

    a.label("poll")
    a.load(7, 13, 0x1C)
    a.extui(7, 7, 0, 10)
    a.bz("input", 7)
    a.load(7, 13)
    a.store(7, 13)  # UART0 echo traverses the real modeled receive FIFO.
    a.label("input")
    a.load(14, 12, 0x3C)
    a.extui(14, 14, 0, 1)
    a.beq("poll", 14, 11)
    a.addi(11, 14, 0)
    a.bz("released", 14, True)
    uart("BUTTON DOWN\n")
    a.li(15, 128)
    a.li(9, 0x07)
    a.li(10, 0xE0)
    a.jump("draw")
    a.label("released")
    uart("BUTTON UP\n")
    a.li(15, 0)
    a.li(9, 0xF8)
    a.li(10, 0)
    a.label("draw")
    if screen:
        command(0x2A, [0, 52, 0, 186])
        command(0x2B, [0, 40, 1, 23])  # 40 + 240 - 1 = 279
        command(0x2C)
        a.li(8, 135 * 240)
        a.label("pixels")
        spi_byte(9, True)
        spi_byte(10, True)
        a.addi(8, 8, -1)
        a.bz("pixels", 8, True)
        a.li(3, 0x46)  # CS high; leave reset and backlight enabled.
        a.bitop(3, 3, 15, 2)
        a.store(3, 12, 0x10)
        a.bz("frame-red", 11, True)
        uart("FRAME GREEN\n")
        a.jump("frame-done")
        a.label("frame-red")
        uart("FRAME RED\n")
        a.label("frame-done")
    a.li(3, MARKER)
    a.store(11, 3)  # SRAM completion/BOOT observation for native diagnostics.
    a.jump("poll")
    code = a.finish()

    descriptor = bytearray(256)
    struct.pack_into("<I", descriptor, 0, 0xABCD5432)
    for offset, text in [(16, "diagnostic-1"), (48, "s2-sram-diagnostic"), (80, "00:00:00"),
                         (96, "Nov 22 2026"), (112, "original Xtensa SRAM probe")]:
        descriptor[offset:offset + len(text)] = text.encode("ascii")
    descriptor[144:176] = hashlib.sha256(code).digest()
    header = bytearray(24)
    header[0:4] = bytes([0xE9, 2, 0, 0x20])  # 4 MiB image-staging header
    struct.pack_into("<I", header, 4, ENTRY)
    struct.pack_into("<H", header, 12, 2)  # ESP32-S2, diagnostic revision0
    header[23] = 1
    result = header
    checksum = 0xEF
    for address, data in [(DRAM, descriptor), (IRAM, code)]:
        result += struct.pack("<II", address, len(data)) + data
        for byte in data:
            checksum ^= byte
    checksum_at = (len(result) // 16) * 16 + 15
    result += bytes(checksum_at - len(result)) + bytes([checksum])
    result += hashlib.sha256(result).digest()
    return bytes(result), {
        "kind": "original-authored-Xtensa-S2-SRAM-diagnostic",
        "thirdPartyFirmware": False,
        "generator": "tools/build-s2-sram-diagnostic.py",
        "chipId": 2,
        "diagnosticRevision": 0,
        "entry": ENTRY,
        "marker": MARKER,
        "codeAddress": IRAM,
        "codeBytes": len(code),
        "screen": {"width": 135, "height": 240, "released": "RGB565 f800 red", "pressed": "RGB565 07e0 green"} if screen else None,
        "bootPin": 0,
        "uart0": UART,
        "uartMarkers": ["S2 SRAM READY\n", "BUTTON UP\n", "BUTTON DOWN\n"] + (["FRAME RED\n", "FRAME GREEN\n"] if screen else []),
        "uartEcho": "At the idle input-poll loop, each received UART0 byte is transmitted unchanged.",
        "limits": "SRAM app-direct only; GPIO software SPI, not hardware SPI. No ROM/SDK/full factory boot, USB, radio, timers or physical timing certification.",
        "sha256": hashlib.sha256(result).hexdigest(),
        "bytes": len(result),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=Path(__file__).resolve().parents[1] / "esp32s2/tests/fixtures")
    args = parser.parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)
    for screen, name in [(False, "s2-sram-console"), (True, "dongle-sram-diagnostic")]:
        image, metadata = probe(screen)
        (args.output_dir / (name + ".bin")).write_bytes(image)
        (args.output_dir / (name + ".json")).write_text(json.dumps(metadata, indent=2) + "\n")
        print(name + ": " + str(len(image)) + " bytes; SHA-256 " + metadata["sha256"])


if __name__ == "__main__":
    main()
