# Classic PSRAM delay-clock and MicroPython contract

Current integrated base: upstream main `77e43c20a70054ac84d1724d757fa9ce12668151`.
Memory-miss implementation: `9823ed9a978c0fc798ffebc322662624566758f5`.
Current checks, final disassembly pair, archive manifest and model limits: [consolidated receipt](../classic-psram-followup/README.md).

The rebase retains both the octal CS0 flash state/commands and size-dependent
CS1 octal MR2 from #232, and the independently opted-in classic quad device.
Classic quad phase decoding is reached only with `classic_quad.is_some()`;
octal flash commands only with `octal_flash`. No JIT implementation changes.

ESP-IDF v5.5.1, `components/esp_psram/esp32/esp_psram_impl_quad.c:421-519`
and `:936-940` selects delay-clock mode when PSRAM and flash clock pins differ;
it is not a rev0-only protocol. For 40 MHz PSRAM it sends:

| operation | phases | result |
| --- | --- | --- |
| exit QPI | MOSI only, 16 bits, word `0xF500` | returns to SPI mode |
| read ID | two zero command bits, 32 address bits `0x9F000000`, 64 MISO bits | words `0x00225D0D`, `0x00000000` for 4 MiB |
| enter QPI | two zero command bits, 8 address bits `0x35` | enters QPI mode |

The extra bits delay the external clock; this model decodes the resulting
transaction without modelling wire timing. The ordinary 80/80 MHz command
shapes remain supported. Tests also reject malformed command width/value,
address width and MISO length, retain the QPI gate, and return all ones without
a backing device. The existing octal/quad flash and PSRAM tests remain controls.

The classic CLI now passes explicit 0/4 MiB to the existing bus setter. Its
default remains absent. Unsupported classic sizes, including a value that
would wrap on shifting, fail with an option error before shifting. The CLI
boot test executes the committed IDF allocator fixture through the real binary,
not a test-only bus call; 4 MiB passes, explicit zero and the absent default
produce the no-device ID error.

## Official MicroPython SPIRAM

Download an unmodified image locally (not redistributed by this change):

```sh
curl -fL https://micropython.org/resources/firmware/ESP32_GENERIC-SPIRAM-20260824-v1.29.0.bin -o micropython-spiram.bin
# SHA-256 a8dc95b75cbbda9e5702a37166f94d08469f5b9046262be7187b1911ac07a358
# size 1612336 bytes
ESP32SIM_ROM_DIR="$PWD/web/wasm/fw" MICROPYTHON_SPIRAM_IMAGE="$PWD/micropython-spiram.bin" \
  nice -n 10 cargo +1.99.0 test --release -j2 -p esp32sim \
  --test classic_psram_micropython -- --ignored --nocapture
```

The harness writes the complete official image at flash offset `0x1000`, boots
from the ECO3 ROM, waits for `>>>`, and sends
`import gc; print('PSRAM_FREE', gc.mem_free())` over the actual UART.
It does not patch guest code, registers, mappings or return values, and requires
zero stubs, intercepted calls and reboots. The banner is
`MicroPython v1.29.0 on 2026-08-24; Generic ESP32 module with SPIRAM with ESP32`.

| configured PSRAM | observed `gc.mem_free()` bytes | ID read error |
| --- | ---: | --- |
| 4 MiB | 4178272 | absent |
| none | 160096 | `0xffffffff`, startup continues without external RAM |

This is an end-to-end delay-clock regression, not timing or silicon validation.
The older stock Arduino fixtures select 80/80 MHz and do not independently
exercise these startup shapes. Public upstream MicroPython is MIT licensed;
this qualification retains only URL/hash/size and the small host test, not the
downloaded image or a claim to have audited every linked binary component.


## Current verification and limits

See the consolidated receipt for fresh results, final codegen and unchanged model limits.
Run the retained SPI and CLI mutation scripts in isolated source trees.
