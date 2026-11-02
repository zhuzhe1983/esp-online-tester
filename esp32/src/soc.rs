use crate::bus::SocBus;
use crate::periph::{self, Peripherals};
use esp_periph::Misc;
use esp_soc::{BoardModel, CoreState, Soc};
use xtensa_lx7::state::ps;
use xtensa_lx7::Cpu;

pub struct Esp32;
pub type Machine = esp_soc::Machine<Esp32>;
pub fn machine(mac: [u8; 6], flash_size: usize) -> Machine {
    let mut m = Machine::new(mac, SocBus::new(flash_size, mac));
    m.set_debug(&esp_soc::DebugFlags::from_env());
    m
}

impl Soc for Esp32 {
    type Core = Cpu;
    type Bus = SocBus;
    const NAME: &'static str = "esp32";
    const BOOTLOADER_OFFSET: usize = 0x1000;
    const ROM_ELF: &'static str = "esp32_rev300_rom.elf";
    const CPU_HZ: u64 = periph::CPU_HZ;
    const CORES: usize = 2;
    const IDLE_CHUNK: u64 = 512;
    const ROM_DATA_TABLE: &'static [&'static str] = &["_data_start"];
    fn function_hooks(bus: &SocBus) -> &[u32] { &bus.ble.hooks }
    fn function_hook(core: &mut Cpu, bus: &mut SocBus) -> bool { crate::ble::intercept(core, bus) }
    fn new_core(i: usize) -> Cpu {
        let mut c = Cpu::new(if i == 0 { 0xcdcd } else { 0xabab });
        c.lx6 = true;
        c.configid = [0xC2BC_FFFE, 0x1CC5_FE96];
        c
    }
    fn reset_core(c: &mut Cpu, _i: usize) { c.reset(); }
    #[inline]
    fn reset_core_with_bus(c: &mut Cpu, bus: &mut SocBus, i: usize) {
        Self::reset_core(c, i);
        if bus.deep_sleep {
            c.waiting = true;
            c.intenable = 0;
            c.interrupt = 0;
        }
        // Reset erases external levels and edge history. The release write may have
        // been presented in the previous round, before the scheduler performs reset.
        bus.irq_dirty = true;
    }
    fn boot_core(c: &mut Cpu, entry: u32) {
        c.reset();
        c.pc = entry;
        c.ps = ps::WOE | ps::UM;
        c.vecbase = 0x4000_0000;
        c.set_ar(1, 0x3ffe_0000);
    }
    fn irqs(bus: &SocBus, out: &mut [u32]) {
        bus.periph.all_cpu_lines(out);
    }
    fn peripheral_event(bus: &SocBus, cores: &mut [Cpu]) {
        if bus.deep_sleep {
            // The digital domain loses CPU state; park its snapshots using the existing
            // idle path. Only an RTC wake/system reset may release them, never a digital
            // interrupt (including an already latched NMI or local CCOMPARE timer).
            for core in cores {
                core.waiting = true;
                core.intenable = 0;
                core.interrupt = 0;
            }
        }
    }
    fn core_state(bus: &SocBus, core: usize) -> CoreState {
        if core == 0 {
            CoreState::Running
        } else {
            let (clock, reset, stall) = bus.periph.dport.core1_control();
            if reset { CoreState::Reset } else if clock && !stall { CoreState::Running } else { CoreState::Held }
        }
    }
}

