//! C61 bus: ESP-IDF v5.5.4 soc.h, ext_mem_defs.h and register/soc/{reg_base,spi_mem_c}_reg.h.
use crate::periph::{Peripherals, CPU_SUB_BASE, CPU_SUB_END, PERIPH_BASE, PERIPH_END};
use riscv_rv32::bus::{Bus, Fault};

pub const ROM_LOW: u32 = 0x4000_0000;
pub const ROM_HIGH: u32 = 0x4004_0000;
pub const SRAM_LOW: u32 = 0x4080_0000;
pub const SRAM_HIGH: u32 = 0x4085_0000;
pub const FLASH_LOW: u32 = 0x4200_0000;
pub const FLASH_HIGH: u32 = 0x4400_0000;
pub const MMU_ENTRIES: usize = 512;

pub const MMU_VALID: u32 = 1 << 10;

pub const SPI_MMU_ITEM_CONTENT: u32 = 0x37c;
pub const SPI_MMU_ITEM_INDEX: u32 = 0x380;
pub const SPI_MMU_POWER_CTRL: u32 = 0x384;

pub struct SocBus {
    pub rom: Vec<u8>,
    pub sram: Vec<u8>,
    pub flash: Vec<u8>,
    pub psram: Vec<u8>,
    pub mmu: [u32; MMU_ENTRIES],
    pub mmu_index: u32,

    pub mmu_power_ctrl: u32,
    pub periph: Peripherals,

    pub board: esp_soc::Board,
    uart_pins: bool,
    pub cycles: u64,
    pub last_fault: Option<(u32, bool)>,

    pub irq_dirty: bool,

    pub gpio_events: Option<Vec<(u64, u8, bool)>>,
    pub debug: esp_soc::DebugFlags,
    board_edges: bool,
    board_power: bool, pub controller_powered: bool,
}

impl SocBus {

    pub fn attach_board_devices(&mut self) {
        if self.board.uses_gpio_routes() {
            self.board.gpio_waveform_at(self.cycles, crate::periph::PINS.routes(&self.periph.gpio, &self.periph.io_mux));
        }
        self.uart_pins = self.board.uses_uart_pins();
        self.board_edges = self.board.uses_gpio_edges();
        self.board_power = self.board.power_state().is_some();
        self.controller_powered = self.board.power_state().is_none_or(|state| state.controller_powered);
        for (bus, address, device) in self.board.i2c_devices() {
            if bus == 0 { self.periph.i2c.attach(address, device); }
        }
        for (pin, level) in self.board.input_levels() {
            self.irq_dirty |= self.periph.gpio.set_input(pin, level);
        }
        for pin in self.board.released_inputs() { esp_soc::SocBus::gpio_release_input(self, pin); }
    }

    pub fn new(flash_size: usize, mac: [u8; 6]) -> Self {
        let mut periph=Peripherals::new(mac);let capacity=flash_size.trailing_zeros() as u8;
        periph.spi0.0.jedec[2]=capacity;periph.spi1.0.jedec[2]=capacity;
        SocBus {
            rom: vec![0; (ROM_HIGH - ROM_LOW) as usize],
            sram: vec![0; (SRAM_HIGH - SRAM_LOW) as usize],
            flash: vec![0xff; flash_size], psram: Vec::new(),
            mmu: [0; MMU_ENTRIES], mmu_index: 0, mmu_power_ctrl: 0,
            periph, board: Box::new(esp_soc::NoBoard), uart_pins: false,
            cycles: 0, last_fault: None, irq_dirty: true, gpio_events: None, debug: Default::default(), board_edges: false, board_power: false, controller_powered: true,
        }
    }

    #[inline]
    pub fn page_shift(&self) -> u32 { 16 - ((self.mmu_power_ctrl >> 3) & 3) }

