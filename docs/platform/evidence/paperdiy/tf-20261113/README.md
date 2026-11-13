# PaperDIY TF refresh, 2026-11-13

The unchanged official User Demo v1.0 now mounts the default SPI card and completes its B-button 1 MiB write, read-verification and deletion path. The old main screen displayed `NO TF CARD`. This is a board-specific SPI/power/card-detect fix; no guest firmware, shared bus or browser worker implementation is patched.

- `sources.json`: fetched official firmware/source/schematic hashes and the U5/DCDC supply route.
- `factory-wasm.json`: a fresh run of the actual page worker and final production WASM, official serial output, frame hashes, FAT32 sector checks and exact card-image persistence across reset/new worker.
- `checks.json`: build, lint, native/JIT tests, manual framebuffer observations, initial failures and their resolution.
- `cpu.json`: three alternating native pairs for each hello target and Pocket Tank, with identical console and instruction output required in every run.
- `compare-cpu.py`: caller-selected trees and firmware; no private path or remote host is embedded.

Reproduce the functional probe after building WASM and installing the verified mask ROMs:

```sh
tools/wasm-build.sh
node tools/probe-paperdiy-factory.mjs \
  --url https://m5burner-cdn.m5stack.com/community/1/2110309776206622722/2110309776210817025/bin/2110309776210817026.bin \
  --sha256 d7de13e62342c1ab7115897a4a731abb054efda8ecb01d711aeb66c8bc6baa37 \
  --output <local-frame-directory>
```

The automatic probe checks every byte of the retained benchmark sectors and both freed FAT copies. The official program deletes its temporary file after its own read verification; there is no claim that this file remains in the directory. Its successful numeric WRITE/READ display was separately inspected in the final captured frame. Test storage is isolated and memory-backed, so the fresh-worker persistence assertion covers the runtime storage lifecycle, not a physical card or an IndexedDB crash guarantee.

The initial native suite lacked the P4 images that CI fetches, so its ten input failures are retained in the checks receipt. After running the pinned P4 and MicroPython fetch scripts, the full release suite passed without modifying goldens. Rust stable 1.100 introduced range-loop lint failures in existing code; the separate `8b7c84d5` commit makes localized equivalent iteration changes. The CPU comparison checks that cleanup plus the PaperDIY route; it is a regression check, not a new timing or execution mechanism. Native main is the clean committed `0a11637395496587ee6ff539ccce0f1aca7c0be5` source; its Rust source matches this lane's base. Source hashes identify the PaperDIY addition measured before its final evidence commit.

The screen's `16-Levels` text is printed by the firmware and does not establish gray optical emulation. Only black/white digital display response is accepted. Wi-Fi scan finds the configured virtual AP twice; Internet, physical RF, FPC, and real PMIC sleep/power cycling remain unverified. The displayed card throughput is calculated from simulated guest time.

Only curated public sources, hashes, measured values and relevant guest output are committed. Raw logs, frames, the complete SD image and third-party firmware bytes are not committed. No physical hardware was used.
