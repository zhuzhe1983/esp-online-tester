# Security Policy

## Reporting a vulnerability

Please do not open public issues for security problems.

Report privately through GitHub: go to the repository's Security tab and click "Report a
vulnerability" (<https://github.com/mikroverk/esp32sim/security/advisories/new>).

Security contacts: Joakim Eriksson ([@joakimeriksson](https://github.com/joakimeriksson)) and
Alice ([@aliceisjustplaying](https://github.com/aliceisjustplaying)), the security managers of the
mikroverk organization.

We aim to acknowledge reports within 7 days and will coordinate a fix and a disclosure timeline
with you. Fixes are published as GitHub Security Advisories, with a CVE requested where
appropriate. Reporters are credited unless they prefer otherwise.

## Supported versions

Only the latest `main` is supported. Releases are tags on `main`; there are no maintained release
branches.

## Scope

esp32sim runs untrusted firmware, so the following are in scope:

- Guest-to-host escapes from the emulated SoC: the instruction decoders and interpreters, the
  AArch64 and WebAssembly JITs, and the peripheral models
- The virtual network and NAT (`--net nat`): guest traffic reaching hosts or ports it should not,
  or bypassing `--net none`. `--hostfwd` listens on a host TCP port (loopback when the host
  address is omitted) and forwards accepted connections into the guest
- The live board UI (`--web PORT`) and its WebSocket protocol
- The WebAssembly build and the hosted demo at <https://mikroverk.github.io/esp32sim/>

Out of scope: vulnerabilities in third-party firmware images, ESP-IDF, the Espressif WiFi blob or
other fetched third-party components (report those upstream).