    fn resolve(&mut self, addr: u32) -> Option<(&mut Vec<u8>, usize, bool)> {
        match addr {
            SRAM_LOW..=0x4084_FFFF => Some((&mut self.sram, (addr - SRAM_LOW) as usize, true)),
            ROM_LOW..=0x4003_FFFF => Some((&mut self.rom, (addr - ROM_LOW) as usize, false)),
            FLASH_LOW..=0x43FF_FFFF => {
                let shift = self.page_shift();
                let idx = ((addr - FLASH_LOW) >> shift) as usize;
                if idx >= MMU_ENTRIES { return None; }
                let entry = self.mmu[idx];
                if entry & MMU_VALID == 0 { return None; }
                let off = (((entry & 0x1ff) as usize) << shift) + (addr & ((1 << shift) - 1)) as usize;
                let buffer = if entry & (1 << 9) != 0 { &mut self.psram } else { &mut self.flash };
                if off < buffer.len() { Some((buffer, off, entry & (1 << 9) != 0)) } else { None }
            }
            _ => None,
        }
    }

    #[inline]
    fn is_periph(addr: u32) -> bool { (PERIPH_BASE..PERIPH_END).contains(&addr) || (CPU_SUB_BASE..CPU_SUB_END).contains(&addr) }

    fn periph_read(&mut self, addr: u32, size: u32) -> u32 {

        if self.board_edges && matches!(addr & !3, 0x6009_1064) { self.irq_dirty |= esp_soc::gpio::deliver_board_inputs(&mut *self.board, &mut self.periph.gpio, &mut self.gpio_events, self.cycles); }
        let w = if (CPU_SUB_BASE..CPU_SUB_END).contains(&addr) {
            self.periph.cpu_sub_read(addr - CPU_SUB_BASE)
        } else if (addr & !0xfff) == PERIPH_BASE + 0x2000 && matches!(addr & 0xfff, SPI_MMU_ITEM_CONTENT | SPI_MMU_ITEM_INDEX | SPI_MMU_POWER_CTRL) {
            match addr & 0xfff {
                SPI_MMU_ITEM_CONTENT => self.mmu[(self.mmu_index as usize) & (MMU_ENTRIES - 1)],
                SPI_MMU_ITEM_INDEX => self.mmu_index,
                _ => self.mmu_power_ctrl,
            }
        } else if matches!(addr & !3,0x6008_00d8|0x6008_0198) {
            self.spi2_out_ready(((addr-0x6008_00d8)/0xc0) as usize)
        } else {

            self.irq_dirty |= addr & !3 == 0x6000_f000;
            self.periph.read32(addr & !3)
        };
        match size { 1 => (w >> ((addr & 3) * 8)) & 0xff, 2 => (w >> ((addr & 2) * 8)) & 0xffff, _ => w }
    }

    fn periph_write(&mut self, addr: u32, v: u32, size: u32) {
        let a = addr & !3;
        let merge = |old: u32| match size {
            4 => v,
            1 => { let sh = (addr & 3) * 8; (old & !(0xff << sh)) | ((v & 0xff) << sh) }
            _ => { let sh = (addr & 2) * 8; (old & !(0xffff << sh)) | ((v & 0xffff) << sh) }
        };
        if (CPU_SUB_BASE..CPU_SUB_END).contains(&a) {
            let old = self.periph.cpu_sub_read(a - CPU_SUB_BASE);
            self.periph.cpu_sub_write(a - CPU_SUB_BASE, merge(old), match size { 1 => 1 << (addr & 3), 2 => 3 << (addr & 2), _ => 15 });
            self.irq_dirty = true;
            return;
        }
        if (a & !0xfff) == PERIPH_BASE + 0x2000 {
            match a & 0xfff {
                SPI_MMU_ITEM_CONTENT => { self.mmu[(self.mmu_index as usize) & (MMU_ENTRIES - 1)] = merge(self.mmu[(self.mmu_index as usize) & (MMU_ENTRIES - 1)]) & 0xfff; return; }
                SPI_MMU_ITEM_INDEX => { self.mmu_index = merge(self.mmu_index) & 0x1ff; return; }
                SPI_MMU_POWER_CTRL => { self.mmu_power_ctrl = merge(self.mmu_power_ctrl); return; }
                _ => {}
            }
        }
        let v = if size == 4 { v } else { merge(self.periph.read32(a)) };
        let old_drive = (self.periph.gpio.enable, self.periph.gpio.out);
        self.periph.write32(a, v);
        if matches!(a & !0xfff, 0x6009_0000 | 0x6009_1000) && self.board.uses_gpio_routes() {
            self.board.gpio_waveform_at(self.cycles, crate::periph::PINS.routes(&self.periph.gpio, &self.periph.io_mux));
        }
        if self.periph.sha.dma_pending { self.sha_dma_write(a); }

        if old_drive != (self.periph.gpio.enable, self.periph.gpio.out) {
            self.deliver_gpio_output();
        }

        if self.uart_pins {if let Some(port)=match a {0x60000000=>Some(0),0x60001000=>Some(1),0x60006000=>Some(2),_=>None} {self.board.uart_tx(self.cycles,self.periph.uart_route(port),v as u8);}}
        if self.periph.spi_exec { self.run_spi(); }

        if self.periph.spi2.dma_tx_pending.is_some() { self.spi2_dma_tx(); }
        if self.periph.spi2.has_pending_transfer() || !self.periph.gpio.changes.is_empty() { self.deliver_board_events(); }
        self.irq_dirty = true;
    }

