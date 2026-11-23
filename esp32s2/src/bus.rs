//! ESP32-S2 address map from pinned ESP-IDF v5.5.4 soc/register headers.
//! SRAM has two linear aliases of the same bytes. Unknown peripherals fault;
//! no register-RAM fallback can make S3 addresses or SDK initialization succeed.
use emu_core::{Bus, Fault};
use esp_periph::{Gpio, RegRam, Uart, UartLayout};
use esp_soc::{Board, NoBoard, pins::ChipPins};
use std::ops::Range;

pub const ROM_BASE: u32 = 0x4000_0000; // No ROM memory is supplied.
pub const RESET_VECTOR: u32 = 0x4000_0400; // S2 core-isa.h, still unmapped.
pub const IRAM_BASE: u32 = 0x4002_0000;
pub const DRAM_BASE: u32 = 0x3ffb_0000;
pub const SRAM_SIZE: usize = 0x50000;
pub const MAX_STAGING_FLASH: usize = 32 << 20;
pub const CPU_HZ: u64 = 240_000_000; // Fixed nominal diagnostic clock, not calibrated timing.
pub const UART0: u32 = 0x3f40_0000;
pub const UART1: u32 = 0x3f41_0000;
pub const GPIO_BASE: u32 = 0x3f40_4000;
pub const IO_MUX_BASE: u32 = 0x3f40_9000;
pub const VALID_GPIO: u64 = ((1u64 << 47) - 1) & !(15 << 22);
pub const OUTPUT_GPIO: u64 = VALID_GPIO & !(1 << 46); // GPIO46 is input-only.
pub const PINS: ChipPins = ChipPins { valid: OUTPUT_GPIO, input_select: 0x80, output_mask: 0x3ff };

/// Normalize either linear alias to the single physical 320 KiB SRAM array.
pub fn physical_range(address: u32, size: usize) -> Option<Range<usize>> {
    for base in [IRAM_BASE, DRAM_BASE] {
        if let Some(start) = address.checked_sub(base).map(|v| v as usize) {
            if let Some(end) = start.checked_add(size).filter(|&end| end <= SRAM_SIZE) { return Some(start..end); }
        }
    }
    None
}
pub fn valid_pin(pin: u8) -> bool { pin < 47 && VALID_GPIO & (1u64 << pin) != 0 }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessFault { pub pc: u32, pub address: u32, pub write: bool, pub width: u8 }

