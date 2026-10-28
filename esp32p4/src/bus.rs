//! ESP32-P4 memory map (IDF v5.5.3 components/soc/esp32p4/include/soc/soc.h and ext_mem_defs.h).
//!
//! L2MEM (768 KB at 0x4FF00000) holds code and data; the 128 KB mask ROM sits at 0x4FC00000, the
//! 8 KB TCM at 0x30100000, the LP ROM and LP RAM at 0x50100000 / 0x50108000. Flash
//! (0x40000000) and PSRAM (0x48000000) are 64 MB windows, each behind its own 1024-entry MMU of
//! 64 KB pages, programmed through the item index / content registers of the flash controller
//! (SPI_MEM_C) and of the PSRAM controller (SPI_MEM_S). Each of these has a non-cacheable alias
//! at +0x4000_0000 (SOC_NON_CACHEABLE_OFFSET); caches are not modelled, so both views are one.

use crate::periph::Peripherals;
use riscv_rv32::bus::{Bus, Fault};

pub const ROM_LOW: u32 = 0x4FC0_0000;
pub const ROM_HIGH: u32 = 0x4FC2_0000;
pub const L2MEM_LOW: u32 = 0x4FF0_0000;
pub const L2MEM_HIGH: u32 = 0x4FFC_0000;
pub const TCM_LOW: u32 = 0x3010_0000;
pub const TCM_HIGH: u32 = 0x3010_2000;
pub const LP_ROM_LOW: u32 = 0x5010_0000;
pub const LP_ROM_HIGH: u32 = 0x5010_4000;
pub const LP_RAM_LOW: u32 = 0x5010_8000;
pub const LP_RAM_HIGH: u32 = 0x5011_0000;
pub const FLASH_LOW: u32 = 0x4000_0000;
pub const FLASH_HIGH: u32 = 0x4400_0000;
pub const PSRAM_LOW: u32 = 0x4800_0000;
pub const PSRAM_HIGH: u32 = 0x4C00_0000;
pub const UNCACHED_OFFSET: u32 = 0x4000_0000;
pub const MMU_ENTRIES: usize = 1024;
pub const MMU_PAGE_SHIFT: u32 = 16;
/// Flash MMU entry: page in bits 10:0, VALID bit 12 (SENSITIVE bit 13 marks encrypted pages).
pub const MMU_FLASH_VALID: u32 = 1 << 12;
pub const MMU_FLASH_PAGE_MASK: u32 = 0x7ff;
/// PSRAM MMU entry: page in bits 9:0, ACCESS_PSRAM bit 10, VALID bit 11.
pub const MMU_PSRAM_VALID: u32 = 1 << 11;
pub const MMU_PSRAM_PAGE_MASK: u32 = 0x3ff;
/// The controllers the MMUs are reached through, and their item registers
/// (register/hw_ver1/soc/spi_mem_c_reg.h:2627-2654, spi_mem_s_reg.h).
pub const FLASH_SPI0: u32 = 0x5008_C000;
pub const PSRAM_MSPI0: u32 = 0x5008_E000;
pub const SPI_MMU_ITEM_CONTENT: u32 = 0x37c;
pub const SPI_MMU_ITEM_INDEX: u32 = 0x380;
/// Both cores' CLIC blocks (include/soc/clic_reg.h).
pub const CLIC_LOW: u32 = 0x2080_0000;
pub const CLIC_HIGH: u32 = 0x2082_0000;

/// Which MMU an item register belongs to.
#[derive(Clone, Copy)]
enum Mmu { Flash, Psram }

/// What the SDMMC host's IDMAC and the USB-OTG core's DMA reach: L2MEM and PSRAM
/// (SOC_SDMMC_PSRAM_DMA_CAPABLE, SOC_PSRAM_DMA_CAPABLE, IDF v5.5.3 include/soc/soc_caps.h;
/// components/usb/hcd_dwc.c:50, the USB-DWC's DMA reaching external RAM on the P4), at
/// their cached or uncached addresses. Caches are not modelled, so a DMA access sees what the
/// cores wrote.
struct DmaView<'a> { l2mem: &'a mut [u8], psram: &'a mut [u8], mmu: &'a [u32; MMU_ENTRIES] }
impl DmaView<'_> {
    /// The memory at `addr` and how many bytes of it are contiguous there.
    fn span(&mut self, addr: u32) -> Result<&mut [u8], Fault> {
        let a = if (0x8000_0000..0x9000_0000).contains(&addr) { addr - UNCACHED_OFFSET } else { addr };
        match a {
            L2MEM_LOW..=0x4FFB_FFFF => Ok(&mut self.l2mem[(a - L2MEM_LOW) as usize..]),
            PSRAM_LOW..=0x4BFF_FFFF => {
                let entry = self.mmu[((a - PSRAM_LOW) >> MMU_PAGE_SHIFT) as usize];
                if entry & MMU_PSRAM_VALID == 0 { return Err(Fault::Unmapped); }
                let off = (((entry & MMU_PSRAM_PAGE_MASK) as usize) << MMU_PAGE_SHIFT) | (a & 0xffff) as usize;
                let page_end = (off | 0xffff) + 1;
                self.psram.get_mut(off..page_end.min(self.psram.len())).filter(|s| !s.is_empty()).ok_or(Fault::Unmapped)
            }
            _ => Err(Fault::Unmapped),
        }
    }
}
impl esp_periph::sdmmc::SdmmcMemory for DmaView<'_> {
    fn read(&mut self, mut addr: u32, mut buf: &mut [u8]) -> Result<(), Fault> {
        while !buf.is_empty() {
            let s = self.span(addr)?;
            let n = s.len().min(buf.len());
            buf[..n].copy_from_slice(&s[..n]);
            buf = &mut buf[n..];
            addr = addr.wrapping_add(n as u32);
        }
        Ok(())
    }
    fn write(&mut self, mut addr: u32, mut data: &[u8]) -> Result<(), Fault> {
        while !data.is_empty() {
            let s = self.span(addr)?;
            let n = s.len().min(data.len());
            s[..n].copy_from_slice(&data[..n]);
            data = &data[n..];
            addr = addr.wrapping_add(n as u32);
        }
        Ok(())
    }
}