    #[cold]
    #[inline(never)]
    fn sha_dma_write(&mut self, a: u32) {
        if !matches!(a & !0xfff, 0x6008_9000 | 0x6008_0000) { return; }
        if let Some(ch) = self.periph.gdma.gdma.out_channel_for(7) {
            let shift=self.page_shift();
            let mut memory = C61DmaOut { sram: &mut self.sram, psram: &mut self.psram, mmu: &self.mmu, shift, channel: &mut self.periph.gdma.gdma.out[ch] };
            self.periph.sha.dma_step(&mut memory);
        }
    }

    #[cold]
    fn spi2_out_ready(&self,ch:usize)->u32 {
        let c=&self.periph.gdma.gdma.out[ch];
        if c.peri_sel!=1 {return self.periph.gdma.gdma.read(ch as u32*0xc0+0x78);}
        let mut available=0;
        if c.running && c.desc&3==0 {
            let mut walk=esp_periph::gdma::DescriptorWalk::new(1);
            if let Ok((_,d))=walk.read_with(c.desc,|a|self.spi_dma_word(a)) {
                if d.length<=d.size && c.buf_pos<d.length && (c.conf1&(1<<12)==0 || d.owner_dma) {
                    let start=d.buf.checked_add(c.buf_pos);let count=(d.length-c.buf_pos).min(32);
                    if start.is_some_and(|a|self.spi_dma_range(a,count as usize).is_some()) {available=count;}
                }
            }
        }
        (15<<23)|(available<<2)|if available==0 {2}else{0}
    }
    fn spi_dma_range(&self,address:u32,count:usize)->Option<std::ops::Range<usize>> {let start=address.checked_sub(SRAM_LOW)? as usize;let end=start.checked_add(count)?;(end<=self.sram.len()).then_some(start..end)}
    fn spi_dma_word(&self,address:u32)->Result<u32,Fault> {if address&3!=0 {return Err(Fault::Misaligned);}let r=self.spi_dma_range(address,4).ok_or(Fault::Unmapped)?;Ok(u32::from_le_bytes(self.sram[r].try_into().unwrap()))}
    fn spi2_dma_tx(&mut self) {
        let Some(bits)=self.periph.spi2.dma_tx_pending else {return};
        let want=(bits as usize).div_ceil(8);
        let Some(ch)=self.periph.gdma.gdma.out_channel_for(1) else {return};
        let mut c=self.periph.gdma.gdma.out[ch];let mut data=Vec::with_capacity(want);let mut visited=std::collections::HashSet::new();
        let mut walk=esp_periph::gdma::DescriptorWalk::new(esp_periph::gdma::GDMA_DESCRIPTOR_STEP_BUDGET);
        let mut failed=false;
        while c.running && c.desc!=0 && data.len()<want {
            if !visited.insert(c.desc) {failed=true;break;}
            let Ok((control,d))=walk.read_with(c.desc,|a|self.spi_dma_word(a)) else {failed=true;break;};
            if d.length>d.size || c.buf_pos>d.length || (c.conf1&(1<<12)!=0 && !d.owner_dma) {failed=true;break;}
            let remaining=(d.length-c.buf_pos) as usize;let count=remaining.min(want-data.len());
            let Some(range)=d.buf.checked_add(c.buf_pos).and_then(|a|self.spi_dma_range(a,count)) else {failed=true;break;};
            data.extend_from_slice(&self.sram[range]);c.buf_pos+=count as u32;
            if count<remaining {break;}
            if c.conf0&(1<<2)!=0 {let Some(r)=self.spi_dma_range(c.desc,4) else {failed=true;break;};self.sram[r].copy_from_slice(&(control&!(1<<31)).to_le_bytes());}
            c.int_raw|=1;if d.eof {c.int_raw|=2;c.eof_desc=c.desc;}
            c.desc=d.next;c.buf_pos=0;
            if d.eof || d.next==0 {c.running=false;c.int_raw|=8;break;}
        }
        if failed {c.running=false;c.int_raw|=4;}
        self.periph.gdma.gdma.out[ch]=c;self.irq_dirty=true;

        self.periph.spi2.complete_dma_tx(&data);
    }

