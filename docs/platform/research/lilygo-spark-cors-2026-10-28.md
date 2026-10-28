# LILYGO Spark firmware discovery, 2026-10-28

The workbench adds **远程地址 → LILYGO Spark** at the same address-type selector used by M5Burner. It offers product-scoped search and version selection, then uses the existing remote BIN loader, SHA-256 validation, chip/layout checks, trial confirmation and browser-only history. Selection/search does not download firmware. A successful run records the actual digest; restoring history neither searches nor starts firmware. Embedded scenes carry the same ordinary remote source and use the existing loader/flash controls.

## Official source and browser access

Inspected source: [`Xinyuan-LilyGO/LILYGO-Spark@0fd728e800d9ebaa5a15515b49aa6244f3453340`](https://github.com/Xinyuan-LilyGO/LILYGO-Spark/tree/0fd728e800d9ebaa5a15515b49aa6244f3453340). `lilygo_config.json` names the OSS manifest; `electron/config-handler.ts` derives the v2 filename and falls back to its bundled v2 catalog. `src/utils/manifestV2.ts` defines project-level product IDs and per-version overrides.

The [source receipt](../evidence/lilygo-spark/source-receipt-2026-10-28.json) records actual GET response sizes, SHA-256, media types, UTC times and CORS headers, with `Origin: https://zhuzhe1983.github.io`.

| Source | HTTP | Bytes | Access-Control-Allow-Origin |
| --- | --- | ---: | --- |
| Official OSS `firmware_manifest_v2.json` | 200 | 608984 | absent |
| Official GitHub raw `main/firmware_manifest_v2.json` | 200 | 436499 | `*` |

The browser therefore reads the official repository catalog directly with anonymous CORS GET, rather than using the inaccessible OSS mirror or a third-party proxy. At inspection it contained 151 products and `generated_at: 2026-06-26T08:16:10.974Z`. The selector displays this generation date and identifies the repository source. This is the shipped Spark catalog, **not a claim that it contains the newest OSS uploads**. Current, independently audited factory firmware remains in the shared firmware library. Requests have a 20-second timeout and 2 MiB streaming limit. The remote manifest and any user/contributor contact fields are not committed or copied to localStorage; only normalized names, target IDs and selected firmware metadata are retained.

## Target contract

Spark's broad `supported_product_ids` alone is insufficient. In the inspected data, original T-Display includes S3 AMOLED and P4 binaries; T-Deck includes Plus/Pro and keyboard-controller firmware; T-Embed CC1101 includes UI assets and ESP32-C5 binaries; T-Watch S3 includes older ESP32 watches. The provider first matches one audited product ID, respects per-version product overrides, and then checks an exact repository/path or narrowly defined filename contract. It never chooses another hardware revision by fuzzy name.

| Local product | Spark product | Direct BIN boundary |
| --- | --- | --- |
| `lilygo-ttgo-t-display` | `t-display` | No direct BIN rule; exact original filenames can appear as references |
| `lilygo-t-display-s3` | `t-display-s3` | Official T-Display-S3 non-touch/TFT test paths only |
| `lilygo-t-deck` | `t-deck` | Official original T-Deck firmware/UnitTest paths; exclude Plus/Pro/C3 keyboard |
| `lilygo-t-embed-cc1101` | `t-embed-cc1101` | Official original firmware and selected original examples; exclude Plus, NRF24, C5 and data assets |
| `lilygo-t-beam-v1-2` | `t-beam` | Official LoRa-Series `hardware_test/t-beam/T_BEAM_SX1276_Factory_*` only |
| `lilygo-t-watch-s3` | `t-watch-s3` | CircuitPython's exact `lilygo_twatch_s3` target only |

A direct source must be HTTPS, contain no credentials, match the target contract, carry a full 64-character SHA-256 and not be a known small/incomplete artifact (metadata size below 32 KiB). Missing/short digests remain references. The Spark T-Deck legacy `firmware.bin` entry reports only 6640 bytes and is therefore a reference; this does not replace the audited factory catalog entry.

OSS uploads are ZIPs whose manifest digest belongs to the **uncompressed BIN**. They are shown as references where their exact filename target is known, with instructions to download using Spark and import the extracted local BIN. No ZIP is passed as a raw firmware source and no digest is applied to the wrong container. Other ZIP/multi-device archives and unreviewed model paths are omitted. Repository branch URLs can become stale or return different bytes; normal loading reports network/404 errors or rejects a digest mismatch instead of silently accepting changed firmware.

Observed repository-catalog search counts after filtering: T-Display 0; T-Display S3 2 projects/2 direct versions; T-Deck 1 project/0 direct; T-Embed CC1101 8 projects/6 direct; T-Beam 1 project/1 direct; T-Watch S3 1 project/1 direct. These counts describe metadata discovery, not successful firmware execution.

## Checks

`node --test tools/platform-lilygo-spark-test.mjs tools/platform-m5burner-test.mjs tools/platform-firmware-history-test.mjs tools/platform-config-test.mjs tools/platform-input-test.mjs tools/platform-view-test.mjs` passes. The focused test verifies false sibling targets, per-version overrides, malformed/credential-bearing URLs, missing/truncated hashes, ZIP references, query/pagination, bounded CORS reads, cancellation, and device-scoped history. Existing Burner/history tests still pass. The config check in the isolated lane reports missing local ROM verification; integrated runtime/browser checks use the integration workspace's artifacts.

No firmware binary was downloaded for this provider audit, executed, flashed or published. Source/header inspection and Node metadata checks do not establish actual browser or hardware execution. Integrated browser validation is recorded separately with the product onboarding result.
