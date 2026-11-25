# Independent ESP32-C5 IDF hello input

Built the unchanged Espressif `hello_world` with installed ESP-IDF 5.5.1 and its matching GCC, then verified the resulting merged image, application ELF and map. This is a standalone non-WiFi reference input. **No guest or emulator was executed for this evidence.** It does not establish Waveshare FactoryOnly, MAC/PHY/RF, LCD, SD, input or PSRAM acceptance.

The application prints chip/flash/heap information, counts down once per second from 10 to 0, and deliberately calls `esp_restart()`. Its printed WiFi/BT capability labels come from chip information; its main does not initialize wireless drivers. A later runtime check should observe the complete countdown, intentional restart and a second boot/countdown. An initial hello line or an unexplained reset loop is insufficient.

## Source and build

- SDK: [espressif/esp-idf 5.5.1](https://github.com/espressif/esp-idf/tree/fcae32885b0296b32044cb99ecbdc50d98dddb83), exact commit `fcae32885b0296b32044cb99ecbdc50d98dddb83`. Tracked SDK state and submodule revisions matched before and after the build. No SDK reset, update or tool installation was performed.
- [Example main](https://github.com/espressif/esp-idf/blob/fcae32885b0296b32044cb99ecbdc50d98dddb83/examples/get-started/hello_world/main/hello_world_main.c) and both CMake files were copied without edits and checked against fixed hashes before and after building. Main retains its `CC0-1.0` header; the SDK `LICENSE` is retained with the private source copy. The original top CMake enables `MINIMAL_BUILD`; main explicitly requires `spi_flash`.
- Compiler: `riscv32-esp-elf-gcc (crosstool-NG esp-14.2.0_20241119) 14.2.0`, matching SDK `tools/tools.json`. Python, CMake, Ninja and esptool versions are in [result.json](result.json).
- The only additional project input is `sdkconfig.defaults`: `CONFIG_ESPTOOLPY_FLASHSIZE_32MB=y` and `# CONFIG_SPIRAM is not set`. Target selection uses `set-target esp32c5`.
- Builds use jobs1. A private Ninja wrapper forces `-j1` for IDF actions and the bootloader ExternalProject; CMake also receives `--parallel 1`, with `CMAKE_BUILD_PARALLEL_LEVEL=1`. Tool activation is confined to child-process environment variables. Component-manager downloads are disabled.
- Build base: repository commit `beb97707605a19e0aaab4c7f192a966e9f7f657a`. The default `PROJECT_VER` was derived by IDF from that containing repository: `v0.3.0-1576-gbeb97707`; this is not a Waveshare application release identifier.

The factory firmware uses IDF 5.5.3 and has a different ELF SHA. This reference is **IDF 5.5.1**, not a reproduction of the factory build. Build timestamps, staging paths/debug information and the default project version affect hashes; bit-identical rebuilds are not claimed. The retained hashes identify this exact set of inputs and outputs.

## Artifacts and static checks

All raw artifacts stay at `$REFERENCE_DIR/project/build/`, where this run used the ignored repository directory `target/c5-idf-hello-reference-20261125`. No binary, ELF, map, copied SDK/example source or raw stdout is committed.

| Artifact | Flash offset | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `merged.bin` | `0` | 223280 | `17be53de095e731e8a2068df6e138caefe2ae4e19f83b489983736ce93f20cd9` |
| `bootloader/bootloader.bin` | `0x2000` | 22176 | `92074be07256b0ce520f1ce9b91e97d234f4f0263e42dc445f1636d958fee311` |
| `partition_table/partition-table.bin` | `0x8000` | 3072 | `7f00b6c042a89b15b0cac534f82ed988caf29278ff5700b0c511eb1b5bb7c820` |
| `hello_world.bin` | `0x10000` | 157744 | `ef77f4b4ac177e9a75a30e56da67f276def22fab3846ac942f62df7057ad9754` |
| `hello_world.elf` | — | 3282636 | `18f5a36486fbb13b8527bfae5db2b6c1a0d8306c3e59bbf12006dd79d0e585cc` |
| `hello_world.map` | — | 1895846 | `380a12f63ad73eb81c348ab1441dd2865e2c16df5126e5ec94aff0ed538b36bb` |

Verified both Espressif images' segment bounds, XOR checksums and appended SHA256; C5 chip ID23 and 32MiB header size; partition-table MD5 and factory offset; each merged flash-file range; erased-FF prefix/gaps and exact final length; ELF32/RISC-V entry agreement; and the complete ELF SHA recorded in the app descriptor. The map contains `app_main`, `vTaskDelay` and `esp_restart`, and no textual `esp_wifi_init`. Payload/source/output hashes and the complete partition/segment metadata are in `result.json`.

The generated configuration is DIO/80MHz flash, CPU240MHz, UART0, PSRAM disabled, and `XTAL_FREQ_AUTO` (`CONFIG_XTAL_FREQ=0`). C5's [5.5.1 Kconfig.xtal](https://github.com/espressif/esp-idf/blob/fcae32885b0296b32044cb99ecbdc50d98dddb83/components/esp_hw_support/port/esp32c5/Kconfig.xtal) selects the crystal from ROM/boot pins/eFuses. This build is not a measurement of a 48MHz crystal; a later runtime must supply and verify the intended fixture. A 32MiB image declaration does not test accesses above16MiB. PSRAM, although present in some fixtures, is unused here.

Five in-memory malformed-input checks rejected a changed segment, changed appended digest, oversized segment, changed partition entry and non-RISC-V ELF. Read-only reinspection of the completed outputs passed. No original output was modified for these checks. Syntax/CLI and `git diff --check` passed; the evidence privacy check was also run. Rust/Cargo, WASM, browser and runtime validation belong to separate work; this input helper changes none of them. All build times/timestamps are operational metadata, not performance measurements.

## Reproduce without global changes

Provide existing matching tool paths; the helper does not install them. `$REFERENCE_DIR` must be a fresh directory inside this checkout's ignored `target/`; existing project data is preserved by rejecting reuse.

```sh
python3 tools/c5-idf-hello-reference-build.py build \
  --idf-path "$IDF_SDK" --idf-python "$IDF_PYTHON" \
  --idf-tools-path "$IDF_TOOLS_ROOT" --gcc-bin "$GCC_BIN" \
  --ninja "$NINJA" --cmake "$CMAKE" \
  --work-dir "$REFERENCE_DIR"

python3 tools/c5-idf-hello-reference-build.py inspect \
  --work-dir "$REFERENCE_DIR"
```

The helper records private `state.json` with PID/PGID, phases and exact argv, plus copied source/license, `build-helper.py` and `summary.json`. This run was launched in a new process session; its complete task group finished before static checks. Raw stdout is retained privately as `build.stdout.log` (hash/size in `result.json`). Personal paths are replaced by tool/reference aliases in the committed receipt; process identifiers and raw command/log content are omitted. Historical artifact/log hashes remain unchanged. The private directory is retained because it contains the inputs required for subsequent independent runtime validation.
