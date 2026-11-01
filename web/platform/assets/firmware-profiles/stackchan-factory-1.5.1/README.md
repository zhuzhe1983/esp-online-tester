# StackChan V1.5.1 VHCI symbol profile

This profile lets the existing EX200 S3 virtual controller receive the **real
guest NimBLE host's HCI traffic**. It is restricted to the 12,783,789-byte official
M5Burner merged image with SHA-256
`411578a2ebca2cfe3541fdc32aeddbc4703cf912a5daa7e1253f69306d6e1d87`.
It does not paint an application page, change touch coordinates, skip the official
setup flow, or report a fabricated phone pairing.

`symbols.elf` is a generated 708-byte ELF32 symbol table. It contains no firmware
instructions, loadable segments, allocated sections, or executable entry point.
The native/WASM adapter consumes its absolute symbol addresses. Its SHA-256 is
`7d4996d8cddd6b735abb4cb0600f32148a7fb48c0d3b50489e7e0e1ca8928a14`.
`profile.json` supplies the exact-image gate, 31 additional byte-window hashes,
and the two permitted persistence ranges. `provenance.json` records readable
addresses, partition/segment layout, source versions, fetch receipts and evidence.

## How the addresses were identified

No release ELF/map was available: the public `m5stack/StackChan` releases and
Actions-artifact listings were both empty on 2026-11-01. The newer public source
at `f901cc2969189f3f0e96af1442c0f96e77e1e2c1` is useful for understanding the page
flow, but **is not a byte-identical build of this image**. The image descriptor
identifies ESP-IDF v5.5.4, application `stack-chan` 1.5.1, built July 31, 2026.

The three VHCI wrappers were identified by their IDF v5.5.4 status guards,
argument handling and direct calls into the named controller archive functions.
The exact IDF v5.5.4 submodule revision is
`espressif/esp32c3-bt-lib@ebd6043a8e3c3bbde45ee483895303b9c1229ab5`.
Its S3 archive hash is pinned in the provenance. The reproduction tool compares
the three archive functions against the image, masking only ELF relocation
fields, and decodes the wrappers' CALL8 destinations.

Controller init additionally references its unique `__func__` string and matches
the IDF config checks (task stack, priority 23 and BLE mode). The real NimBLE
transport calls it at `0x4207ed5f` and calls enable at `0x4207ed85`. Init's reserved
extent ends at the next decoded function entry, `0x4208148c`; its 932-byte size
includes alignment padding. Enable, disable and deinit match the IDF state
transitions and controller call chains. The two FreeRTOS functions have unique
assertion function-name literals; their task-allocation and delay/yield control
flow was checked against v5.5.4 `freertos_tasks_c_additions.h` and `tasks.c`.
These source/semantic checks support the addresses; matching a function-name
string alone was not treated as sufficient.

| ELF function | Address | Reserved/decoded extent |
| --- | --- | ---: |
| esp_bt_controller_init | 0x420810e8 | 932 |
| esp_bt_controller_enable | 0x420814bc | 169 |
| esp_bt_controller_disable | 0x42081568 | 76 |
| esp_bt_controller_deinit | 0x4208148c | 46 |
| esp_vhci_host_check_send_available | 0x42081090 | 20 |
| esp_vhci_host_send_packet | 0x420810a4 | 34 |
| esp_vhci_host_register_callback | 0x420810c8 | 31 |
| xTaskCreatePinnedToCore | 0x40392a10 | 141 |
| vTaskDelay | 0x40391fc0 | 59 |

## Controller BSS reservation

**The original linker BSS bounds were not recovered.** The ELF names
`_bt_controller_bss_start` and `_bt_controller_bss_end` intentionally alias only
the verified internal reservation `[0x3fcbb280, 0x3fcbb480)`, 512 bytes. They must
not be presented as recovered original linker symbols.

IDF v5.5.4 `linker_common.lf` defines `bt_bss` as `.bss+`.
`linker_rw_bt_controller.lf` puts `libbtdm_app.a`'s matching sections in one
contiguous `SURROUND(bt_controller_bss)` group, separate from `libbt.a` host BSS.
Independent archive-object relocations prove this group covers at least
`[0x3fcbb280, 0x3fcbb530)`:

| Controller object / BSS symbol | Linked address | Size | Independent reference |
| --- | --- | ---: | --- |
| arch_main.o / pll_track_state | 0x3fcbb280 | 16 | ble_try_turn_on_pll_track |
| lld_adv.o / g_ble_ext_config | 0x3fcbb3a8 | 36 | ble_ext_config_init |
| rf_coexist_hook.o / s_ble_st_group_map | 0x3fcbb526 | 10 | coex_hook_check_coex_schm_st, bt_rf_coex_hook_register |

The tool checks each object's non-relocated function bytes, relocation symbol,
NOBITS section, section/symbol size, linked L32R instruction, literal address and
resolved RAM address. Thus the reservation lies inside controller-owned BSS,
not guest NimBLE/RTOS state. This use is valid **only while native controller
init, enable, disable, deinit and VHCI entry points are intercepted**. Native
controller execution would invalidate the reservation. EX200 supplies the guest
task trampoline and virtual HCI controller; the host stack and its RTOS remain
the official guest code.

## Saved flash and OTA identity

The sole installed application is `ota_0` at `[0x20000, 0x510000)`.
The entire alternative slot `[0x510000, 0xa00000)` is present and erased (all
`0xff`), SHA-256
`a1c373a3265318764fce2aea477d303f4412415e9e9462bbdcc91c49db45e956`.
Normal first boot writes NVS and OTA state. Only NVS `[0x9000, 0xd000)` and
otadata `[0xd000, 0xf000)` may change in restored flash. The runtime must verify
every other byte against the original image, treating bytes beyond its end as
erased flash. This preserves the bootloader, partition table, installed app,
empty alternative slot and assets; modified otadata cannot select a different
written executable. Never choose a profile using only the original download
hash while accepting arbitrary persisted application/OTA changes.

## Reproduce

Supply the official BIN and exact public controller archive locally; neither is
vendored or hosted by this repository. URLs, versions, sizes and hashes are in
`provenance.json`. From the repository root:

```sh
python3 tools/stackchan-firmware-profile.py --firmware "$FACTORY_BIN" --controller-lib "$CONTROLLER_LIB"
python3 tools/stackchan-firmware-profile-test.py
```

The first command verifies image identity, parses the actual ESP segments and
partition table, checks the erased alternate OTA slot, named string anchors,
CALL8 targets and archive/BSS relocations, then checks byte-for-byte reproduction
of both generated files. Add `--write` only to regenerate them. The script uses
Python's standard library and performs no network access. Raw disassembly can
be reproduced with Espressif `xtensa-esp-elf-objdump -D -b binary -m xtensa`,
using the segment offsets/addresses in provenance; start each function at its
recorded entry because linear decoding across alignment bytes can misalign.

Native and production WASM checks with this exact symbol ELF exercised the
official welcome/servo/setup buttons. After APP SETUP Next, the guest advertised
`StackChan` with service `e2e5e5ff-1234-5678-1234-56789abcdef0` and rendered the
next page. WASM repeated this after saved-flash restoration; only NVS and otadata
changed. The integrated runtime receipt is
[`factory-next-ble.json`](../../../../../docs/platform/evidence/m5stack-k151/runtime/factory-next-ble.json).
This proves the tested setup/advertising path, not phone provisioning, pairing,
encrypted links, RF timing, Wi-Fi completion or every later controller API.
