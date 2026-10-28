# Browser-local SoftAP and realtime audio verification

Source `17a5f68a` was exported from Git into a separate static tree, with the production WASM and pinned S3 ROM supplied. This excludes unrelated work in progress in the working directory. Numeric results and artifact hashes are in [receipt.json](receipt.json).

## Firmware and network results

Official M5Stack StopWatch Xiaozhi 2.2.6, SHA-256 `6ed1ed429c275504c1cc32a1535f8f961b16509e87b231fd1dfd15fcff1fc21a`, booted the complete flash image through the original ROM. In a fresh browser context with **no helper or relay configured**, the browser client associated with SoftAP, obtained its real DHCP lease, fetched the original HTML and executed its `/scan` request. The workbench marked HTTP as verified and displayed the page in an opaque iframe. The host network's `192.168.4.1` was never contacted.

The original page's manual path completed `/submit` → `/done.html` → `/exit` with HTTP 200 and subsequent station Internet access. The final committed-build round enabled automatic provisioning and a local instance of the public-relay service: it reached station DHCP, firmware TLS certificate validation and the real HTTPS service response. No local-helper endpoint was configured. There were no network transition errors in that final round. The relay used native sockets and its public destination policy; it was hosted on loopback for validation, **not deployed to a public server**.

The page adapter retains the firmware's obsolete standalone `referrer no-referrer` policy and adds the modern equivalent. Other CSP/XFO restrictions remain rejected. Guest local/session storage is isolated bounded memory. Native forms, delegated event handlers and empty-fragment links have browser regressions; the latter must not reload the page and cancel provisioning. These checks do not establish compatibility with every firmware page, cookies, module scripts, WebSockets or HTTPS pages.

## Audio results and limits

A synthetic 440 Hz MediaStream passed through the browser capture AudioWorklet into the real WASM input queue. With no observed RX consumption, realtime mode stayed enabled while accounting for discarded audio; its final queue held 80 ms, with zero rejected frames. The 4,024,320 accepted input frames include frames later discarded: this is not evidence that firmware processed microphone audio or performed speech recognition. This initial integration used the earlier production WASM identified separately in the receipt.

A real playback AudioWorklet rendered 16,000 source frames at 16 kHz as 48,000 output frames at 48 kHz: one second, with no discarded duration. The deterministic jitter test separately reduced empty-queue transitions from 36 to 2 with 200 ms prebuffering, preserving all 16,000 source frames. These tests establish buffering, accounting and rate conversion; they do not replace listening to physical hardware or testing a sustained online voice conversation. No time stretching or pitch change was applied.

## Reproduction and checks

Serve the committed `web/` tree with its built WASM and ROM assets. In a fresh headless browser, configure the catalog's Xiaozhi firmware, set `{mode:'public-relay',autoProvision:false}`, load and start. Wait for `readNetwork().interfaces[1].leaseReady`, inspect the workbench iframe and its real scan response. Start the relay with an explicitly allowed localhost preview origin, then set its `relayUrl` and `autoProvision:true`. Check `provisioning`, the station lease and guest serial markers separately; never treat a lease alone as an Internet response.

Commands run include both strict Clippy variants, the release workspace suite with ignored tests included and only `external_` skipped (1,700 passed), the five WASM demos named in the receipt, native network-helper integration, relay protocol/policy tests, platform Node tests, and skill validation. The exported tree's focused audio/network/page suite passed 58 tests. A full exported-tree run initially lacked its ignored MicroPython test asset; the missing pinned asset was supplied and the affected runtime check rerun. No source change or golden regeneration was needed.

Desktop rendering and the 375 px controls/page layout were inspected. No raw activation screen or serial dump is committed. The receipt omits session names, host identity, credentials and generated device identifiers; assertions and public hashes retain reproducibility without those values.
