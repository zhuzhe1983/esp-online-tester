//! What a chip provides to `Machine`: its cores, and a bus that also answers the machine's
//! questions about console output, reset, boot, the board, audio and interrupt routing.
use crate::board::BoardModel;
use emu_core::{Bus, Core, LifecycleKind};
use esp_periph::Misc;

/// Why `Machine::run` returned.
#[derive(Clone, Debug)]
pub enum Stop {
    MaxInsns,
    Halted,
    Breakpoint(u32),
    Unimplemented(u32, u32),
    /// a chip reset was requested (software, watchdog): `reboot()` and run again
    SwReset,
    /// `simcall` — Xtensa semihosting
    Simcall(u32),
    /// `ebreak` with no handler installed — a panic or an assert in a RISC-V guest
    Ebreak(u32),
    /// `--watch`: a word changed value (addr, old, new)
    Watch(u32, u32, u32),
    Exceptions(u64),
    /// The model could not price an execution event. Its effects remain committed.
    CostModel { core: usize, pc: u32, reason: String },
    /// A timing configuration or lifecycle event was rejected.
    CostModelLifecycle { kind: LifecycleKind, reason: String },
}

/// Why `Machine::run_until_cycle` returned.
#[derive(Debug)]
pub enum RunUntil {
    /// device time is at (or, by the last instruction, just past) the target cycle
    Reached,
    /// the bus flagged a host event (`SocBus::take_host_event`): time stands at the instruction that caused it
    Yield,
    Stop(Stop),
}

/// A secondary core's state as its SoC registers say.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CoreState { Running, Held, Reset }

pub trait Soc: 'static {
    type Core: Core;
    type Bus: SocBus;
    const NAME: &'static str;
    /// Flash offset for a separately supplied bootloader image.
    const BOOTLOADER_OFFSET: usize = 0;
    /// The mask ROM ELF's file name in espressif/esp-rom-elfs.
    const ROM_ELF: &'static str;
    const CPU_HZ: u64;
    const CORES: usize;
    /// How far time jumps when every core sleeps (at most; deadlines and limits cut it).
    const IDLE_CHUNK: u64;
    /// Whether a run boots from the mask ROM and shows UART0 unless told otherwise; the others
    /// default to `--boot app` and both consoles.
    const ROM_BOOT_DEFAULT: bool = Self::CORES == 1;
    /// Symbols, in order of preference, that start the ROM's RAM-initialiser table.
    const ROM_DATA_TABLE: &'static [&'static str];
    /// Symbols, in order of preference, that end it.
    const ROM_DATA_TABLE_END: &'static [&'static str] = &["_data_end"];
    /// Bytes per table entry: (dst_start, dst_end, rom_src[, 0]) — 16 on the S3 and C3, 12 on the C6.
    const ROM_DATA_TABLE_STRIDE: u32 = 16;
    fn new_core(i: usize) -> Self::Core;
    /// Connect core-local state to resources owned by this machine's bus.
    fn new_core_with_bus(i: usize, _bus: &Self::Bus) -> Self::Core { Self::new_core(i) }
    /// Bring core `i` back to its reset state (after a chip reset or a release from reset).
    fn reset_core(core: &mut Self::Core, i: usize);
    /// Reset a core and invalidate any bus-owned presentation state erased by that reset.
    /// The default only resets the core; event-driven buses must specialize it to invalidate IRQs.
    #[inline]
    fn reset_core_with_bus(core: &mut Self::Core, _bus: &mut Self::Bus, i: usize) { Self::reset_core(core, i); }
    /// Set a core up to start the app image at `entry` as the 2nd-stage bootloader would have.
    fn boot_core(core: &mut Self::Core, entry: u32);
    /// The interrupt input of every core, from the bus's current source state.
    fn irqs(bus: &Self::Bus, out: &mut [<Self::Core as Core>::Irq]);
    /// Cold peripheral-change boundary, after interrupt presentation. A chip that powers
    /// its CPUs off can park them here without adding a sleep poll to instruction dispatch.
    #[inline]
    fn peripheral_event(_bus: &Self::Bus, _cores: &mut [Self::Core]) {}
    /// Optional function-entry substitutions, resolved from a guest ELF by the chip.
    fn function_hooks(_bus: &Self::Bus) -> &[u32] { &[] }
    fn function_hook(_core: &mut Self::Core, _bus: &mut Self::Bus) -> bool { false }
    fn core_state(_bus: &Self::Bus, _core: usize) -> CoreState { CoreState::Running }
    /// Core `core` took interrupt `irq` (the number its `Trap::Interrupt` carries): the place for
    /// what the interrupt controller does on acceptance, such as clearing an edge-triggered
    /// pending bit.
    #[inline(always)]
    fn interrupt_taken(_bus: &mut Self::Bus, _core: usize, _irq: u32) {}
    /// The machine is about to run core `core`: the place for a bus that decodes core-local
    /// addresses to learn whose they are.
    #[inline(always)]
    fn select_core(_bus: &mut Self::Bus, _core: usize) {}
    /// Refuse a bootloader or application image (an `esp_image_header_t` first) this model cannot
    /// run, such as one built for another silicon revision; the error completes "the image ...".
    /// The front ends call it for the images they load, before the first instruction.
    fn check_image(_image: &[u8]) -> Result<(), String> { Ok(()) }
}

