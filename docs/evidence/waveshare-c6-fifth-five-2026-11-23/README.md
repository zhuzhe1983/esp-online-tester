# Fifth five Waveshare C6/C3/H2 listing groups

This batch covers five official listing groups and ten exact sale SKUs. Four groups have partial C6/C3 digital runtimes (five boards because LCD1.9 has distinct touch/non-touch hardware). ESP32-H2-Zero remains explicitly `esp32h2`, `runtime.status=unavailable`, `board=null`; it is source/model coverage and must not be counted as runtime-adapted. Independent H2 runtime work follows separately.

| Official listing | Exact sale SKUs | Current runtime / original-image result |
| --- | --- | --- |
| ESP32-C6-LCD-1.9 | 30937 without touch; 30941 with touch | 8MiB C6FH8; source ST7789V2/SPI, low-active backlight, exact shared TF. Touch-only CST816. Original non-touch image at5s stops after LCD creation with noRAMWR; touch image has6RAMWR then original BLE encrypt assertion. Both actually read3 card blocks with64MiB FAT32. |
| ESP32-C3-LCD-0.71 | 29327 bare; 29512 protective case | True4MiB C3/GC9D01 digital565 path, fixed backlight. Original complete example runs5s without CPU exceptions and1623 framebuffer colors. Narrow eye capture remains an unresolved whole-frame observation. No touch/TF/IMU/RGB invented. |
| ESP32-C6-LCD-1.47 | 28563 bare; 30381 header | True4MiB C6FH4/ST7789/X34 window, source BGR/mirror/byte order, real shared TF and RGB8. Original4MiB example reports SD initialization success with4 read blocks, then BLE encrypt assertion before drawing. |
| ESP32-C6-Zero-B | 34981 bare; 34919 header | True8MiB FH8, BOOT9/RGB8/GPIO7 RF-control digital subset. Official BLEAssist ZIP is only one Android APK, not an ESP image; no default firmware invented. |
| ESP32-H2-Zero | 32261 bare; 32260 header | True H2 chipID16/4MiB.18 upstream example images are pinned references; representative blink image boot/app/partition checks pass. No C6 emulation or ROM is assigned and no H2 image was executed. |

## Source and model evidence

`integration-map.json` records the exact product/board/capacity mapping. The five independent fragments in `tools/waveshare-batches/c6/` include ten full product knowledge records, current catalog/watchlist schema validation, physical connections, source conflicts and the original firmware/archive identities.229 original primary source receipts are retained at `docs/platform/research/waveshare-c6-fifth-five-sources/manifest.json`; original electrical documents and code are compressed without content editing. Complete binaries remain remote links with full hashes and exact ZIP member names.

All ten models passed the real production Three.js loader, source-hash checks, topology bounds/dimensions, active display-plane ray tests and applicable UNKNOWN-label construction checks. Every four-view geometry plot in `model-review/` was opened and inspected. These are engineering geometry plots, not WebGL/material or hardware-fit acceptance. Six models preserve original CAD; four basic-board models use explicit18-mm source XY and visibly UNKNOWN PCB structures, with clearly estimated USB/header geometry.

The C6LCD1.9 official resource archives contain S3-named native STEP assemblies. All283/286 native components and millimetre scale are retained; C6 electrical-layout/revision identity remains unverified. The touch source total depth differs from its product drawing and is not scaled to fit. Both native assemblies contain the22.7×42.72-mm LCD active face; the touch23.1×43.12-mm viewing opening is retained as a distinct larger aperture.

C3LCD0.71 native29-component CAD retains the18-mm active circle and its actual center. It omits several IC/packages visible in current photos; missing geometry is not invented. Case SKU29512 has no retained protective-case solid/dimensions, so the explicitly partial model shows the bare assembly inside an UNKNOWN, unrendered case. LCD147 preserves all184 native components and17.39×32.35-mm active face; M headers distinguish measured pitch/count from estimated depth and pin cross-section.

The review plotting tool now limits its historical S3 photo-registration constants to the exact S3 product IDs. H2/C6 Zero names no longer inherit an unrelated S3 pixel transform. No unregistered photo overlay is included in this batch.

## Verification and limits

Native tests: C3 four and C6 twenty-three source-driven board tests passed (including the separately delivered JD9853 column fix). Clippy passed for both libraries, and both release CLIs built. The new checks exercise GC9D01 source format/reset/route and negative cases, LCD147 physical MISO/card selection and source pixel order, Zero-B RGB/BOOT/GPIO7 control and wrong-route rejection. The exact commands and completed logs are retained.

`native-runs.json` and `with-sd/native-runs.json` retain original ROM/image hashes, commands, measured final frames and execution outcomes. Empty64MiB FAT32 media used the project's actual deterministic image generator. Raw UART bytes are preserved in `.raw.log.gz`; readable `.log` files normalize line endings/trailing whitespace only, with both hashes in `log-normalization.json`. Original blank/error frames remain included. CPU exitCode0 alone is not claimed as application success.

`factory-image-verification.json` checks original chip IDs, image checksums/digests, partition-table MD5 and exact flash bounds. C3LCD071 and C6LCD147 filenames alone do not establish shipping-recovery purpose; they remain official example trials. H2 reference entries have no runnable source/default. Wireless, analogue charging/battery/power, private display gamma/timing, full interaction, hardware and browser acceptance remain outside the verified subset.
