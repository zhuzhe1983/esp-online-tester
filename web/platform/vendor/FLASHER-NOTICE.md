# Browser flashing dependencies

`esptool-js-0.7.0/bundle.js` is the unmodified browser bundle from the official
`esptool-js@0.7.0` npm archive. It contains Espressif's flasher RAM stubs and bundled
`pako@2.1.0` / `atob-lite@2.0.0`; these are tool components, not catalog product firmware.
The package and dependency licenses are adjacent to the bundle.

`spark-md5@3.0.2` supplies the host MD5 value used by esptool's flash read-back
verification. Its UMD entry point is wrapped in an ES module with local
`module`/`exports` objects; its algorithm is unchanged.

`flasher-vendor.json` records exact package sources, archive integrity values,
the MD5 wrapper change and SHA-256 hashes of every delivered file. No runtime CDN
requests or npm installation are needed. To update, fetch the exact official npm
archive, verify its published SHA-512 integrity, review the upstream API and
licenses, replace the listed files and rerun `tools/platform-flasher-test.mjs`.

Primary references checked on 2026-10-26:

- https://github.com/espressif/esptool-js
- https://espressif.github.io/esptool-js/docs/index.html
- https://developer.chrome.com/docs/capabilities/serial
- https://wicg.github.io/serial/
- https://registry.npmjs.org/esptool-js/0.7.0
- https://registry.npmjs.org/spark-md5/3.0.2

Product firmware downloaded for simulation or flashing uses `cache: no-store`
and remains in operation memory only. The watchlist checker reads release metadata
and HTTP headers, never complete firmware bodies.
