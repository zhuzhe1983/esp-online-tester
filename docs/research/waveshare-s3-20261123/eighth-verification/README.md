# Eighth S3 group: five storefront families, ten exact SKUs

This group adds LCD2 (base/camera), TouchLCD2 (base/camera), the touch/non-touch2.8B listing, plain TouchAMOLED1.43 (bare/metalcase), and TouchAMOLED1.8 (battery/no-battery). Each storefront option remains distinct. Fourteen explicit digital profiles retain the1.43 SH8601/CO5300 uncertainty and1.8 V1/V2 differences; a SKU does not invent a known die or revision.

## Digital scope

LCD2 uses actual SPI ST7789, GPIO1 backlight, real shared-SPI TF onCS41, QMI8658C and the sourced CST816D single-touch subset on the touch variant. GPIO0 is BOOT; the optional reset link is NC and is not fabricated as connected. Camera/ADC/charging are not accepted.

Plain1.43 uses QSPI clock10/data11–14/CS9/reset21, polling FT3168, QMI8658C, PCF85063 and native SDMMC41/39/40 with DAT3 GPIO38. GPIO42's R9 is NC: it is not a display power gate. The exact manufacturer software-serial LCD ID sequence selects the observed SH8601 module-maker OTP86 branch; CO5300 remains explicit unknown/read-FF, not a universal die ID. SH8601 manufacturer's RGB565 BSP versus PDF quad24-bit conflict remains stated.

AMOLED1.8 uses QSPI clock11/data4–7/CS12, I2C15/14, AXP2101, TCA9554, ES8311, QMI8658C, PCF85063 and SDMMC2/1/3. TCA P0 controls display reset, P2 touch reset, P7 SD DAT3 and P4 the source-inverted power-button input. DCDC1 digital availability is modelled independently from analog audio/charging. V1 has FT3168/SH8601; V2 uses the CO5300/CST820-compatible report subset. Unsupported identity reads and PMU engines remain unsupported.

The existing2.8B runtime and exact native CAD models are retained; new source/firmware observations supplement their previous evidence.

## Original firmware observations

Eight configuration attempts of seven real merged manufacturer BINs used the original S3 revision0 ROM,16MiB flash,8MiB PSRAM and the same64MiB FAT32 image, with a5-second guest budget. LCD2 and TouchLCD2 produced58/50 colors and recognized SPI SD; their camera path reports an error. Both2.8B profiles produced58 colors and33 native SD commands. Both1.43 die profiles produced49 display frames but a final2-color circle while near BLE initialization; this is not full UI acceptance. AMOLED1.8 V1 produced201 frames/1304 colors and an early Brookesia home screen. V2 stopped at337ms with its genuine CST identity-read error and abort/reset; no guessed A7 identity or fake ACK was injected.

`original-run-index.json` and per-board attempt/state/UART/raw-frame/PNG files preserve these observations. All firmware entries remain `trial` with `simulationVerified:false`. Static ZIP and fixed manufacturer Git snapshots use explicit manual watch metadata; original BINs stay remote-only, with original-container/member SHA and purpose evidence.

## Mechanical scope and checks

All ten models load through the actual production Three loader. Eight new source-based native CAD assemblies and the two preserved2.8B models have final-hash checks, four views and photo registrations. Native topology and component coordinates stay unscaled. Three photo-derived case envelopes are hollow, with separate caps/walls and actual optical apertures; their hidden walls/assembly are estimates, and side openings, rear branding and screw details remain incomplete. The common1.8 CAD predates V2; exact V2 internal correspondence and battery geometry are unknown. Camera-equipped non-touchLCD2 reuses only the explicitly identified original TouchLCD2-C camera component, with source provenance and fit uncertainty retained.

The original photographs, retained CAD/drawings, dimensions and source receipts are in each model plan.10 source-overlay figures preserve measured pixel registrations and residual perspective/dimension deviations. Expanded views retain rigid optical/frame/display groups and demonstrate cavities. Native BOOT controls have front-hit/back-occlusion checks; unverified case button caps are not fabricated as native control meshes. No browser/WebGL material, cable-fit, physical or complete exterior acceptance is claimed.

## Reproduction

Run the repository model reproduction tool against each `model-ids.json` entry and its checked-in source assets. The model plans and final-hash review receipts are authoritative; no private downloaded BIN is needed for model rebuild. Digital tests: `cargo test -p esp32s3 waveshare_lcd2 --lib` and `cargo test -p esp32s3 waveshare_amoled_compact --lib`. Exact original firmware URLs and archive member paths are preserved in the per-listing fragments and firmware manifest; use `waveshare_source_attempt` with the matching explicit board, original ROM and16/8MiB memory.
