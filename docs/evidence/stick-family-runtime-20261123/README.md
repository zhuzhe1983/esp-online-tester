# Combined M5Stack Stick runtime checks

The candidate `c0125b842932f4e3a4a17def16c021923eece745` integrates StickC K016-C, StickC Plus K016-P, StickT2 K016-T2 and StickS3 K150 onto `f61aae75fe4733fde6368d0ea9f9b8f4ef48019a`. The product checks used the 6,926,500-byte production WASM built at `cd49997f178a7aa95b283004e5c80963fafcc9f4`. SHA-256 `e01badb6b24df012ec97eee06592d5e96d430ef18d497cee8101054efdee8e15`.

| Model | Guest results |
| --- | --- |
| StickC | The original V0.0.1 image reached the 80×160 IMU, RTC and microphone pages. IMU changed two poses and settled at center. Two PCM tones reached the guest, with silence and a wrong-port input as controls. [Runtime checks](../../platform/stick-k016-c-runtime.md) |
| StickC Plus | The original v1.0 image reached RTC and the microphone page. Holding A produced a 4 kHz tone from guest GPIO2 in the browser. [Browser receipt](../../platform/evidence/stick-k016-p/browser-verification-20261123.json) |
| StickT2 | The original v1.0 image changed its actual heatmap in response to the synthetic Raw14 profile and changed palette with A. The embed and recording replay retained the guest results. A modified image failed the pinned hash check. [Browser receipt](../../platform/evidence/stickt2-20261122/browser-20261123.json) |
| StickS3 | A source-built diagnostic reached the LCD, confirmed keyboard and 3D A/B controls and observed a BMI270 reading of 4096 for 1 g. The official packages require vendor download authorization. [Combined diagnostic receipt](../../platform/evidence/sticks3-runtime-20261122/combined-20261123.json) |

Native and WASM Clippy passed with warnings denied on Rust 1.100.0. The release workspace suite passed 2,579 checks with ignored tests included except developer-supplied `external_*` cases. The native virtual-quantum check passed 44 tests. Node 24 passed 477 platform checks, the real recording and StopWatch cases, and the 14 WASM demo manifests. The WASM JIT suite passed 111,126 differential cases and 24 GPIO waveform cases. The profiling build and catalog/evidence validation passed.

## Native CPU comparison

Both repeat arms used `CARGO_BUILD_JOBS=3 cargo +stable build --release --bins` with rustc 1.100.0 on macOS arm64. The harness measured user CPU time in alternating pairs. ROM and firmware bytes were pinned. The repeats ran after the other builds, test browsers and preview servers exited, on a shared development host. Input hashes, binary hashes, individual times and work counters are in the receipts.

| Workload | Guest seconds | Pairs | Main user s | Candidate user s | Change |
| --- | ---: | ---: | ---: | ---: | ---: |
| hello | 30 | 3 | 0.240814 | 0.237482 | -1.38% |
| c3-hello | 30 | 3 | 3.382254 | 2.594498 | -23.29% |
| c6-hello | 30 | 3 | 1.543099 | 1.556902 | +0.89% |
| classic-hello | 30 | 9 | 0.382734 | 0.391074 | +2.18% |
| pocket-tank | 10 | 3 | 10.412775 | 10.388501 | -0.23% |

Instruction counts, console hashes, cycles, exceptions and interrupts matched within each workload. The classic repeat has seven slower pairs out of nine with overlapping timing ranges. It measured a small positive overhead on this host.

The earlier hello comparison ran during release test compilation. Classic measured +10.69% across five pairs. The first 30-second Pocket Tank comparison also overlapped a candidate CLI rebuild. Its samples are retained and excluded from timing conclusions. The 10-second Pocket Tank repeat used copied, immutable executables.

[Hello repeat](hello-repeat.json) · [Classic repeat](classic-repeat.json) · [Pocket Tank repeat](pocket-tank-repeat.json) · [Initial hello](hello-initial.json) · [Initial Pocket Tank](pocket-tank-initial.json)
