# TCA9554 digital port model

`esp_soc::devices::tca9554` implements the four-register digital subset of TI's
TCA9554. It is opt-in: adding this module does not change any existing board.
The source is TI **SCPS233E**, revised February 2017, especially sections 8.3,
8.4 and 8.6. The original PDF was downloaded again on 2026-11-23 from
[TI's product documentation](https://www.ti.com/lit/ds/symlink/tca9554.pdf).
Its exact bytes and retrieval receipt are in the
[source manifest](../research/tca9554-pcf85063-sources/manifest.json).

## Interface and behavior

The board creates an `Arc<Mutex<Tca9554State>>`, attaches `Tca9554::new(state)`
at the address selected by its real A0/A1/A2 straps, and supplies its actual I2C
pads. Hardware I2C and GPIO I2C may share that one device. Controller reattachment
does not reset a still-powered expander.

The model retains the output latch, polarity and direction registers separately.
POR sets the output and direction registers to `FFh`, and polarity to `00h`.
Default unconnected inputs resolve high using the documented weak pull-ups;
board-specific external drivers can update them with `set_inputs(mask, levels)`.
`driven(pin)` returns `None` while a pin is an input. `levels()` and `high(pin)`
return the resolved digital levels. Output reads return the latch, while Input
Port reads return physical levels with inversion applied only to inputs.

The four-register pointer does not auto-increment. Repeated data bytes update
the selected register. Input Port writes acknowledge without changing state,
as specified in section 8.6.3. Unsupported register pointers NACK instead of
being masked into another register.

`irq_high()` implements the input-change condition and clears it when the input
returns to the last read state or the Input Port is read. Output changes do not
raise INT. `falling_edges(pin)` and `rising_edges(pin)` retain physical transitions
so board reset and chip-select wiring can observe changes made through I2C even
when there is no MCU GPIO event between them.

## Boundaries and verification

The model does not simulate voltage, electrical contention, load current,
rise time, interrupt propagation delay, or an edge occurring inside the ACK
clock. INT clearing is observed at the read-byte boundary. No MCU interrupt
connection is inferred: each board must actually wire the expander INT pin.

Four protocol tests cover POR, non-incrementing reads/writes, read-only Input
Port behavior, direction and polarity, IRQ clearing, transition counts and
shared state after controller reattachment. Exact commands, source hashes and
fresh results are retained in the
[verification manifest](../evidence/tca9554-pcf85063-2026-11-23/manifest.json).
These tests establish the register subset; they are not a board factory-firmware
or physical-hardware acceptance claim.
