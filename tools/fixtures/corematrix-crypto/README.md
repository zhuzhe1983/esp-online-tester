# CoreMatrix original-firmware crypto known-answer checks

`known-answer.py` runs inside the original UIFlow 2.5.4 MicroPython interpreter after upload through USB raw REPL. All expected values are fixed before guest execution. The fixture uses public test keys and messages; it requires no TLS traffic, private key, server, clock adjustment or firmware patch.

```sh
cargo run --release -p esp32c61 --example firmware -- \
  "$C61_ROM_ELF" "$ORIGINAL_COREMATRIX_BIN" 100000000 \
  tools/fixtures/corematrix-crypto/known-answer.py
```

The firmware SHA-256 is `8d5b8d4e22ebc293937086b2f9f218aa5a5ebda27b5ccb32b3f8f580ee02939b`. ROM identity and execution receipts are in `docs/platform/evidence/corematrix-crypto-20261106/`. Require 109 `CM_CRYPTO_RESULT` lines, no `CM_CRYPTO_MISMATCH`, and the final `CM_CRYPTO_PASS`. The native example's process exit code alone does not establish guest success.

The checks cover:

- SHA-256, SHA-1 and MD5 across empty, published short, SHA padding/block boundaries, the 3,968-byte SHA DMA limit, 4 KiB boundaries, 8 KiB and 16 KiB inputs. Selected inputs are also fed in 1, 63, 64, 65, 1,024 and 4,096-byte updates. `hash-vectors.json` records the input recipes and independent CPython `hashlib` expected values.
- HMAC-SHA256 RFC 4231 cases 1, 2 and 6, composed in guest Python using the firmware's `hashlib.sha256`. These exercise the digest path; they are not direct calls to mbedTLS's HMAC API.
- RFC 5246 section 5 `P_SHA256`, composed from that HMAC with an explicitly public fixed secret/seed. The expected 128 bytes come from host CPython `hmac`.
- FIPS 197 AES-128/256 ECB and SP 800-38A AES-128 CBC, encryption and decryption, through the firmware's `cryptolib` API. The original image rejects CTR mode 6 with `ValueError: mode`; the fixture records that API limitation explicitly. GCM is not exposed by this API and requires the separate real TLS test.

The 8 KiB SHA-256 case failed before `fac00a34` because C61's SHA DMA memory adapter could not read MMU-mapped PSRAM. The same original firmware passes all 109 answers after that correction. Shared AES, SHA arithmetic and ECC were not changed by this fixture.
