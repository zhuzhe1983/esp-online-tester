# C5 Touch2.8 SRAM transport diagnostic

Original project code, not the Waveshare factory image. Build with the official RISC-V GCC and esptool5.1.0 using `build.sh`; supply `RISCV_GCC`, `ESPTOOL` and, for a local Python package directory, its `PYTHONPATH`.

Run on `waveshare-c5-touch28`, nominal48MHz crystal, **32MiB physical Flash and8MiB PSRAM**, with an attached SPI card. All8MiB PSRAM is written/read before GDMA display use. SPI24MHz and I2C400kHz use real C5 registers. CH32V00324h resets LCD EX1/touch EX0 and drives PWM; CST353058h sends actual4-byte commands and ACK. GPIO28 BOOT and IRQ5 touch change guest-rendered pixels.

The SD test temporarily writes LBA32, after saving it, then restores and verifies the original512 bytes. The supplied runner confirms all bytes of its nonzero8MiB card remain identical after complete execution. This is not a filesystem mount or a guarantee if the guest is interrupted before restoration. Wait for `C5 TOUCH28 READY` before interactive input.

See [the runtime report](../../docs/platform/waveshare-c5-touch28.md) for exact factory limitations, commands, source receipts and output evidence. The original SRAM diagnostic does not establish manufacturer firmware or real hardware compatibility.
