# ESP32-C61 ECC accelerator

`esp32c61/src/ecc.rs` models C61's ECC_MULT register block independently of C6.
The source contract is ESP-IDF **v5.5.4**, peeled tag commit
`735507283d5b2f9fb363a1901172dbd9e847945d` (retrieved 2026-11-06).

## Register and arithmetic contract

- Interrupt RAW/ST/ENA/CLR at offsets `0x0c/0x10/0x14/0x18` use bit 0;
  RAW and CLR clear by writing one. `Device::irq_sources()` returns the masked bit.
- CONF at `0x1c`: START bit 0, RESET bit 1, P-256 selector bit 2,
  modulus selector bit 3 (`0=n`, `1=p`), WORK_MODE bits 4–7,
  SECURITY_MODE bit 8, read-only verification result bit 29, clock controls bits 30–31.
  C6's mode shift and verification bit are incompatible with this block.
- Six little-endian 32-byte parameter banks hold K/PX/PY/QX/QY/QZ at
  `0x100/0x120/0x140/0x160/0x180/0x1a0`. P-192 consumes six words per bank;
  computed results zero-pad the remaining words. Unaligned/reserved accesses are ignored.
- Modes 0/2/3 perform affine multiplication, verification, and verified multiplication;
  4/6/7 perform Jacobian-output multiplication, Jacobian verification, and
  affine verification followed by Jacobian-output multiplication.
- Mode 5 adds affine P to Jacobian Q and returns affine PX/PY and Jacobian QX/QY/QZ.
  The generated register header labels mode 5 reserved, but the same revision's
  `ecc_ll_set_mode(ECC_MODE_POINT_ADD)`, HAL and official tests explicitly use it.
- Modes 8/9/10/11 perform modular add/subtract/multiply/divide on PX and PY.
  Add/subtract return PX; multiply/divide return PY. Division is `PX / PY` modulo
  the selected curve order or field prime. This follows `test_ecc.c`, which passes
  zero K and numerator/denominator in PX/PY; the generic enum comment describing
  inverse K is not the C61 register contract.

The field and group orders are those of secp192r1 and secp256r1. Affine validation
rejects noncanonical coordinates and off-curve points. Jacobian validation checks
`Y² = X³ − 3XZ⁴ + bZ⁶` and rejects Z=0. Zero/order scalar results and inverse-point
addition use all-zero output for the point at infinity.

## Validation and limits

Eight unit tests cover both official IDF scalar and modular arithmetic fixtures,
Jacobian validity and equivalence to the official affine results, scaled-coordinate
addition against independent OpenSSL 2G coordinates, malformed/noncanonical points,
infinity, P-192 padding, modulus selection, read-only bits, W1C/masking/reset and
bounded reserved accesses. `cargo test --release -p esp32c61 ecc::tests` runs them
when the module is attached to the C61 crate.

The module was initially tested independently through a temporary Cargo harness
with a path dependency on `esp-periph`: all 8 tests passed, and release Clippy
with all targets and `-D warnings` passed. Official-firmware TLS execution is a
separate integration acceptance check; these arithmetic tests do not establish it.

Computation completes synchronously. The SECURITY_MODE and clock-control bits are
stored, but accelerator latency, clock/power gating and constant-time execution
are not modeled. Jacobian outputs are mathematically equivalent projective
coordinates; they are **not** claimed to reproduce the accelerator's intermediate
coordinate words. Reserved modes and division by zero leave START set and DONE
clear without changing output RAM; this is a bounded conservative emulator policy,
not a measured silicon error response. Infinity encoding and complete parameter-RAM
clearing on reset are deterministic model behavior, not independently measured
silicon observations. The model is for firmware execution, not cryptographic key
protection or side-channel evaluation.

## Source receipt

The files below were retrieved from the pinned public revision. SHA-256 hashes
refer to the downloaded source bytes, not generated Rust files.

| Source | SHA-256 |
| --- | --- |
| [ecc_mult_reg.h](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/soc/esp32c61/register/soc/ecc_mult_reg.h) | `ab1a3edb0951bcd0a7b865757938c79eda6352c4df5a8c5e8d5101adf234e856` |
| [ecc_ll.h](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/hal/esp32c61/include/hal/ecc_ll.h) | `30825f591009b8d80f064fd291b6b95459f5cca824552b0dae35989fa7f91887` |
| [ecc_hal.c](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/hal/ecc_hal.c) | `003f7644ec2aaddff667ff92467e5d024aa1a25837911ebb9144063ab5cdbc09` |
| [test_ecc.c](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/hal/test_apps/crypto/main/ecc/test_ecc.c) | `ec2546fe560e48adebb2c41b231f6e511dfde25f269a9e9894d32c1f618d327d` |
| [ecc_params.h](https://github.com/espressif/esp-idf/blob/735507283d5b2f9fb363a1901172dbd9e847945d/components/hal/test_apps/crypto/main/ecc/ecc_params.h) | `763caecb6ff9e581ccd2e85cf5caaf9bcbb8d50191679dc396e7c1a14d8b57eb` |
