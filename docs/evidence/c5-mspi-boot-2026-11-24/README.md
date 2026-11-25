# C5 MSPI ROM bootstrap and data path

The unmodified official MP rev100 ROM with the original Waveshare LCD273 image,
32 MiB flash and 8 MiB PSRAM advances from 274276 steps to 274383 steps after
modeling the reached SPI0 FIFO status. With the default strict configuration it
then faults at PC `0x4002967e`, reading `0x60002028`. UART remains the genuine
101-byte ROM SPI boot banner; no factory bootloader/application or pixels are
claimed by this module check. [checks.json](checks.json) retains inputs and results.

The LCD273 factory uses IDF v5.5.3 (`2c211b236707889e8400c4dc5644dd5c4ee071e0`);
the LCD147 control uses v5.5.4 (`735507283d5b2f9fb363a1901172dbd9e847945d`). Full-file
comparisons found the five C5 MSPI register, struct and LL sources below
byte-identical between these tags. SPI0 and SPI1 have separate C5 register maps;
neither is a C6 alias.

- [SPI0 registers](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/spi_mem_c_reg.h):
  `SMEM_AXI_ADDR_CTRL` at `+0x178`, RO empty flags 26–31. The synchronous mapped
  array engine has no queued AXI requests and no active SPI0 command, so its
  actual observable state is empty/idle. Reading this status does not complete
  work. Software SPI0 USR, PE and unknown command engines remain rejected.
- [SPI1 registers](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/spi1_mem_reg.h):
  separate data-length registers, 16 data words, command triggers, `RD_STATUS`
  latch and `WB_MODE` fields. The status latch is separate from the physical
  virtual flash status; a real RDSR transfer refreshes its low byte. Dedicated
  READ uses `ADDR[31:24]` as byte count and `ADDR[23:0]` as address, bounded to
  1–64 bytes. Done interrupts follow real synchronous transactions/data movement.
- [C5 LL](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/include/hal/spimem_flash_ll.h):
  user commands have separate phases, right-aligned addresses, bit-count-minus-one
  lengths and real data-buffer copies. Shared execution was audited for ordinary
  byte transfers; unsupported encodings and unknown/combined triggers reject.

## Explicit ROM/header conflict

The fixed [SPI0 struct](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/spi_mem_c_struct.h)
declares `reserved_024[2]`. The public [C5 TRM v1.1](https://documentation.espressif.com/esp32-c5_technical_reference_manual_en.pdf)
§33.1 reserves MSPI for internal use and documents GP-SPI2; it supplies no MSPI
table establishing a reset/read-ignore contract for these words. Nevertheless,
the official unchanged mask ROM unconditionally reads both SPI0 words and writes
their low ten bits to `255` (`SPI_init`, PCs `0x4002967e`–`0x4002969c`). The ROM is
primary behavioral evidence of those accesses, not evidence that a C6 field
definition or silicon reset value applies.

`Mspi::enable_mp_rom_compat()` enables exactly two independent opaque words, only
after the caller validates the MP ROM. Their initial zero is an explicit virtual
choice, not a silicon claim. Only the witnessed low-ten-bits `255` write with
unchanged upper bits succeeds; other values reject transactionally. PCR reset
clears the words while retaining this opt-in policy. They do not alias SPI1
lengths or affect data, timing, engine completion or IRQs. The default SRAM path
continues to reject both reserved addresses.

A direct-address ROM scan finds only the four bootstrap accesses. Manually
audited computed-port `esp_rom_spi_cmd_config` also reads/writes these offsets
with caller-controlled lengths, and `esp_rom_spi_cmd_start` can issue USR/PE on
the caller-selected port. Those general SPI0 modes remain unsupported. No
whole-ROM completeness claim follows from the bootstrap transcript.

## Checks and limits

Seven focused unit tests pass, including opt-in/reset/gate behavior, rejected
writes/commands without false done, actual ROM-style byte reads, dedicated READ
lengths, status/config separation, and 32 MiB MMU/capacity with independent 8 MiB
PSRAM. Direct unit-crate Clippy passes with warnings denied and no lint allowance.
Ordinary library Clippy stops solely at the new API's missing integration call;
the integration owner must wire it after ROM validation and rerun that check.

The existing virtual flash JEDEC profile is `20 40 <capacity>` (`20 40 19` for
32 MiB), not a physical manufacturer identification. The capacity test initially
expected a different vendor byte; it was corrected to this existing explicit
virtual profile. MMU coverage is not proof of a flash die's four-byte command
mode. This tranche adds no new four-byte flash protocol, analog/cache latency,
timing calibration, ECC/PMS/XTS, NAND, HPM or dedicated 32 KiB erase engine.
Existing PSRAM die behavior is unchanged. Factory/native/Worker acceptance and
broader regression checks belong to integration.
