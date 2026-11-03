# ESP32-S3 coprocessor state after reset (EX248)

The ESP32-S3 now starts with `CPENABLE=0x0f` at construction, ROM boot,
direct app boot, chip reboot and application-core reset/release. Guest writes
still control coprocessor access. The shared Xtensa execution engine and classic
ESP32/LX6 reset behavior are unchanged.

Baseline: `49a277876d7f276d4b7d576e2bc167ffa9078482` (main).
Fix: `f0041b94`. Toolchain: Rust stable 1.99.1 (`58cf8f9c3`, 2026-10-13),
native aarch64 macOS release build. This is a register-reset correctness change,
not a timing or throughput optimization.

## Source contract

The Cadence **Xtensa LX7 Microprocessor Data Book**, 2017 edition, section 9.1,
defines the coprocessor count as the highest configured coprocessor ID plus one.
Table 9-15, printed page 127 (PDF page 161), specifies the LX7 `CPENABLE` reset
value as `(1 << CoprocCount) - 1`. The same table distinguishes this implementation
value from the architecture-wide unspecified reset state. The original Cadence
document is available in this [immutable public mirror](https://raw.githubusercontent.com/larsbrinkhoff/awesome-cpus/7cda4da1c8/Xtensa/Xtensa_LX7_Data_Book.pdf);
SHA-256 `fa6bf57ea5def5fde4336f65250d777550cdc78c72ad1ccadc7c67756175b5f9`.
The table was checked both as extracted text and as a rendered page.

[ESP-IDF v4.4.5's generated S3 tie.h](https://github.com/espressif/esp-idf/blob/v4.4.5/components/xtensa/esp32s3/include/xtensa/config/tie.h#L29-L43)
defines `XCHAL_CP_MAX=4`, `XCHAL_CP_MASK=0x09`, CP0 as the FPU and CP3 as
`cop_ai` (PIE). Thus the LX7 reset expression evaluates to `0x0f`; the count
is not `XCHAL_CP_NUM=2`. Only CP0 and CP3 carry implemented coprocessor state.
The factory image identifies its IDF version as v4.4.5. IDF v5.5.1 has the same
three count/mask definitions. No on-device reset capture was used for this fix.

## Original failure and rejected explanations

Both official images are unchanged bytes from
[m5stack/M5StampFly at 2b96752c72c465c63b260996046c71c7675f81b2](https://github.com/m5stack/M5StampFly/tree/2b96752c72c465c63b260996046c71c7675f81b2/firmware):

| Image | SHA-256 | Baseline result |
| --- | --- | --- |
| `stampfly_firmware_v1.0.0_20240802.bin` | `d903acc1815cb8b08224aa4e9a581bb23b69b606813f9edbf4af5002efda113b` | CP0 exception at `0x42003a76` |
| `stampfly_firmware_v1.1.0_20240812.bin` | `0138cc333d05790f8804fabae223f4c7eb40d162ad1d23d8c8daead6412d4359` | CP0 exception at `0x42003a6e` |

The v1.1.0 image fails in an `Alt_kalman` global constructor, before the scheduler
and board initialization: `lsi f1,a2,208` with A2=`0x3fc9c484` and PS=`0x00060f20`.
The fresh baseline native JIT and `--no-jit` runs fail at the same address, ruling
out a stale binary or a JIT-only instruction error.

The core raises the correct exception cause **32**. The guest handler calls
`_frxt_task_coproc_state`, finds no running task, and then explicitly writes **4**
to EXCCAUSE before reporting its coprocessor panic. The printed cause 4 is not
evidence of an emulator level-1 interrupt misclassification. The relevant
[IDF handler](https://github.com/espressif/esp-idf/blob/v4.4.5/components/freertos/port/xtensa/xtensa_vectors.S)
and [FreeRTOS port](https://github.com/espressif/esp-idf/blob/v4.4.5/components/freertos/port/xtensa/portasm.S)
match that path. No CPENABLE firmware patch, instruction stub or forced per-step
enable was applied.

Scanning all ROM, bootloader and app load segments for WSR/XSR CPENABLE encodings
found only four candidate writes, all in app code: `40379747`, `4037df66`,
`4037dfb8`, `4037dfdf`. With breakpoints at all four and the failing LSI, the LSI
is reached first. Core 0 has retired 5,568,382 instructions, core 1 has retired
495,936, with 429 prior exceptions and no interrupts. The firmware relies on the
LX7 reset value before FreeRTOS begins managing the register.

## Validation

```
cargo +stable test --release -p esp32s3 --test coprocessor_reset --test irq_release
cargo +stable build --release -p esp32sim --bin esp32sim
```

All five tests pass. The two new tests cover construction, both-core reset, ROM
boot, direct app boot, chip reboot, successful reset-state FPU load and an actual
guest WSR CPENABLE followed by a cause-32 trap without modifying the FPU result.
The existing three interrupt-release tests cover application-core reset versus
clock hold/stall, including the reset value across repeated releases.

Original-image command (substitute the verified image and local ROM paths):

```
esp32sim --board stamps3 --flash-image "$IMAGE" --flash-mb 8 --psram-mb 0 \
  --boot rom --rom "$ROM" --max-seconds 1 --console both --no-dump
```

Both versions now reach scheduled execution with no coprocessor panic or reboot
in the one-second run. v1.1.0 also passes with `--no-jit`, with identical per-core
instruction counts, interrupt counts and RMT results. Each image makes three
WS2812 updates, ending at RGB `[2,0,0]`, and six RMT transmissions. This uses the
existing `stamps3` board deliberately: it establishes removal of the core blocker,
not full StampFly sensor, radio or flight behavior. The product-specific runtime
qualification is separate.

Both existing S3 workloads ran for 30 emulated seconds on main and the fix,
using the same ROM, firmware, board settings and pocket-tank model:

| Workload | Core 0 instructions, both revisions | Core 1 instructions, both revisions | Console SHA-256, both revisions |
| --- | ---: | ---: | --- |
| hello | 11,990,305 | 6,798,164 | `c74f198e19e94c47772b3403433b5532057180aa63df46ff90ffe9a51710966f` |
| pocket-tank | 4,595,734,479 | 6,171,560,740 | `5f9d2aecbb20769a9f076516c33955f82a9fa13fba79249edde134aed8986e0a` |

Cycles, exception counts and interrupt counts also match for each pair. No golden
files were regenerated. The `--console uart0` output bytes are hashed directly,
without normalization. The input manifests are `web/wasm/fw/hello.json` and
`web/wasm/fw/pocket-tank.json`; the latter's model is fetched by
`tools/fetch-pocket-tank.sh`. Add `--boot rom --max-seconds 30 --no-dump
--console uart0` to each manifest's board, memory and file arguments.

The compact [results](results.json) retain counts, output hashes, source/binary
hashes and the hello/pocket-tank before/after comparison. Full raw traces and
console logs remain local, outside Git; only the fault facts needed to reproduce
the diagnosis are retained here. Absolute machine paths and wall-clock speed
lines are omitted. No throughput claim is made from correctness runs concurrent
with unrelated build work.

## Integrated StampFly CPU follow-up

The [integrated comparison](integrated-cpu.md) measures the complete StampFly
revision against the same main baseline with five alternating pairs on S3 hello,
Pocket Tank, C3 hello and C6 hello, all at 30 guest seconds. It retains the initial
S3 increase, a separate confirmation and substantial shared-host drift; it does
not turn the original correctness-only runs into throughput evidence.

## Experiment relationship

Search terms included coprocessor, CPENABLE, floating, LSI, exception and Xtensa.
EX057 and EX140 concern floating-point result timing/readiness, not reset state;
neither patch is retried. EX248 is a distinct register-reset correctness question.
The negative baseline and the initial architecture-wide-versus-LX7 reset-state
ambiguity remain recorded above.
