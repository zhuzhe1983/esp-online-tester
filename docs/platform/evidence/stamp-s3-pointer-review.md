# Stamp-S3 cap pointer to guest verification

S007, S007-PIN127 and S007-PIN254 each passed independently in the public workbench with their final model metadata and the source-built IDF GPIO0/RMT21 fixture. The test uses actual mouse down/up on the visible 3D cap, and registers a fresh UART0 waiter before each edge. UART0 is kept separate from the duplicate USB console stream, which can interleave in the combined log.

Each run starts with guest red `[255,0,0]`. Press produces fresh `STAMPS3_BUTTON 0` and `STAMPS3_RMT 0 255 0`, with observed and visible green `[0,255,0]`. Release produces fresh `STAMPS3_BUTTON 1`. No API `input`/`press`, synthetic serial injection, host-set LED or firmware substitution is used. The fixture is internal protocol evidence, not official application compatibility.

Per-product `pointer-runtime.json` records exact fetched model/geometry bytes, firmware segment/flash/ROM hashes, guest times, actual queued input events, RGB output and before/held/released image hashes. Actual CDP response bodies identify the frozen test WASM as `fb3f3e469e467058785f10d6d8c2ca84ca06498b065db52d6043f68837f4ad41`; filesystem build timestamps are not used as evidence of what the browser ran. Each loaded model/geometry matches its committed asset. The task browser was closed after verification.

Reproduce after building the documented internal fixture and opening a task-owned headless Playwright wrapper session on the supplied server:

```sh
python docs/platform/evidence/stamp-s3-pointer-review.py m5stack-s007 --url <workbench-url> --fixture-dir <fixture-build-directory> --session <task-session>
```

Repeat independently for `m5stack-s007-pin127` and `m5stack-s007-pin254`; then close and verify that exact task session. The script changes neither public default firmware nor model assets. A dedicated hidden upload input constructs browser Files without the workbench upload handler's asynchronous reconfiguration. It does not deliver button events. Source/build instructions and runtime limitations remain in `docs/platform/stamps3-runtime.md` and `tools/fixtures/stamps3-firmware/README.md`.
