# ESP32-C61 BLE controller bring-up

This is a separate C61 digital controller implementation. It does not patch an
image, replace HCI entry points, or borrow the C6 controller's firmware hooks.
Original UIFlow 2.5.4 now executes activation, legacy advertising, passive
scanning and deactivation through real registers, SRAM descriptors and IRQs.
Advertising bytes and nearby scan callbacks are qualified below. Connections,
GATT, active scan-response exchange, privacy and extended/coded PHY operations
remain unsupported; this is not full Bluetooth qualification.

## Fixed sources

ESP-IDF v5.5.4 `components/bt/CMakeLists.txt` selects the **C61** archive at
`esp32c61/libble_app.a` from `espressif/esp32c6-bt-lib` commit
`9d05fc931ae771de52de7e4b4c7090f5b3ec0ec4`. The repository name does not imply
that the C6 and C61 hardware layouts are interchangeable.

The archive's relocatable symbols and disassembly establish these accesses:

- `r_ble_hw_resolv_list_search`: block `0x600a9880`, START +0 bit0,
  list address +8, search address low20/type20 at +12, completion +60 bit2,
  match status +32 bits8..15, entry address +36 low20 and match present +40.
- `r_ble_ll_resolv_insert_eof_head`: a 64-byte list node whose control word
  is `0x80000000` for the empty sentinel. Next pointer is +4.
- `r_ble_ll_resolv_list_add`: identity address +32, valid control bit15,
  address type bit16 and end-of-list bit31. The first implementation searches
  exact identity addresses; RPA/IRK resolution is not yet implemented.
- `r_ble_hw_encrypt_block`: separate block `0x600a9800`, key +16..28,
  input/output SRAM addresses +32/+36, 16-byte size +12, mode1 +8,
  trigger +4 bit0 and polled completion `0x600a98c4`. The existing pure
  AES128 calculation is reused; the register and DMA layout is independent.

These security operations are polled. BLE interrupt sources in the C61
v5.5.4 `interrupts.h` are MAC4, BB5, BB NMI6, timer9 and security10; IRQ behavior
is not inferred from another chip.

## Original firmware execution

Inputs and ROM hashes are recorded in
[the native receipt](platform/evidence/corematrix-runtime/c61-native.md).
Original UIFlow 2.5.4, without modified bytes:

```python
import bluetooth
print('BLE_BEGIN')
ble = bluetooth.BLE()
ble.active(True)
print('BLE_ACTIVE', ble.active())
ble.gap_scan(1000, 30000, 30000)
print('BLE_SCAN_STARTED')
```

Before the security device, an observer at the original firmware's `r_ll_assert`
(0x40806c80) caught line533 of `r_ble_hw_resolv_list_search`, from a timeout
polling `0x600a98bc & 4`. The list at that point was the genuine empty sentinel.
With only the security device implemented, the original controller returned `BLE_ACTIVE True` with zero
CPU exceptions. At that stage the scan call did not return within the 300M instruction
budget; last PC is 0x408063b8 in a controller timer/event polling path. This
is progress evidence, not a successful scan or radio-support claim.

Unit checks cover empty/exact/type-mismatch searches, malformed cyclic lists,
NIST AES128 (`001122...eeff`, key `000102...0e0f`) and prohibited output DMA.

## Timer and control-loop progression

`r_ble_lll_sleep_timer_tick_get` waits for `0x600a1928` bit27 to self-clear and
reads the latched microsecond counter at `0x600a1924`.
`r_ble_lll_sleep_time_sync_set` loads a replacement counter from `0x600a192c`
with bit31. The microsecond unit is explicit in its RTC conversion arithmetic
(division by 1,000,000). These commands now operate on the C61 160 MHz clock.

With the latch implemented, the original 2.5.4 image runs activation,
`gap_scan(1000,30000,30000)`, receives the software scan-duration IRQ6 after
1500 ms, starts/stops a 100 ms advertising interval, and deactivates BLE.
It completes with zero CPU exceptions within an 800M script budget. This
control-only result alone did **not** establish reception or advertising;
the subsequent descriptor and packet tests below supply that evidence.

## Scheduler boundary

The controller's `r_ble_phy_init` calls `intr_alloc` with source4 and
`r_ble_phy_isr`; the C61 digital MAC is connected to that source.
`r_ble_lll_sched_restart_link` writes the first schedule pointer to
`0x600a18fc`, followed by `r_ble_lll_sched_run` writing START at `0x600a1028`.
STOP is `0x600a102c`. A 72-byte schedule carries its next low20-bit pointer at
+0, operation in +4 bits20..23, start/end microseconds at +8/+12 and pending
bit13 at +40. Operations6/12/13 are the exercised advertising/passive/active
scan families. Other operations do not fabricate successful completion.

