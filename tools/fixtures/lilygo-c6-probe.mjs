// Original RV32I SRAM diagnostic for T-QT-C6 MCU V1.2 / GC9107.
// All pixels originate from guest GPIO-matrix/GP-SPI MMIO, not host RAM writes.
// This is not vendor factory firmware and does not claim ESP-IDF boot coverage.
// Register source: ESP-IDF v5.5.4 C6 gpio_reg.h/io_mux_reg.h/spi_reg.h;
// board/controller evidence: lilygo-c6-profiles-2026-11-22.json and the native
// independent MMIO tests in esp32c6/tests/lilygo_profiles.rs.
import {createHash} from 'node:crypto';

export const LILYGO_C6_PROBE = Object.freeze({
  id: 'lilygo-t-qt-c6', width: 128, height: 128, x: 2, y: 1,
  clock: 18, mosi: 15, cs: 14, dc: 19, reset: 20, backlight: 2,
  button: 'boot', buttonPin: 9,
});
const SRAM = 0x40800000, ENTRY = 0x40801000;
const GPIO = 0x60091000, IO_MUX = 0x60090004, SPI = 0x60081000;

export function lilygoC6Probe() {
  const board = LILYGO_C6_PROBE, code = [], labels = new Map(), fixups = [];
  const emit = word => code.push(word >>> 0);
  const i = (op, rd, fn, rs, immediate) => emit(((immediate & 4095) << 20) | (rs << 15) | (fn << 12) | (rd << 7) | op);
  const li = (rd, number) => {
    const value = number >>> 0;
    emit(((value + 0x800) & 0xfffff000) | (rd << 7) | 0x37);
    i(0x13, rd, 0, rd, value & 4095);
  };
  const addi = (rd, rs, immediate) => i(0x13, rd, 0, rs, immediate);
  const lw = (rd, base, offset = 0) => i(0x03, rd, 2, base, offset);
  const sw = (value, base, offset = 0) => emit(((offset >> 5) << 25) | (value << 20) | (base << 15) | (2 << 12) | ((offset & 31) << 7) | 0x23);
  const and = (rd, a, b) => emit((b << 20) | (a << 15) | (7 << 12) | (rd << 7) | 0x33);
  const label = name => {
    if (labels.has(name)) throw new Error('Duplicate label ' + name);
    labels.set(name, code.length * 4);
  };
  const jump = name => {fixups.push({at: code.length, name, kind: 'jal'}); emit(0);};
  const branch = (name, a, b, fn = 0) => {fixups.push({at: code.length, name, kind: 'branch', a, b, fn}); emit(0);};
  // x5/x6 are scratch; x11/x12 retain SPI base/CMD.USR across all MMIO helpers.
  const write = (address, value) => {li(5, address); li(6, value); sw(6, 5);};
  const output = (pin, high) => write(GPIO + (high ? 8 : 12), 1 << pin);
  const mode = (pin, signal) => {
    write(IO_MUX + 4 * pin, (1 << 12) | (1 << 9));
    write(GPIO + 0x554 + 4 * pin, signal);
    if (signal === 128) write(GPIO + 0x24, 1 << pin);
  };
  let transaction = 0;
  const startSpi = () => {
    sw(12, 11);
    const idle = 'spi-idle-' + transaction++;
    label(idle); lw(13, 11); and(13, 13, 12); branch(idle, 13, 0, 1);
  };
  const transfer = bytes => {
    if (!bytes.length) return;
    write(SPI + 0x10, 1 << 27); write(SPI + 0x1c, bytes.length * 8 - 1);
    for (let offset = 0; offset < bytes.length; offset += 4) {
      let word = 0;
      for (let index = 0; index < 4 && offset + index < bytes.length; index++) word |= bytes[offset + index] << (8 * index);
      write(SPI + 0x98 + offset, word);
    }
    startSpi();
  };
  const command = (value, data = []) => {output(board.dc, false); transfer([value]); output(board.dc, true); transfer(data);};
  const uart = text => {for (const char of text) write(0x60000000, char.charCodeAt(0));};
  const window = (start, length) => [start >> 8, start & 255, (start + length - 1) >> 8, (start + length - 1) & 255];

  li(11, SPI); li(12, 1 << 24);
  for (const [pin, signal] of [[board.clock, 63], [board.mosi, 65], [board.cs, 68]]) mode(pin, signal);
  write(SPI + 0x20, 0x3e); // SPI2 CS0 active low, all other selects disabled.
  for (const pin of [board.reset, board.dc, board.backlight]) {mode(pin, 128); output(pin, true);}
  output(board.reset, false); output(board.reset, true);
  output(board.backlight, false); // GC9107 board PMOS backlight is low-active.
  write(IO_MUX + 4 * board.buttonPin, (1 << 12) | (1 << 9));
  command(0x11); command(0x29); command(0x3a, [5]);
  command(0x36, [0xc8]); command(0x21); // Manufacturer rotation0 and IPS polarity.
  uart('LILYGO C6 SRAM READY\n');
  li(8, 0xffffffff); // Impossible prior GPIO state forces the first red frame.
  label('poll'); li(5, GPIO + 0x3c); lw(7, 5); li(6, 1 << board.buttonPin); and(7, 7, 6);
  branch('poll', 7, 8); addi(8, 7, 0);
  branch('released', 7, 0, 1);
  uart('BOOT DOWN\n'); li(10, 0xe007e007); jump('draw');
  label('released'); uart('BOOT UP\n'); li(10, 0x00f800f8);
  label('draw');
  command(0x2a, window(board.x, board.width)); command(0x2b, window(board.y, board.height)); command(0x2c);
  write(SPI + 0x10, 1 << 27); write(SPI + 0x1c, 31); li(9, board.width * board.height / 2);
  label('pixels'); sw(10, 11, 0x98); startSpi(); addi(9, 9, -1); branch('pixels', 9, 0, 1);
  uart('FRAME COMPLETE\n'); jump('poll');

  for (const fixup of fixups) {
    if (!labels.has(fixup.name)) throw new Error('Missing label ' + fixup.name);
    const offset = labels.get(fixup.name) - fixup.at * 4;
    if (fixup.kind === 'jal') {
      if (offset % 2 || offset < -1048576 || offset > 1048574) throw new Error('JAL range');
      code[fixup.at] = (((offset >> 20) & 1) << 31) | (((offset >> 1) & 1023) << 21) | (((offset >> 11) & 1) << 20) | (((offset >> 12) & 255) << 12) | 0x6f;
    } else {
      if (offset % 2 || offset < -4096 || offset > 4094) throw new Error('Branch range');
      code[fixup.at] = (((offset >> 12) & 1) << 31) | (((offset >> 5) & 63) << 25) | (fixup.b << 20) | (fixup.a << 15) | (fixup.fn << 12) | (((offset >> 1) & 15) << 8) | (((offset >> 11) & 1) << 7) | 0x63;
    }
  }
  const data = Buffer.alloc(code.length * 4);
  code.forEach((word, index) => data.writeUInt32LE(word >>> 0, index * 4));
  const descriptor = Buffer.alloc(256);
  descriptor.writeUInt32LE(0xabcd5432); descriptor.write('diagnostic-1', 16);
  descriptor.write('lilygo-c6-sram-probe', 48); descriptor.write('00:00:00', 80);
  descriptor.write('Nov 22 2026', 96); descriptor.write('original RV32I SRAM probe', 112);
  createHash('sha256').update(data).digest().copy(descriptor, 144);
  // ESP app descriptor plus original code in separate, non-overlapping SRAM
  // segments. The real app-direct loader stages these through the normal API.
  const end = 24 + 8 + descriptor.length + 8 + data.length;
  const checksumAt = Math.floor(end / 16) * 16 + 15, image = Buffer.alloc(checksumAt + 1 + 32);
  image[0] = 0xe9; image[1] = 2; image[3] = 0x20; // 4 MiB flash, C6 chip ID13.
  image.writeUInt32LE(ENTRY, 4); image.writeUInt16LE(13, 12); image[23] = 1;
  image.writeUInt32LE(SRAM, 24); image.writeUInt32LE(descriptor.length, 28); descriptor.copy(image, 32);
  image.writeUInt32LE(ENTRY, 288); image.writeUInt32LE(data.length, 292); data.copy(image, 296);
  image[checksumAt] = data.reduce((sum, byte) => sum ^ byte, descriptor.reduce((sum, byte) => sum ^ byte, 0xef));
  createHash('sha256').update(image.subarray(0, checksumAt + 1)).digest().copy(image, checksumAt + 1);
  return image;
}
