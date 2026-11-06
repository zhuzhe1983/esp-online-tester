# ESP32-C61 BLE controller bring-up

This is a separate C61 digital controller implementation. It does not patch an
image, replace HCI entry points, or borrow the C6 controller's firmware hooks.
It is incomplete: controller activation executes successfully, but scheduling,
scan, advertising, connections and a virtual radio link remain unqualified.

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

Only these polled operations are connected. BLE interrupt sources in the C61
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
After the device, the original controller returns `BLE_ACTIVE True` with zero
CPU exceptions. The scan call does not return within the 300M instruction
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
still does **not** establish reception or on-air advertising: the hardware
scheduler descriptor execution and packet delivery are the next boundary.

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
and BLE deactivation still finish with zero CPU exceptions. Packet parsing
and delivery remain a separate qualification step.