impl esp_soc::SocBus for SocBus {
    fn set_psram_size(&mut self, bytes: usize) -> Result<(), String> { self.configure_psram(bytes) }
    fn enable_ble(&mut self, elf: &esp_soc::elf::Elf) -> Result<(), String> {
        self.ble.enable(&elf.by_name, elf.symbol_sizes.get("esp_bt_controller_init").copied().unwrap_or(0), &<Self as esp_soc::ble::vhci::VhciBus>::abi())
    }
    fn ble_enabled(&self) -> bool { !self.ble.hooks.is_empty() }
    fn ble_pending_commands(&self) -> usize { self.ble.session.pending_commands() }
    fn ble_command(&mut self, command: &str) -> Result<(), String> { self.ble.command(command, self.cycles, periph::CPU_HZ) }
    fn ble_nearby(&mut self, list: Vec<esp_soc::ble::peer::Nearby>) -> Result<(), String> { self.ble.set_nearby(list, self.cycles, periph::CPU_HZ) }
    fn attach_wifi(&mut self, cfg: esp_soc::wifi::ApConfig, nat: Option<esp_soc::nat::Nat>) -> Result<(), String> {
        self.periph.wifi.link.attach_new(cfg, nat, &self.debug);
        Ok(())
    }
    fn set_ethernet_relay(&mut self, enabled: bool) -> Result<(), String> {
        self.periph.wifi.link.set_relay(enabled);
        Ok(())
    }
    fn take_ethernet_frames(&mut self) -> Vec<Vec<u8>> { self.periph.wifi.link.take_relay_frames() }
    fn receive_ethernet_frame(&mut self, frame: &[u8]) -> Result<(), String> {
        self.periph.wifi.link.receive_relay_frame(frame)
    }
    fn cycles(&self) -> u64 {
        self.cycles
    }
    fn cpu_cycles_for_bus(&self, cycles: u64) -> u64 {
        cycles.saturating_mul(2).saturating_sub(self.clock_half).div_ceil(self.periph.cpu_scale_half())
    }
    fn next_deadline(&self) -> Option<u64> {
        let timer = match self.periph.cycles_until_timer() {
            u32::MAX => None,
            n => Some(self.cpu_cycles_for_bus(u64::from(n)).max(1)),
        };
        if !self.pins_active { return timer; }
        timer.into_iter().chain(self.board.next_deadline().map(|n| self.cpu_cycles_for_bus(n.saturating_sub(self.cycles)).max(1))).min()
    }
    fn irq_dirty(&mut self) -> &mut bool {
        &mut self.irq_dirty
    }
    fn refresh_irq(&mut self) -> bool {
        true
    }
    fn misc(&mut self) -> &mut Misc {
        &mut self.periph.misc
    }
    fn load_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), String> {
        SocBus::load_bytes(self, addr, data)
    }
    fn write_flash(&mut self, offset: usize, data: &[u8]) -> Result<(), String> {
        SocBus::write_flash(self, offset, data)
    }
    fn boot_app(&mut self, _app_off: usize) -> Result<u32, String> {
        Err("boot from ROM on the classic ESP32".into())
    }
    fn reboot(&mut self, mac: [u8; 6]) -> u32 {
        self.deep_sleep = false;
        self.reset_psram();
        self.ble.restore_flash(&mut self.flash);
        self.ble.reset();
        let cause = self.periph.rtc.0.reset_cause;
        let mut old = std::mem::replace(&mut self.periph, Peripherals::new(mac));
        for port in 0..2 { self.periph.i2s[port].inner.keep_rx_input(&mut old.i2s[port].inner); }
        self.periph.restore_rtc_analog(old.adc);
        self.periph.gpio.restore_inputs(&old.gpio);
        self.periph.spi0.0.jedec = old.spi0.0.jedec;
        self.periph.spi1.0.jedec = old.spi1.0.jedec;
        self.periph.efuse = old.efuse;
        self.periph.wifi.link = old.wifi.link.surviving_reboot();
        self.periph.wifi.log = old.wifi.log;
        self.periph.misc.log_unknown = old.misc.log_unknown;
        self.periph.gpio.gpio.strap = old.gpio.gpio.strap;
        self.periph.rtc.0.ram = old.rtc.0.ram;
        self.periph.rtc.0.slow_ticks = old.rtc.0.slow_ticks;
        self.periph.rtc.0.ram.write(0x38, cause | cause << 6);
        // ESP-IDF v5.5.4 esp32/rtc_cntl_reg.h:1615: WDTCONFIG0, not S3's +0x98.
        self.periph.rtc.0.ram.write(0x8c, 0);
        self.periph.rtc.0.reset_cause = cause;
        self.attach_board_devices();
        for area in self.debug.iter() {
            esp_periph::Dispatch::debug(&mut self.periph, area, true);
        }
        self.periph.misc.log_all = self.debug.has("mmio");
        cause
    }
    fn sw_reset(&self) -> bool {
        self.periph.rtc.0.sw_reset
    }
    fn request_reset(&mut self, cause: u32) {
        self.periph.rtc.0.sw_reset = true;
        self.periph.rtc.0.reset_cause = cause;
    }
    fn reset_cause(&self) -> u32 {
        self.periph.rtc.0.reset_cause
    }
    fn last_fault(&self) -> Option<(u32, bool)> {
        self.last_fault
    }
    fn console_take(&mut self) -> [Vec<u8>; 4] {
        [
            Vec::new(),
            std::mem::take(&mut self.periph.uart[0].tx_out),
            std::mem::take(&mut self.periph.uart[1].tx_out),
            std::mem::take(&mut self.periph.uart[2].tx_out),
        ]
    }
    fn serial_input(&mut self, data: &[u8]) {
        self.periph.uart[0].host_input(data);
        self.irq_dirty = true;
    }
    fn uart_input(&mut self, n: usize, data: &[u8]) {
        if let Some(u) = self.periph.uart.get_mut(n) {
            u.host_input(data);
            self.irq_dirty = true;
        }
    }
    fn gpio_release_input(&mut self, pin: u8) { self.periph.gpio.release_input(pin); self.ext0_wake(); self.irq_dirty = true; }
    fn gpio_state(&self, pin: u8) -> Option<esp_soc::GpioState> {
        // IDF v5.5.4 soc_caps.h:182 excludes 24 and 28–31. GPIO20 is only
        // bonded on PICO-V3-02, not the D0WD model; GPIO23 (VSPI MOSI) is valid.
        if pin >= 40 || pin == 20 || pin == 24 || (28..=31).contains(&pin) { return None; }
        let gpio = &self.periph.gpio;
        let mux = gpio.mux(pin as usize);
        Some(esp_soc::GpioState::new( gpio.gpio.out & (1 << pin) != 0,
            gpio.gpio.enable & (1 << pin) != 0,
            pin < 34 && mux & (1 << 8) != 0, pin < 34 && mux & (1 << 7) != 0))
    }
    fn gpio_set_input(&mut self, pin: u8, level: bool) {
        self.periph.gpio.set_input(pin, level);
        self.ext0_wake();
        self.irq_dirty = true;
        if let Some(ev) = &mut self.gpio_events {
            ev.push((self.cycles, pin, level));
        }
    }
    fn gpio_input(&self) -> u64 {
        self.periph.gpio.gpio.input
    }
    fn adc_set_raw(&mut self, pin: u8, raw: u16) -> bool { crate::adc::valid_pin(pin) && self.periph.adc.analog.set_raw(pin, raw) }
    fn adc_observation(&self, pin: u8) -> Option<esp_periph::AdcObservation> { crate::adc::valid_pin(pin).then(|| self.periph.adc.analog.observation(pin)) }
    fn analog_set(&mut self, pin: u8, src: esp_periph::AnalogSource) { self.periph.adc.analog.set(pin, src); }
    fn set_touch_input(&mut self, pin: u8, touched: bool) { self.periph.adc.set_touch_input(pin, touched); }
    fn has_touch_pad(&self, pin: u8) -> bool { crate::adc::touch_pin(pin) }
    fn pwm_output(&self, pin: u8) -> Option<(f64, u32)> {
        self.periph.pwm_output(pin as u32)
    }
    fn observe_gpio(&mut self, on: bool) {
        self.gpio_events = on.then(Vec::new);
    }
    fn take_gpio_events(&mut self) -> Vec<(u64, u8, bool)> {
        self.gpio_events
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    }
    fn board(&mut self) -> &mut dyn BoardModel {
        &mut *self.board
    }
    fn board_ref(&self) -> &dyn BoardModel {
        &*self.board
    }
    fn pcm_sources(&self) -> Option<&esp_periph::i2s::PcmSources> { self.periph.i2s[0].inner.pcm_bank_ref() }
    /// Classic ticks peripherals inline (no deferred ticks), so the bank syncs to the bus clock directly.
    fn attach_pcm_sources(&mut self) -> Option<&mut esp_periph::i2s::PcmSources> {
        Some(esp_soc::soc::attach_pcm_sources(self.periph.i2s[0].inner.pcm_bank(), self.cycles, periph::CPU_HZ))
    }
    fn i2s_input(&mut self, port: usize) -> Option<&mut esp_periph::i2s::PcmInput> {
        self.periph.i2s.get_mut(port).map(|i| i.inner.rx_input())
    }
    fn audio(&self) -> (&[i16], u32) {
        (&[], 44_100)
    }
    fn irq_sources_of(&self, core: usize, line: u32) -> Vec<usize> {
        (0..periph::NUM_SOURCES)
            .filter(|&source| self.periph.dport.map[core][source] == line)
            .collect()
    }
    fn set_debug(&mut self, f: &esp_soc::DebugFlags) {
        self.debug = f.clone();
        for area in f.iter() {
            esp_periph::Dispatch::debug(&mut self.periph, area, true);
        }
        self.periph.misc.log_all = f.has("mmio");
    }
    fn set_flash_size(&mut self, bytes: usize) {
        self.flash = vec![0xff; bytes];
        let cap = bytes.trailing_zeros() as u8;
        self.periph.spi0.0.jedec[2] = cap;
        self.periph.spi1.0.jedec[2] = cap;
    }
    fn set_strap(&mut self, v: u32) {
        self.periph.gpio.gpio.strap = v;
    }
    fn set_reset_cause(&mut self, cause: u32) {
        self.periph.rtc.0.ram.write(0x38, cause | cause << 6);
        self.periph.rtc.0.reset_cause = cause;
    }
    fn report(&self) -> String {
        let mut lines = Vec::new();
        for (pin, millivolts) in self.periph.adc.dac_outputs() { lines.push(format!("[dac] GPIO{pin}: {millivolts} mV")); }
        for (index, spi) in self.periph.spi.iter().enumerate() {
            if spi.transfers != 0 {
                lines.push(format!("[emu] spi{}: {} transfers", index + 2, spi.transfers));
            }
        }
        if self.periph.rmt.tx_count != 0 {
            lines.push(format!("[emu] rmt: {} transmissions", self.periph.rmt.tx_count));
        }
        let wifi = self.periph.wifi.link.report();
        if !wifi.is_empty() { lines.push(wifi); }
        let board = self.board.report();
        if !board.is_empty() { lines.push(board); }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn direct_app_boot_requires_rom() {
        let mut bus = super::SocBus::new(4 << 20, [0; 6]);
        assert_eq!(esp_soc::SocBus::boot_app(&mut bus, 0x10000),
            Err("boot from ROM on the classic ESP32".into()));
    }
}