    fn deliver_gpio_output(&mut self) {
        let ch = &self.periph.gpio.changes;
        if let Some(ev) = &mut self.gpio_events { for &(pin, level) in ch { ev.push((self.cycles, pin, level)); } }
        self.board.gpio_output_at(self.cycles, ch, self.periph.gpio.enable, self.periph.gpio.out);
        self.periph.gpio.changes.clear();
    }

    fn deliver_board_events(&mut self) {
        if !self.periph.gpio.changes.is_empty() { self.deliver_gpio_output(); }
        if let Some(transfer) = self.periph.spi2.take_transfer() {
            let rx = if self.board.uses_spi_pins() {

                let pins = crate::periph::PINS.routes(&self.periph.gpio, &self.periph.io_mux)
                    .spi_pins(&self.periph.spi2, [64, 65, 66, 69, 102, 103, 104, 105, 106], 256, &[(2, 6, 7, 2, &[8])]);
                if self.periph.spi2.log {eprintln!("[C61 SPI ROUTE] {:?} tx {} {:?} rx {}",pins,transfer.tx.len(),&transfer.tx[..transfer.tx.len().min(12)],transfer.rx_len);}
                self.board.spi_transfer_pins(2, pins, &transfer.tx, transfer.rx_len)
            } else {
                self.board.spi_transfer(2, &transfer.tx, transfer.rx_len)
            };
            // C61 AHB_GDMA peripheral selector1 is SPI2. RX DMA writes the
            // descriptor chain before the SPI completion interrupt is observed.
            if transfer.rx_len!=0 && self.periph.spi2.regs.read(0x30)&(1<<27)!=0 {
                if let Some(n)=(0..2).find(|&n|{let r=&self.periph.gdma.gdma.inp[n];r.running&&r.peri_sel==1}) {
                    let mut r=self.periph.gdma.gdma.inp[n];let mut changed=false;
                    r.receive(self,&rx,Some(transfer.rx_len as u32),true,Self::is_periph,&mut changed);
                    self.periph.gdma.gdma.inp[n]=r;self.irq_dirty|=changed;
                }
            }
            self.periph.spi2.finish_transfer(transfer, &rx);
        }

    }

    #[cold]
    #[inline(never)]
    fn run_spi(&mut self) {
        self.periph.spi_exec = false;
        self.periph.spi1.0.execute(&mut self.flash, &mut self.psram);
        self.periph.spi1.0.dirty.clear();

    }

    pub fn write_flash(&mut self, offset: usize, data: &[u8]) -> Result<(), String> {
        let target = self.flash.get_mut(offset..).and_then(|tail| tail.get_mut(..data.len()))
            .ok_or("flash image too large")?;
        target.copy_from_slice(data);
        Ok(())
    }


