# Adding things

Four recipes. A peripheral or an observer is one implementation plus one line in a table and
leaves the scheduler alone. A board or a chip also has to be named by hand in the front ends;
each recipe lists the places.

## A peripheral

1. Write the model in `esp-periph/src/<name>.rs` (shared IP) or in the chip crate's `periph.rs`
   (chip-only). It owns its registers and implements `esp_periph::Device`:

   ```rust
   impl Device for Ledc {
       fn read(&mut self, off: u32) -> u32 { ... }
       fn write(&mut self, off: u32, v: u32) -> WriteEffect { ...; WriteEffect::NONE }
       fn irq_sources(&self) -> u64 { self.irq() as u64 }              // bit i = i-th source asserted
       fn clock(&self) -> Option<ClockDomain> { Some(ClockDomain::Apb) }   // if it keeps time
       fn tick(&mut self, ticks: u64) { ... }
       fn has_deadline(&self) -> bool { true }                          // if it can say when it fires next
       fn next_deadline(&self) -> Option<u64> { ... }                  // in its clock's ticks
       fn debug(&mut self, on: bool) { self.log = on; }                 // `--debug ledc`
   }
   ```
   Unhandled offsets go to a `RegRam` (read back what was written) — that is what an unmodelled
   register does today, so start from `--log-periph` and model only what the firmware polls.

2. Add a field to the chip's `Peripherals` and one line to its `device_set!` table:

   ```rust
   0x19 "LEDC" (ledc) => [SRC_LEDC];
   ```
   block number, name (also the `--debug` area), field, and the chip's source numbers in the
   order `irq_sources` numbers them. `@ lo..=hi` limits an entry to part of a block, `delta` shifts
   the offset the device sees, `alias` mounts a further block of the same device.

3. If it moves data through memory (a DMA engine, a frame source), the pump goes in the chip's
   bus next to the I2S/camera ones (`esp32s3/src/bus/dma.rs` on the S3, `bus.rs` on the other
   chips); the device exposes what the pump needs.

4. If a reboot must keep some of its state (a strap, a JEDEC id, captured audio), copy it across
   in the chip's `SocBus::reboot`.

Check: `cargo test --release --workspace -- --include-ignored` unchanged, `--log-periph` no longer
lists the registers you modelled.

## A board (a device with things wired to the chip)

A board is one `impl esp_soc::BoardModel`: the SoC emits pin-level events and asks the board for
what the UI needs. The chip model never knows what board it is on. Two real ones to copy from:
`Atech14` (bit-banged SPI display on GPIOs, WS2812 ring on RMT, buttons and an encoder) and
`WaveshareLcd4b` (I2C IO expander and touch controller, RGB panel on LCD_CAM, codec on I2S), both
under `esp32s3/src/board/`. The device models a board is built from — DCS display
controllers, WS2812 chains, a bit-banged SPI slave — are in `esp-soc/src/devices/`.

Say the new board has a BME280-style I2C sensor, a status LED on GPIO 5, and a button on GPIO 0.

1. **The I2C device.** A register-style device is an `esp_periph::i2c::I2cDevice`: the master
   addresses it (`start`), writes bytes (`write`, return ACK), reads bytes (`read`), stops. For a
   plain "first byte selects the register, then data" device there is `Reg8Device::new(name,
   defaults)` — give it the register defaults the driver identifies the chip by:

   ```rust
   Reg8Device::new("bme280", &[(0xd0, 0x60), (0x88, 0x6e), (0x89, 0x6c) /* calibration... */])
   ```
   Anything with more logic (a FIFO, a measurement that changes) implements the trait itself;
   `Gt911` (touch, `esp32s3/src/i2c.rs`) and `Ov5640` (camera SCCB, `esp-soc/src/camera.rs`) are
   the two shapes.

2. **The board.**

   ```rust
   pub struct Sensorboard { led: bool, gpio_events: u64 }
   impl BoardModel for Sensorboard {
       fn name(&self) -> &'static str { "sensorboard" }
       // output edges, in order; pick out the pins that are yours
       fn gpio_changes(&mut self, changes: &[(u8, bool)]) {
           for &(pin, level) in changes { self.gpio_events += 1; if pin == 5 { self.led = level; } }
       }
       fn gpio_events(&self) -> u64 { self.gpio_events }
       // (bus, 7-bit address, device): attached to the I2C controller at start
       fn i2c_devices(&mut self) -> Vec<(u8, u8, Box<dyn I2cDevice>)> {
           vec![(0, 0x76, Box::new(Reg8Device::new("bme280", &[(0xd0, 0x60)])))]
       }
       // what scripts and the UI may call the button; the encoder pins if there is one
       fn named_pin(&self, n: &str) -> Option<u8> { (n == "btn1").then_some(0) }
       // show the LED as a one-LED strip; the UI already draws LEDs
       fn leds(&self) -> Option<(&[[u8; 3]], u64)> { ... }
       fn report(&self) -> String { format!("[emu] led {}", if self.led { "on" } else { "off" }) }
   }
   ```
   Inputs are not board methods: a button is `press btn1 150` in a script (`docs/cli.md`) or a
   click in the page, both of which end up in `gpio_set_input` on the SoC.