/// An AXI-GDMA OUT channel's view of memory: descriptors and buffers in L2MEM or PSRAM.
struct AxiOut<'a> { mem: DmaView<'a>, channel: &'a mut esp_periph::gdma::GdmaOutCh }
impl esp_periph::gdma::OutMemory for AxiOut<'_> {
    fn out_channel(&mut self) -> &mut esp_periph::gdma::GdmaOutCh { self.channel }
    fn word(&mut self, address: u32) -> Result<u32, Fault> {
        let mut b = [0u8; 4];
        esp_periph::sdmmc::SdmmcMemory::read(&mut self.mem, address, &mut b)?;
        Ok(u32::from_le_bytes(b))
    }
    fn append(&mut self, address: u32, count: usize, out: &mut Vec<u8>) -> Result<(), (u32, Fault)> {
        let start = out.len();
        out.resize(start + count, 0);
        esp_periph::sdmmc::SdmmcMemory::read(&mut self.mem, address, &mut out[start..]).map_err(|f| { out.truncate(start); (address, f) })
    }
    fn writeback(&mut self, address: u32, value: u32) -> Result<(), Fault> {
        esp_periph::sdmmc::SdmmcMemory::write(&mut self.mem, address, &value.to_le_bytes())
    }
}

pub struct SocBus {
    pub rom: Vec<u8>,
    pub l2mem: Vec<u8>,
    pub tcm: Vec<u8>,
    pub lp_rom: Vec<u8>,
    pub lp_ram: Vec<u8>,
    pub flash: Vec<u8>,
    pub psram: Vec<u8>,
    pub mmu_flash: Box<[u32; MMU_ENTRIES]>,
    pub mmu_psram: Box<[u32; MMU_ENTRIES]>,
    pub mmu_flash_index: u32,
    pub mmu_psram_index: u32,
    pub periph: Peripherals,
    /// wafer version the efuses report, kept across a reset
    pub wafer: (u32, u32),
    pub board: esp_soc::Board,
    pub cycles: u64,
    pub last_fault: Option<(u32, bool)>,
    /// a peripheral write may have moved an interrupt line: re-derive before the next instruction
    pub irq_dirty: bool,
    pub gpio_events: Option<Vec<(u64, u8, bool)>>,
    pub debug: esp_soc::DebugFlags,
    /// the core the machine is running, for the core-local CLIC window (`Soc::select_core`)
    pub cur_core: usize,
    /// each hart's LR reservation (the word); a store by the other hart to it breaks it
    pub reservation: [Option<u32>; 2],
    /// the core whose store requested the pending software reset, and that store's PC
    pub reset_request: Option<(usize, u32)>,
    /// where each core was when the reset took effect (`SocBus::note_core_pcs`)
    pub core_pcs: [u32; 2],
}

impl SocBus {
    pub fn new(flash_size: usize, mac: [u8; 6], wafer: (u32, u32)) -> Self {
        SocBus {
            rom: vec![0; (ROM_HIGH - ROM_LOW) as usize], l2mem: vec![0; (L2MEM_HIGH - L2MEM_LOW) as usize], tcm: vec![0; (TCM_HIGH - TCM_LOW) as usize],
            lp_rom: vec![0; (LP_ROM_HIGH - LP_ROM_LOW) as usize], lp_ram: vec![0; (LP_RAM_HIGH - LP_RAM_LOW) as usize],
            flash: vec![0xff; flash_size], psram: Vec::new(),
            mmu_flash: Box::new([0; MMU_ENTRIES]), mmu_psram: Box::new([0; MMU_ENTRIES]), mmu_flash_index: 0, mmu_psram_index: 0,
            periph: Peripherals::new(mac, wafer), wafer, board: Box::new(esp_soc::NoBoard),
            cycles: 0, last_fault: None, irq_dirty: true, gpio_events: None, debug: Default::default(), cur_core: 0, reservation: [None; 2], reset_request: None, core_pcs: [0; 2],
        }
    }

    /// Resolve to (buffer, offset, writable). The flash and PSRAM windows go through their MMUs.
    fn resolve(&mut self, addr: u32) -> Option<(&mut Vec<u8>, usize, bool)> {
        let a = if (0x8000_0000..0x9000_0000).contains(&addr) { addr - UNCACHED_OFFSET } else { addr };
        match a {
            L2MEM_LOW..=0x4FFB_FFFF => Some((&mut self.l2mem, (a - L2MEM_LOW) as usize, true)),
            FLASH_LOW..=0x43FF_FFFF => {
                let entry = self.mmu_flash[((a - FLASH_LOW) >> MMU_PAGE_SHIFT) as usize];
                if entry & MMU_FLASH_VALID == 0 { return None; }
                let off = (((entry & MMU_FLASH_PAGE_MASK) as usize) << MMU_PAGE_SHIFT) | (a & 0xffff) as usize;
                if off < self.flash.len() { Some((&mut self.flash, off, false)) } else { None }
            }
            PSRAM_LOW..=0x4BFF_FFFF => {
                let entry = self.mmu_psram[((a - PSRAM_LOW) >> MMU_PAGE_SHIFT) as usize];
                if entry & MMU_PSRAM_VALID == 0 { return None; }
                let off = (((entry & MMU_PSRAM_PAGE_MASK) as usize) << MMU_PAGE_SHIFT) | (a & 0xffff) as usize;
                if off < self.psram.len() { Some((&mut self.psram, off, true)) } else { None }
            }
            ROM_LOW..=0x4FC1_FFFF => Some((&mut self.rom, (a - ROM_LOW) as usize, false)),
            TCM_LOW..=0x3010_1FFF => Some((&mut self.tcm, (a - TCM_LOW) as usize, true)),
            LP_RAM_LOW..=0x5010_FFFF => Some((&mut self.lp_ram, (a - LP_RAM_LOW) as usize, true)),
            LP_ROM_LOW..=0x5010_3FFF => Some((&mut self.lp_rom, (a - LP_ROM_LOW) as usize, false)),
            _ => None,
        }
    }

    /// The CPU-subsystem window (CLIC, debug), the CPU peripherals (cache, ASSIST_DEBUG) and the
    /// HP and LP peripherals; the LP ROM and RAM between the last two are memory.
    #[inline]
    fn is_periph(addr: u32) -> bool {
        match addr >> 28 {
            0x2 => true,
            0x3 => (0x3FF0_0000..0x3FF2_0000).contains(&addr),
            0x5 => addr < 0x5010_0000 || (0x5011_0000..0x5013_0000).contains(&addr),
            _ => false,
        }
    }

