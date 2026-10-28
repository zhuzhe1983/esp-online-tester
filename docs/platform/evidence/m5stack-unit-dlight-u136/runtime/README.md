# U136 runtime evidence

Base: `d3c7820f`; candidate is identified by source hashes in `checks.json`, `probe.json` and `cpu.json`. Native toolchain: Rust1.99.1, macOS arm64. No binary fixtures are published.

Reproduce:

```sh
tools/wasm-build.sh
# ESP-IDF5.5.1 environment, target esp32s3:
idf.py -C tools/fixtures/unit-dlight-firmware -B "$PWD/output/unit-dlight-build" -D SDKCONFIG="$PWD/output/unit-dlight-build/sdkconfig" set-target esp32s3 build
node tools/probe-unit-dlight.mjs --build output/unit-dlight-build
node --test tools/platform-unit-dlight-test.mjs tools/platform-manufacturer-model-test.mjs
node tools/check-dlight-connector-compat.mjs
python tools/rebuild-unit-library.py m5stack-unit-dlight-u136 --scratch output/dlight-rebuild
```

The fixture includes unchanged pinned M5-DLight source. `probe.json` covers13 actual worker/WASM groups; it hashes firmware segments, ROM, WASM and implementation files. Input delivery alone is insufficient: assertions examine guest register reads printed by the firmware. The driver does not compensate H2/MTreg, so its result and a separate datasheet result are retained.

`browser.json` records human studio input and mini scene restoration using the same source-built local fixture. To reproduce, serve `web/` locally, choose StopWatch plus U136 on physical Grove, select `flasher_args.json` and all three images under local firmware, load/run, set4321.5lx, then send `q` on UART0. Readraw5186. In a shared mini scene setinitial120lx, supply the same local files, then call `accessory-illuminance` with777 and send `q` after a conversion: raw932. No fixture bytes are embedded in share URLs. Both browser session and server were closed; no process inventory or private logs are retained.

`rebuild.json` lists every byte-identical asset from the public reconstruction command. `connector-compatibility.json` and its public checker prove default/custom-color closed-roof mesh arrays, indices, transforms and materials are unchanged from the parent. Historical StopWatch/GPS visual reports retain original measurements and add that provenance. Only explicit U136 open-roof uses the new structure. U136 photo/drawing evidence is in the parent directory; transparent and gold views do not replace original-material photo tests.

Performance reproduction uses independent native release builds for base/candidate, matching pinned inputs, one excluded warmup and five alternating pairs. Use caller-supplied directories:

```sh
python docs/platform/evidence/m5stack-unit-dlight-u136/runtime/bench.py --base-ref d3c7820f --base-bins /path/to/base/target/release --candidate-bins /path/to/candidate/target/release --rom-dir /path/to/roms --receipt output/dlight-cpu.json
```

`cpu.json` preserves every sample, user CPU, anonymous load, instructions, binary/input hashes and stdout hashes. `--resume` only skips already completed workloads with identical candidate source hashes. Missing local-input failures and the duplicate-console harness failure remain in `negative-results.json`; no golden was edited. Previous U027/U003/ENV-IV costs remain in their receipts and experiment entries; this is an incremental comparison, not evidence that earlier costs disappeared.

Logs with local workspace paths remain ignored; their SHA-256s are retained in `checks.json`. Screenshots contain only simulator UI and synthetic fixture output. Sources retain public URLs, actual fetch UTC seconds, final URL, MIME, byte size and hashes; there was no hardware test or official-firmware compatibility sweep.
