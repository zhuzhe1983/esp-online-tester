# Additional exact-SKU production Worker evidence

U202 Unit C6L passed an independent public runtime/unchanged production Worker run against central020 WASM SHA-256 `0cc18d96042c0b2707afbe3b80102cabcbdd6868fccfcc84504cabed8b3179a2`. Its catalog button uses `transport:'board'`, corrected at `390c407a`: P0 is an input of PI4IOE5V6408 at0x43, not a fabricated ESP32 G0 switch. The guest programs real C6 I²C on SDA10/SCL8, reads register0x0f/P0 and drives the SSD1306 through SPI CLK20/MOSI21/CS6/DC18/RESET15. The64×48 native window uses controller column32. Press/release moves the one white pixel from(0,0) to(0,1) and back; RGBA hashes and consumed guest UART are retained.

Public reset while the key is held creates a new guest, releases the input, clears old serial and repeats the real input/output sequence. A separate original diagnostic on incorrect SDA11 observes an actual I²C NACK, keeps its last safe released state and cannot report `KEY DOWN` after the same public physical-key input. Guest time is observed to advance during that negative interval. The diagnostic explicitly checks NACK before interpreting FIFO data; an absent reply is not treated as a key press.

The [U202 receipt](evidence/m5-coverage-worker-20261126/u202-worker020.json) retains both diagnostic hashes, source/runtime/catalog/Worker/WASM hashes, actual UART, frame hashes, reset results and the wrong-route observation. This is an authored SRAM RV32 app-direct diagnostic, not manufacturer firmware, ROM/Flash boot acceptance, radio propagation, buzzer, USB or physical hardware validation. Existing source-only and trial firmware boundaries remain.

```sh
M5_COVERAGE_PRODUCT_IDS=m5stack-u202 \
ESP32SIM_WASM=<immutable-central020.wasm> ESP32SIM_ROM_DIR=<rom-directory> \
M5_COVERAGE_WORKER_RECEIPT=<receipt.json> \
node --test tools/platform-m5-coverage-worker-test.mjs
```

The final shared harness independently exercises all11 product identities below. Each has a fresh product-scoped runtime, a positive image, a separate wrong-route image and held-input public reset followed by a new press/release. The [combined receipt](evidence/m5-coverage-worker-20261126/all-eleven-worker020.json) and individual `<productId>-worker020.json` receipts retain actual guest serial, input guest times, output hashes or observed LEDs, and every product's existing limits.

| Exact SKU | Guest-consumed input and actual output | Independent negative |
| --- | --- | --- |
| C148, K148 | Physical G11 user key → LX7 GPIO read → guest UART | Guest reads unrelated G10; real G11 press cannot become `KEY DOWN` |
| K129, K129-V11 | Physical G42 switch → LX7 GPIO read → real G21 RMT red/green; V11 additionally supplies RGB through G38 | RMT G20 has an actual completion IRQ but publishes no LED frame |
| K125 | Physical G41 → LX7 GPIO read → G35 RMT red/green | RMT G34 publishes no LED frame |
| C126-Ext | Physical G41 → LX7 GPIO read → guest UART | Guest reads unrelated G40; real G41 press cannot become `KEY DOWN` |
| K124-B, K123 | G37 → LX6 GPIO read → actual ST7789 pixel; hardware I²C21/22 enables AXP192 LDO3 | Wrong MOSI22 cannot reach the SPI18/23, CS5/DC19/RST15 panel |
| U202 | Actual expander P0 read over I²C10/8 → SSD1306 SPI output | SDA11 NACK; no real key read or false press |
| C149 | G9 → true H2 RV32 GPIO → RMT11 red/green, supplied through G10; G4 blue kept off | RMT8 completes without publishing a real LED frame |
| K016-C | G37 → LX6 GPIO → actual ST7735SV pixel on SPI13/15, CS5/DC23/RST18; AXP192 LDO2/3 enabled through I²C21/22 | Wrong MOSI22 leaves the display black |

The Capsule/AtomS3U/NanoH2 negative images produce `leds:[]`: no first RMT update was published. They do not fabricate a black RGB frame. An earlier harness incorrectly expected a black publication after disconnecting the route; its failure occurred after both positive/reset runs had already succeeded. The retained attempt summary distinguishes that assertion mistake from the separate raw-image descriptor, RV word encoding, H2 interrupt-register and actual U202 metadata issues. No Rust/board behavior was changed to satisfy those tests.

These are deliberately bounded digital subsets. PowerHub/Ext do not gain invented LEDs, PMU or expansion behavior; Capsule's physical switch is polled and does not prove sleep/wake or RTC alarm operation. Other sensor, audio, radio, USB and factory application results keep their existing individual boundaries. The optional `M5_COVERAGE_WASM_SHA256` input pin permits reproducing the frozen020 result without making future builds inherit that artifact hash.
