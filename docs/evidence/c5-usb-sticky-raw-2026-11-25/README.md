# C5 USB RAW3 retains interrupt history after TX

The original factory now reaches the actual FreeRTOS tick callback. The supplied integration run stopped at `40801c8e:471c`, a whole-word read of `6000f008`, after 29,525,038 instructions / 29,524,992 nominal cycles, with 3991 UART bytes and zero nonzero LCD pixels. The log prints `main_task: Started on CPU0`; it does not establish app_main or LCD output. A second pure observation captured the same stop/counts: clock qualification true, RAW unknown even without the clock guard, ENA 0, DATA_FREE 0, APB 40MHz and PLL48 available. These are retained negative results from the earlier conservative owner, not a new run performed here. [Checks](checks.json) pin both raw captures and their source limits.

A third supplied observation records the actual early ROM path: the EP1 store at `40025d46:0089a023` is reached at nominal cycles 8064/8320/8576/8832 with ASCII `ESP-`, and the WR_DONE store at `40025d56:c3d8` is reached first at 15936, then 261184/273792/285376. The stop/counts remain identical. Thus earlier USB FIFO writes are observed independently of UART output; the later app VFS callers remain static candidates. The receipt keeps these bounded PC/cycle facts and the original capture hash, without its raw register or UART stream. The new latch implementation has not been run against the original factory here.

Commit `3570031d7162834a786486ce0e5896edd613c8e8` made whole RAW unknown after TX/WR_DONE because the exact IN_EMPTY edge was unqualified. Its [receipt](../c5-usb-sof-2026-11-25/README.md), tests and hashes remain unchanged. The new C5-specific evidence is the TRM 1.1 access-type glossary, printed pages 1740–1741: hardware self-set SS and self-clear SC are separate types; WTC clears upon a write of 1 to the corresponding WT field. Register 37.14 and the fixed C5 header mark RAW3 R/WTC/SS, without SC. Register 37.17 defines CLR3 as WT. Consequently, an already-set RAW3 is not cleared merely because the FIFO becomes occupied. This inference follows the published register contract; it is not a measurement of silicon timing. [Sources](sources.json) pin the exact PDF/text and fixed IDF 5.5.3/5.5.4 headers.

TRM §37.3.2 separately explains FIFO occupancy: CPU writes prepare an IN packet; 64 bytes or WR_DONE flush it for a host to fetch. After the host has fully read it, a new IN_EMPTY event occurs. With no host the packet remains occupied. The fixed [IDF5.5.3 driver ISR](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/esp_driver_usb_serial_jtag/src/usb_serial_jtag.c#L67) explicitly clears IN_EMPTY and waits for a later completed transfer, while installation preserves its initial flag. Neither [C5 LL's EP1/WR_DONE writes](https://github.com/espressif/esp-idf/blob/2c211b236707889e8400c4dc5644dd5c4ee071e0/components/hal/esp32c5/include/hal/usb_serial_jtag_ll.h#L125) nor the register descriptions define an implicit RAW3 clear there. The model therefore stores a RAW latch initialized to 8. FIFO contents/seal and this interrupt history are independent.

The unchanged accepted-operation set closes the published named RAW state as follows:

| Bits | Published event | Why accepted TX/WR_DONE does not create it |
| --- | --- | --- |
|0,1,2,8,10,11 | Received JTAG flush, SOF, OUT packet, IN token or zero-length OUT packet | No host, received packet/token or JTAG command input; CPU WR_DONE prepares IN, not OUT |
|4–7 | USB PID/CRC5/CRC16/stuff error detection | No USB line/PHY input or packet exchange; preparing an IN buffer alone is not an on-wire transfer |
|9 | Detected USB bus reset | No bus-reset producer or accepted reset path |
|12–15 | Host RTS/DTR or GET/SET_LINE_CODING requests | No host control requests or accepted control/PHY register mutators |
|3 | IN_EMPTY, initially 1; R/WTC/SS | No accepted CLR3/RAW write or documented SC; its prior flag stays set |

This closure describes the explicit detached digital profile, not physical line noise or a complete USB device. The other fifteen named bits retain 0; upper reserved bits retain the earlier documented cold POR 0 boundary. The LL all-interrupt mask is not a reason to add hidden/debug events. Overflow writes, RAW writes, CLR/ENA bits outside SOF, PHY/JTAG and unimplemented reset/host paths still reject before mutation. CLR3 after a fresh event, host consumption and precise reassertion timing remain unsupported. No public event/IRQ injection API is added.

The caller-qualified API and exact offsets are unchanged: `read_monitor(off, clock_qualified)` reads RAW+08, ST+0c or ENA+10; `write_monitor(off,value,clock_qualified)` accepts only ENA+10 or CLR+14 values 0/2. CLR applies its SOF-only clear mask to the stored latch; it cannot erase RAW3. ST is `RAW & ENA`, hence 0 in this subset. Unqualified clock, unaligned/other offsets and CLR reads still reject. PCR clock/reset and actual APB/PLL48 checks remain the integration owner's responsibility, with no new PHY-ready or reset-completion claim. The earlier 64-byte FIFO/seal behavior and its two original tests are unchanged.

The two former post-TX/WR_DONE Unknown cases now assert the stronger counterexample: RAW3 stays 1 even when DATA_FREE is 0 and TX remains sealed, and SOF-only clears cannot free the FIFO or erase its history. Qualification changes do not reset it. A new case covers a full 64-byte FIFO plus rejected overflow, RAW/CLR3/ENA3 attempts. All 11 hermetic owner tests pass with no ignored/filtered cases; rustc and direct Clippy checks deny warnings. The new sticky expectation fails against the old `3570031d` implementation in a separate component regression artifact under ignored `target/`. This preserves the old negative and shows exactly what the new source contract changes.

Reproduce without Cargo or dependencies:

```sh
mkdir -p target/c5-usb-sticky-raw-owned
rustc +stable --edition=2021 --test tools/c5-usb-sof-harness.rs -D warnings \
  -o target/c5-usb-sticky-raw-owned/owned-tests
target/c5-usb-sticky-raw-owned/owned-tests --include-ignored --skip external_ --test-threads=1
CLIPPY_DRIVER="$(rustup which --toolchain stable clippy-driver)"
"$CLIPPY_DRIVER" --edition=2021 --test tools/c5-usb-sof-harness.rs -D warnings \
  -o target/c5-usb-sticky-raw-owned/owned-lint-tests
target/c5-usb-sticky-raw-owned/owned-lint-tests --include-ignored --skip external_ --test-threads=1
```

Only `usb_detached.rs` and new evidence change. No Cargo/dependency, firmware, native guest or WASM build, benchmark, browser action or shared Bus/lib/Soc/board edit was performed. The original tick's continuation has not been tested by this change and the public C5 gate remains closed. Raw SDK/ROM listings, binary inputs and capture/test streams remain ignored; the receipt retains bounded facts, commands and hashes without local identities. No other chip or emulator implementation supplies the hardware contract.
