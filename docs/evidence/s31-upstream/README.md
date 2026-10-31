# ESP32-S31 upstream contribution receipt

Status for [EX260](../../experiments.md#ex260) and docs in
[`docs/esp32s31.md`](../../esp32s31.md).

## Acceptance (upstream)

| Item | Status |
| --- | --- |
| `esp_soc::Machine<S31>` / `Soc` / `SocBus` | Compiles; SRAM load + interpreter steps; UART0 console drain; CLIC/matrix IRQ presentation |
| Flash-MMU `--boot app` | Not ready (clear `Err` from `boot_app`) |
| IDF v6.1 `hello_world` (`--preview set-target esp32s31`) | **Blocked** on this Mac worktree (IDF 5.3/5.4 only). See `tools/fetch-s31-firmware.sh` |
| Metalio INK4 Plus product image | **Not** an acceptance target; historical bring-up exploration only (superseded) |

## Oracle / license

Espressif **esp-emulator 0.51.0** (MIT) was used as an **observable oracle only**
(register/behaviour observations that informed EXT_ILL defaults and ROM probes).
This tree is **not** a source port of that emulator.

## Bringup vs Machine

- `esp32s31::Machine` — review path for mikroverk/esp32sim PR integration.
- `esp32s31::Bringup` — temporary fixture runner for hermetic / `external_*` probes.

## What to pin when IDF 6.1 is available

1. Build `hello_world` (+ bootloader + partition table) with IDF v6.1 preview target `esp32s31`.
2. Record SHA-256 of the three binaries in `tools/fetch-s31-firmware.sh` (mirror of `tools/fetch-p4-firmware.sh`).
3. Place sdkconfig under `esp32s31/firmware/` (placeholder directory today).
4. Re-run Machine boot from mask ROM / flash image and attach console receipts here.
