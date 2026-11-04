# StampFly v1.1 / K138-V11

K138-V11 has its own product ID, `m5stack-k138-v11`, knowledge record and factory default. The original K138 remains separate. The official v1.1 page identifies a Stamp-S3A controller and 320 mAh battery. This record does not treat a firmware version number as a hardware revision.

## Factory image identity

The [official v1.1 page](https://docs.m5stack.com/en/app/StampFly_v1.1) links `K138-V11-StampFly-V1.1-user-demo_0x0.exe`. Its .NET resource `MiniBurner.Resource1.resources / firmware` is a `System.ByteArray`, with `StartAddress` set to `0x0`. Parsing with dnfile 0.18.0 did not execute the installer.

The resource is byte-identical to the official repository's [v1.0.0 image at commit 2b96752](https://github.com/m5stack/M5StampFly/blob/2b96752c72c465c63b260996046c71c7675f81b2/firmware/stampfly_firmware_v1.0.0_20240802.bin): 861,360 bytes, SHA-256 `d903acc1815cb8b08224aa4e9a581bb23b69b606813f9edbf4af5002efda113b`. The same complete sequence occurs once in the installer, at byte offset 6,775,142. The platform's merged-image validator accepted its ESP32-S3 identity, bootloader at `0x0`, partition table at `0x8000`, application at `0x10000` and 8 MiB capacity.

The current generic M5Burner StampFly project instead publishes v1.1.0, whose hash differs. It is not used to overwrite this v1.1-specific factory default. The watchlist observes upstream releases but has no automatic catalog target until a new release is explicitly mapped to K138-V11. The investigation found no exact StampFly UIFlow1/UIFlow2 or Xiaozhi board/image mapping in the recorded official categories and upstream board directories; generic Stamp-S3 images are not flight-controller firmware.

This is an identity and layout check. The firmware remains `trial`, with both simulation and hardware verification false. Neither the installer nor firmware was executed or flashed during this source audit. Firmware and installer binaries remain outside Git and site assets; the catalog links the official immutable file.

## Mechanical and electrical boundaries

The official specification says 73.6 × 73.6 × 32 mm. Its dimension drawing also gives a 65 mm diagonal motor spacing and 36.2 mm guard diameter, which imply an approximately 82.2 mm outer guard envelope for the shown square arrangement. The 73.6 mm dimension line does not span the outer guards. Preserve the manufacturer's fields and drawing separately; do not scale the Ecosystem's approximately 81.842 mm assembly to force agreement.

The v1.1 structure section does not supply a complete v1.1 assembly. This model retains the Ecosystem frame, carrier, motors and adapter without scaling, and replaces its coarse Stamp-S3 part with the exact S007-V033 Stamp-S3A cover STL. The cover is 18 × 24 × 4.7 mm; the separate printed sheet is 17.6 × 23.2 mm and remains inside its supporting face. Source coordinates, selections and transforms are recorded in [model-plan.json](../../web/platform/assets/m5stack-k138-v11/model-plan.json).

Official 1200 × 1200 top and bottom photos supply separate masked PCB textures. Four mounting-hole centers rectify each PCB plane, and texture projection is limited to its actual source triangles. The bottom optical-flow daughterboard is absent from the simplified source: its visible 13.2 × 23.2 mm outline and exposed photographic face are added as an estimate, with ±1.5 mm uncertainty in seating height. Its unknown inner face remains marked `UNKNOWN`. This does not reconstruct hidden circuitry or certify component heights. Four visible screw heads use photograph-supported hex recesses; hidden screw threads and fittings remain unknown.

The four propeller blades use a recorded v1.1 photo outline, with mirrored handedness for alternate rotors. Blade thickness, pitch, optical transparency and the hub remain visual approximations. The older battery is retained only as raw source material and omitted from the displayed assembly: the chosen official photos show no installed battery, and the source pack cannot establish the dimensions of the v1.1 320 mAh pack. The full source envelope is approximately 81.842 × 81.842 × 31.528 mm, so the manufacturer's size comparisons remain explicitly failed rather than being hidden by rescaling.

Stamp-S3A's module RGB differs electrically from original Stamp-S3: in [schematic v0.3.3](https://m5stack-doc.oss-cn-shenzhen.aliyuncs.com/1150/Sch_StampS3_v0.3.3.pdf), G38 (`DISP_BL`) drives U2 AW35122FDR enable. Its `BL_3V3` output powers U3 WS2812; G21 is the data input. This is separate from the flight controller's G39 RGB. The older carrier schematic does not override this module-specific supply gate.

## Reproduction and retained evidence

The [knowledge record](../../web/platform/knowledge/m5stack-k138-v11.json) holds source URLs, retrieval timestamps, sizes and SHA-256 values. [source-audit.json](evidence/m5stack-k138-v11/source-audit.json) lists retained reference assets. [factory-firmware-audit.json](evidence/m5stack-k138-v11/factory-firmware-audit.json) records the payload comparison and bounded UIFlow/Xiaozhi findings.

Run the local retained-asset check:

```sh
node tools/verify-stampfly-v11-sources.mjs
```

To repeat the complete source-byte check, retrieve each fetched source URL into a temporary directory using its recorded `archiveName`, then run:

```sh
node tools/verify-stampfly-v11-sources.mjs --source-dir /tmp/stampfly-v11-audit
```

Mutable HTML and API endpoints may now have different bytes; such a mismatch identifies a new source snapshot and must not be silently accepted. The command only reads bytes and validates image layout. It does not execute an EXE, emulate firmware or flash a device.

Manufacturer photos, drawings and schematics retain their original rights; they are reference material, not relicensed under the repository's software license. Raw HTML/API responses, user profile fields, installer and firmware binaries are omitted from committed evidence. Hashes identify the original responses without retaining unrelated personal information.

The model can be rebuilt without network access using the pinned local source files:

```sh
python tools/rebuild-product-model.py plan m5stack-k138-v11
python tools/rebuild-product-model.py build m5stack-k138-v11 --scratch /tmp/stampfly-v11-rebuild
```

Use the dependencies in `tools/requirements-model.txt`. A fresh scratch directory preserves prior comparison results. The [verification receipt](evidence/m5stack-k138-v11/verification.json) distinguishes source-fidelity checks, dimensional discrepancies, photographs and browser interaction checks; it does not claim complete mechanical or flight validation.
