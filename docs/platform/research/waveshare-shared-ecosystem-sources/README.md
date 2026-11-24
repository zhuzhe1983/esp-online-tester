# Shared UIFlow / Xiaozhi discovery, 2026-11-23 UTC

The adjacent JSON files are **generated inspection receipts**, not byte-for-byte HTTP responses. The recorded repository commits were fetched by `git clone --depth 1 --filter=blob:none --no-checkout`; all board paths were enumerated with `git ls-tree -r --name-only HEAD`. Commit identity pins the tree; report hashes must not be presented as raw manufacturer-page hashes.

UIFlow `m5stack/uiflow-micropython` at `a61a29409f09f1f4bebae05e15c4bd6dcc4956b6` contains no board path with the Waveshare name. This is a bounded negative finding for that tree, not proof that no unofficial port exists. Generic ESP32 firmware is not an exact Waveshare factory image.

Xiaozhi `78/xiaozhi-esp32` at `92d400d6374bbe750e0a0188fa969fc4042820c2` contains the exact Waveshare board source directories listed in its receipt. A directory match establishes source support only. Hardware revision, optional display/camera assembly, source configuration and a complete distributable image must still be matched per SKU. It does not establish factory role, successful emulator boot, hardware validation, or cloud service availability.

Manufacturer repository and exact board documentation searches remain separate. Per-product knowledge records retain these boundaries and any original factory or demo-image receipts.
