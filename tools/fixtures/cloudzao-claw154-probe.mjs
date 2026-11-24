// Original Xtensa LX7 SRAM diagnostics, assembled without a cross compiler.
// These are not vendor factory firmware. All pixels originate from guest
// GPIO-matrix/GP-SPI writes; there are no host framebuffer writes or ROM stubs.
// Exact board pins/crops: web/platform/knowledge/cloudzao-claw-154.json.
// Encodings follow xtensa-lx7/src/jit/wasm_tests/asm.rs; MMIO follows the native
// independent-pin probes in esp32s3/tests/cloudzao_claw154.rs.
import {createHash} from 'node:crypto';

const IRAM = 0x40370000, POOL_BYTES = 4096, ENTRY = IRAM + POOL_BYTES;
const GPIO = 0x60004000, SPI = 0x60025000, IO_MUX = 0x60009004;

export function cloudzaoClaw154Probe() {
  const board = {id:'cloudzao-claw-154',width:240,height:240,x:0,y:0,clock:9,mosi:10,cs:14,dc:8,reset:18,backlight:13,lightHigh:true,button:'boot',buttonPin:0};
  const code = [], labels = new Map(), fixups = [], literals = new Map();
  const emit = word => code.push(word & 0xffffff);
  const rri8 = (op, r, s, t, n) => emit(op | (t << 4) | (s << 8) | (r << 12) | ((n & 255) << 16));
  const label = name => {
    if (labels.has(name)) throw new Error('Duplicate label ' + name);
    labels.set(name, ENTRY + code.length * 3);
  };
  const ref = (kind, name, args = {}) => {fixups.push({at: code.length, kind, name, ...args}); emit(0);};
  const li = (register, number) => {
    const value = number >>> 0;
    if (!literals.has(value)) literals.set(value, literals.size * 4);
    ref('literal', value, {register});
  };
  const store = (value, base, offset = 0) => rri8(2, 6, base, value, offset / 4);
  const load = (value, base, offset = 0) => rri8(2, 2, base, value, offset / 4);
  const addi = (target, source, value) => rri8(2, 12, source, target, value);
  const and = (target, a, b) => emit((b << 4) | (a << 8) | (target << 12) | (1 << 20));
  const jump = name => ref('jump', name);
  const beq = (name, a, b) => ref('equal', name, {a, b});
  const bz = (name, register, nonzero = false) => ref('zero', name, {register, nonzero});
  // a2/a3 are scratch MMIO operands; a6/a13 retain SPI base/CMD.USR mask.
  const write = (address, value) => {li(3, address); li(2, value); store(2, 3);};
  const output = (pin, high) => write(GPIO + (pin < 32 ? (high ? 8 : 12) : (high ? 20 : 24)), 1 << (pin % 32));
  const mode = (pin, signal) => {
    write(IO_MUX + 4 * pin, (1 << 12) | (1 << 9));
    write(GPIO + 0x554 + 4 * pin, signal);
    if (signal === 256) write(GPIO + (pin < 32 ? 0x24 : 0x30), 1 << (pin % 32));
  };
  let transaction = 0;
  const waitSpi = () => {
    const name = 'spi-idle-' + transaction++;
    label(name); load(4, 6); and(4, 4, 13); bz(name, 4, true);
  };
  const startSpi = () => {store(13, 6); waitSpi();};
  const dataWords = bytes => {
    write(SPI + 0x1c, bytes.length * 8 - 1);
    for (let offset = 0; offset < bytes.length; offset += 4) {
      let value = 0;
      for (let index = 0; index < 4 && offset + index < bytes.length; index++) value |= bytes[offset + index] << (index * 8);
      write(SPI + 0x98 + offset, value);
    }
  };
  const transfer = bytes => {
    if (!bytes.length) return;
    write(SPI + 0x10, 1 << 27); dataWords(bytes); startSpi();
  };
  const command = (value, data = []) => {
    if (board.dc !== undefined) {
      output(board.dc, false); transfer([value]); output(board.dc, true); transfer(data);
    } else {
      // QSPI command/address produce the physical header [02,00,CMD,00].
      write(SPI + 0x10, (1 << 31) | (1 << 30) | (data.length ? 1 << 27 : 0));
      write(SPI + 0x18, (7 << 28) | 0x02); write(SPI + 0x14, 23 << 27);
      write(SPI + 4, value << 16);
      if (data.length) dataWords(data);
      startSpi();
    }
  };
  const uart = text => {for (const char of text) write(0x60000000, char.charCodeAt(0));};
  const window = (start, length) => [start >> 8, start & 255, (start + length - 1) >> 8, (start + length - 1) & 255];

  li(6, SPI); li(13, 1 << 24);
  for (const [pin, signal] of [[board.clock, 66], [board.mosi, 68], [board.cs, 71]]) mode(pin, signal);
  write(SPI + 0x20, 0x3e); // CS0 enabled, active low; all other CS lines disabled.
  for (const pin of [board.reset, board.dc, board.enable, board.backlight].filter(pin => pin !== undefined)) {
    mode(pin, 256); output(pin, true);
  }
  if (board.backlight !== undefined) output(board.backlight, board.lightHigh);
  write(IO_MUX + board.buttonPin * 4, (1 << 12) | (1 << 9));
  command(0x11); command(0x29); command(0x3a, [0x55]);
  uart('CLOUDZAO CLAW154 SRAM READY\n');
  li(8, 0xffffffff); // Impossible previous state forces the initial red frame.
  label('poll');
  li(3, GPIO + (board.buttonPin < 32 ? 0x3c : 0x40)); load(7, 3);
  li(2, 1 << (board.buttonPin % 32)); and(7, 7, 2);
  beq('poll', 7, 8); addi(8, 7, 0);
  bz('released', 7, true);
  uart('BUTTON DOWN\n'); li(10, 0xe007e007); jump('draw');
  label('released'); uart('BUTTON UP\n'); li(10, 0x00f800f8);
  label('draw');
  command(0x2a, window(board.x, board.width)); command(0x2b, window(board.y, board.height)); command(0x2c);
  if (board.dc !== undefined) {
    write(SPI + 0x10, 1 << 27);
  } else {
    // Each burst uses RAMWRC; RAMWR here would reset the address on every pixel.
    write(SPI + 0x10, (1 << 31) | (1 << 30) | (1 << 27));
    write(SPI + 0x18, (7 << 28) | 0x32); write(SPI + 0x14, 23 << 27); write(SPI + 4, 0x3c << 16);
  }
  write(SPI + 0x1c, 31); li(9, board.width * board.height / 2);
  label('pixels'); store(10, 6, 0x98); startSpi(); addi(9, 9, -1); bz('pixels', 9, true);
  uart('FRAME COMPLETE\n'); jump('poll');

  if (literals.size * 4 > POOL_BYTES) throw new Error('Literal pool overflow');
  for (const fixup of fixups) {
    const pc = ENTRY + fixup.at * 3;
    if (fixup.kind === 'literal') {
      const offset = IRAM + literals.get(fixup.name) - ((pc + 3) & ~3);
      if (offset % 4 || offset < -262144 || offset > -4) throw new Error('L32R range');
      code[fixup.at] = 1 | (fixup.register << 4) | (((offset >> 2) & 65535) << 8);
      continue;
    }
    if (!labels.has(fixup.name)) throw new Error('Missing label ' + fixup.name);
    const offset = labels.get(fixup.name) - (pc + 4);
    if (fixup.kind === 'jump') {
      if (offset < -131072 || offset > 131071) throw new Error('J range');
      code[fixup.at] = 6 | ((offset & 0x3ffff) << 6);
    } else if (fixup.kind === 'equal') {
      if (offset < -128 || offset > 127) throw new Error('BEQ range');
      code[fixup.at] = 7 | (fixup.b << 4) | (fixup.a << 8) | (1 << 12) | ((offset & 255) << 16);
    } else {
      if (offset < -2048 || offset > 2047) throw new Error('BEQZ/BNEZ range');
      code[fixup.at] = 0x16 | (Number(fixup.nonzero) << 6) | (fixup.register << 8) | ((offset & 4095) << 12);
    }
  }
  const data = Buffer.alloc((POOL_BYTES + code.length * 3 + 3) & ~3);
  for (const [value, offset] of literals) data.writeUInt32LE(value, offset);
  code.forEach((word, index) => data.writeUIntLE(word & 0xffffff, POOL_BYTES + index * 3, 3));

  // A full ESP app descriptor is the first segment; the second holds original
  // code + literals. Both reside in SRAM. App-direct runs this program without
  // pretending it is an ESP-IDF bootloader/factory application.
  const descriptor = Buffer.alloc(256);
  descriptor.writeUInt32LE(0xabcd5432); descriptor.write('diagnostic-1', 16);
  descriptor.write('cloudzao-claw154-probe', 48); descriptor.write('00:00:00', 80);
  descriptor.write('Nov 23 2026', 96); descriptor.write('original Xtensa SRAM probe', 112);
  createHash('sha256').update(data).digest().copy(descriptor, 144);
  const end = 24 + 8 + descriptor.length + 8 + data.length;
  const checksumAt = Math.floor(end / 16) * 16 + 15;
  const image = Buffer.alloc(checksumAt + 1 + 32);
  image[0] = 0xe9; image[1] = 2; image[3] = 0x40;
  image.writeUInt32LE(ENTRY, 4); image.writeUInt16LE(9, 12); image[23] = 1;
  image.writeUInt32LE(0x3fc90000, 24); image.writeUInt32LE(descriptor.length, 28); descriptor.copy(image, 32);
  image.writeUInt32LE(IRAM, 288); image.writeUInt32LE(data.length, 292); data.copy(image, 296);
  image[checksumAt] = data.reduce((sum, byte) => sum ^ byte, descriptor.reduce((sum, byte) => sum ^ byte, 0xef));
  createHash('sha256').update(image.subarray(0, checksumAt + 1)).digest().copy(image, checksumAt + 1);
  return image;
}
