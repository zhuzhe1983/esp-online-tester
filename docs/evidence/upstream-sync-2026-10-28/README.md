# Upstream integration, 2026-10-28

The product fork at `fad5cc64` is integrated with mikroverk/esp32sim `2e36224d`.
The history retains the upstream commits in three merge tranches: stability and
observability (`95558777`, with the S3 RX fix in `8f351c7b`), classic PSRAM and
BLE (`c76bb108`), then P4/shared-core integration (`164382ea`). The code at `656c84ba` also
retains main's committed audio/network follow-ups through `c83cb4f5` and fixes
the legacy workbench's touch bezel mapping (`55c3f2f8`). Native and WASM source
trees are unchanged after `164382ea`. Final code `fdf83053` also retains main
`078ad582` (workbench network/audio UI and worker changes); its affected checks
and the browser firmware/input path were rerun.

The experiment mechanisms are existing EX234 (classic PSRAM), EX238 (S3 receive
handover), EX241/EX247 (CLIC/P4), and EX246-LILYGO (the fork's physical display).
The new measurement condition is their combined integration with this fork's
GPIO open-drain, S31, storage, audio and network implementation, against the pinned
fork base. This is a regression comparison, not a new speed experiment.

## Merge contract

- Keep the product workbench as the root entry, and retain all platform checks.
- Preserve the classic light-sleep state and reset path alongside PSRAM.
- Preserve open-drain resolution when moving PIN writes into `set_pin_config`;
  test both the existing 49-pin map and the P4's additional pins.
- Use one `clic_std` flag for S31 and P4 rev >= 3. Preserve the S31 constructor,
  reset behavior and standard CSR tests; retain the old P4 CSR layout.
- Flash snapshots cover the primary chip, including new P4 variants. C6 flash
  is separate; tests verify its writes do not replace the primary snapshot.
- Keep the fork's SoftAP RX flags while restarting exhausted S3 receive lists.
- Preserve the upstream golden bytes. `goldens.json` lists all 17 changed or new
  golden files and their hashes; none was regenerated for this integration.

The old fork EX246 (T-Display-S3) is disambiguated as EX246-LILYGO. Upstream
EX246 is BLE advertisers. The historical results are unchanged.

## Capability limits

The optional classic PSRAM model is 4 MiB only. T-Beam V1.2's 8 MiB request
continues to start without PSRAM; its product status is not upgraded.

P4 rev >= 3, P4+C6 and the Waveshare LCD-3.5 run through the upstream ABI and
legacy demo runner. Tab5 and Stamp-P4 remain unavailable in the product catalog.
P4+C6's built-in virtual AP is on the C6, but the fork's host Ethernet relay and
SoftAP helper ABI still address the primary bus; this integration does not claim
those services or C6 flash persistence for the combined machine.

## Reproduction and evidence

Use Rust stable 1.99.1 and Node 24.22.1. Build the production WASM before running
platform tests; the first preparation run without it failed three tests with
missing-module/404 errors and does not establish a runtime defect.

Run the complete workflow in `.github/workflows/ci.yml`, including both Clippy
commands, ignored non-external Rust tests, real WASM demos, platform recording,
BLE/timing APIs and JIT differential checks. Browser checks use an isolated
headless session and the local static server; no user browser or device storage
is reused.

`cpu-compare.py` takes caller-supplied binary directories and ROMs. Build the
base and integrated revision with `cargo build --release --bins` under the same
Rust toolchain, then run from the integrated checkout:

```sh
python3 docs/evidence/upstream-sync-2026-10-28/cpu-compare.py \
  --base /path/to/base/target/release --new target/release \
  --base-rev fad5cc64 --new-rev 656c84ba --rom-dir roms \
  --output docs/evidence/upstream-sync-2026-10-28/cpu.json
```

The receipt retains input and binary hashes, exact arguments, instruction counts,
cycles, console hashes, individual user-CPU samples and aggregate load. It does
not retain personal paths, machine identities, process inventories or raw guest
logs. Each workload must match exactly across both sides before timing is used.

## Validation

[checks.json](checks.json) records commands, revisions, counts and log hashes.
All checks ran locally on Darwin arm64 with Rust 1.99.1 and Node 24.22.1; remote
Linux CI is a separate condition.

- Both Clippy commands pass with warnings denied. The complete release suite
  passes 1,699 tests, including ignored tests except `external_` inputs.
- All 14 production WASM demo manifests pass, including P4, P4+C6, the P4 synth,
  T-Display Schematik and StopWatch. The platform suite initially passes 136 tests after
  retaining the real mono/stereo RX-GDMA-TX PCM loopback follow-up, then 144
  tests after integrating `078ad582`. The added real TCP/TLS/UDP helper test,
  worker checks and real recording replay also pass at `fdf83053`.
- The real worker records and replays 18 input events; firmware acknowledges
  buttons and touches, and the final framebuffer matches. Replay intervals may
  extend under load; this is not exact timing reproduction.
- Virtual-quantum, timing/BLE ABI, PSRAM lifecycle, worker, UI, benchmark-harness
  and historical receipt contracts pass. JIT passes 111,126 differential cases
  in 78 groups plus 24 GPIO waveform cases. Profiling features compile.
- [browser.json](browser.json) records product entry, P4 synth/touch and P4+C6
  virtual-AP boot observations. Audio output was not auditioned. The isolated
  browser and static server were stopped after the checks.

### Browser defect and correction

The P4 synth displayed correctly but a click on the next-preset arrow did not
change the guest. The legacy runner included its 20 px CSS bezel in the canvas
coordinate scale. A compensating click reached the firmware, separating this
from controller/DMA behavior. The fix subtracts border and padding before
scaling and rotation. A normal click now changes both the guest console and
visible instrument from `Chunky Synth` to `Wah brass`. Nine touch tests pass,
including the scaled, padded, rotated canvas and out-of-bounds capture case.

Raw logs and screenshots remain in ignored local artifact directories. Their
hashes and sizes are retained, not personal filesystem paths or full captures.
No private data was removed from measured samples; command paths use caller
arguments or repository-relative paths. Screenshots have not been published.

## Fixed-work CPU comparison

Seven alternating measured pairs after one warmup per side; user CPU seconds.
All runs have identical instruction counts, cycles and console hashes.

| Workload | Base median (s) | Integrated median (s) | Change | New slower pairs |
| --- | ---: | ---: | ---: | ---: |
| s3-hello | 0.276175 | 0.276130 | -0.016% | 2/7 |
| c3-hello | 2.564655 | 2.582241 | +0.686% | 4/7 |
| c6-hello | 1.752878 | 1.750686 | -0.125% | 4/7 |
| pocket-tank | 7.558201 | 7.681337 | +1.629% | 3/7 |
| classic-hello | 0.385585 | 0.385578 | -0.002% | 3/7 |

The predefined flag is six or seven slower pairs; none met it. Pocket Tank's
median is +1.629% while only three of seven pairs are slower, so it is retained
as a noisy local result, not erased or presented as a speedup. Aggregate
one-minute load ranged from 5.09 to 6.53 on 18 logical CPUs.
The task's browser and builds were stopped, but other host activity was not
controlled. These short samples do not prove universal performance neutrality.
Individual samples, ranges, input hashes and exact commands are in [cpu.json](cpu.json).

The final product browser check loads the explicit internal StopWatch diagnostic,
observes its 468×466 framebuffer and a fresh firmware `BUTTON_A` response, then
pauses. On the ordinary local static server, `/network-info` returns its expected
404 (no helper); Chromium also warns about the initially empty guest-page iframe's
sandbox flags. No firmware HTTP page was loaded in this check; the network helper
suite separately verifies HTTP/TLS/UDP and isolated proxy behavior. The observation
does not replace the original `078ad582` firmware provisioning qualification.