    fn mmu_reg(a: u32) -> Option<(Mmu, u32)> {
        let o = a & 0xfff;
        if o != SPI_MMU_ITEM_CONTENT && o != SPI_MMU_ITEM_INDEX { return None; }
        match a & !0xfff { FLASH_SPI0 => Some((Mmu::Flash, o)), PSRAM_MSPI0 => Some((Mmu::Psram, o)), _ => None }
    }

    fn mmu_read(&self, m: Mmu, o: u32) -> u32 {
        match (m, o) {
            (Mmu::Flash, SPI_MMU_ITEM_INDEX) => self.mmu_flash_index,
            (Mmu::Flash, _) => self.mmu_flash[self.mmu_flash_index as usize % MMU_ENTRIES],
            (Mmu::Psram, SPI_MMU_ITEM_INDEX) => self.mmu_psram_index,
            (Mmu::Psram, _) => self.mmu_psram[self.mmu_psram_index as usize % MMU_ENTRIES],
        }
    }

    fn mmu_write(&mut self, m: Mmu, o: u32, v: u32) {
        match (m, o) {
            (Mmu::Flash, SPI_MMU_ITEM_INDEX) => self.mmu_flash_index = v & 0x3ff,
            (Mmu::Flash, _) => self.mmu_flash[self.mmu_flash_index as usize % MMU_ENTRIES] = v & 0x3fff,
            (Mmu::Psram, SPI_MMU_ITEM_INDEX) => self.mmu_psram_index = v & 0x3ff,
            (Mmu::Psram, _) => self.mmu_psram[self.mmu_psram_index as usize % MMU_ENTRIES] = v & 0x1fff,
        }
    }

    /// The CLIC window is per core: the running core's own block at 0x20800000, the other core's
    /// at 0x20810000 (include/soc/interrupt_reg.h, INTERRUPT_CURRENT/OTHER_CORE_INT_THRESH_REG).
    #[inline]
    fn clic_view(&self, a: u32) -> u32 { if self.cur_core == 1 && (CLIC_LOW..CLIC_HIGH).contains(&a) { a ^ 0x1_0000 } else { a } }

    fn periph_read(&mut self, addr: u32, size: u32) -> u32 {
        let a = self.clic_view(addr & !3);
        let w = match Self::mmu_reg(a) { Some((m, o)) => self.mmu_read(m, o), None => self.periph.read32(a) };
        // A STATUS read can raise the SDMMC's busy-clear interrupt.
        if a >> 12 == 0x50083 && self.periph.sdmmc.take_read_irq() { self.irq_dirty = true; }
        match size { 1 => (w >> ((addr & 3) * 8)) & 0xff, 2 => (w >> ((addr & 2) * 8)) & 0xffff, _ => w }
    }

    fn periph_write(&mut self, addr: u32, v: u32, size: u32) {
        let a = self.clic_view(addr & !3);
        let merge = |old: u32| match size {
            4 => v,
            1 => { let sh = (addr & 3) * 8; (old & !(0xff << sh)) | ((v & 0xff) << sh) }
            _ => { let sh = (addr & 2) * 8; (old & !(0xffff << sh)) | ((v & 0xffff) << sh) }
        };
        if let Some((m, o)) = Self::mmu_reg(a) { let old = self.mmu_read(m, o); self.mmu_write(m, o, merge(old)); return; }
        // The CLIC's control words are four byte registers: a byte write touches its own only.
        if (CLIC_LOW..CLIC_HIGH).contains(&a) {
            let lanes = match size { 4 => 0xf, 1 => 1 << (addr & 3), _ => 3 << (addr & 2) };
            self.periph.clic.write_lanes(a - CLIC_LOW, merge(0), lanes);
            self.irq_dirty = true;
            return;
        }
        let v = if size == 4 { v } else { merge(self.periph.read32(a)) };
        let resetting = self.periph.lp.sw_reset;
        self.periph.write32(a, v);
        // the store that asks for a software reset is where ASSIST_DEBUG finds this core
        if !resetting && self.periph.lp.sw_reset { self.reset_request = Some((self.cur_core, self.periph.misc.cur_pc)); }
        if self.periph.spi_exec { self.run_spi(); }
        if self.periph.psram_exec { self.run_psram(); }
        if self.periph.sdmmc.pending() { self.run_sdmmc(); }
        // A GP-SPI transfer reaches the board now, after the GPIO edges before it (a panel's D/C
        // line is a GPIO the driver sets right before each transaction).
        if self.periph.spi2.dma_tx_pending.is_some() { self.spi2_dma_tx(); }
        if self.periph.spi2.has_pending_transfer() || !self.periph.gpio.g.changes.is_empty() { self.deliver_board_events(); }
        self.irq_dirty = true;
    }

    /// The board's I2C devices on I2C1 (bus 0 of `BoardModel::i2c_devices`).
    pub fn attach_board_devices(&mut self) {
        for (bus, address, device) in self.board.i2c_devices() {
            if bus == 0 { self.periph.i2c1.attach(address, device); }
        }
        for (pin, level) in self.board.input_levels() { self.irq_dirty |= self.periph.gpio.g.set_input(pin, level); }
    }