    pub fn load_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), String> {

        if addr < 0x10000 || (FLASH_LOW..FLASH_HIGH).contains(&addr) { return Ok(()); }
        for (i, b) in data.iter().enumerate() {
            let a = addr.wrapping_add(i as u32);
            match self.resolve(a) {
                Some((buf, off, _)) if off < buf.len() => buf[off] = *b,
                _ => return Err(format!("load: address {:#010x} not mapped", a)),
            }
        }
        Ok(())
    }

    fn devices(&mut self, cycles: u32) {

        if self.board_edges { self.board.advance_to(self.cycles); }

        self.irq_dirty |= esp_periph::Dispatch::tick(&mut self.periph, cycles as u64);
        self.periph.gpio.input_changes.clear();
        if self.board_edges { self.irq_dirty |= esp_soc::gpio::drain_board_inputs(&mut *self.board, &mut self.periph.gpio, &mut self.gpio_events, self.cycles); }

        if self.periph.spi2.dma_tx_pending.is_some() { self.spi2_dma_tx(); }

        if self.uart_pins {for input in self.board.uart_rx(self.cycles) {self.periph.uart_pin_input(&input);self.irq_dirty=true;}}
        if self.periph.wifi_mac.has_pending_work() { self.irq_dirty |= self.periph.wifi_mac.service(&mut self.sram, self.cycles / (crate::periph::CPU_HZ / 1_000_000)); }
        if self.board_power {
            self.controller_powered = self.board.power_state().is_none_or(|state| state.controller_powered);
            if let Some(esp_soc::board::BoardPowerEvent::Reset { download }) = self.board.take_power_event() {
                // GPIO_STRAP boot bit3 (GPIO9); C61 ROM get_boot_mode tests bits2/3.
                self.periph.gpio.strap = if download { 7 } else { 15 };
                self.periph.lpsys.reset_cause = esp_periph::RST_POWERON; self.periph.lpsys.sw_reset = true;
                self.irq_dirty = true;
            }
        }
        self.deliver_board_events();
    }
}

macro_rules! rd {
    ($self:ident, $addr:expr, $n:expr, $conv:expr) => {{
        let addr = $addr;
        match $self.resolve(addr) {
            Some((b, o, _)) if b.len().saturating_sub(o) >= $n => Ok($conv(&b[o..o + $n])),
            _ => { $self.last_fault = Some((addr, false)); Err(Fault::Unmapped) }
        }
    }};
}

impl Bus for SocBus {
    fn note_code_page(&mut self, _vidx: u32) {}
    fn read8(&mut self, addr: u32) -> Result<u8, Fault> {
        if Self::is_periph(addr) { return Ok(self.periph_read(addr, 1) as u8); }
        rd!(self, addr, 1, |b: &[u8]| b[0])
    }
    fn read16(&mut self, addr: u32) -> Result<u16, Fault> {
        if Self::is_periph(addr) { return Ok(self.periph_read(addr, 2) as u16); }
        rd!(self, addr, 2, |b: &[u8]| u16::from_le_bytes(b.try_into().unwrap()))
    }
    fn read32(&mut self, addr: u32) -> Result<u32, Fault> {
        if Self::is_periph(addr) { return Ok(self.periph_read(addr, 4)); }
        rd!(self, addr, 4, |b: &[u8]| u32::from_le_bytes(b.try_into().unwrap()))
    }
    fn write8(&mut self, addr: u32, v: u8) -> Result<(), Fault> {
        if Self::is_periph(addr) { self.periph_write(addr, v as u32, 1); return Ok(()); }
        match self.resolve(addr) {
            Some((b, o, true)) if o < b.len() => { b[o] = v; Ok(()) }
            _ => { self.last_fault = Some((addr, true)); Err(Fault::Prohibited) }
        }
    }
    fn write16(&mut self, addr: u32, v: u16) -> Result<(), Fault> {
        if Self::is_periph(addr) { self.periph_write(addr, v as u32, 2); return Ok(()); }
        match self.resolve(addr) {
            Some((b, o, true)) if o + 2 <= b.len() => { b[o..o + 2].copy_from_slice(&v.to_le_bytes()); Ok(()) }
            _ => { self.last_fault = Some((addr, true)); Err(Fault::Prohibited) }
        }
    }
    fn write32(&mut self, addr: u32, v: u32) -> Result<(), Fault> {
        if Self::is_periph(addr) { self.periph_write(addr, v, 4); return Ok(()); }
        match self.resolve(addr) {
            Some((b, o, true)) if o + 4 <= b.len() => { b[o..o + 4].copy_from_slice(&v.to_le_bytes()); Ok(()) }
            _ => { self.last_fault = Some((addr, true)); Err(Fault::Prohibited) }
        }
    }
    fn fetch(&mut self, pc: u32) -> Result<[u8; 4], Fault> {
        match self.resolve(pc) {
            Some((b, o, _)) if o < b.len() => {
                let mut r = [0u8; 4];
                for i in 0..4 { if o + i < b.len() { r[i] = b[o + i]; } }
                Ok(r)
            }
            _ => { self.last_fault = Some((pc, false)); Err(Fault::Unmapped) }
        }
    }
    fn tick(&mut self, cycles: u32) -> u32 {
        self.cycles += cycles as u64;
        self.devices(cycles);
        u32::from(self.irq_dirty)
    }
    #[inline(always)]
    fn note_pc(&mut self, pc: u32) { self.periph.misc.cur_pc = pc; }

