# Original CoreMatrix UIFlow2 network startup

The original GitHub UIFlow2 2.5.4 image can boot and connect to the virtual WPA2 access point by selecting its existing **network-only** option, `boot_option=2`. This changes NVS configuration, not firmware code. It does **not** restore the absent `M5Things` cloud module or the cloud startup menu. The original I2C initialization warning remains visible.

`runtime.json` records execution with the production WASM, real C61 ROM/bootloader, unchanged source image SHA-256 `8d5b8d4e22ebc293937086b2f9f218aa5a5ebda27b5ccb32b3f8f580ee02939b`, and guest-written NVS. The image's original `boot.py` matches the pinned upstream file byte for byte; its SHA-256 is checked in the guest before and after reset. No Python file, frozen module, application partition, cloud response or display was substituted.

## Root cause and supported selection

At upstream revision `e437118892520ba987c2d930a2428db1cab65ab1`, `m5stack/fs/user/boot.py` reads the NVS unsigned byte `uiflow/boot_option`. `startup/__init__.py` selects these branches:

| Value | Original behavior | Result for this GitHub image |
| --- | --- | --- |
| 0 | Skip network/menu; permit `main.py` | Source-reviewed, not selected by this test |
| 1 | CoreMatrix startup menu and network | `startup/corematrix.py` imports `startup/headless.py`, whose unconditional `M5Things` import raises before network setup |
| 2 | Network setup and then `main.py` | Original startup reads NVS and calls `network.WLAN.connect`; real guest obtained a DHCP lease |

Upstream explicitly described [M5Things as private in issue #27](https://github.com/m5stack/uiflow-micropython/issues/27#issuecomment-2311406552). The pinned public C-module list contains no M5Things implementation. Its GitHub release workflow runs the public `pack_all` build without a separate cloud-module input. Rebuilding that public configuration therefore does not supply the missing implementation. This conclusion combines the source review with the [earlier original-image/module/partition comparison](../corematrix-firmware-20261106/README.md); it is not an emulator-generated import error.

The [official CoreMatrix programming guide](https://docs.m5stack.com/en/uiflow2/corematrix/program) distributes firmware through M5Burner. Its separately cataloged build `509a67aa` is not the GitHub `e437118` build. The previously inspected official download endpoint requires CAPTCHA and has not yielded a verified image. No claim is made that the Burner build has been tested, and no binary from another product can stand in for it. A complete cloud-menu repair still requires obtaining that exact official image through its supported access flow and testing it, or an upstream release of the real implementation. No stub or authentication bypass was introduced.

## NVS contract

The actual image partition table places NVS at `0x9000`, length `0x6000` (data/NVS type `1/2`). The namespace is `uiflow`. The pinned UIFlow port extends `esp32.NVS` with `get_str`/`set_str` and `get_u8`/`set_u8`; `boot_option` is **u8**, not i32 or a string.

| Keys | Type | Relevant value/behavior |
| --- | --- | --- |
| `ssid0`, `pswd0` | NVS string | The network-only branch reads slot 0 and passes it to the real WLAN API |
| `net_mode`, `protocol` | NVS string | `WIFI`, `DHCP` in this test |
| `server` | NVS string | Upstream default `uiflow2.m5stack.com`; selecting network-only does not imply cloud access |
| `ip_addr`, `netmask`, `gateway`, `dns` | NVS string | Read by startup, but this revision's network-only helper does not forward the static configuration arguments; its WLAN call uses the DHCP default |
| `tz`, `sntp0`, `sntp1`, `sntp2` | NVS string | Existing timezone/SNTP settings; this test does not modify or validate RTC behavior |
| `boot_option` | NVS u8 | Factory GitHub configuration is 1; this explicitly requested network profile selects 2 |

Source retrieval times, hashes and exact immutable URLs are in `sources.json`. The official boot script's comments also document option 2. It is an upstream configuration path, not a replacement startup program.

## Execution checks

`tools/probe-corematrix-startup.mjs` accepts caller-supplied artifacts and refuses a source image with a different hash. It starts the original default image and reproduces `ImportError: no module named 'M5Things'`. Through USB raw REPL, the original `esp32.NVS` API then writes an isolated public test SSID/passphrase and `boot_option=2`, commits, and calls `machine.reset()`.

After the next ROM boot, the original startup prints its Wi-Fi initialization messages without a traceback. The guest's typed NVS reads match the saved values; `WLAN.isconnected()` is true, `WLAN.status()` is 1010, and `ifconfig()` is `10.0.2.15`, `255.255.255.0`, `10.0.2.2`, `10.0.2.3`. These values come from guest execution, not a host-side connected flag. Without a relay/helper, host interface-0 lease metadata is absent even though the emulated access point's DHCP exchange succeeds.

The test explicitly attempts `import M5Things` again and confirms it is still absent. The serial receipt retains `i2c.master: this port has not been initialized, please initialize it first`; selecting option 2 does not fix or suppress that warning. The separate upstream RTC epoch [issue #120](https://github.com/m5stack/uiflow-micropython/issues/120) is untouched. There is no assertion of physical Wi-Fi, internet reachability, cloud access-code pairing, or silicon validation.

The source also has a button override: on CoreMatrix, holding A, B or C in its startup detection window selects menu mode and writes `boot_option=1` back to NVS. This source-reviewed behavior can restore the missing-module failure. The successful network-only test has all buttons released; it does not neutralize the official override.

Reproduce with the repository's production worker and caller-supplied image/WASM/ROM:

```sh
node tools/probe-corematrix-startup.mjs ORIGINAL_BIN PRODUCTION_WASM C61_ROM_ELF RECEIPT_JSON
```

The test uses isolated in-memory device storage and disposes its worker in `finally`. Public fixture credentials are asserted, never printed as a password. The committed receipt retains relevant startup messages and aggregate host network state; unrelated browser/network identity fields and local paths are omitted. The source image identity remains separate from guest Flash changes.