    /// I2S1's transmit DMA through the AHB-GDMA (trigger 4), paced by the frame rate: the frames
    /// due since the last call are taken from the OUT chain, a descriptor at a time, each one
    /// written back to the CPU when AUTO_WRBACK is set, with OUT_DONE, OUT_EOF on a SUC_EOF one and
    /// OUT_TOTAL_EOF when that one ends the chain, as the S3's pump does (esp32s3/src/bus/dma.rs).
    /// Descriptors are read through the shared `DescriptorWalk`, with its step budget. The
    /// first slot of each frame goes to the audio sink. IDF's I2S driver keeps a ring of
    /// descriptors, so the stream runs as long as it refills them.
    fn i2s1_tx(&mut self, cycles: u64) {
        let (frames, bpf, sample_bytes) = { let i = &mut self.periph.i2s1; (i.frames_due(cycles), i.bytes_per_frame as usize, i.sample_bytes()) };
        if frames == 0 || bpf == 0 { return; }
        let Some(ch) = self.periph.ahb_dma.gdma.out_channel_for(4) else { return };
        let mut need = frames as usize * bpf;
        // the bytes of every descriptor taken, so a frame split between two stays whole
        let mut stream: Vec<u8> = Vec::with_capacity(need);
        let mut mem = DmaView { l2mem: &mut self.l2mem, psram: &mut self.psram, mmu: &self.mmu_psram };
        let mut walk = esp_periph::gdma::DescriptorWalk::new(esp_periph::gdma::GDMA_DESCRIPTOR_STEP_BUDGET);
        loop {
            if need == 0 { break; }
            let c = self.periph.ahb_dma.gdma.out[ch];
            if !c.running || c.desc == 0 { break; }
            let read = walk.read_with(c.desc, |a| { let mut b = [0u8; 4]; esp_periph::sdmmc::SdmmcMemory::read(&mut mem, a, &mut b).map(|_| u32::from_le_bytes(b)) });
            let (dw0, d) = match read {
                Ok(read) => read,
                Err(esp_periph::gdma::DmaDescriptorFault::StepBudgetExceeded { .. }) => break,   // the rest at the next call
                Err(_) => { let c = &mut self.periph.ahb_dma.gdma.out[ch]; c.running = false; c.int_raw |= 1 << 2; break; }   // OUT_DSCR_ERR
            };
            let (buf, next) = (d.buf, d.next);
            let remaining = d.length.saturating_sub(c.buf_pos) as usize;
            if remaining == 0 {
                if c.conf0 & (1 << 2) != 0 { let _ = esp_periph::sdmmc::SdmmcMemory::write(&mut mem, c.desc, &(dw0 & !(1 << 31)).to_le_bytes()); }   // AUTO_WRBACK: owner -> CPU
                let c = &mut self.periph.ahb_dma.gdma.out[ch];
                c.int_raw |= 1 << 0;                                                    // OUT_DONE
                if dw0 & (1 << 30) != 0 { c.int_raw |= 1 << 1; c.eof_desc = c.desc; }   // OUT_EOF
                if next == 0 { c.running = false; c.desc = 0; if d.eof { c.int_raw |= 1 << 3; } break; }   // OUT_TOTAL_EOF
                c.desc = next; c.buf_pos = 0;
                continue;
            }
            let take = remaining.min(need);
            let start = stream.len();
            stream.resize(start + take, 0);
            if esp_periph::sdmmc::SdmmcMemory::read(&mut mem, buf.wrapping_add(c.buf_pos), &mut stream[start..]).is_err() {
                stream.truncate(start);
                let c = &mut self.periph.ahb_dma.gdma.out[ch]; c.running = false; c.int_raw |= 1 << 2; break;
            }
            self.periph.ahb_dma.gdma.out[ch].buf_pos += take as u32;
            need -= take;
        }
        self.irq_dirty = true;
        // a frame cut short by the chain's end, an error or the step budget is dropped
        let samples: Vec<i16> = stream.chunks_exact(bpf)
            .map(|f| if sample_bytes == 1 { (f[0] as i8 as i16) << 8 } else { i16::from_le_bytes([f[sample_bytes - 2], f[sample_bytes - 1]]) })
            .collect();
        if !samples.is_empty() { let i = &mut self.periph.i2s1; i.frames_out += samples.len() as u64; i.pcm.extend_from_slice(&samples); }
    }

    /// GPSPI2's transmit DMA through the AXI-GDMA (trigger 1): one finite gather per transaction,
    /// the channel stopping after it with OUT_TOTAL_EOF, or OUT_DSCR_ERR on a descriptor fault
    /// (the transfer then completes with what was gathered, as on the C6).
    fn spi2_dma_tx(&mut self) {
        let Some(bits) = self.periph.spi2.dma_tx_pending else { return };
        let Some(ch) = self.periph.axi_dma.gdma.out_channel_for(1) else { return };   // not started yet: the round's end retries
        let mut data = Vec::with_capacity((bits as usize).div_ceil(8));
        let mut memory = AxiOut { mem: DmaView { l2mem: &mut self.l2mem, psram: &mut self.psram, mmu: &self.mmu_psram }, channel: &mut self.periph.axi_dma.gdma.out[ch] };
        let result = esp_periph::gdma::gather_out(&mut memory, (bits as usize).div_ceil(8), &mut data);
        let c = &mut self.periph.axi_dma.gdma.out[ch];
        c.running = false; c.buf_pos = 0;
        match result { Ok(next) => { c.desc = next; c.int_raw |= 1 << 3; } Err(_) => c.int_raw |= 1 << 2 }
        self.periph.spi2.complete_dma_tx(&data);
        self.irq_dirty = true;
    }

    /// GPIO edges, then a finished GP-SPI transaction, to the board.
    fn deliver_board_events(&mut self) {
        let changes = std::mem::take(&mut self.periph.gpio.g.changes);
        if !changes.is_empty() {
            if let Some(ev) = &mut self.gpio_events { for &(pin, level) in &changes { ev.push((self.cycles, pin, level)); } }
            self.board.gpio_output_at(self.cycles, &changes, self.periph.gpio.g.enable, self.periph.gpio.g.out);
        }
        if let Some(transfer) = self.periph.spi2.take_transfer() {
            let rx = self.board.spi_transfer(2, &transfer.tx, transfer.rx_len);
            self.periph.spi2.finish_transfer(transfer, &rx);
            self.irq_dirty = true;
        }
    }

    /// Carry out the SDMMC command or IDMAC resume the last write started, on the slots'
    /// current pin routing.
    fn run_sdmmc(&mut self) {
        self.periph.sdmmc.route = self.periph.sd_routes();
        let mut dma = DmaView { l2mem: &mut self.l2mem, psram: &mut self.psram, mmu: &self.mmu_psram };
        self.periph.sdmmc.run(&mut dma);
        if self.periph.has_sdio { self.sync_sdio(); }
    }

    /// An SDIO function's DAT1 after a command or at the co-simulation's slice end.
    #[cold]
    #[inline(never)]
    pub fn sync_sdio(&mut self) {
        if self.periph.sync_sdio() { self.irq_dirty = true; }
    }