3. **Register it**: one arm in the chip's `make_board` (`"sensorboard" =>
   Some(Box::new(Sensorboard::new()))`; `esp32s3/src/board/mod.rs`, `esp32/src/board/mod.rs`,
   `esp32c6/src/board.rs`, `esp32p4/src/board.rs`). That is all `--board sensorboard` and
   `esp32sim_new` need: the wasm build finds the chip by asking each chip's `make_board` (a P4
   board implies rev ≥ 3). The names are also listed by hand, so add it to:
   - `cli/src/lib.rs`: the `--board` help text, the chip's unknown-board message, and the memory
     defaults in `setup_s3` if the module differs from 8 MB flash / 2 MB PSRAM (`stopwatch`);
   - `web/emu.js`: the board `<select>`, `MEMORY` and, off the S3, `romFor`;
   - `web/workbench.js`: the `BOARD` info and, off the S3, `CHIP`;
   - `web/run.html`: the page title in the `board` message handler, and `PANEL_TEXT` /
     `PANEL_ROTATE` for a panel.

4. **Displays, cameras, audio.** A display fed pixel by pixel implements `display()`,
   `display_version()` and says `display_quiet_push() == true` (`Atech14`); one that receives whole
   frames from LCD_CAM implements `lcd_frame` and `display()` (`WaveshareLcd4b`). A camera returns
   YUYV frames from `camera_frame` and a preview from `camera_preview`. Audio needs nothing from
   the board: I2S output is captured by the SoC (`--wav`, the page's player).

5. **If the SoC does not yet produce the event you need** (say, the board hangs a device on SPI3,
   or wants LEDC PWM duty for a backlight), that is a peripheral, previous recipe; the board then
   gets the event through a new `BoardModel` method with a default, as `spi_tx` and `lcd_frame` did.

Run it: `esp32sim --board sensorboard --boot rom ... --log-periph` and watch which registers the
firmware polls that nothing answers yet. Add a row to `docs/boards.md`, the name to the
`--board` row of `docs/cli.md`, and a run script under `examples/<board>/`.

## A CPU and a chip

1. A core crate implementing `emu_core::Core` over `emu_core::Bus` (see `riscv-rv32/src/core.rs`
   for the minimal one: `step`, `set_irq`, `idle_advance`, the trace/dump surface). A fast path
   overrides `run` and stops at `bus.block_break()` so the machine can re-derive interrupt lines
   at the same instruction the slow path would.
2. A chip crate with: the memory map (`impl Bus for SocBus`), the `Peripherals` set (`device_set!`
   plus `impl DeviceSet`: block names, `pre_access` for the registers that depend on another
   device), the interrupt controller, and `soc.rs`: `impl Soc` (cores, `irqs` per core, secondary
   core control) and `impl SocBus` (console streams, reset, app boot, board, audio, interrupt
   routing, flash size, strap, reset cause, report).
3. `pub type Machine = esp_soc::Machine<C6>;` and a `machine()` constructor. The front ends name
   each chip by hand:
   - the workspace `members` in `Cargo.toml`;
   - the chip's `ChipPins` and `UartPins` in `esp-soc/src/pins.rs` and `esp-soc/src/uart.rs`;
   - `cli/`: the dependency, a `setup_c6` in `src/lib.rs` with its list of flags the chip
     refuses, the `--chip` arm in `run_cli`, the help text, and an `esp32sim-c6` alias in
     `src/bin/` if the chip gets one;
   - `wasm/`: the dependency, a branch in `esp32sim_new` (`src/lib.rs`), and a `MachineKind`
     variant with its `Deref`/`DerefMut` arms (`src/machine.rs`);
   - `web/emu.js` (`MEMORY`, `romFor`, the board `<select>`) and `web/workbench.js` (`CHIP`,
     `BOARD`);
   - `tools/fetch-rom-elfs.sh`: the ROM ELF and its SHA-256.

This is exactly the path the ESP32-C6 took (`esp32c6/`, [esp32c6.md](esp32c6.md)): the C3 crate
copied, the map and the interrupt controller rewritten, then `--log-periph` and the board's
console until hello_world matched. If the ROM's RAM-initialiser table has a different shape, the
`Soc` trait's `ROM_DATA_TABLE_END` / `ROM_DATA_TABLE_STRIDE` say so (12-byte entries on the C6).

Everything else — scheduler, device time, console, scripts, stubs, observers, the web UI,
reboot, image loading — is `esp_soc::Machine` and needs no change, unless the ROM initialises RAM
outside its data table: the C3's BT initializer is copied by chip name in `Machine::load_rom`.

## An observer

`impl esp_soc::Observer<S>` in `esp-soc/src/observers/`: say what you want (`Wants::BLOCK` runs at
full speed; `INSN` single-steps; `NO_IDLE_SKIP` changes timing — only ask for it if you count
instructions), implement the hooks, and produce a `report`. Register it with
`Machine::add_observer` from a CLI flag in `cli/src/lib.rs` and, if the page should have it, a
name in `MachineApi::observer` in `wasm/src/lib.rs`. `block_profile.rs` is the template.