/// Programmed GPIO state, not the resolved electrical level or peripheral-matrix output.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct GpioState {
    pub output: bool,
    pub output_enable: bool,
    pub pull_up: bool,
    pub pull_down: bool,
}

impl GpioState {
    /// Construct a snapshot of the output latch, output enable and programmed pulls.
    pub const fn new(output: bool, output_enable: bool, pull_up: bool, pull_down: bool) -> Self {
        Self { output, output_enable, pull_up, pull_down }
    }
}

pub trait SocBus: Bus {
    fn enable_ble(&mut self, _elf: &crate::elf::Elf) -> Result<(), String> { Err("BLE is unsupported on this chip".into()) }
    fn enable_ble_full(&mut self, _observe: bool) -> Result<(), String> { Err("full BLE requires C3".into()) }
    fn ble_enabled(&self) -> bool { false }
    fn ble_pending_commands(&self) -> usize { 0 }
    fn ble_command(&mut self, _command: &str) -> Result<(), String> { Err("BLE is not enabled".into()) }
    /// Put a virtual access point (`--wifi`) and the network behind it, with `nat` to the host if
    /// given, on the air the chip's WiFi MAC hears, logging as the bus's debug flags say. They
    /// survive a guest reboot. A second call replaces them and restarts the link's queues, pacing
    /// and counts; the relay mode stays.
    fn attach_wifi(&mut self, _cfg: crate::wifi::ApConfig, _nat: Option<crate::nat::Nat>) -> Result<(), String> { Err("this chip has no WiFi".into()) }
    /// Select host Ethernet transport instead of the built-in virtual network. Off by default.
    /// Changing modes clears queued Ethernet frames; the selection survives chip reset.
    fn set_ethernet_relay(&mut self, _enabled: bool) -> Result<(), String> { Err("this chip has no Ethernet transport".into()) }
    /// Drain complete Ethernet frames without FCS, in transmit order. Empty outside relay mode.
    fn take_ethernet_frames(&mut self) -> Vec<Vec<u8>> { Vec::new() }
    /// Queue a 14..=1518-byte Ethernet frame without FCS. Requires relay mode and an AP.
    /// At most 64 frames may await delivery; a full queue returns an error without changing it.
    fn receive_ethernet_frame(&mut self, _frame: &[u8]) -> Result<(), String> { Err("this chip has no Ethernet transport".into()) }
    fn cycles(&self) -> u64;
    /// Convert a positive bus-reference interval to CPU clocks, rounding up to
    /// the first CPU boundary at/after it. Fixed-clock chips compile to identity.
    #[inline]
    fn cpu_cycles_for_bus(&self, cycles: u64) -> u64 { cycles }
    /// CPU cycles from the current device horizon to the next transition that may wake a core.
    fn next_deadline(&self) -> Option<u64> { None }
    fn irq_dirty(&mut self) -> &mut bool;
    /// Arm or disarm device-register deferral for a multi-quantum run (EX133); clears the flag.
    /// Whether `set_defer` is honoured; without it a multi-quantum run is never attempted.
    fn can_defer(&self) -> bool { false }
    fn set_defer(&mut self, on: bool) { let _ = on; }
    /// Whether the last dispatch stopped in front of a device-register access; clears it.
    fn take_deferred(&mut self) -> bool { false }
    /// Re-derive the interrupt lines after a device change; true if a core's input may differ.
    fn refresh_irq(&mut self) -> bool;
    /// Deliver deferred device time now (a bus that defers it).
    fn flush_ticks(&mut self) {}
    /// A device event the host must see at the instruction that caused it (a radio
    /// transmission starting): `Machine::run_until_cycle` stops its round there and returns
    /// `RunUntil::Yield`. Reading it clears it.
    fn take_host_event(&mut self) -> bool { false }
    fn misc(&mut self) -> &mut Misc;
    fn load_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), String>;
    fn write_flash(&mut self, offset: usize, data: &[u8]) -> Result<(), String>;
    /// Map and copy the app image at flash `app_off` as the bootloader would; returns the entry point.
    fn boot_app(&mut self, app_off: usize) -> Result<u32, String>;
    /// Chip reset: re-create the digital peripherals, keep what survives on silicon. Returns the cause.
    fn reboot(&mut self, mac: [u8; 6]) -> u32;
    fn sw_reset(&self) -> bool;
    /// The board's reset button: the chip resets with this cause at the next scheduling round,
    /// through the same path as a reset the firmware asked for.
    fn request_reset(&mut self, cause: u32);
    fn reset_cause(&self) -> u32;
    fn last_fault(&self) -> Option<(u32, bool)>;
    /// Console bytes since the last call: USB-Serial/JTAG, UART0, UART1, UART2.
    fn console_take(&mut self) -> [Vec<u8>; 4];
    /// Bytes from the host into the USB-Serial/JTAG console.
    fn serial_input(&mut self, data: &[u8]);
    /// Bytes from the host into UART `n`'s receive FIFO (a terminal on the chip's UART0 pins).
    fn uart_input(&mut self, n: usize, data: &[u8]);
    /// Release a host drive so the chip can resolve its pad pulls again.
    fn gpio_release_input(&mut self, _pin: u8) {}
    fn gpio_set_input(&mut self, pin: u8, level: bool);
    /// Drive an analog pad (script `adc` / `adcwave`); chips without an ADC model ignore it.
    fn analog_set(&mut self, _pin: u8, _src: esp_periph::AnalogSource) {}
    /// Set a post-attenuation 12-bit ADC input; false for unsupported pins or counts above 4095.
    fn adc_set_raw(&mut self, _pin: u8, _raw: u16) -> bool { false }
    /// Completed conversions for an ADC pad, or None for an unsupported pin. Generation wraps at u64::MAX.
    fn adc_observation(&self, _pin: u8) -> Option<esp_periph::AdcObservation> { None }
    /// Touch or release a capacitive GPIO pad, separate from the board touch panel.
    fn set_touch_input(&mut self, _pin: u8, _touched: bool) {}
    /// True for a GPIO with a capacitive touch pad; the script rejects `touchpad` elsewhere.
    fn has_touch_pad(&self, _pin: u8) -> bool { false }
    /// Deliver host touch at the bus's current time horizon.
    fn touch_input(&mut self, x: u16, y: u16, down: bool) { self.board().touch(x, y, down); }
    fn gpio_input(&self) -> u64;
    /// GPIO latch, enable and IO_MUX pulls. None for an unavailable pin or unsupported chip.
    /// Pulls describe the programmed bits; they do not change the input level.
    fn gpio_state(&self, _pin: u8) -> Option<GpioState> { None }
    /// Averaged PWM at a GPIO-matrix output: frequency in Hz and high-time fraction scaled to 0..65535.
    /// `None` means no supported running PWM snapshot: unrouted, disabled, paused or unsupported.
    /// It does not mean a low pin; a paused timer can hold a level. Running 0/100% duty is `Some`.
    fn pwm_output(&self, _pin: u8) -> Option<(f64, u32)> { None }
    /// Start/stop recording GPIO edges (outputs as they reach the board, inputs as they are set).
    fn observe_gpio(&mut self, on: bool);
    /// (cycle, pin, level) edges recorded since the last call.
    fn take_gpio_events(&mut self) -> Vec<(u64, u8, bool)>;
    fn board(&mut self) -> &mut dyn BoardModel;
    fn board_ref(&self) -> &dyn BoardModel;
    /// Host PCM source for an I2S controller, or None when the controller is absent.
    fn i2s_input(&mut self, _port: usize) -> Option<&mut esp_periph::i2s::PcmInput> { None }
    /// The attached pin-routed PCM source bank, read-only, as of its last synchronization.
    /// Never attaches, flushes ticks or advances source time; None until `attach_pcm_sources`.
    fn pcm_sources(&self) -> Option<&esp_periph::i2s::PcmSources> { None }
    /// Attach the pin-routed source bank on first use and synchronize its clock to the bus.
    /// Call before each push or slot edit. None when the chip has no I2S receiver.
    fn attach_pcm_sources(&mut self) -> Option<&mut esp_periph::i2s::PcmSources> { None }
    /// Captured audio so far (left channel) and its sample rate.
    fn audio(&self) -> (&[i16], u32);
    fn camera_frames(&self) -> u64 { 0 }
    /// Peripheral source numbers routed to CPU interrupt `line` of `core` (for the end-of-run report).
    fn irq_sources_of(&self, core: usize, line: u32) -> Vec<usize>;
    /// Apply the debug areas to the devices and to the bus's own logging.
    fn set_debug(&mut self, f: &crate::debug::DebugFlags);
    /// Resize the flash array (the JEDEC capacity follows).
    fn set_flash_size(&mut self, bytes: usize);
    /// Resize the PSRAM, on chips that have one.
    fn set_psram_size(&mut self, _bytes: usize) -> Result<(), String> { Err("this chip has no PSRAM".into()) }
    /// Strapping pins as the ROM reads them.
    fn set_strap(&mut self, v: u32);
    /// The reset cause the ROM will report (to reproduce a real board's boot).
    fn set_reset_cause(&mut self, cause: u32);
    /// Chip-specific end-of-run statistics (audio, WiFi, crypto, DMA engines).
    fn report(&self) -> String { String::new() }
    /// Put an SD card holding `image` in the board's SD socket, in place of the one there; says
    /// where it went.
    fn attach_sd_card(&mut self, _image: Box<dyn esp_periph::sdmmc::SdStore>, _config: esp_periph::sdmmc::SdCardConfig) -> Result<String, String> { Err("this chip has no SD card host".into()) }
    /// Plug a USB host into the chip's USB-OTG device port, in place of one there; host software
    /// (`crate::usb_host`) drives it through `usb_link`. Unpowered until it asks for VBUS.
    fn attach_usb_host(&mut self, _timing: esp_periph::usb_otg::UsbTiming) -> Result<(), String> { Err("this chip has no USB-OTG device port".into()) }
    /// The cable `attach_usb_host` plugged in.
    fn usb_link(&mut self) -> Option<&mut esp_periph::usb_otg::UsbLink> { None }
    /// Refresh derived board outputs from current peripheral state before host capture. No time advances.
    fn refresh_board_outputs(&mut self) {}
}

/// Attach and synchronize the shared host source bank without adding device-tick work.
pub fn attach_pcm_sources(bank: &mut Option<Box<esp_periph::i2s::PcmSources>>, cycles: u64, cpu_hz: u64) -> &mut esp_periph::i2s::PcmSources {
    let sources = bank.get_or_insert_with(Default::default);
    sources.advance_to(cycles, cpu_hz);
    sources
}