    /// Put an SDIO function on the bus at `pins` (the co-processor at `periph::SdPins::HOSTED`).
    pub fn attach_sdio(&mut self, pins: crate::periph::SdPins, dev: Box<dyn esp_periph::sdmmc::SdDevice>) {
        self.periph.attach_sd(pins, dev);
        self.sync_sdio();
    }

    /// Let the USB host on the cable carry out what is due, its transactions moving data through
    /// the core's DMA.
    fn run_usb(&mut self) {
        let mut dma = DmaView { l2mem: &mut self.l2mem, psram: &mut self.psram, mmu: &self.mmu_psram };
        self.periph.usb_otg.run(&mut dma);
        esp_periph::Dispatch::refresh_optional(&mut self.periph, crate::periph::USB_OTG_BLOCK);
        self.irq_dirty = true;
    }

    /// Execute a pending SPI1 command against the flash image.
    fn run_spi(&mut self) {
        self.periph.spi_exec = false;
        self.periph.spi1.execute(&mut self.flash, &mut []);
        self.periph.spi1.dirty.clear();
    }

    /// Execute a pending PSRAM controller command against the PSRAM array.
    fn run_psram(&mut self) {
        self.periph.psram_exec = false;
        self.periph.psram1.execute(&mut [], &mut self.psram);
        self.periph.psram1.dirty.clear();
    }

    /// The device deadline `timer` (CPU cycles, `u32::MAX` for none) or a timed SD bus's next event.
    #[cold]
    #[inline(never)]
    pub(crate) fn timed_sd_deadline(&self, timer: u32) -> Option<u64> {
        let mut c = timer as u64;
        if let Some(t) = self.periph.sdmmc.next_event_ns() {
            let ns = t.saturating_sub(self.periph.now_ns());
            c = c.min((ns as u128 * crate::periph::CPU_HZ as u128).div_ceil(1_000_000_000) as u64);
        }
        if c >= u32::MAX as u64 { None } else { Some(c.max(1)) }
    }

    /// Write straight into flash (image loaders, not the guest).
    pub fn write_flash(&mut self, offset: usize, data: &[u8]) -> Result<(), String> {
        let target = self.flash.get_mut(offset..).and_then(|tail| tail.get_mut(..data.len())).ok_or("flash image too large")?;
        target.copy_from_slice(data);
        Ok(())
    }

    pub fn load_bytes(&mut self, addr: u32, data: &[u8]) -> Result<(), String> {
        // The mask ROM ELF links its .eh_frame at the flash window; nothing ever reads it.
        if (FLASH_LOW..FLASH_HIGH).contains(&addr) { return Ok(()); }
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
        if self.periph.spi_exec { self.run_spi(); }
        // board time first, its input edges after the peripherals (as the S3 bus does)
        let edges = self.board.uses_gpio_edges();
        if edges { self.board.advance_to(self.cycles); }
        self.periph.tick(cycles as u64);
        if edges { self.irq_dirty |= esp_soc::gpio::drain_board_inputs(&mut *self.board, &mut self.periph.gpio.g, &mut self.gpio_events, self.cycles); }
        if self.periph.spi2.dma_tx_pending.is_some() { self.spi2_dma_tx(); }
        if self.periph.spi2.has_pending_transfer() || !self.periph.gpio.g.changes.is_empty() { self.deliver_board_events(); }
        if self.periph.i2s1.poll_active() { self.i2s1_tx(cycles as u64); }
        if self.periph.usb_otg.pending() { self.run_usb(); }
        if self.periph.sdmmc.timed { self.sd_time(); }
    }

