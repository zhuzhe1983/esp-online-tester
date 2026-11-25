# Independent C5 hello: ESP-IDF 6.0.2

Built a second unchanged official `hello_world` using the installed, matching ESP-IDF6.0.2/GCC pair. Static checks passed. **The separate initial native run stopped in the bootloader before Key Manager or app_main**, so this is a verifiable reference input, not a runtime pass or factory acceptance. No SDK/firmware workaround was applied.

The IDF5.5.1 input, its artifacts and evidence at commit `68182adf354639aba07aac4844dff4fa0fd7479e` are preserved. Its separate native run stopped at the Key Manager reset-release request with memory POWER4; the model was not widened to supply READY/IDLE. The later official SDK has its own original power-up/STATIC path. Switching reference versions does not resolve the 5.5.1 contract or reproduce FactoryOnly's IDF5.5.3.

## Exact source and tool pair

- SDK: [espressif/esp-idf 6.0.2](https://github.com/espressif/esp-idf/tree/7101770dc6db2667b3c477cc31365dd1acd6db4e), commit `7101770dc6db2667b3c477cc31365dd1acd6db4e`. Tracked SDK and recursive submodule state matched before/after. No SDK reset/update or global installation occurred.
- Compiler: `riscv32-esp-elf-gcc (crosstool-NG esp-15.2.0_20251204) 15.2.0`, exactly the version in this SDK's `tools/tools.json`; existing IDF6.0 virtualenv Python3.11.7.
- [Official main](https://github.com/espressif/esp-idf/blob/7101770dc6db2667b3c477cc31365dd1acd6db4e/examples/get-started/hello_world/main/hello_world_main.c), both CMake files and licenses were retained without source edits. Main SHA256 remains `b2d1c8573307e010d276d2d0df537f1fe752416f8422e267aa8b1661c1744595`; its CC0 header is retained. Top CMake is this version's original `5df271e4d0dd84f1d4765b274c6d53179cf33ca3f9c5eb5d21de69d88310d721` and enables MINIMAL_BUILD.
- Explicit defaults select C5/32MiB flash with PSRAM disabled. Generated settings are DIO80MHz, CPU240MHz, UART0 and XTAL_AUTO. A later fixture supplies the actual crystal choice; this build is not a48MHz measurement.
- Outer build flags were jobs8 and the private nested-bootloader Ninja wrapper selected jobs1. These are operational flags, **not a measured or guaranteed total process/thread-concurrency peak**. No performance measurement was made. Tools were activated only in the task's child-process environment; no emulator, guest, Cargo, browser or benchmark was launched by the build helper.

The descriptor names `hello_world`, IDF=`v6.0.2`, compile date/time=`Nov 25 2026` / `07:21:45`, and app version=`v0.3.0-1577-g68182adf-dirty`. The app version was automatically derived from the containing repository while its helper was being edited; it is not the SDK revision or a vendor release. SDK tracked state and official example-source hashes remained clean/matched. Build time, debug paths and this default project version affect output hashes; bit-identical rebuilds are not claimed.

## Artifacts

Raw sources, license copy, stdout, BIN/ELF/map and exact PID/PGID/argv remain under ignored `target/c5-idf602-hello-reference-20261125/`. `$REFERENCE_DIR` below denotes that directory. No raw artifact or stdout is committed.

| Artifact | Flash offset | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `merged.bin` | `0` | 200944 | `a84d3ce0a8ad44525706a67c596e9bf0007c2040d6280fb2cf8abf2f0a93b8a5` |
| `bootloader/bootloader.bin` | `0x2000` | 22272 | `437778c24be2001339097e42b5cfa9165d9e83279f6a6ff2f3e35d756e427149` |
| `partition_table/partition-table.bin` | `0x8000` | 3072 | `7f00b6c042a89b15b0cac534f82ed988caf29278ff5700b0c511eb1b5bb7c820` |
| `hello_world.bin` | `0x10000` | 135408 | `adc8c8cf9e760b7611f75249ea14bfaddf6a67f8e5cef131f73d7a412215841d` |
| `hello_world.elf` | — | 3597104 | `2043dc07c96cf7e9ccc1b357397af279c8bfe3ac62132a9d8b99740009a2611a` |
| `hello_world.map` | — | 2395129 | `e9db6f63b3ae77b6874494b333ac6c0cb1c6c0207e97480b8a335c5bf434f50d` |

Verified C5 chip ID23, 32MiB image declarations, segment bounds, both images' XOR/appended SHA256, partition MD5/factory offset, ELF32 RISC-V entry `408006b8`, complete descriptor/ELF SHA equality, merged flash-file bytes, FF prefix/gaps and exact container end. Main/CMake stayed unchanged; map text contains app_main/vTaskDelay/esp_restart and no esp_wifi_init. These are static checks, not evidence of wireless execution or sustained scheduling. A32MiB declaration does not test high flash addresses; PSRAM is not enabled.

### Original Key Manager sequence is present

[6.0.2 init.c](https://github.com/espressif/esp-idf/blob/7101770dc6db2667b3c477cc31365dd1acd6db4e/components/esp_security/src/init.c#L32) SHA256=`0ec3693522cf58c738383e1a13110f6179631e5685c60397d4f72b9267393667`; [C5 key_mgr_ll.h](https://github.com/espressif/esp-idf/blob/7101770dc6db2667b3c477cc31365dd1acd6db4e/components/esp_hal_security/esp32c5/include/hal/key_mgr_ll.h) SHA256=`9b0247c5bbca44453b0c76849dad38af990df665c3d629d450b4539358ff9a95`. Local selected source files matched the fixed official raw contents.

The real linked ELF/map retains `esp_key_mgr_init=4200053e`, owned by `esp_security/libesp_security.a(init.c.obj)`. Static disassembly proves that the unencrypted branch was not compiled away:

| PC | Operation, still unexecuted in the initial native run |
| --- | --- |
| `42000556 / 4200055c` | Read POWER `60096168`, clear FORCE_PD and write |
| `42000560 / 42000568` | Read POWER again, OR FORCE_PU2 and write |
| `42000574 / 4200057c` | Read CONF `60096164`, OR clock1 and write |
| `42000580` | Call original reset helper at `42000502` |
| `42000586` | Poll STATE at `60087028` |
| `42000590 / 42000596` | Read STATIC `60087018`, OR eFuse-flash selection2 and write |

This supports selecting a different official reference. It does not prove READY/completion, SRAM availability under FORCE_PD, key validity or any PHY/RF behavior. The 5.5.1 memory-off reset question remains unresolved; no PC/SDK-specific model bypass was added.

## Separate runtime negatives

The preserved 5.5.1 native log SHA256 is `1fc1c815f468cb501b95860ff24e0c7429c82d7a11f3bdf04419ecc7da7635ce`. Its ModelLimit was at `PC42002112`, write `60096164`, before Key Manager completion; see retained5.5.1 artifact hashes in [result.json](result.json).

The initial6.0.2 native log, `native-idf602-nonwifi-hello-initial.log`, is5898B / SHA256 `4892829f3c0cc2b3f945ff7b2e0ea96b31cd59ef4299958c28c44ebb6f410cc4`. The run stopped after522744 steps/instructions and522688 nominal cycles, UART400B, with `MEPC=4084eeaa`, load access fault `MCAUSE=5`, `MTVAL=408556ab`, width2. The bootloader's `lhu s2,15(s1)` reads a packed revision field at an odd RAM address. It had not reached Key Manager or app_main. This records the model's actual boundary, not a diagnosis of physical-chip behavior. The runtime contract is audited separately; the SDK/example were not changed or rebuilt to avoid it.

## Post-review helper repairs and validation

The build used private `build-helper.py`=21549B / SHA256 `ea8e71a5d96c62d9477f5297ad0d192c37231963bc9c689611815e4b83229f56`. It is retained unchanged with the actual command/log. The current helper's separate hash is in `post_review_verification_helper` in `result.json`; it was used for later read-only inspection, **not retroactively called the builder**.

Two review findings were repaired without rebuilding either firmware:

- Output reservation now rejects the target root and every existing path, including empty/nonempty directories, files, final/dangling/ancestor symlinks, traversal and paths outside the checkout's ignored target. Occupancy is checked before resolution or writes; the final leaf is created with atomic `mkdir(exist_ok=False)`. Historical5.5.1 and6.0.2 outputs are never reused or removed. The actual build directory was newly created for this task; there was no observed overwrite, but the old driver admitted unsafe pre-existing-directory use.
- `inspect` itself now requires one validated C5 bootloader at2000, disjoint flash ranges, erased-FF prefix/gaps and a merged image ending exactly at the last input. Previously only range-byte equality was built in; the original5.5.1 receipt's separate FF/end check is retained as historical evidence, not represented as an old helper capability.

[verify-static.py](verify-static.py) reproduced21 checks: eight unsafe-output cases rejected with canary/tree unchanged, atomic second-writer rejection, RAW append/prefix/gap/missing-bootloader/overlap rejection, five checksum/bounds/ELF-format corruptions rejected, and successful read-only inspection plus **all retained artifact hashes unchanged for both SDK versions**. Test mutations were in memory; fixture canaries live in a fresh ignored test directory. Original BIN/ELF/map/stdout/source/summary files were not altered. Syntax, CLI, diff and privacy checks passed.

## Reproduce or inspect

The default helper profile remains IDF5.5.1/jobs1 for existing callers; choose `--sdk-profile idf602 --jobs8` explicitly. Provide the pinned6.0.2 SDK, its installed matching toolchain and Python. `$REFERENCE_DIR` must not exist. If launching in the background, put launch/control metadata and the initially opened stdout log in a **different** fresh control directory; do not pre-create the output directory for logging.

```sh
python3 tools/c5-idf-hello-reference-build.py build \
  --sdk-profile idf602 --jobs 8 \
  --idf-path "$IDF_SDK" --idf-python "$IDF_PYTHON" \
  --idf-tools-path "$IDF_TOOLS_ROOT" --gcc-bin "$GCC_BIN" \
  --ninja "$NINJA" --cmake "$CMAKE" --work-dir "$REFERENCE_DIR"

python3 tools/c5-idf-hello-reference-build.py inspect --work-dir "$REFERENCE_DIR"

python3 -B docs/evidence/c5-idf602-hello-2026-11-25/verify-static.py \
  --reference-dir "$REFERENCE_DIR" --historical-reference "$IDF551_REFERENCE_DIR" \
  --case-dir "$FRESH_CASE_DIR"
```

All reproduction directories must be in the same checkout's ignored target; the test directory is also fresh. Raw captures remain local for independent runtime validation. Committed records use tool/reference aliases and hashes, omitting personal paths, process identifiers and raw stdout. Build timestamps are provenance/operational metadata, never performance data. No FactoryOnly or551 data was cleared, relabeled, patched or replaced.
