# LCD1.69 revision and Touch2.1 mechanical evidence

Two storefront families, three unique SKUs and four profiles. LCD1.69 V1/V2 are the same sales SKU27932 with independently sourced power-key/control pins. Flat28169 and curved30697 have separate native CAD and optical areas.

## Observed results

LCD1.69 original unchanged factory image ran for5000 guest milliseconds in each revision:13 RAMWR frames,9931 distinct framebuffer colors and no reboot. The visible captured picture was reviewed. This is not evidence that the closed original program uses the correct power-latch pins on both revisions. Actual LX7 S32I/L32I tests separately verify the two GPIO keys, correct/incorrect SPI wires, half/full duplex modes0/3, reset/PWM, QMI8658C and guest-clock RTC behavior.

Touch2.1 and2.1B independently ran the unchanged manufacturer image for5000 guest milliseconds, each yielding248 RGB frames,43 colors,33 SDMMC commands and1744 bytes read from a prepared64MiB FAT32 image. Two initialization timeouts remain in the original counter. The captured Onboard page reports62MB filesystem capacity;64MiB is the complete card image including filesystem/partition overhead. Earlier matched-time touch and later BLE-stage limitations remain in the existing knowledge/evidence history. No replacement diagnostic image is catalogued as factory firmware.

## Geometry

LCD1.69 retains all287 native components. Flat2.1 retains522 and curved2.1B523, including free shells/faces and original millimetre coordinates. Each optical assembly and live framebuffer moves as one inspection group. Flat live area53.28mm follows the exact original dimensioned photograph; the53.88mm CAD visible window remains distinct. Curved AA53.48mm/VA54.08mm and native lens are separately sourced; the framebuffer projection does not simulate refraction.

All three production Three.js CPU loads, dimensions and input rays passed. The assembled four-view plots and original-photo registrations were reviewed. Sources, plan, geometry and final model hashes are in manifest.json and native-loader-check.json. Missing source CAD packages are not invented: the flat upper rear square IC visible in the photograph is absent from the native assembly. Mechanical/electrical revision correspondence, physical switch travel and connector fitting remain unverified.

## Reproduction and boundaries

Run `cargo test -p esp32s3 --test waveshare_lcd169` for the actual LX7 register tests and `cargo test -p esp32s3 --lib waveshare_lcd169` for the focused board check. `cargo clippy -p esp32s3 --all-targets -- -D warnings`, catalogue schema, knowledge and manual watchlist checks passed. Full logs are retained.

For models, use `python tools/rebuild-product-model.py plan <product-id>` followed by `build <product-id> --scratch <new-directory>` after catalogue integration. This uses committed hash-pinned source assets; the source collection helper is not required for model reproduction.

For optional original-image runs, `waveshare_source_attempt` takes exact board, original S3 ROM ELF, original BIN,16,8,5000,output directory and an optional64MiB FAT32 image. Firmware/ROM/executable hashes and terminal states are recorded; manufacturer BIN/ZIP firmware is not mirrored into Git.

No browser WebGL/DOM, complete original application, wireless, physical power/charging, hardware flashing, optical calibration or real hardware acceptance is claimed. Normalization receipts document only checkout/cache path and personal home-label changes; input bytes and observed numeric outcomes remain unchanged.
