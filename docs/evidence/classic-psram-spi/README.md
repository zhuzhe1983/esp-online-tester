# Classic PSRAM command contract

Current integrated base: upstream main `77e43c20a70054ac84d1724d757fa9ce12668151`.
Memory-miss implementation: `9823ed9a978c0fc798ffebc322662624566758f5`.
Current checks, final disassembly pair, archive manifest and model limits: [consolidated receipt](../classic-psram-followup/README.md).

The adapter uses the existing shared SPI payload engine and a classic-only optional
quad-control state. CS0 flash and the independently opted-in octal state remain
separate; this follow-up changes neither implementation.

Sources: ESP-IDF v4.4.7 `components/soc/esp32/include/soc/spi_reg.h` and
`components/esp_hw_support/port/esp32/spiram_psram.c`; v5.1.4
`components/soc/esp32/include/soc/spi_reg.h` and
`components/esp_psram/esp32/esp_psram_impl_quad.c:277-302,403-503`.

Absent backing returns eight ff ID bytes. A 4 MiB device returns
`0d 5d 22 00 00 00 00 00`, a deterministic model identity, not a silicon capture.
35 enters QPI, f5 exits it, 66 arms the immediately following 99 reset. An
intervening CS1 command consumes the latch; unarmed reset does nothing. Device
reset retains bytes; machine reboot has the separate zero-fill policy below.
SPI 02/03/0b and QPI 38/eb share the CPU backing. Phase and mode gates reject
malformed commands. Disabled chip selects cannot modify flash or PSRAM.

The [CI-runnable SPI tests](../../../esp32/src/psram_spi_tests.rs) include
ordinary and delay-clock shapes, full ID/tail bytes, QPI transitions, armed reset,
CS isolation and payload/phase controls. Run:

    nice -n 10 cargo +1.99.0 test --release -p esp32 psram_spi
    # In an isolated source tree:
    python3 docs/evidence/classic-psram-spi/mutations.py TREE OUTPUT

The DCLK [contract and MicroPython input](../classic-psram-dclk/README.md)
are retained separately. No electrical/wire timing claim.