    #[inline(always)]
    fn block_break(&self) -> bool { self.irq_dirty }
}
// ESP-IDF v5.5.4 esp32c61/soc_caps.h: SOC_AHB_GDMA_SUPPORT_PSRAM and
// SOC_PSRAM_DMA_CAPABLE. DMA sees the MMU's PSRAM mapping, including page
// boundaries; flash and MMIO are not DMA RAM. No CPU bus timing is charged.
struct C61DmaOut<'a> {
    sram: &'a mut [u8], psram: &'a mut [u8], mmu: &'a [u32; MMU_ENTRIES], shift: u32,
    channel: &'a mut esp_periph::gdma::GdmaOutCh,
}
impl C61DmaOut<'_> {
    fn chunk(&mut self,address:u32)->Result<&mut [u8],Fault> {
        if (SRAM_LOW..SRAM_HIGH).contains(&address) {return self.sram.get_mut((address-SRAM_LOW) as usize..).ok_or(Fault::Unmapped);}
        if !(FLASH_LOW..FLASH_HIGH).contains(&address) {return Err(Fault::Unmapped);}
        let page=(address-FLASH_LOW)>>self.shift;let entry=*self.mmu.get(page as usize).ok_or(Fault::Unmapped)?;
        if entry&(MMU_VALID|1<<9)!=(MMU_VALID|1<<9) {return Err(Fault::Unmapped);}
        let page_size=1usize<<self.shift;let within=address as usize&(page_size-1);
        let start=((entry&0x1ff) as usize)*page_size+within;
        let end=(start+page_size-within).min(self.psram.len());
        self.psram.get_mut(start..end).filter(|s|!s.is_empty()).ok_or(Fault::Unmapped)
    }
}
impl esp_periph::gdma::OutMemory for C61DmaOut<'_> {
    fn out_channel(&mut self)->&mut esp_periph::gdma::GdmaOutCh {self.channel}
    fn word(&mut self,address:u32)->Result<u32,Fault> {if address&3!=0 {return Err(Fault::Misaligned);}let bytes=self.chunk(address)?.get(..4).ok_or(Fault::Unmapped)?;Ok(u32::from_le_bytes(bytes.try_into().unwrap()))}
    fn append(&mut self,mut address:u32,mut count:usize,out:&mut Vec<u8>)->Result<(),(u32,Fault)> {
        while count!=0 {let bytes=self.chunk(address).map_err(|fault|(address,fault))?;let take=count.min(bytes.len());out.extend_from_slice(&bytes[..take]);count-=take;if count!=0 {address=address.checked_add(take as u32).ok_or((address,Fault::Unmapped))?;}}
        Ok(())
    }
    fn writeback(&mut self,address:u32,value:u32)->Result<(),Fault> {if address&3!=0 {return Err(Fault::Misaligned);}self.chunk(address)?.get_mut(..4).ok_or(Fault::Unmapped)?.copy_from_slice(&value.to_le_bytes());Ok(())}
}