    /// A timed SD bus raises what has come due.
    #[cold]
    #[inline(never)]
    fn sd_time(&mut self) {
        let now = self.periph.now_ns();
        if self.periph.sdmmc.next_event_ns().is_some_and(|t| t <= now) && self.periph.sdmmc.advance(now) { self.irq_dirty = true; }
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

impl SocBus {
    /// A store by the running hart breaks the other hart's reservation of the same word.
    #[inline(always)]
    fn break_reservation(&mut self, addr: u32) {
        let other = &mut self.reservation[self.cur_core ^ 1];
        if *other == Some(addr & !3) { *other = None; }
    }
}

impl Bus for SocBus {
    fn note_code_page(&mut self, _vidx: u32) {} // This bus has no decode cache.
    fn reserve(&mut self, addr: u32) { self.reservation[self.cur_core] = Some(addr & !3); }
    fn reservation_held(&mut self, addr: u32) -> bool { self.reservation[self.cur_core].take() == Some(addr & !3) }
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
        self.break_reservation(addr);
        match self.resolve(addr) {
            Some((b, o, true)) if o < b.len() => { b[o] = v; Ok(()) }
            _ => { self.last_fault = Some((addr, true)); Err(Fault::Prohibited) }
        }
    }
    fn write16(&mut self, addr: u32, v: u16) -> Result<(), Fault> {
        if Self::is_periph(addr) { self.periph_write(addr, v as u32, 2); return Ok(()); }
        self.break_reservation(addr);
        match self.resolve(addr) {
            Some((b, o, true)) if o + 2 <= b.len() => { b[o..o + 2].copy_from_slice(&v.to_le_bytes()); Ok(()) }
            _ => { self.last_fault = Some((addr, true)); Err(Fault::Prohibited) }
        }
    }
    fn write32(&mut self, addr: u32, v: u32) -> Result<(), Fault> {
        if Self::is_periph(addr) { self.periph_write(addr, v, 4); return Ok(()); }
        self.break_reservation(addr);
        match self.resolve(addr) {
            Some((b, o, true)) if o + 4 <= b.len() => { b[o..o + 4].copy_from_slice(&v.to_le_bytes()); Ok(()) }
            _ => { self.last_fault = Some((addr, true)); Err(Fault::Prohibited) }
        }
    }
    fn fetch(&mut self, pc: u32) -> Result<[u8; 4], Fault> {
        match self.resolve(pc) {
            Some((b, o, _)) if o < b.len() => {
                let mut r = [0u8; 4];
                for (i, x) in r.iter_mut().enumerate() { if let Some(&v) = b.get(o + i) { *x = v; } }
                Ok(r)
            }
            _ => { self.last_fault = Some((pc, false)); Err(Fault::Unmapped) }
        }
    }
    fn tick(&mut self, cycles: u32) -> u32 {
        self.cycles += cycles as u64;
        self.devices(cycles);
        1
    }
    #[inline(always)]
    fn note_pc(&mut self, pc: u32) { self.periph.misc.cur_pc = pc; }
    #[inline(always)]
    fn block_break(&self) -> bool { self.irq_dirty }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// I2S1's transmit chain read from a guest-chosen descriptor address: one at the top of the
    /// address space reads unmapped words, an OUT_DSCR_ERR, not an overflow (a panic in a debug
    /// build); a chain of descriptors that loops ends at the walk's step budget, with no frames.
    #[test]
    fn i2s1_descriptors_at_any_address_end_in_an_error_not_a_panic() {
        let mut b = SocBus::new(0x1000, [0; 6], (3, 1));
        b.periph.i2s1.write(0x24, 1 << 2);                            // TX_START
        let ch = &mut b.periph.ahb_dma.gdma.out[0];
        (ch.peri_sel, ch.running, ch.desc) = (4, true, 0xffff_fffc);
        b.i2s1_tx(crate::periph::CPU_HZ);
        let ch = b.periph.ahb_dma.gdma.out[0];
        assert!(!ch.running && ch.int_raw & 1 << 2 != 0, "OUT_DSCR_ERR: {:#x}", ch.int_raw);
        // An owned descriptor of length 0 pointing at itself, in L2MEM.
        let d = 0x4ff4_0000;
        for (o, v) in [(0, 1u32 << 31), (4, d), (8, d)] { b.write32(d + o, v).unwrap(); }
        let ch = &mut b.periph.ahb_dma.gdma.out[0];
        (ch.running, ch.desc, ch.int_raw) = (true, d, 0);
        b.i2s1_tx(crate::periph::CPU_HZ);
        assert!(b.periph.i2s1.pcm.is_empty());
        assert_eq!(b.periph.ahb_dma.gdma.out[0].int_raw & 1 << 3, 0, "no OUT_TOTAL_EOF");
    }

    /// A frame split between two descriptors (16-bit stereo, 6 bytes and then 2) is one frame,
    /// not a frame and a dropped tail.
    #[test]
    fn i2s1_a_frame_split_between_descriptors_stays_whole() {
        let mut b = SocBus::new(0x1000, [0; 6], (3, 1));
        b.periph.i2s1.write(0x24, 1 << 2);                            // TX_START
        (b.periph.i2s1.tx_conf1, b.periph.i2s1.bytes_per_frame) = (15 << 13, 4);   // 16-bit samples, two slots
        let (d1, d2) = (0x4ff4_0000, 0x4ff4_0010);
        for (o, v) in [(0, 1u32 << 31 | 6 << 12 | 6), (4, d1 + 0x100), (8, d2)] { b.write32(d1 + o, v).unwrap(); }
        for (o, v) in [(0, 1u32 << 31 | 1 << 30 | 2 << 12 | 2), (4, d1 + 0x200), (8, 0)] { b.write32(d2 + o, v).unwrap(); }
        for (a, v) in [(d1 + 0x100, 0x2222_1111), (d1 + 0x104, 0x3333), (d1 + 0x200, 0x4444)] { b.write32(a, v).unwrap(); }
        let ch = &mut b.periph.ahb_dma.gdma.out[0];
        (ch.peri_sel, ch.running, ch.desc) = (4, true, d1);
        b.i2s1_tx(crate::periph::CPU_HZ);
        assert_eq!(b.periph.i2s1.pcm, [0x1111, 0x3333], "the first slot of each frame");
        assert!(b.periph.ahb_dma.gdma.out[0].int_raw & 1 << 3 != 0, "OUT_TOTAL_EOF");
    }

    #[test]
    fn flash_window_and_its_uncached_alias_go_through_the_mmu() {
        let mut b = SocBus::new(0x40000, [0; 6], (1, 0));
        b.flash[0x2_0010..0x2_0014].copy_from_slice(&[1, 2, 3, 4]);
        assert!(b.read32(0x4001_0010).is_err());
        b.write32(FLASH_SPI0 + SPI_MMU_ITEM_INDEX, 1).unwrap();
        b.write32(FLASH_SPI0 + SPI_MMU_ITEM_CONTENT, MMU_FLASH_VALID | 2).unwrap();
        assert_eq!(b.read32(FLASH_SPI0 + SPI_MMU_ITEM_CONTENT).unwrap(), MMU_FLASH_VALID | 2);
        assert_eq!(b.read32(0x4001_0010).unwrap(), 0x0403_0201);
        assert_eq!(b.read32(0x8001_0010).unwrap(), 0x0403_0201);
        assert!(b.write32(0x4001_0010, 0).is_err());
    }

    #[test]
    fn psram_window_is_writable_through_its_own_mmu() {
        let mut b = SocBus::new(0x1000, [0; 6], (1, 0));
        b.psram = vec![0; 0x20000];
        b.write32(PSRAM_MSPI0 + SPI_MMU_ITEM_INDEX, 3).unwrap();
        b.write32(PSRAM_MSPI0 + SPI_MMU_ITEM_CONTENT, MMU_PSRAM_VALID | 1 << 10 | 1).unwrap();
        b.write32(0x4803_0004, 0xdead_beef).unwrap();
        assert_eq!(&b.psram[0x1_0004..0x1_0008], &0xdead_beefu32.to_le_bytes());
        assert_eq!(b.read32(0x8803_0004).unwrap(), 0xdead_beef);
        assert_eq!(b.read32(FLASH_SPI0 + SPI_MMU_ITEM_CONTENT).unwrap(), 0);
    }

    #[test]
    fn l2mem_tcm_and_lp_ram_are_one_store_per_alias() {
        let mut b = SocBus::new(0x1000, [0; 6], (1, 0));
        b.write32(0x4FF4_0000, 0x1234_5678).unwrap();
        assert_eq!(b.read32(0x8FF4_0000).unwrap(), 0x1234_5678);
        b.write8(0x3010_1FFF, 0xab).unwrap();
        assert_eq!(b.read8(0x3010_1FFF).unwrap(), 0xab);
        b.write16(0x5010_FFFE, 0xcdef).unwrap();
        assert_eq!(b.read16(0x5010_FFFE).unwrap(), 0xcdef);
        assert!(b.write32(ROM_LOW, 0).is_err());
        assert!(b.read32(0x4FFC_0000).is_err());
    }

    /// A user command on the PSRAM controller's command side, as psram_ctrlr_ll_common_transaction
    /// issues it: 16-bit command, 32-bit address, CS1 (IDF v5.5.3 hal/esp32p4/include/hal/psram_ctrlr_ll.h:754-798).
    fn psram_cmd(b: &mut SocBus, cmd: u32, addr: u32, mosi: Option<u32>, miso_bits: u32) -> u32 {
        const BASE: u32 = 0x5008_F000;
        let user = 1 << 31 | 1 << 30 | if mosi.is_some() { 1 << 27 } else { 0 } | if miso_bits > 0 { 1 << 28 } else { 0 };
        for (o, v) in [(0x34, 1), (0x18, user), (0x1c, 31 << 26), (0x20, 15 << 28 | cmd), (0x04, addr), (0x24, 31), (0x28, miso_bits.saturating_sub(1))] { b.write32(BASE + o, v).unwrap(); }
        if let Some(v) = mosi { b.write32(BASE + 0x58, v).unwrap(); }
        b.write32(BASE, 1 << 18).unwrap();
        assert_eq!(b.read32(BASE).unwrap(), 0, "USR clears when the command is done");
        b.read32(BASE + 0x58).unwrap()
    }

    #[test]
    fn hex_psram_answers_its_mode_registers_and_shares_the_window() {
        let mut b = SocBus::new(0x1000, [0; 6], (1, 0));
        b.psram = vec![0; 0x20000];
        assert_eq!(psram_cmd(&mut b, 0x4040, 0, None, 16) & 0xffff, 0x0d30);   // MR0, MR1 vendor AP
        assert_eq!(psram_cmd(&mut b, 0x4040, 2, None, 16) & 0xffff, 0x20df);   // MR2 256 Mbit, MR3 slow refresh
        psram_cmd(&mut b, 0x8080, 0x1_0004, Some(0x5a6b_7c8d), 0);
        assert_eq!(psram_cmd(&mut b, 0x0000, 0x1_0004, None, 32), 0x5a6b_7c8d);
        b.write32(PSRAM_MSPI0 + SPI_MMU_ITEM_INDEX, 0).unwrap();
        b.write32(PSRAM_MSPI0 + SPI_MMU_ITEM_CONTENT, MMU_PSRAM_VALID | 1 << 10 | 1).unwrap();
        assert_eq!(b.read32(PSRAM_LOW + 4).unwrap(), 0x5a6b_7c8d);
    }

    /// Two harts on one bus: any store by the other hart to the reserved word makes the SC fail
    /// (word, half and byte stores, an AMO, the other hart's own successful SC); a store elsewhere
    /// or by the same hart does not (RISC-V unprivileged ISA, "Zalrsc").
    #[test]
    fn a_store_from_the_other_hart_breaks_a_reservation() {
        use riscv_rv32::{step, Cpu};
        #[derive(Clone, Copy)]
        enum Store { None, Word(usize, u32), Half(u32), Byte(u32), Amo, Sc }
        let mut b = SocBus::new(0x1000, [0; 6], (1, 0));
        let (prog, other, data) = (L2MEM_LOW + 0x100, L2MEM_LOW + 0x180, L2MEM_LOW + 0x200);
        // lr.w a0,(a1); sc.w a2,a3,(a1)
        for (k, w) in [0x1005_a52fu32, 0x18d5_a62f].iter().enumerate() { b.write32(prog + 4 * k as u32, *w).unwrap(); }
        // amoadd.w a4,a3,(a1); lr.w a5,(a1); sc.w a4,a3,(a1)
        for (k, w) in [0x00d5_a72fu32, 0x1005_a7af, 0x18d5_a72f].iter().enumerate() { b.write32(other + 4 * k as u32, *w).unwrap(); }
        let sc_after = |b: &mut SocBus, store: Store| {
            let mut c = Cpu::new_esp32p4();
            c.pc = prog; c.x[11] = data; c.x[13] = 0x55;
            b.cur_core = 0;
            step(&mut c, b).unwrap();
            b.cur_core = 1;
            let mut h1 = Cpu::new_esp32p4();
            h1.hartid = 1; h1.x[11] = data; h1.x[13] = 3;
            match store {
                Store::None => {}
                Store::Word(hart, addr) => { b.cur_core = hart; b.write32(addr, 7).unwrap(); }
                Store::Half(addr) => b.write16(addr, 7).unwrap(),
                Store::Byte(addr) => b.write8(addr, 7).unwrap(),
                Store::Amo => { h1.pc = other; step(&mut h1, b).unwrap(); }
                Store::Sc => { h1.pc = other + 4; step(&mut h1, b).unwrap(); step(&mut h1, b).unwrap(); assert_eq!(h1.x[14], 0, "hart 1's own SC succeeds"); }
            }
            b.cur_core = 0;
            step(&mut c, b).unwrap();
            c.x[12]
        };
        assert_eq!(sc_after(&mut b, Store::None), 0);
        assert_eq!(sc_after(&mut b, Store::Word(1, data)), 1, "core 1 stored to the word");
        assert_eq!(sc_after(&mut b, Store::Half(data + 2)), 1, "a halfword of it");
        assert_eq!(sc_after(&mut b, Store::Byte(data + 3)), 1, "a byte of it");
        assert_eq!(sc_after(&mut b, Store::Amo), 1, "an AMO");
        assert_eq!(sc_after(&mut b, Store::Sc), 1, "core 1's SC");
        assert_eq!(sc_after(&mut b, Store::Word(1, data + 4)), 0, "the next word");
        assert_eq!(sc_after(&mut b, Store::Word(0, data)), 0, "the same hart's store");
        b.cur_core = 1;
        b.reserve(data);
        b.cur_core = 0;
        assert!(!b.reservation_held(data), "core 0 holds no reservation of its own");
        b.cur_core = 1;
        assert!(b.reservation_held(data));
    }

    /// The IDMAC reaches L2MEM at both addresses and PSRAM through its MMU, page by page.
    #[test]
    fn the_sdmmc_dma_reaches_l2mem_and_psram() {
        use esp_periph::sdmmc::SdmmcMemory;
        let mut b = SocBus::new(0x1000, [0; 6], (1, 0));
        b.psram = vec![0; 0x20000];
        b.mmu_psram[0] = MMU_PSRAM_VALID | 1 << 10 | 1;
        b.mmu_psram[1] = MMU_PSRAM_VALID | 1 << 10;
        let mut d = DmaView { l2mem: &mut b.l2mem, psram: &mut b.psram, mmu: &b.mmu_psram };
        d.write(0x8FF4_0000, &[1, 2, 3, 4]).unwrap();
        let mut r = [0u8; 4];
        d.read(0x4FF4_0000, &mut r).unwrap();
        assert_eq!(r, [1, 2, 3, 4]);
        d.write(0x4800_fffe, &[5, 6, 7, 8]).unwrap();
        assert_eq!((&b.psram[0x1_fffe..], &b.psram[..2]), (&[5u8, 6][..], &[7u8, 8][..]), "split across the page boundary as mapped");
        let mut d = DmaView { l2mem: &mut b.l2mem, psram: &mut b.psram, mmu: &b.mmu_psram };
        assert!(d.read(0x4802_0000, &mut r).is_err(), "an unmapped PSRAM page");
        assert!(d.read(0x4FFB_FFFE, &mut r).is_err(), "past the end of L2MEM");
        assert!(d.write(0x3010_0000, &r).is_err(), "TCM is out of its reach");
    }

    /// A command through the bus: the CMD write is carried out before the next access.
    #[test]
    fn an_sdmmc_command_completes_within_its_register_write() {
        let mut b = SocBus::new(0x1000, [0; 6], (1, 0));
        esp_soc::SocBus::attach_sd_card(&mut b, Box::new(esp_periph::sdmmc::MemStore(vec![0; 1 << 20])), Default::default()).unwrap();
        b.write32(0x5008_3028, 0x1aa).unwrap();
        b.write32(0x5008_302c, 1 << 31 | 1 << 6 | 8).unwrap();
        assert_eq!(b.read32(0x5008_302c).unwrap() >> 31, 0, "taken");
        assert_eq!((b.read32(0x5008_3030).unwrap(), b.read32(0x5008_3044).unwrap()), (0x1aa, 1 << 2), "R7 and CMD_DONE");
        b.write32(0x5008_3024, 1 << 2).unwrap();
        b.write32(0x5008_3000, 1 << 4).unwrap();
        b.periph.refresh_lines();
        assert_eq!(b.periph.source_status()[0] >> crate::periph::src::SDIO_HOST & 1, 1, "source 23 asserted");
    }

    /// A chip reset keeps the USB cable and its VBUS; the host sees the reset core's pull-up go.
    #[test]
    fn a_chip_reset_keeps_the_usb_cable() {
        use esp_periph::usb_otg::Request;
        let mut b = SocBus::new(0x1000, [0; 6], (1, 0));
        esp_soc::SocBus::attach_usb_host(&mut b, Default::default()).unwrap();
        esp_soc::SocBus::usb_link(&mut b).unwrap().submit(Request::Attach).unwrap();
        b.tick(64);
        assert!(esp_soc::SocBus::usb_link(&mut b).unwrap().take_completion().is_some(), "VBUS on at the next device tick");
        assert_eq!(b.read32(0x5000_0000).unwrap() >> 19 & 1, 1, "GOTGCTL.BSESVLD");
        b.write32(0x5000_0804, 0).unwrap();
        assert!(esp_soc::SocBus::usb_link(&mut b).unwrap().port().connected, "DCTL.SDIS cleared: D+ pulled up");
        esp_soc::SocBus::reboot(&mut b, [0; 6]);
        let p = esp_soc::SocBus::usb_link(&mut b).unwrap().port();
        assert!(p.power && !p.connected, "VBUS stays, the reset core is soft-disconnected");
        assert_eq!(b.read32(0x5000_0000).unwrap() >> 19 & 1, 1, "and sees the session");
        b.write32(0x5000_0804, 0).unwrap();
        assert!(esp_soc::SocBus::usb_link(&mut b).unwrap().port().connected);
    }

    /// Core 1 sees its own CLIC block at 0x20800000 and core 0's at 0x20810000.
    #[test]
    fn the_clic_window_is_per_core() {
        let mut b = SocBus::new(0x1000, [0; 6], (1, 0));
        b.cur_core = 1;
        b.write32(0x2080_0008, 0x7f << 24).unwrap();
        b.write8(0x2081_1000 + 4 * 20 + 1, 1).unwrap();
        assert_eq!((b.periph.clic.core[1].thresh, b.periph.clic.core[0].thresh), (0x7f << 24, 0));
        assert!(b.periph.clic.core[0].enabled(20) && !b.periph.clic.core[1].enabled(20));
        b.cur_core = 0;
        assert_eq!((b.read32(0x2080_0008).unwrap(), b.read32(0x2081_0008).unwrap()), (0, 0x7f << 24));
    }

    /// The analog blocks behind regi2c keep their registers through a chip reset (a CPLL
    /// divider here, block 0x67 register 2).
    #[test]
    fn a_chip_reset_keeps_the_analog_registers() {
        let mut b = SocBus::new(0x1000, [0; 6], (1, 0));
        b.write32(0x5012_4000, 0x67 | 2 << 8 | 1 << 24 | 0x5a << 16).unwrap();
        esp_soc::SocBus::reboot(&mut b, [0; 6]);
        b.write32(0x5012_4000, 0x67 | 2 << 8).unwrap();
        assert_eq!(b.read32(0x5012_4000).unwrap() >> 16 & 0xff, 0x5a);
    }

    #[test]
    fn byte_writes_merge_into_clic_control_words() {
        let mut b = SocBus::new(0x1000, [0; 6], (1, 0));
        let ctrl = 0x2080_1000 + 4 * 20;
        b.write8(ctrl + 1, 1).unwrap();                               // IE, as esprv_intc_int_enable writes it
        b.write8(ctrl + 3, 2 << 5).unwrap();                          // priority 2
        assert_eq!(b.read32(ctrl).unwrap(), 0x40c0_0100);
        assert_eq!(b.read8(ctrl + 3).unwrap(), 0x40);
    }
}
