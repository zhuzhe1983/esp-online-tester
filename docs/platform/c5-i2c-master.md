# C5 HP I2C0 master transport

This is a C5-specific adapter around the shared byte-level I2C engine. Its register addresses, masks, reset values, GPIO signals and interrupt source come from ESP-IDF v5.5.4, fixed commit `735507283d5b2f9fb363a1901172dbd9e847945d`. The original headers and download receipts are in [`c5-i2c-sources-2026-11-23`](../research/c5-i2c-sources-2026-11-23/manifest.json). `esp32c5/tools/generate_i2c_registers.py` regenerates the register table from the retained header.

| Connection | C5 value |
| --- | --- |
| HP I2C0 | `0x60004000` |
| PCR configuration / source clock | `0x60096020` / `0x60096024` |
| SDA / SCL matrix signals | 47 / 46 |
| Interrupt source | 56 |
| Implemented source | Explicit nominal board crystal, currently 48 MHz |

The 240 MHz virtual CPU converts elapsed cycles to the shared engine's 80 MHz scheduling units, preserving the fractional remainder. The source divider and bus timing registers determine each byte deadline. A 48 MHz source with 120 source cycles per bit produces a 400 kHz bus; address callbacks occur after the programmed byte interval. The controller publishes board state at that time, and input edges produced by a transaction are delivered to the GPIO input/interrupt path.

Both input and output routes must resolve through the C5 IO_MUX and GPIO matrix to each attached device's actual SDA/SCL pins. Accepted START captures those routes; a second START while busy cannot replace them. Devices receive the derived SCL frequency, their selected address, read/write direction and actual STOP. Controller reset cancels its transfer and FIFOs but neither power-cycles external devices nor synthesizes STOP. Board replacement and SoC restart explicitly rebuild the controller's device list.

The implemented subset is seven-bit master FIFO operation with RESTART, WRITE, READ, STOP and END commands, ACK/NACK, 32-byte FIFOs, command completion and END/transaction-complete/NACK interrupts. The C5 status header specifies ACK as zero and NACK as one; this adapter converts the shared older-chip status convention. The DATA register is writable for TX because the C5 low-level driver's `i2c_ll_write_txfifo()` explicitly writes it, although the generated register header only describes the RX field as HRO.

## Explicit limits

RC_FAST, slave mode, ten-bit addressing, LSB-first operation, multi-master arbitration, clock stretching, timeout execution, FIFO RAM aliases and dynamic watermark interrupts are not implemented. Unsupported interrupt enables, command programs and FIFO overflow are rejected. PCR/source/local clock gates and reset are enforced; unknown source selection cannot silently fall back to 40 MHz. LP I2C at `0x600b1800` remains a separate unsupported peripheral. This is byte scheduling, not a generated SDA/SCL electrical waveform or a claim about analog rise times.

The controller is allocated only when a board has I2C devices or the guest accesses its MMIO. C3, C6, S3 and P4 paths are unchanged by this adapter. The shared explicit-source API remains opt-in and its old-source regression tests are recorded separately.

## Verification

`cargo test -p esp32c5 --release --test i2c` checks actual address/data/read/STOP callbacks and byte deadlines, correct C5 routes, wrong C6 routes producing NACK, CLIC interrupt mapping, reset defaults, busy START capture, disabled and RC clock rejection, FIFO capacity and unsupported LP/FIFO-alias accesses. The four-line wiring in the test is intentionally checked through MMIO rather than by directly calling a board's I2C device.

This transport alone is not a factory firmware or touch application acceptance result. Product-specific guest execution and physical pin sources are documented separately.
