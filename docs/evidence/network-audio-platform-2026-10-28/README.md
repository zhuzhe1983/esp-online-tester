# Browser network and audio integration, 2026-10-28

## Inputs and implementation

Repository: `zhuzhe1983/esp-online-tester`. Core changes: `602e1073` (PCM),
`ef2a3bf1` (browser devices), `bbc8ee6d` (helper), `93b48032` (SoftAP),
`fad5cc64` (compressed HTTP), `44244faf` (shutdown), `a114756a` (real WASM PCM).
The platform integration is the commit introducing this receipt.

Official M5Stack StopWatch Xiaozhi 2.2.6, ESP-IDF 5.5.2, full merged image:
`https://m5burner-cdn.m5stack.com/firmware/05a2cf9f8c71e46dbd0dca65137ad0c1.bin`.
SHA-256: `6ed1ed429c275504c1cc32a1535f8f961b16509e87b231fd1dfd15fcff1fc21a`.
Browser WASM SHA-256:
`cc26a6bc9747055fa7cb2904de4173f3c4f00543286fb1b74b9a8b93792f7e55`.
StopWatch uses its normal interpreter, ROM and bootloader; no app-direct mode,
NVS patching or synthetic provisioning responses. Host: macOS/arm64, stable Rust
1.99.1; headless Chromium with actual AudioWorklet and WebSocket implementations.

## Observed network results

1. Manual mode: virtual client authenticated/associated with the guest open SoftAP;
   the firmware DHCP server assigned `192.168.4.2`, gateway `192.168.4.1`.
   Guest HTTP returned 200 and 59,971 decoded HTML characters. Its original page
   rendered in an isolated loopback iframe, listed the virtual router from `/scan`,
   and exposed its own SSID/password form. A desktop sticky-column overflow that
   initially placed the iframe beyond reachable viewport bounds was fixed and
   visually rechecked.
2. Provisioning: POST `/submit` returned `{"success":true}` and guest serial
   confirmed saved credentials. This alone did **not** leave config mode. The
   original success page (`/done.html`) required POST `/exit`; the adapter now
   follows that observed contract. Real firmware confirmed config exit and STA IP
   `10.0.2.15`.
3. Internet: firmware reported certificate validation and an HTTPS connection to
   `api.tenclass.net:443`, parsed its OTA response, and displayed a cloud activation
   challenge. No private account was bound; full cloud voice interaction is untested.
   One STA snapshot recorded 5 TCP connections, 1 UDP flow, 4,497 outgoing bytes
   and 25,278 incoming bytes. ICMP was not used as evidence.
4. Persistence: reloading the same image restored Flash, did not enter config mode,
   obtained its STA address and validated the server certificate again.
5. Fresh automatic mode: only task-owned browser test storage was cleared; the
   complete automatic GET → submit → success page → exit → STA DHCP → HTTPS
   sequence passed. At 18.08 guest seconds, `provisioning=connected`, no network
   error, and the guest had validated the certificate.
6. Helper integration tests separately passed real HTTP/TLS/UDP outbound traffic,
   HTTP RPC/proxy, UDP inbound mapping, wrong-origin/token rejection, compression
   bounds and cleanup. TLS kept certificate verification enabled.

## Observed audio results and limits

The official image emitted 24,000 Hz mono PCM. During one boot capture, 445,072
frames included 38,794 nonzero samples with peak absolute PCM value 32,079.
The stage speaker control reached `on`; its worklet reported one supply underflow,
explicitly displayed without speeding up or stretching audio. No physical speaker
listening or subjective quality certification is claimed.

A synthetic 440 Hz browser MediaStream exercised the microphone button and actual
AudioWorklet. In this firmware's unactivated state RX did not drain the source:
the bounded input filled, the UI reported overflow and stopped the track. This
negative result is retained; a working host microphone does not imply firmware
recognition. The separate real-WASM test drives RX → GDMA → SRAM → TX and compares
exact PCM, including mono 8 kHz and stereo 16-to-8 kHz source conversion.

Browser audio tests also verify a 16,000-sample 16 kHz signal renders as 48,000
48 kHz samples, preserving one-second duration, plus WAV, queue limits, partial
acceptance, cancellation and resource release. ES8311 analog gain/power/reset,
physical GPIO routing, speech recognition and arbitrary audio firmware compatibility
remain outside the verified scope.

## Reproduction and checks

Build with `tools/wasm-build.sh`, `cargo build --release --workspace`, and fetch ROMs
as documented. Start `node tools/network-helper.mjs`, open its local workspace URL,
configure StopWatch with the above URL/SHA, then load and start. Use
`configureNetwork({autoProvision:false})` before starting for the manual page;
default automatic mode performs the recognized original web flow. Preserve user
Flash; a fresh-state test must use an isolated browser profile.

Passed locally:

- Both CI Clippy invocations with `-D warnings`.
- `ESP32SIM_ROM_DIR=$PWD/roms cargo test --release --workspace -- --include-ignored --skip external_`: 1,631 passed, zero failed.
- `ESP32SIM_VQ_NATIVE=1 cargo test --release -p esp32s3 --test machine --test virtual_stops`.
- `node --test tools/platform-*-test.mjs`: 144 passed; includes real-WASM audio.
- Real recording replay and all 11 CI manifest/demo workloads including StopWatch.
- `node tools/network-helper-test.mjs`.
- Production WASM sections, timing/BLE ABIs, worker pacing/channel, touch/UI tests,
  browser benchmark contracts, retained evidence contracts and native AArch64 tests.
- `tools/wasm-jit-test.sh`: 111,126 differential cases, plus 24 GPIO waveform cases.
- Production-flag WASM profiling-feature build, skill validation and evidence privacy checks.

The disabled-path CPU comparison is retained in [the SoftAP receipt](../softap-client-2026-10-28/README.md).
WPA2 SoftAP has handshake unit tests; actual guest SoftAP verification here used an
open hotspot. Non-S3 SoftAP, generic protocomm/PoP, mDNS discovery and arbitrary
silent service enumeration are not claimed.

Privacy: retained observations omit device identifiers, account challenges, session
IDs, helper tokens, raw serial logs and personal paths. The omitted challenge does
not affect the TLS/HTTP result; cloud activation itself was deliberately not completed.
