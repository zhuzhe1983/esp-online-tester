# Sixth S3 storefront group: five families, ten exact SKUs

This batch adds LCD1.47B (31501/31031), TouchLCD3.5B (31137/31334), TouchLCD1.47 (31202/31199), TouchAMOLED1.75 (31261/31262/31264), and TouchAMOLED1.64 (31197, explicitly PCB Rev1.1). Each has its own source-derived wiring; shared controller protocols do not turn distinct PCBs into aliases. All profiles are 16 MiB Flash / 8 MiB PSRAM from their retained official records. Every physical TF socket starts with a prepared 64 MiB FAT32 image and has an independently routed native SD transport. AM164 additionally models an explicitly selected SPI transport on the same schematic pins; the original BIN uses native SDMMC while the current source example uses SPI.

## Actual original-ROM observations

Every run used the original manufacturer merged BIN, original ESP32-S3 ROM, specified memory capacities, a 5,000 ms guest budget and an independently prepared 64 MiB FAT32 image. A budget stop reported as `Halted` is not an application completion assertion. No diagnostic program replaced a manufacturer application. Initial and final raw outputs are both retained.

| Profile | Final bounded observation | Remaining boundary |
| --- | --- | --- |
| LCD1.47B | 172×320, 0 frames / one color; TF 33 commands, 1,744 bytes read; original UART reports successful mount then starts BLE | Original demo starts BLE before LCD initialization; QMI register subset and radio incomplete |
| TouchLCD3.5B | 320×480, 0 frames / one color; factory returns after PMU configuration / ESP_ERR_INVALID_STATE | Existing AXP2101 model does not accept the complete original power setup; no fabricated register success |
| TouchLCD1.47 | 172×320, 7 frames / 16 colors; readable “Touch testing mode / Exit with BOOT button”; TF 28 commands / 1,616 bytes | First run had a black border because main.c set_gap was missed; fixed by the exact original call chain, not image shifting. Full application/input and physical calibration remain incomplete |
| TouchAMOLED1.75 | 466×466, 498 frames / 1,365 colors; actual Brookesia application home page | Initial STARTUP FAILED/Gravitysphere failure was a missing QMI8658 path. Source schematic U3@6B/VCC3V3=DCDC1 now routes the existing control/sample subset. IRQs, calibration, GNSS, PCM, radio and full interaction remain incomplete |
| TouchAMOLED1.64 Rev1.1 | 280×456, 4 frames / one color; TF 33 commands, 1,744 bytes read; original UART mounts the card, then BLE initialization | Original BIN and current source select different real TF modes; complete application and physical panel/touch direction remain incomplete |

The three final reruns include the original binary identity, ROM hash, actual runner hash, source hashes, guest state and SD image identity. Earlier attempts retain only the identity fields originally captured; extra historical fields are not retrospectively fabricated. Raw UART and little-endian RGB565 are losslessly gzip-compressed; readable UART normalizes line endings/trailing whitespace and escapes non-UTF8 bytes. PNGs are direct emulator frame outputs, without cropping, rotation or reconstructed screen content. Individual artifact manifests preserve raw and retained hashes. All firmware catalog entries remain `simulationVerified: false`, `hardwareValidated: false`, `trial`, and `partial`.

## Source and firmware identity

The batch source manifest pins original bytes, HTTP UTC receipts, exact archive members and fixed repository commits. `sixth-original-archive-audit.json` preserves complete ZIP directories and member CRC/hash identities for the static demo distributions. No manufacturer BIN or ZIP is mirrored into Git or the Site.

LCD1.47B Test.bin and the current AM164-v2 BIN are official demonstrations with complete boot layouts, but their factory recovery role has not been established; neither is silently made the default. Touch1.47, Touch3.5B, and AM175 retain explicitly documented factory defaults. AM175 firmware/README identifies 20260824 as current while the top-level README still mentions 20260805; both original documents are retained. Touch3.5B boot/application chip IDs are 9, independently checked despite its file length matching a classic ESP32 product's unrelated image. Current AM164 pins CS46 and shared LCD/FT RESET21 follow Rev1.1/R34-NC/R35-zero; earlier CS9 is rejected, not accepted simultaneously.

UIFlow and Xiaozhi investigation references the retained shared pinned source tree records and current original release metadata. An exact named release asset is only a distribution candidate until its image and target layout are audited; it does not inherit compatibility. Static ZIP distributions explicitly use the `manual` metadata provider, not a fabricated GitHub update endpoint.

## Mechanical and interaction scope

Ten local assets preserve the manufacturers' full original STEP geometry, stable source component indices, original millimetre dimensions and only rigid transforms. Soldered header variants are separate; where header geometry is absent, it remains UNKNOWN rather than synthesized. Existing CAD solids and open shells are retained. AM175's thin PCB source shell 666 is grouped with its PCB, not moved with the optical assembly. Original CAD bounds, sources and per-component export errors are recorded by the deterministic generator.

Manufacturer labels and exact source XCAF assembly paths identify native BOOT/PWR meshes. No generic hitbox replaces a native button. LCD1.47B LED2 is native component 142 and the real GPIO38 WS2812 chain; wrong-pin RMT input leaves it unchanged. The production loader verifies exposed button rays, opposite-side obstruction, actual framebuffer opening, model inspection restoration and material isolation. Case variants lacking source button apertures do not invent a press-through-wall hitbox; shared 2D/agent controls remain available.

The 3.5B-C and AM175-B have dimensioned front/side/back enclosure drawings but no manufacturer full shell CAD. Their models use independently separated front frames, hollow wall rings and back caps with open optical windows. Estimated 1 mm walls/caps are clearly non-measured. Ports, key apertures, fasteners, internal fit and camera/GPS option geometry remain UNKNOWN. Source CAD is never stretched or truncated to fit an assumed enclosure. AM175 CAD's 12.70 mm normal envelope differs from the 10.40 mm bare-board photograph; the mismatch is retained, with no hardware-fit claim.

The exact LCD AA and optical-window dimensions stay distinct. LCD1.47B drawing explicitly identifies 17.39×32.35 mm LCD AA. Touch1.47's 17.75×32.93 mm CG VA, 3.5B's 48.96×73.44 mm LENS VA, AM175's TP AA/VA annotations and AM164's 22.34×36.06 mm aperture do not independently establish emissive AA. The stated small aspect discrepancies are retained rather than forcing the native model to a convenient pixel ratio.

## Checks and limits

- 59 relevant S3 Waveshare board tests passed, including source-window pixels, invalid buses/CS, controller-specific command rejection, touch reports/resets, active-high expander PWR press/release, QMI power-off NACK, SD CMD0/MISO routing and LED wrong-pin rejection.
- `cargo clippy -p esp32s3 --all-targets -- -D warnings` passed.
- Final native original-ROM runner built successfully and produced the retained outputs above.
- Ten products, five firmware entries, five watchlist entries and ten source-knowledge records passed the current production schema and knowledge contracts; central catalog integration is performed by the parent lane.
- Production Three.js CPU geometry/ray evidence is retained separately. It is not browser WebGL, actual hardware fit, complete application or physical acceptance.

Rebuild the models from their retained `model-plan.json` files using `tools/build-waveshare-models.py` with the parent's open-plate support from commit 896214a3. Do not regenerate any already delivered older model with an old bulk source script; the parent's `open_photo_enclosures.py` preserves the earlier enclosure corrections. Firmware recipes and exact remote members stay in the five listing fragments under `tools/waveshare-batches/s3/`.
