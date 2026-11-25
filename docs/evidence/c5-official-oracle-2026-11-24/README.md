# ESP32-C5 official binary comparison

The unchanged Waveshare ESP32-C5-LCD-2.73 factory image reaches `app_main()` in official esp-emulator v1.2.0, after its full 8 MiB PSRAM memory test passes. Its first application failure is RTC initialization returning `0x103 / ESP_ERR_INVALID_STATE`; it then aborts and executes repeated SW_CPU resets. This establishes a useful bounded boot target, not LCD, buttons, Wi-Fi or TF compatibility.

## Fixed sources and inputs

- [Official repository](https://github.com/espressif/esp-emulator/tree/6bc8abda651d123aee59cc7e9822fafcee975fe9), fixed documentation revision `6bc8abda651d123aee59cc7e9822fafcee975fe9`.
- [v1.2.0 release](https://github.com/espressif/esp-emulator/releases/tag/v1.2.0), tag commit `34ba27a6e5a3da41743fc861b462cf2452f0ca28`.
- macOS arm64 archive `esp-emu-1.2.0-aarch64-apple-darwin.tar.gz`: 6,667,747 bytes, SHA256 `2406dbec04bcb8ee207e512cbb98a8c5478c9dad94ee18f1b3b14c384ab5eb6f`. Release checksums and GitHub asset digest agreed before execution.
- Extracted executable: 14,968,112 bytes, SHA256 `920a8f68bf024251dc62b4b164b96f093f6349d8acf40402c211666f5d5a0544`; reports version1.2.0.
- Rev100 ROM ELF: 472,300 bytes, SHA256 `49693d745eb4b4e6e6c77be4f5213765d545d8c7a1d9742a8a619bf596c4e27a`.
- [Manufacturer factory image](https://github.com/waveshareteam/ESP32-C5-LCD-2.73/blob/418de818f4f40c5a04c4292dbe293bfaa1351068/firmware/ESP32-C5-LCD-2.73_FactoryOnly_20260724.bin): 3,346,992 bytes, SHA256 `cf10a4358d03898458c16390f3f16ceb0306befe2260160de0919d755b920942`.
- Prepared 64 MiB FAT32 card: 67,108,864 bytes, SHA256 `a0e026d9aeafab624884455d25147647980bd6a83011618da04140659c28bfc6`.

All three inputs were copied into an isolated run directory. Original and copied input hashes were unchanged afterward. The binary was not installed globally. No application ELF, sensor regfile, forced ACK, memory patch, skip-ROM or skip-bootloader option was supplied. Automatic updates and all network access were disabled, with an additional macOS deny-network sandbox. No physical device was used.

## Reproduction and observations

In the isolated directory, with `settings.json` containing `{"autoupdate":false}` and `network-deny.sb` containing `(version 1) (allow default) (deny network*)`:

```sh
ESP_EMU_DISABLE_AUTOUPDATES=1 RUST_LOG=info NO_COLOR=1 \
  /usr/bin/sandbox-exec -f ../network-deny.sb ../esp-emu \
  --config ../settings.json --chip esp32c5 \
  --firmware lcd273-factory.bin --rom esp32c5_rev100_rom.elf \
  --psram-size 8M --sd lcd273-sd64m.img,cs=9 \
  --net user,restrict=yes,dns=127.0.0.1 \
  --wifi-ssid WSTEST --wifi-password waveshare0755 \
  --timeout 20s --log-color never --log-file engine.log
```

The SSID/password above are the fixed manufacturer's public factory-test defaults, not account credentials. RTC fails before application Wi-Fi initialization. The `restrict` option alone still permits DNS, so the additional OS sandbox is material.

The first cold boot reports IDF5.5.3, QIO80MHz/32MB Flash, application offset `0x20000`, and 8MB PSRAM. Guest timestamps report memory test completion at299ms and `app_main()` at307ms. RTC failure is at `ESP_ERROR_CHECK` PC `0x42013888`, in `bsp_pcf85063a_drv_init`, BSP line624. Abort reports PC `0x4080ae49`; the subsequent dump has `MEPC=0x4080ae8e`, `MCAUSE=2`, `MTVAL=0`. Repeated boots use reset cause `0x0c`.

The20s run records39 application entries,39 PSRAM-test passes and39 RTC errors. Exit0 means the engine reached its timeout; it is not firmware success. UART is328,230 bytes, SHA256 `8bc70d3c131c2e2411b950c239ab54396a942282a969d662ed168bb93a7c9f4c`. A separate debug run stopping at the first `ESP_ERROR_CHECK failed` records opcode `0x7a` as **unhandled**, then continues. Its debug log is607,531 bytes, SHA256 `c65370e7595e4aca9820810df9fc87a38901e0e1f6bda41c84db6a69a954110d`.

The RTC driver's success message precedes its error return. Neither that message nor `0x103` alone proves a working device or a physical NACK: this run did not capture independent I2C ACK/NACK traffic. It does not establish the RTC error's exact engine cause.

## Interpretation limits

The [README](https://github.com/espressif/esp-emulator/blob/6bc8abda651d123aee59cc7e9822fafcee975fe9/README.md) explicitly documents C5 support and ROM/driver interceptions. The public tree contains installers, documentation, parts, protocols and tests, but no core CPU/SoC implementation to port or inspect for REGI2C/PLL/MSPI correctness. The actual run reports19 ROM function interceptions without listing their names. [CLI options](https://github.com/espressif/esp-emulator/blob/6bc8abda651d123aee59cc7e9822fafcee975fe9/docs/guides/CLI.md) provide no hook-free mode or XTAL override; this run does not independently verify48MHz. The initially attempted `--no-panic-intercept` without ELF was rejected before execution; the reproduced run omits both. That option is not a general no-interceptions switch.

Thus the comparison cannot prove instruction equivalence, Flash-resume semantics, physical clock accuracy or complete product support. No retired-instruction/cycle counter was available; guest milliseconds and host time must not be compared as native cycle counts. Public C5 SRAM-only restrictions remain appropriate until this project's own unchanged-firmware acceptance passes. Raw engine output and exact invocation receipts remain in the ignored local evidence cache; this document retains the public versions, hashes, commands, aggregate results and limitations without host identities or unrelated process data.