pub struct S2Bus {
    pub sram: Vec<u8>, version: u32,
    pub uart: [Uart; 2], pub gpio: Gpio, pub io_mux: RegRam, pub board: Board,
    pub flash: Vec<u8>, // Host image staging only, never mapped into guest address space.
    pub first_fault: Option<AccessFault>, pc: u32, pub cycles: u64,
    pub(crate) app_off: Option<usize>, pub(crate) app_entry: Option<u32>,
    pub(crate) reset_requested: bool, pub(crate) reset_cause: u32, pub(crate) irq_dirty: bool,
    pub(crate) misc: esp_periph::Misc, pub(crate) gpio_events: Option<Vec<(u64,u8,bool)>>,
    pub debug: esp_soc::DebugFlags, gpio_date: u32,
}
impl Default for S2Bus { fn default() -> Self { Self::new() } }
impl S2Bus {
    pub fn new() -> Self {
        // S2 uart_reg.h: full threshold 9 bits, FIFO count 10, CONF0 reset bit17.
        // Autobaud is a separate register on S2 and is deliberately not exposed.
        let layout = UartLayout { thrhd_mask: 0x1ff, rxfifo_rst: 1 << 17, rxfifo_cnt_mask: 0x3ff, autobaud_en: 0, autobaud_regs: 0, autobaud_clock_hz: 0 };
        let mut uart=[Uart::new(layout),Uart::new(layout)];
        for port in &mut uart {
            port.write(0x14,0x2b6);
            port.write(0x20,(1<<28)|(1<<27)|0x1c);
            port.write(0x24,0x60|(0x60<<9));
        }
        let mut gpio = Gpio::with_pins(47); gpio.strap = 0; gpio.input &= VALID_GPIO;
        let mut io_mux = RegRam::new();
        gpio.set_pad(46, 1 << 7); io_mux.write(4 + 46 * 4, 1 << 7);
        Self { sram: vec![0; SRAM_SIZE], version: 1, uart, gpio, io_mux,
            board: Box::new(NoBoard), flash: Vec::new(), first_fault: None, pc: ROM_BASE, cycles: 0,
            app_off: None, app_entry: None, reset_requested: false, reset_cause: 1, irq_dirty: true,
            misc: Default::default(), gpio_events: None, debug: Default::default(), gpio_date: 0x1905061 }
    }
    pub fn attach_board(&mut self, board: Board) { self.board = board; self.attach_board_devices(); }
    pub fn attach_board_devices(&mut self) {
        for (pin, high) in self.board.input_levels() { if valid_pin(pin) { self.gpio.set_input(pin, high); } }
        self.publish_gpio();
    }
    pub(crate) fn publish_gpio(&mut self) {
        self.board.gpio_waveform_at(self.cycles, PINS.routes(&self.gpio, &self.io_mux));
        if let Some(events) = &mut self.gpio_events { events.extend(self.gpio.changes.drain(..).map(|(pin,high)|(self.cycles,pin,high))); }
        else { self.gpio.changes.clear(); }
        self.gpio.input_changes.clear();
    }
    pub fn load_ram(&mut self, address: u32, bytes: &[u8], size: usize) -> Result<(), String> {
        if bytes.len() > size { return Err("S2 file bytes exceed requested SRAM span".into()); }
        let range = physical_range(address, size).ok_or_else(||format!("S2 SRAM range {address:#010x}+{size:#x} is unavailable"))?;
        self.sram[range.clone()].fill(0);
        self.sram[range.start..range.start+bytes.len()].copy_from_slice(bytes);
        self.version = self.version.wrapping_add(1); Ok(())
    }
    fn fail<T>(&mut self, address: u32, width: u8, write: bool, fault: Fault) -> Result<T, Fault> {
        self.first_fault.get_or_insert(AccessFault { pc: self.pc, address, write, width }); Err(fault)
    }
    fn uart_register(address: u32) -> Option<(usize,u32)> {
        // APB registers and the documented AHB FIFO-only aliases. The latter
        // genuinely overlap S3 UART0's base; no other S3 MMIO is thereby accepted.
        if address == 0x6000_0000 { return Some((0,0)); }
        if address == 0x6001_0000 { return Some((1,0)); }
        for (i,base) in [UART0,UART1].into_iter().enumerate() {
            if let Some(off) = address.checked_sub(base) {
                if matches!(off, 0 | 4 | 8 | 12 | 16 | 20 | 28 | 32 | 36) { return Some((i,off)); }
            }
        }
        None
    }
    fn gpio_register(address: u32) -> Option<u32> {
        let off = address.checked_sub(GPIO_BASE)?;
        if off & 3 != 0 { return None; }
        match off {
            4..=0x18 | 0x20..=0x58 | 0x5c | 0x68 | 0x154..=0x550 | 0x6fc => Some(off),
            0x74..=0x12c if valid_pin(((off-0x74)/4) as u8) => Some(off),
            0x554..=0x60c if valid_pin(((off-0x554)/4) as u8) => Some(off),
            _ => None,
        }
    }
    fn mux_pin(address: u32) -> Option<u8> {
        let off = address.checked_sub(IO_MUX_BASE + 4)?;
        if off & 3 != 0 || off > 46 * 4 { return None; }
        let pin = (off/4) as u8; valid_pin(pin).then_some(pin)
    }
    fn input_enabled(&self) -> u64 {
        (0..47u8).filter(|&pin|valid_pin(pin) && self.io_mux.read(4+4*u32::from(pin)) & (1<<9) != 0)
            .fold(0,|mask,pin|mask | (1u64<<pin))
    }
    fn read(&mut self, address: u32, width: u8) -> Result<u32, Fault> {
        if address & (u32::from(width)-1) != 0 { return self.fail(address,width,false,Fault::Misaligned); }
        if let Some(range) = physical_range(address,usize::from(width)) {
            let mut b = [0;4]; b[..usize::from(width)].copy_from_slice(&self.sram[range]); return Ok(u32::from_le_bytes(b));
        }
        if let Some((i,off)) = Self::uart_register(address) {
            if width == 4 || (width == 1 && off == 0) { return Ok(self.uart[i].read(off)); }
        }
        if width == 4 {
            if let Some(off) = Self::gpio_register(address) {
                return Ok(match off {
                    0x3c => (self.gpio.input & self.input_enabled()) as u32,
                    0x40 => ((self.gpio.input & self.input_enabled()) >> 32) as u32,
                    0x6fc => self.gpio_date,
                    _ => self.gpio.read(off),
                });
            }
            if Self::mux_pin(address).is_some() { return Ok(self.io_mux.read(address-IO_MUX_BASE)); }
        }
        self.fail(address,width,false,Fault::Unmapped)
    }
    fn write(&mut self, address: u32, value: u32, width: u8) -> Result<(), Fault> {
        if address & (u32::from(width)-1) != 0 { return self.fail(address,width,true,Fault::Misaligned); }
        if let Some(range) = physical_range(address,usize::from(width)) {
            self.sram[range].copy_from_slice(&value.to_le_bytes()[..usize::from(width)]);
            self.version = self.version.wrapping_add(1); return Ok(());
        }
        if let Some((i,off)) = Self::uart_register(address) {
            if width == 4 || (width == 1 && off == 0) { self.uart[i].write(off,value); self.irq_dirty=true; return Ok(()); }
        }
        if width == 4 {
            if let Some(off) = Self::gpio_register(address) {
                // Read-only strap, input and CPU interrupt-status registers.
                if matches!(off,0x38 | 0x3c | 0x40 | 0x5c | 0x68) { return Ok(()); }
                if off == 0x6fc { self.gpio_date=value & 0x0fff_ffff; return Ok(()); }
                let value = match off {
                    4 | 8 | 12 => value & VALID_GPIO as u32,
                    0x10 | 0x14 | 0x18 => value & (VALID_GPIO>>32) as u32,
                    0x20 | 0x24 | 0x28 => value & OUTPUT_GPIO as u32,
                    0x2c | 0x30 | 0x34 => value & (OUTPUT_GPIO>>32) as u32,
                    0x554..=0x60c => value & 0xfff,
                    _ => value,
                };
                self.gpio.write(off,value); self.irq_dirty=true; self.publish_gpio(); return Ok(());
            }
            if let Some(pin) = Self::mux_pin(address) {
                self.io_mux.write(address-IO_MUX_BASE,value);
                self.gpio.set_pad(pin,value); self.irq_dirty=true; self.publish_gpio(); return Ok(());
            }
        }
        self.fail(address,width,true,Fault::Unmapped)
    }
}
impl Bus for S2Bus {
    fn note_pc(&mut self, pc: u32) { self.pc=pc; self.misc.cur_pc=pc; }
    fn note_code_page(&mut self, _page: u32) {} // Every write through either alias moves the same version.
    fn page_versions(&self) -> &[u32] { std::slice::from_ref(&self.version) }
    fn code_page(&mut self, _pc: u32) -> u32 { 0 }
    fn block_break(&self) -> bool { self.irq_dirty }
    fn tick(&mut self, cycles: u32) -> u32 { self.cycles=self.cycles.wrapping_add(u64::from(cycles)); u32::from(self.irq_dirty) }
    fn read8(&mut self, a:u32)->Result<u8,Fault>{self.read(a,1).map(|v|v as u8)}
    fn read16(&mut self, a:u32)->Result<u16,Fault>{self.read(a,2).map(|v|v as u16)}
    fn read32(&mut self, a:u32)->Result<u32,Fault>{self.read(a,4)}
    fn write8(&mut self,a:u32,v:u8)->Result<(),Fault>{self.write(a,u32::from(v),1)}
    fn write16(&mut self,a:u32,v:u16)->Result<(),Fault>{self.write(a,u32::from(v),2)}
    fn write32(&mut self,a:u32,v:u32)->Result<(),Fault>{self.write(a,v,4)}
    fn fetch(&mut self, pc:u32)->Result<[u8;4],Fault>{
        let Some(off)=pc.checked_sub(IRAM_BASE).map(|v|v as usize).filter(|&v|v<SRAM_SIZE) else { return self.fail(pc,3,false,Fault::Prohibited); };
        let available=(SRAM_SIZE-off).min(4); let mut bytes=[0;4];
        bytes[..available].copy_from_slice(&self.sram[off..off+available]);
        let length=xtensa_lx7::decode::decode(pc,bytes).len;
        if usize::from(length)>available {return self.fail(pc,length,false,Fault::Prohibited);}
        Ok(bytes)
    }
}