The scheduler makes current/prefetched pointers visible through 0x600a1900
(running31, current29, pointer19..0) and 0x600a1904. At an interval's end it
clears the completed item's status, advances pointers and raises the observed
completion/recycle path (0x600a130c bit28, and 0x600a131c bit3 at list end).
Masks are 0x600a1304/1314; W1C is 0x600a1308/1318. Public archive symbols
prove software consumption of these bits; the undocumented hardware event
names are not asserted. Error bits15 and7/8 are not substituted for completion.

The original firmware recycles and reschedules descriptors through its real
ISR. A 250 ms advertising program produces three-channel schedule groups
repeated at its configured 100 ms interval; scan/start/duration callback/stop
and BLE deactivation finish with zero CPU exceptions. Packet parsing and
delivery were then qualified separately below.

## Original legacy advertising packets

The TX buffer format is distinct from RX. `r_ble_lll_adv_reset_link_state`
and `r_ble_lll_adv_pri_chan_txbuf_alloc_and_make` establish link-state +0
(low20 pointer to TX header+4), header +8 (low20 TX buffer pointer), buffer
+16/+17 (PDU header/length) and +18 (advertising data). Legacy advertising
inserts the six-byte advertiser address separately. Address selector0 in
link-state +28 bits29:28 uses `r_ble_phy_set_dev_address`'s public registers
0x600a180c/1810; selector1 uses link-state +52..57. These are not inferred
from C3 descriptors.

The original firmware's 250 ms program with 100 ms advertising interval and
AD bytes `0201060409433631` produces nine frames in three groups of channels
37/38/39. Every PDU is `200e6300000000020201060409433631`: ChSel header,
length14, six advertiser bytes programmed by the guest, and its exact AD
payload. Consecutive groups have the controller's interval plus advertising
random delay. The external test asserts byte identity, channels, repetition,
completed guest disable and zero CPU exceptions. No synthetic HCI event or
host-selected payload enters this TX path.

```sh
ESP32SIM_ROM_DIR="$ROM_DIR" ESP32SIM_COREMATRIX_FIRMWARE="$COREMATRIX_BIN" \
  cargo test --release -p esp32c61 --test ble -- --include-ignored
```

## Original passive scan reception

RX traverses link-state +8 (header+4), header +4 (next header+4), and header
+8 (RX buffer). The intentionally empty initial header is skipped; guest-owned
software queue fields remain guest-owned. `r_ble_lll_set_rxbuf_default_value`
sets RX +12.low24 and +24.low16 to all ones. Only an unused backed buffer is
written. A received packet uses +28/+29 for the BLE header and +30 for payload,
+15 for RSSI, +16 for microsecond timestamp, +20.low20=`0xfffff` for no associated
info, and +24 for schedule identity, channel frequency and PHY. Completed
valid packet status +12.low24=0 passes the controller's ordinary CRC gate.
`r_ble_lll_scan_alloc_memory +0xf8..0x128` programs schedule +32.low16
with `1 + sdkconfig[66:u16] + sdkconfig[56:u16]`. That schedule identity is
returned in RX +24.low16, exactly the lower-bound formula checked by
`r_ble_lll_scan_rx_process +0x2a..0x7e`. The emulator reads the hardware
descriptor, not private firmware configuration symbols or hardcoded IDs.

The current header and link-state +20.bit27 are updated for the real
`r_ble_lll_get_rxed_buffer` consumer, and the normal scheduler IRQ drives
`r_ble_lll_scan_rx_process`; there is no generated HCI callback.

The external test supplies two bounded nearby advertisers and executes an
original `gap_scan(500,30000,30000,False)`. It receives both exact records:

```text
BLE_RX 0 b'123456789abc' 2 -42 b'02010609094e65617262793631'
BLE_RX 1 b'cafe00000042' 2 -71 b'02010606094f74686572'
BLE_SCAN_DONE
C61_BLE_RX_DONE
```

Type2 here is the host advertising-report type for on-air ADV_SCAN_IND
(type6). The original host stack performs that conversion and address-byte
ordering. At least eight packets arrive across both devices, then normal scan
completion and BLE disable execute with zero CPU exceptions. Unit checks cover
empty headers, SRAM bounds, cyclic chains, ownership, malformed packets and
transactional validation of the nearby list. Invalid descriptors do not write
memory or fabricate completion.

The host environment accepts at most 32 unique advertisers, advertising data
at most 31 bytes, valid public/random address types and bounded RSSI. Clearing
the list removes the environment; CPU reboot retains it. A nonempty
`scan_response` is rejected explicitly. Nearby frames are offered at a nominal
100 ms interval when an eligible 1M scan descriptor executes. This models a
bounded digital packet environment, not calibrated RF airtime, collisions,
channel attenuation, or active scan-request/response timing.
