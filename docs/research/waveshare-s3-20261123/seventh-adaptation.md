# Waveshare S3: five storefront families, twelve exact SKUs

This batch adds the source-described digital subsets below. A model registration, a native unit test, and a bounded run of an original manufacturer image are distinct evidence. None establishes physical hardware equivalence or complete application compatibility. Every firmware retains `simulationVerified: false` and `hardwareValidated: false`.

| Storefront family | Exact SKUs | Digital route / revision |
| --- | --- | --- |
| ESP32-S3-LCD-1.9 | 30995 non-touch; 30939 touch | Separate ST7789V2 profiles. Only 30995 has the two GPIO15 RGB LEDs. Touch SKU uses CST816, without treating its disconnected R31 reset pad as a wired reset. |
| ESP32-S3-Touch-LCD-3.5 | 30733 bare; 30934 case and OV5640 | Current V2 ST7796 SPI board, distinct from 3.5B / AXS15231B. LCD CS is physically grounded, TCA reset and power input follow this board. |
| ESP32-S3-LCD-1.3 | 30558 standard; 29962 plastic base; 30559 prism kit | ST7789VW 240-square, original offsets and backlight polarity, real one-bit SD transport. GPIO0 is a strap pad, not an invented physical BOOT switch. |
| ESP32-S3-Touch-LCD-1.85C | 30683 standard case; 30685 BOX-EN; 30684 BOX battery | Current V2 with ST77916, CST816 and ES8311 / ES7210. V1 PCM5101A is not treated as the same audio hardware. |
| ESP32-S3-Touch-AMOLED-2.41 | 30563 bare; 30589 B case | V2 RM690B0 / FT6336U route with GPIO3 touch IRQ, separate expander reset lines and one-bit SD. |

## Runtime and source boundaries

The independent RM690B0 module follows the retained public controller data sheet and the pinned manufacturer's RGB565 BSP command path. It keeps its own reset defaults, GRAM dimensions, pages, brightness and invalid-format behavior. The factory's MADCTR `0x30` conflicts with the public bit description; the implementation explicitly describes a manufacturer-BSP digital transpose subset rather than asserting a measured glass orientation. Unknown commands and reserved formats do not create a successful image. FT6336U report compatibility does not invent undocumented identity register values.

The original manufacturer's full images are retained only in the download cache, not committed to Git. Source URLs, complete image hashes, ROM identity, exact runner/source hashes, UART evidence, pixel outputs and stop states are preserved in `seventh-verification/`. All six board profiles were attempted with the original S3 ROM and a prepared 64 MiB FAT32 card where physically present. No firmware patch, synthetic replacement application, or permissive PMU ACK was used.

| Original image attempt | Observed result |
| --- | --- |
| 1.9 non-touch and touch | Five guest seconds; four display frames, 108800 written pixels, one-color terminal image; card mounted. QMI / BLE progress remains incomplete. |
| 1.3 | Five guest seconds; display initialized and backlight enabled, but zero written pixels; card mounted; BLE-stage progress remains incomplete. |
| 3.5 V2 | PMU configuration returned `ESP_ERR_INVALID_STATE` around 669 guest milliseconds. No display frame. The unsupported PMU engines are not acknowledged as implemented. |
| 1.85C V2 | About 1.67 guest seconds, 90 frames and 52 terminal colors; SD mounted and the onboard-parameter page appeared before `Unimplemented(1107407604, 3902473390)`. This is an observed core stop, not a successful full application run. |
| AMOLED 2.41 V2 | Five guest seconds; eleven frames and 270000 written pixels, one-color white terminal image. Raw GRAM and display state confirm the image is not merely hidden by brightness; initialization stops progressing at the bounded BLE stage. |

Three additional runs record raw panel state for the single-color / zero-pixel cases. They are diagnostic observations of the same original images, not alternate firmware. Raw RGB565 output is retained as deterministic gzip, with PNG renderings for review. UART normalization and original byte hashes are recorded beside each attempt.

The 1.3 factory program is selected from the official 169812763-byte archive via an explicit `zip-bundle` descriptor. The exact ZIP SHA, BIN SHA and member path are retained. The audited archive declares 5470 entries / 521594027 uncompressed bytes, within the integrated bundle reader's 256 MiB compressed / 1 GiB declared-total contract. It uses a manual metadata provider; no fictitious GitHub release API was generated.

