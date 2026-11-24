# C5 Touch LCD 2.8 public runtime acceptance

The original project diagnostic runs through the production platform runtime, Worker and WASM in Node. Six display/input states match independent native RGB565 captures exactly. Touch and BOOT release restore the baseline; quiet controls change zero pixels. The whole prepared 64 MiB FAT32 card is byte-identical after the guest block read/write/restore test. This does not claim guest FAT mounting, manufacturer firmware boot, browser UI or WebGL acceptance.

Run `node tools/waveshare-c5-touch169-acceptance.mjs output/waveshare-c5-touch28-public touch28`. Exact input/output hashes, capacity, bounded execution and pixel counts are in [result.json](result.json). Raw UART and framebuffer captures stay under the ignored output directory and the development recovery archive.
