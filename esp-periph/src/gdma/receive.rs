use super::{DmaDesc, GdmaInCh};
use emu_core::{Bus, Fault};
pub const GDMA_DESCRIPTOR_STEP_BUDGET: usize = 4096;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DmaDescriptorWord {
    Control,
    Buffer,
    Next,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DmaDescriptorFault {
    Read { descriptor: u32, word: DmaDescriptorWord, fault: Fault },
    BufferRead { descriptor: u32, address: u32, fault: Fault },
    Writeback { descriptor: u32, fault: Fault },
    NotOwned { descriptor: u32 },
    Cycle { descriptor: u32 },
    StepBudgetExceeded { budget: usize },
    PayloadTooShort { expected: usize, actual: usize },
}

/// The one GDMA descriptor reader shared by every DMA user (I2S RX/TX, LCD_CAM, SPI2, crypto).
/// Bound work even when guest descriptors make no progress. Streaming rings are valid: the
/// bound applies to one pump call, not the lifetime of an I2S or LCD channel. Each read
/// charges one step; reads past `budget` fail with `StepBudgetExceeded` without touching memory.
pub struct DescriptorWalk { pub remaining: usize, budget: usize }
impl DescriptorWalk {
    pub fn new(budget: usize) -> Self { Self { remaining: budget, budget } }
    /// Read a descriptor through the guest bus (DMA accesses are unpriced).
    pub fn read(&mut self, bus: &mut impl Bus, addr: u32) -> Result<(u32, DmaDesc), DmaDescriptorFault> {
        if self.remaining == 0 { return Err(DmaDescriptorFault::StepBudgetExceeded { budget: self.budget }); }
        self.remaining -= 1;
        try_dma_desc(bus, addr)
    }
    /// Read a descriptor from any word source, for callers that hold DMA memory outside a `Bus`.
    /// Same budget, decoding and faults as `read`; returns DW0 unmodified for write-back.
    pub fn read_with(&mut self, addr: u32, mut word: impl FnMut(u32) -> Result<u32, Fault>) -> Result<(u32, DmaDesc), DmaDescriptorFault> {
        if self.remaining == 0 { return Err(DmaDescriptorFault::StepBudgetExceeded { budget: self.budget }); }
        self.remaining -= 1;
        let mut word = |offset, kind| word(addr.wrapping_add(offset))
            .map_err(|fault| DmaDescriptorFault::Read { descriptor: addr, word: kind, fault });
        let dw0 = word(0, DmaDescriptorWord::Control)?;
        let (buf, next) = (word(4, DmaDescriptorWord::Buffer)?, word(8, DmaDescriptorWord::Next)?);
        Ok((dw0, DmaDesc::decode(addr, dw0, buf, next)))
    }
}

/// Read a descriptor through the same decoder as the infallible host-memory API.
fn try_dma_desc(bus: &mut impl Bus, addr: u32) -> Result<(u32, DmaDesc), DmaDescriptorFault> {
    let mut word = |offset, word| bus.read32_unpriced(addr.wrapping_add(offset))
        .map_err(|fault| DmaDescriptorFault::Read { descriptor: addr, word, fault });
    let dw0 = word(0, DmaDescriptorWord::Control)?;
    let (buf, next) = (word(4, DmaDescriptorWord::Buffer)?, word(8, DmaDescriptorWord::Next)?);
    Ok((dw0, DmaDesc::decode(addr, dw0, buf, next)))
}

impl GdmaInCh {
    pub fn fail_receive(&mut self, irq_changed: &mut bool) {
        let before = self.int_raw;
        self.running = false;
        self.int_raw |= 1 << 3; // IN_DSCR_ERR
        *irq_changed |= self.int_raw != before;
    }

    /// Scatter input and publish both completion and descriptor-error interrupt changes.
    pub fn receive(&mut self, bus: &mut impl Bus, data: &[u8], eof_bytes: Option<u32>, finish: bool,
        is_periph: impl Fn(u32) -> bool, irq_changed: &mut bool) -> bool {
        self.receive_counted(bus, data, eof_bytes, finish, is_periph, irq_changed).0
    }

    /// As `receive`, also returning payload bytes actually written, including a prefix
    /// before a buffer/descriptor fault. Descriptor write-backs are not payload bytes.
    pub fn receive_counted(&mut self, bus: &mut impl Bus, data: &[u8], eof_bytes: Option<u32>, finish: bool,
        is_periph: impl Fn(u32) -> bool, irq_changed: &mut bool) -> (bool, usize) {
        let before = self.int_raw;
        let mut written = 0;
        let result = self.scatter_counted(bus, data, eof_bytes, finish, is_periph, &mut written);
        if !result { self.fail_receive(irq_changed); }
        *irq_changed |= self.int_raw != before;
        (result, written)
    }

    /// Hand a filled or EOF-ended IN descriptor back to the CPU (its length, owner, SUC_EOF) and
    /// move the channel to the next one. False when the write-back faults.
    pub fn close(&mut self, bus: &mut impl Bus, dw0: u32, next: u32, eof: bool) -> bool {
        let v = (dw0 & !(0xfff << 12) & !(3 << 30)) | (self.buf_pos << 12) | if eof { 1 << 30 } else { 0 };
        if bus.write32_unpriced(self.desc, v).is_err() { return false; }
        self.int_raw |= 1 << 0;                                                  // IN_DONE
        if eof { self.int_raw |= 1 << 1; self.eof_desc = self.desc; self.rx_eof_pos = 0; }                  // IN_SUC_EOF
        self.desc = next; self.buf_pos = 0;
        true
    }

    /// Shared camera/crypto receive path. A malformed destination never reports successful EOF.
    pub fn scatter(&mut self, bus: &mut impl Bus, data: &[u8], eof_bytes: Option<u32>, finish: bool, is_periph: impl Fn(u32) -> bool) -> bool {
        self.scatter_counted(bus, data, eof_bytes, finish, is_periph, &mut 0)
    }

    fn scatter_counted(&mut self, bus: &mut impl Bus, data: &[u8], eof_bytes: Option<u32>, finish: bool,
        is_periph: impl Fn(u32) -> bool, written: &mut usize) -> bool {
        let mut pos = 0usize;
        let mut walk = DescriptorWalk::new(GDMA_DESCRIPTOR_STEP_BUDGET);
        while pos < data.len() || (finish && self.rx_eof_pos != 0) {
            let mut r = *self;
            // IDF v5.5.4 soc/gdma_reg.h: IN_DSCR_EMPTY bit 4 means data remains
            // but there is no more inlink; CHECK_OWNER is bit 12.
            if r.desc == 0 {
                // Belt-and-braces: a live receive with no descriptor must still have data left.
                if pos < data.len() { self.int_raw |= 1 << 4; }
                return false;
            }
            let Ok((control, d)) = walk.read(bus, r.desc) else { return false };
            if (r.conf1 & (1 << 12) != 0 && !d.owner_dma) || d.size == 0 || r.buf_pos > d.size { return false; }
            let Some(until_eof) = eof_bytes.unwrap_or(u32::MAX).checked_sub(r.rx_eof_pos).filter(|n| *n != 0) else { return false };
            let n = (d.size - r.buf_pos).min(until_eof) as usize;
            let n = n.min(data.len() - pos);
            let Some(dest) = d.buf.checked_add(r.buf_pos) else { return false };
            let Some(end) = dest.checked_add(n as u32) else { return false };
            // DMA buffers are memory, never MMIO.
            if n != 0 && (is_periph(dest) || is_periph(end - 1)) { return false; }
            let mut i = 0;
            while i < n && (dest + i as u32) & 3 != 0 {
                if bus.write8_unpriced(dest + i as u32, data[pos + i]).is_err() { return false; }
                i += 1; *written += 1;
            }
            while i + 4 <= n {
                let word = u32::from_le_bytes(data[pos + i..pos + i + 4].try_into().unwrap());
                if bus.write32_unpriced(dest + i as u32, word).is_err() { return false; }
                i += 4; *written += 4;
            }
            while i < n {
                if bus.write8_unpriced(dest + i as u32, data[pos + i]).is_err() { return false; }
                i += 1; *written += 1;
            }
            pos += n;
            r.buf_pos += n as u32;
            r.rx_eof_pos += n as u32;
            let eof = eof_bytes == Some(r.rx_eof_pos) || finish;
            // Keep a final full descriptor pending in VS_EOF mode until VSYNC marks it.
            if (eof || (r.buf_pos == d.size && (eof_bytes.is_some() || pos < data.len())))
                && !r.close(bus, control, d.next, eof) { return false; }
            if r.desc == 0 { r.running = false; }
            *self = r;
        }
        true
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counted_receive_keeps_exact_payload_prefix_on_fault_and_excludes_writeback() {
        use emu_core::FlatRam;
        // End the destination after each possible unaligned byte/word prefix.
        for start in 0..4 {
            for available in 0..12 {
                let mut ram=FlatRam::new(0,0x200+start+available);
                ram.write32(0x100,(1<<31)|16).unwrap();
                ram.write32(0x104,0x200+start as u32).unwrap();
                ram.write32(0x108,0).unwrap();
                let mut channel=GdmaInCh{desc:0x100,running:true,..Default::default()};
                let mut irq=false;let payload=[0xabu8;16];
                let (ok,written)=channel.receive_counted(&mut ram,&payload,Some(16),false,|_|false,&mut irq);
                assert!(!ok);assert!(!channel.running);assert!(irq);
                let actual=ram.mem[0x200+start..].iter().take_while(|&&v|v==0xab).count();
                assert_eq!(written,actual,"start {start}, available {available}");
                assert_eq!(channel.int_raw&2,0,"a payload fault is not EOF");
            }
        }
        let mut ram=FlatRam::new(0,0x220);
        for (off,word) in [(0x100,(1<<31)|8),(0x104,0x200),(0x108,0)] {ram.write32(off,word).unwrap();}
        let mut channel=GdmaInCh{desc:0x100,running:true,..Default::default()};
        assert_eq!(channel.receive_counted(&mut ram,&[1;8],Some(8),false,|_|false,&mut false),(true,8));
        assert_eq!(ram.read32(0x100).unwrap(),8|(8<<12)|(1<<30));
        // A short chain writes its payload prefix, then raises EMPTY/DSCR_ERR.
        channel=GdmaInCh{desc:0x100,running:true,..Default::default()};
        assert_eq!(channel.receive_counted(&mut ram,&[2;12],Some(12),false,|_|false,&mut false),(false,8));
        assert_eq!(&ram.mem[0x200..0x208],&[2;8]);
        channel=GdmaInCh{desc:0x100,running:true,conf1:1<<12,..Default::default()};
        assert_eq!(channel.receive_counted(&mut ram,&[3;8],Some(8),false,|_|false,&mut false),(false,0));
        channel=GdmaInCh{desc:0x100,running:true,..Default::default()};
        assert_eq!(channel.receive_counted(&mut ram,&[3;8],Some(8),false,|a|a>=0x200,&mut false),(false,0));
    }

    #[test]
    fn walk_reads_any_word_source_and_stops_at_budget() {
        // A self-linked descriptor: owner DMA, EOF, size 4, length 2.
        let mem = |a: u32| match a { 0x100 => Ok((1 << 31) | (1 << 30) | (2 << 12) | 4), 0x104 => Ok(0x200), 0x108 => Ok(0x100), _ => Err(Fault::Prohibited) };
        let mut walk = DescriptorWalk::new(2);
        let (dw0, d) = walk.read_with(0x100, mem).unwrap();
        assert_eq!((dw0 >> 30, d.size, d.length, d.buf, d.next, d.eof, d.owner_dma), (3, 4, 2, 0x200, 0x100, true, true));
        assert!(walk.read_with(0x100, mem).is_ok());
        assert_eq!(walk.read_with(0x100, mem).err(), Some(DmaDescriptorFault::StepBudgetExceeded { budget: 2 }));
        let mut walk = DescriptorWalk::new(1);
        assert_eq!(walk.read_with(0x104, mem).err(), Some(DmaDescriptorFault::Read { descriptor: 0x104, word: DmaDescriptorWord::Next, fault: Fault::Prohibited }));
    }
}