## Mechanical provenance and review

The models retain every source solid, free shell and face from the official 1.9, touch-1.9 and 3.5 STEP assemblies. Source indices bind the actual BOOT / power surfaces and the two non-touch 1.9 RGB devices. The 1.3 official RAR contributes the board's 38 original components and two real hollow case / prism-stand STEP files. These files are incomplete manufacturer references: absent large parts are not fabricated. The prism stand is presented in an explicitly unassembled reference position, because its exact assembly pose was not established.

The 1.3 module data sheet supplies the actual 23.4 mm square active area. Other records distinguish a manufacturer optical window from a separately unknown LCD active area. The touch-1.9 engineering drawing's 23.40 mm CG VA is preserved alongside the storefront photograph's 23.10 mm disagreement. No CAD was stretched to conceal it. The 1.3 native base and prism stand also retain their actual CAD dimensions even where current product-photo dimensions differ.

For the 3.5-C case, 1.85C cases and 2.41-B case, external photographs / drawings constrain the visible envelope. Separate front frames, hollow side walls and back covers preserve real empty cavities and optical apertures. The 3.5-C drawing establishes the 7.30 mm front corner radius and 0.60 mm front lip. The other wall and back-cover thicknesses are explicitly estimated, and unresolved recesses, fastening, battery geometry, camera mounts and hidden PCB geometry remain UNKNOWN. Case appearance does not establish internal mechanical fit.

`model-views/` contains twelve actual production-loader four-view plots, original-image overlays at one source pixel per output pixel, and seven expanded inspection plots. Each result binds its final model SHA, original image SHA and renderer inputs. Review plots use CPU geometry colors and an identified screen swatch; they do not claim browser / WebGL texture decoding or real firmware pixels. Original firmware screenshots are the separate attempt artifacts.

The checks cover finite geometry, exact source byte hashes, original dimensions, every declared model control, reverse-face occlusion, framebuffer rays, circular corners, RGB indices, material isolation, reversible inspection and readable UNKNOWN layer visibility. Five approximate case models additionally prove an empty axial cavity, a real lateral inner wall, an open front optical aperture and a separate closing rear cap. Actual manufacturer hollow CAD is retained without substitution.

## Validation and integration

- Existing source-driven board protocol tests: 65 passed; 12 external-input tests remained explicitly ignored. Independent RM690B0 tests: 2 passed. Final small-SPI control regression: 2 passed.
- Final `cargo clippy -p esp32s3 -p esp-soc -p esp-periph --all-targets -- -D warnings`: passed with one build job.
- Twelve new knowledge records passed the repository's exact exported `validateKnowledge` contract. Product, firmware and watchlist fragment entries passed the current integration branch's JSON schemas.
- Twelve final models passed the production Three.js loader's CPU geometry / input contract. Original source-image bytes and UV boundaries were checked; these Node checks do not substitute for the browser image decoder.
- Fixed-release UIFlow and Xiaozhi metadata investigations remain bounded source findings. Exact named Xiaozhi release candidates do not become runnable entries without their own binary audit. Source targets and similarly named boards are not generic substitutes.

Per-listing fragments are under `tools/waveshare-batches/s3/`. Central catalog, chip-memory profile, CLI / WASM routing, actual public Worker attempts and the incremental Sites patch are applied by the integration lane. No upstream GitHub push is part of this batch.

## Optical inspection alignment correction

The 3.5-C, all three 1.85C case variants, and 2.41-V2-B now keep the front frame, available optical glass/photo and live framebuffer in a single inspection group. The prior separate offsets visibly displaced the screen from its aperture; changing both the model plan and rebuilt model corrects the actual expanded geometry. Native LCD1.3 base/prism pieces are independent removable supports, so their existing separate motions remain explicit.

`seventh-verification/optical-inspection-regression.json` records the production loader matrices, nine unobstructed screen/aperture rays per corrected model and exact restoration. The new expanded views, loader checks and review index reference the rebuilt model hashes. Assembled geometry and native source assets are unchanged. These are CPU geometry/inspection checks, not browser/WebGL or mechanical-fit acceptance.
