# C5 detached USB SOF monitor subset

This change prepares a digital register subset in the existing `DetachedUsb` owner. It has no host transport or SOF producer. The unchanged LCD273 factory's tick callback is a static future consumer: `40801c8e:471c` reads `6000f008`, and `40801c98:cb54` writes `6000f014=2`. Its priority 230 SECONDARY initializer registers callback `40801c84`; it does not execute that callback. The original factory had not reached this hook at preparation time. No guest, scheduler, USB enumeration, app_main or LCD success is established here.

The source matches the original descriptor's IDF 5.5.3, fixed at `2c211b236707889e8400c4dc5644dd5c4ee071e0`. The [connection monitor](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_driver_usb_serial_jtag/src/usb_serial_jtag_connection_monitor.c#L37) reads RAW's SOF bit and clears it; [C5 LL](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/include/hal/usb_serial_jtag_ll.h#L23) defines SOF as bit 1. Monitor, C5 LL and C5 register headers are byte-identical to the checked fixed 5.5.4 revision. [Sources](sources.json) pin hashes, versions, original bytes and the complete typed-listing hash; raw source and listings remain ignored.

The [C5 register header](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/usb_serial_jtag_reg.h#L58) and TRM 1.1 §37.4/Register 37.14 distinguish SOF reset 0 from IN_EMPTY reset 1. Named whole RAW POR is therefore 8. No host means no received SOF or USB packet/error/control events. The [driver ISR and installation](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_driver_usb_serial_jtag/src/usb_serial_jtag.c#L67) use IN_EMPTY as an event and preserve its initial flag; they do not establish its precise edge after CPU FIFO/WR_DONE writes. Thus accepting TX data or WR_DONE permanently makes whole RAW readback unknown for that instance. The model neither clears RAW bit 3 speculatively nor fabricates host consumption. SOF-only clear cannot repair unknown RAW.

The integration API uses offsets relative to `6000f000`:

```rust
read_monitor(&self, off: u32, clock_qualified: bool) -> Option<u32>
write_monitor(&mut self, off: u32, value: u32, clock_qualified: bool) -> bool
```

| Address / offset | Qualified read | Qualified write |
| --- | --- | --- |
| `6000f008 / 08` RAW |8 before TX/WR_DONE; otherwise Unsupported (`None`) |Unsupported |
| `6000f00c / 0c` ST |0; only SOF may be enabled and has no producer |Unsupported |
| `6000f010 / 10` ENA |Last accepted 0 or 2; POR 0 |Only 0 or 2, transactionally |
| `6000f014 / 14` CLR |Unsupported |Only 0 or 2; leaves RAW bit 3/FIFO/ENA intact |

Exact offsets only; unaligned offsets and all other monitor operations reject. In particular CLR bit 3, enabling IN_EMPTY/other interrupts, RAW software writes, PHY, JTAG and debug registers remain Unsupported. The wider shared LL all-interrupt mask is not a supported mask here. There is no new IRQ producer or injection API; source 54 is not activated by this owner subset. No timer, packet ACK or USB bus reset is generated. Legacy `read`/`write` remain limited to offsets 0/4, with the same 64-byte FIFO, DATA_FREE and packet-seal behavior.

`clock_qualified=false` rejects every monitor read/write without mutation. The caller must prove an unchanged cold [PCR_USB_DEVICE_CONF](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/soc/esp32c5/register/soc/pcr_reg.h#L1088) named POR (CLK_EN 1, RST_EN 0), with no accepted unmodeled PCR writes, and known APB plus available PLL48 tap. TRM 1.1 §37.5 and C5 LL state the BBPLL dependency. The boolean is a qualification for this digital observation, not PHY lock/settling, READY or attachment evidence. This owner does not add PCR routing, reset completion or clock transitions. The caller must preserve unknown/failure results as Unsupported. Changing the boolean does not reset the instance; only actual owner reconstruction starts its existing cold FIFO/monitor state. CPU reset is not a new USB reset API.

[Checks](checks.json) record 10/10 direct module cases, including the two existing FIFO tests unchanged, plus warning-denied rustc/Clippy checks. All cases are hermetic with no ignored/filtered tests or private firmware dependency. They cover the static register pair, repeated clear, SOF masks/ST, TX/WR_DONE uncertainty, rejected RAW/CLR bit 3/unqualified operations, and state retention across qualification changes. They exercise register components, not the original instructions or a FreeRTOS tick. No Cargo/dependency, firmware, native guest or WASM build, benchmark, browser operation, Bus/lib/Soc/board edit or publication was performed.

Reproduce from this tree with stable Rust; the output directory must be ignored:

```sh
mkdir -p target/c5-usb-sof-owned
rustc +stable --edition=2021 --test tools/c5-usb-sof-harness.rs -D warnings \
  -o target/c5-usb-sof-owned/owned-tests
target/c5-usb-sof-owned/owned-tests --include-ignored --skip external_ --test-threads=1
CLIPPY_DRIVER="$(rustup which --toolchain stable clippy-driver)"
"$CLIPPY_DRIVER" --edition=2021 --test tools/c5-usb-sof-harness.rs -D warnings \
  -o target/c5-usb-sof-owned/owned-lint-tests
target/c5-usb-sof-owned/owned-lint-tests --include-ignored --skip external_ --test-threads=1
```

The receipt omits absolute local identities, full source/disassembly, binary input and raw test logs. Their hashes remain recorded. Integration clock guards and genuine factory continuation require separate checks; this preparation does not open the public C5 gate.
