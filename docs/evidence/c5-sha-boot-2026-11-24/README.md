# C5 guest SHA CPU-block engine

The private `C5Sha` module performs real SHA-1, SHA-224 and SHA-256 compression
from guest-written message words and START/CONTINUE commands. It reuses the
audited shared compression code, with C5-specific register/mode/reset guards.
It does not supply padding, accept a factory footer on the host, or alter an
image. The unchanged LCD273 pre-integration baseline reaches 285569 steps and
faults at PC `0x400277da`, PCR SHA `0x600960d0`; UART has 128 bytes including the
ROM's DIO/clock-divider message, with no application/pixels. Full ROM digest-path
acceptance after wiring remains integration work.

The API is `C5Sha::new()`, `read(&mut self, absolute_address) -> Option<u32>` and
`write(&mut self, absolute_address, value) -> bool`. Supported addresses are:

| Address | Implemented behavior |
| --- | --- |
| `0x600960d0` | SHA clock/reset and derived READY |
| `0x600960e4/e8/ec` | DS/HMAC/ECDSA reset inputs linked to SHA; related clocks must remain off |
| SHA `0x60089000 +0` | Modes 0, 1, 2 only |
| `+0x0c` | C5 16-bit DMA-count staging; no DMA execution |
| `+0x10/+0x14` | Write 1: actual first/continuation compression; write 0: no operation |
| `+0x18` | Actual BUSY state, idle after synchronous compression returns |
| `+0x28/+0x2c` | Disabled DMA IRQ configuration and C5 version register |
| `+0x40..0x5c` | Eight real R/W intermediate/digest words |
| `+0x80..0xbc` | Sixteen real R/W message words |

All accesses honor the SHA gate and effective reset. As documented by the C5
[LL](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/include/hal/sha_ll.h),
DS, HMAC and ECDSA reset inputs also hold SHA reset. Asserting any such input
actually clears the SHA state; READY reflects reset release, not hash completion.
Related PCR handling does not implement those cryptographic engines or keys.
Their clocks and all engine MMIO remain unsupported.

The fixed IDF v5.5.3 (`2c211b236707889e8400c4dc5644dd5c4ee071e0`) and v5.5.4
(`735507283d5b2f9fb363a1901172dbd9e847945d`) SHA
[register](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/sha_reg.h),
struct, LL and ROM API headers are byte-identical. Base `0x60089000` is checked
against fixed `reg_base.h` and [TRM v1.1](https://documentation.espressif.com/esp32-c5_technical_reference_manual_en.pdf)
Table 6.3-2. The TRM chapter26 specifies C5's three algorithms and CPU-block
process, including guest padding and intermediate H-state save/restore. The LL
uses ordinary memory-order word copies for M/H; the shared implementation's
message/digest byte ordering matches this, as independently verified below.

Source differences are retained: matching-firmware headers specify MODE reset2,
DATE `0x20240823` and a 16-bit DMA count; TRM diagrams instead show MODE reset0,
DATE `0x20201229` and a 6-bit count (up to63 blocks). Header defaults/staging
width are used without extrapolating DMA support. TRM normal C5 windows contain
eight H and sixteen M words; generic SHA-512/debug windows and modes reject.
The evaluated C5 enum gives SHA interrupt source79; Typical SHA generates no
interrupt, and this subset never starts DMA or manufactures its IRQ.

Five focused tests pass. Fifteen independent cases come from NIST's official
[byte-oriented SHA vectors](https://csrc.nist.gov/Projects/Cryptographic-Algorithm-Validation-Program/Secure-Hashing):
all three algorithms at lengths0/24/440/448/512 bits, including two-block padding.
Additional checks save/restore actual H words around an interleaved hash, verify
START/CONTINUE and byte order, and exercise linked resets, clock retention,
unsupported modes/debug/DMA, and zero/invalid triggers without false hashing.
Clippy passes with warnings denied and no allowance. This is informal functional
verification, not CAVP validation or physical accelerator timing verification.

No DMA transport, active DMA IRQ, SHA-512/SM3, debug T registers, other crypto
engines, shared/bus edits, broad checks, benchmark or publication is included.
Source revisions, hashes, independent-vector provenance and exact scope are in
[checks.json](checks.json).
