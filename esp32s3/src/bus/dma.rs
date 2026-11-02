//! GDMA transfer engines and descriptor access.
use super::*;
use std::collections::HashSet;

pub(super) const SPI2_DMA_DESCRIPTOR_STEP_BUDGET: usize = 1024;
use esp_periph::gdma::GDMA_DESCRIPTOR_STEP_BUDGET;
// 4096 RGB pixels plus the RMT end marker.
const RMT_DMA_SYMBOL_LIMIT: usize = 24 * 4096 + 1;

/// Which end of a memory-to-memory copy faulted.
pub(super) enum M2mFault { Source, Destination }

pub use esp_periph::gdma::{DmaDescriptorFault, DmaDescriptorWord};
use esp_periph::gdma::DescriptorWalk;

/// One GP-SPI data phase collected from GDMA (SPI2 or SPI3).
pub(super) struct Spi2DmaCompletion {
    /// The GP-SPI host, 2 or 3.
    host: u8,
    pub(super) channel: usize,
    desc: u32,
    buf_pos: u32,
    running: bool,
    eof_desc: Option<u32>,
    raised_interrupts: u32,
    descriptor_writebacks: Vec<(u32, u32)>,
    payload: Vec<u8>,
}


impl SocBus {
    /// IDF v5.5.4 components/soc/esp32s3/include/soc/gdma_channel.h:21: trigger 9.
    /// Error bit: register/soc/rmt_reg.h:1650-1656. GDMA writeback/owner:
    /// register/soc/gdma_reg.h:585-618; OUT interrupts:649-678.
    /// Descriptor fields: components/hal/include/hal/dma_types.h:23-32.
    /// Stage a finite RMT DMA transaction on register writes. No idle tick pump.
    /// Descriptor reads reuse the existing bounded walker and unpriced bus access.
    /// With CH3 DMA_ACCESS_EN set, CH3 CONF0 and writes to a running trigger-9 GDMA OUT
    /// channel stage the transaction.
    #[cold]
    #[inline(never)]
    pub(super) fn rmt_dma_write(&mut self, a: u32, v: u32) {
        let stage = match a >> 12 {
            0x60016 => a & 0xfff == 0x2c,
            0x6003f => {
                let channel = ((a & 0xfff) / crate::periph::GDMA_CH_STRIDE) as usize;
                let Some(c) = self.periph.gdma.out.get(channel) else { return; };
                let off = (a & 0xfff) % crate::periph::GDMA_CH_STRIDE;
                // IDF v5.5.4 soc/esp32s3/register/soc/gdma_reg.h:
                // OUT_RST (OUT_CONF0 bit 0), OUTLINK_START bit 21, RESTART bit 22.
                // Reset self-clears in GDMA, so inspect the written value. A fresh
                // START owns its new link; only continuation keeps the saved node.
                if c.peri_sel == 9 && ((off == 0x60 && v & 1 != 0) || (off == 0x80 && v & (1 << 21) != 0)) {
                    self.periph.rmt.clear_dma_resume();
                }
                c.running && c.peri_sel == 9
            }
            _ => false,
        };
        if stage { self.stage_rmt_dma(); }
    }
    /// Stage TX3 symbols through the shared OUT gather, one SUC_EOF node at a time.
    /// IDF v5.5.5 components/esp_driver_rmt/src/rmt_tx.c:79-95,693-701 and 1105-1132 feed long
    /// frames through a two-node ring (SUC_EOF on both nodes) refilled from the OUT_EOF callback
    /// and continued with gdma_append (OUT_LINK RESTART). Staging is instantaneous, so it reaches
    /// a node the callback has not refilled yet, where real hardware is still held back by the
    /// RMT FIFO: such a CPU-owned node waits for the next append instead of faulting, and
    /// RESTART resumes after the last staged node.
    fn stage_rmt_dma(&mut self) {
        let Some(ch) = self.periph.gdma.out_channel_for(9) else { return; };
        let resume = self.periph.rmt.dma_resume();
        if resume != 0 { self.periph.gdma.out[ch].desc = resume; }
        let limit = RMT_DMA_SYMBOL_LIMIT.saturating_sub(self.periph.rmt.dma_fifo_len()) * 4;
        // Ask for one byte more than fits: `RmtDmaOut` accepts whole symbols only, so a chain
        // longer than the queue's room fails as a misaligned length instead of being cut.
        let mut bytes = Vec::new();
        match esp_periph::gdma::gather_out(&mut RmtDmaOut { bus: self, channel: ch }, limit + 1, &mut bytes) {
            Ok(next) => {
                self.periph.rmt.dma_fifo().extend(bytes.as_chunks::<4>().0.iter().map(|&word| u32::from_le_bytes(word)));
                self.periph.rmt.set_dma_resume(next);
                let c = &mut self.periph.gdma.out[ch];
                c.desc = next;
                if next == 0 { c.running = false; c.int_raw |= 1 << 3; }   // OUT_TOTAL_EOF
            }
            Err(DmaDescriptorFault::NotOwned { descriptor }) if resume != 0 && descriptor == resume && bytes.is_empty() => {}
            _ => {
                self.fail_dma_out(ch);
                self.periph.rmt.int_raw |= 1 << 28;
                self.periph.rmt.clear_dma_fifo();
            }
        }
        self.irq_dirty = true;
    }

    pub(super) fn complete_spi2_dma(&mut self) {
        if let Some((deadline, _)) = &self.spi2_scheduled {
            if self.cycles < *deadline { return; }
            let (_, completion) = self.spi2_scheduled.take().unwrap();
            self.commit_spi2_dma(completion);
            return;
        }
        if self.periph.spi2.dma_tx_pending.is_some() { self.complete_gpspi_dma(2); }
    }

    /// SPI3's DMA data phase, through the GDMA OUT channel of trigger 1
    /// (SOC_GDMA_TRIG_PERIPH_SPI3). Always committed at once: the experimental wire timing
    /// (`spi2_timing`) is SPI2's only.
    #[inline(always)]
    pub(super) fn complete_spi3_dma(&mut self) { if self.periph.spi3.dma_tx_pending.is_some() { self.complete_gpspi_dma(3); } }

    #[inline(never)]
    fn complete_gpspi_dma(&mut self, host: u8) {
        if self.periph.gpspi(host).dma_tx_pending.is_none() {
            return;
        }
        let peri = u32::from(host - 2);                                 // GDMA trigger: SPI2 0, SPI3 1
        let channel = self.periph.gdma.out_channel_for(peri);
        if channel.is_none() && !self.periph.gdma.out.iter().any(|c| c.peri_sel == peri && c.desc != 0) {
            // A DMA command without a bound channel cannot progress. Abort it just
            // like a descriptor failure so firmware can issue a later CPU command.
            // A stopped channel with a descriptor remains bound and can be restarted.
            self.periph.gpspi_mut(host).fail_dma_tx();
            self.irq_dirty = true;
            return;
        }
        match self.spi2_dma_completion(host) {
            Ok(Some(completion)) => {
                if host == 2 && self.spi2_timing {
                    // Rough wire-time floor, assuming the normal 80 MHz SPI source.
                    // Payload is snapshotted at submission; no progressive SRAM reads yet.
                    let cycles = self.periph.spi2.wire_source_cycles() * (crate::periph::CPU_HZ / 80_000_000);
                    self.spi2_scheduled = Some((self.cycles.saturating_add(cycles), completion));
                    // Collection reads descriptor words through the bus, and one may be another
                    // device's register (mmio-s1 compares only SPI2/GDMA sources; review blocker).
                    self.irq_dirty = true;
                } else {
                    self.commit_spi2_dma(completion);
                }
            }
            Ok(None) => {}
            Err(fault) => {
                if let Some(channel) = channel {
                    self.fail_spi2_dma(host, channel, fault);
                }
            }
        }
    }

    fn commit_spi2_dma(&mut self, completion: Spi2DmaCompletion) {
        for (descriptor, control) in completion.descriptor_writebacks {
            if let Err(fault) = self.write32_unpriced(descriptor, control) {
                self.fail_spi2_dma(completion.host, completion.channel, DmaDescriptorFault::Writeback { descriptor, fault });
                return;
            }
        }
        // Publish only this transfer's effects, not a stale copy of writable registers
        // or interrupt status that software may have cleared while it was on the wire.
        let channel = &mut self.periph.gdma.out[completion.channel];
        channel.desc = completion.desc;
        channel.buf_pos = completion.buf_pos;
        // Descriptor progress publishes as captured, but an OUT_LINK STOP during the
        // wire delay must keep the channel stopped: completion never resurrects it.
        channel.running = channel.running && completion.running;
        if let Some(descriptor) = completion.eof_desc { channel.eof_desc = descriptor; }
        channel.int_raw |= completion.raised_interrupts;
        self.periph.gpspi_mut(completion.host).complete_dma_tx(&completion.payload);
        self.irq_dirty = true;
    }

    fn fail_spi2_dma(&mut self, host: u8, channel: usize, fault: DmaDescriptorFault) {
        if self.periph.gpspi(host).log { eprintln!("[spi{host}] DMA descriptor fault: {fault:?}"); }
        if host == 2 { self.spi2_dma_fault = Some(fault); } else { self.spi3_dma_fault = Some(fault); }
        let gdma = &mut self.periph.gdma.out[channel];
        gdma.running = false;
        gdma.int_raw |= 1 << 2;                                         // OUT_DSCR_ERR
        self.periph.gpspi_mut(host).fail_dma_tx();
        self.irq_dirty = true;
    }

    // EX255: every device flush and register write services both GP-SPI hosts, almost always
    // with nothing queued. Test that inline; the out-of-line body keeps its own checks.
    #[inline(always)]
    pub(super) fn deliver_spi2_transfer(&mut self) { if self.periph.spi2.has_pending_transfer() { self.deliver_gpspi_transfer(2); } }
    /// The same for SPI3 (the board's `host` 3).
    #[inline(always)]
    pub(super) fn deliver_spi3_transfer(&mut self) { if self.periph.spi3.has_pending_transfer() { self.deliver_gpspi_transfer(3); } }

    #[inline(never)]
    fn deliver_gpspi_transfer(&mut self, host: u8) {
        if self.periph.gpspi(host).dma_tx_pending.is_some() {
            return;
        }
        let Some(transfer) = self.periph.gpspi_mut(host).take_transfer() else { return };
        // Chip select and command/data lines are GPIOs. The board must see their preceding edges
        // before it receives the transaction.
        if !self.periph.gpio.changes.is_empty() {
            let changes = &self.periph.gpio.changes;
            if let Some(events) = &mut self.gpio_events {
                for &(pin, level) in changes {
                    events.push((self.cycles, pin, level));
                }
            }
            self.board.gpio_output_at(self.cycles, changes, self.periph.gpio.enable, self.periph.gpio.out);
            self.periph.gpio.changes.clear();
        }
        let pins = if host == 2 { self.spi2_pins.take() } else { self.spi3_pins.take() };
        let rx = if let Some(pins) = pins {
            self.board.spi_transfer_pins(host, pins, &transfer.tx, transfer.rx_len)
        } else {
            self.board.spi_transfer(host, &transfer.tx, transfer.rx_len)
        };
        self.periph.gpspi_mut(host).finish_transfer(transfer, &rx);
    }

    /// Collect one GP-SPI data phase (`host` 2 or 3) and its GDMA
    /// completion without partially committing a malformed descriptor chain.
    fn spi2_dma_completion(&mut self, host: u8) -> Result<Option<Spi2DmaCompletion>, DmaDescriptorFault> {
        let Some(bits) = self.periph.gpspi(host).dma_tx_pending else { return Ok(None) };
        let Some(channel_index) = self.periph.gdma.out_channel_for(u32::from(host - 2)) else { return Ok(None) };
        let wanted = (bits as usize).div_ceil(8);
        let mut payload = Vec::with_capacity(wanted);
        let mut visited = HashSet::new();
        let mut channel = self.periph.gdma.out[channel_index];
        channel.int_raw = 0; // Accumulate only new completion events.
        let mut descriptor_writebacks = Vec::new();
        let mut walk = DescriptorWalk::new(SPI2_DMA_DESCRIPTOR_STEP_BUDGET);
        while payload.len() < wanted {
            let current = channel;
            if !current.running || current.desc == 0 {
                break;
            }
            let (control, descriptor) = walk.read(self, current.desc)?;
            if !visited.insert(current.desc) {
                return Err(DmaDescriptorFault::Cycle { descriptor: current.desc });
            }
            let length = descriptor.length;
            if !descriptor.owner_dma {
                return Err(DmaDescriptorFault::NotOwned { descriptor: current.desc });
            }
            let (buffer, next, eof) = (descriptor.buf, descriptor.next, descriptor.eof);
            let remaining = length.saturating_sub(current.buf_pos) as usize;
            if remaining != 0 {
                let count = remaining.min(wanted - payload.len());
                let address = buffer.wrapping_add(current.buf_pos);
                self.append_mapped_bytes(address, count, &mut payload).map_err(|(address, fault)| DmaDescriptorFault::BufferRead {
                    descriptor: current.desc,
                    address,
                    fault,
                })?;
                channel.buf_pos += remaining as u32;                     // GDMA drains the descriptor
            }
            if channel.conf0 & (1 << 2) != 0 {
                let writable = current.desc.checked_add(4).is_some_and(|end| {
                    self.lookup(current.desc).is_some_and(|entry| entry.writable != 0 && end <= entry.hi)
                });
                if !writable {
                    return Err(DmaDescriptorFault::Writeback { descriptor: current.desc, fault: Fault::Prohibited });
                }
                descriptor_writebacks.push((current.desc, control & !(1 << 31)));
            }
            channel.int_raw |= 1 << 0;
            if eof {
                channel.int_raw |= 1 << 1;
                channel.eof_desc = current.desc;
            }
            if next == 0 {
                channel.running = false;
                channel.desc = 0;
                channel.int_raw |= 1 << 3;
            } else {
                channel.desc = next;
                channel.buf_pos = 0;
            }
            if eof {
                break;
            }
            if payload.len() == wanted {
                break;
            }
        }
        if payload.len() != wanted {
            return Err(DmaDescriptorFault::PayloadTooShort { expected: wanted, actual: payload.len() });
        }
        Ok(Some(Spi2DmaCompletion {
            host, channel: channel_index, desc: channel.desc, buf_pos: channel.buf_pos,
            running: channel.running,
            eof_desc: (channel.int_raw & (1 << 1) != 0).then_some(channel.eof_desc),
            raised_interrupts: channel.int_raw, descriptor_writebacks, payload,
        }))
    }

    /// Append a memory range a mapping at a time. Peripheral and unmapped addresses use the
    /// ordinary byte read so their access semantics and first-fault bookkeeping stay unchanged.
    fn append_mapped_bytes(&mut self, mut address: u32, mut count: usize, out: &mut Vec<u8>) -> Result<(), (u32, Fault)> {
        while count != 0 {
            if !Self::is_periph(address) {
                if let Some(entry) = self.lookup(address) {
                    let take = count.min(entry.hi.wrapping_sub(address) as usize);
                    if take != 0 {
                        let offset = entry.off as usize + address.wrapping_sub(entry.lo) as usize;
                        if let Some(bytes) = self.buf(entry.src as u8).get(offset..offset + take) {
                            out.extend_from_slice(bytes);
                            address = address.wrapping_add(take as u32);
                            count -= take;
                            continue;
                        }
                    }
                }
            }
            match self.read8_unpriced(address) {
                Ok(byte) => out.push(byte),
                Err(fault) => return Err((address, fault)),
            }
            address = address.wrapping_add(1);
            count -= 1;
        }
        Ok(())
    }

    fn dma_i2s_rx(&mut self, cycles: u64, port: u32) {
        let Some(ch) = self.periph.gdma.in_channel_for(3 + port) else { return };
        let signals = esp_periph::i2s::RxSignals { data: if port == 0 { 25 } else { 30 }, clock: if port == 0 { [26, 27] } else { [31, 32] }, input_select_bit: 7, output_mask: 0x3ff };
        let mut bank = self.periph.i2s0.take_pcm_bank();
        let i2s = if port == 0 { &mut self.periph.i2s0 } else { &mut self.periph.i2s1 };
        let bytes = i2s.receive(cycles, self.cycles, port == 0, &self.periph.gpio, signals, bank.as_deref_mut());
        let eof = i2s.read(0x64);
        self.periph.i2s0.restore_pcm_bank(bank);
        let _ = self.scatter_dma_in(ch, &bytes, Some(eof), false);
        let i2s = if port == 0 { &mut self.periph.i2s0 } else { &mut self.periph.i2s1 };
        i2s.recycle_rx_buffer(bytes);
    }

    /// Move I2S TX data out of, and RX data into, DMA descriptors at the sample rate. One cached
    /// flag per controller gates both directions, so an idle controller costs what TX alone did.
    pub(super) fn dma_i2s_step(&mut self, cycles: u64) {
        if self.periph.i2s0.poll_active() { self.dma_i2s_port(cycles, 0); }
        if self.periph.i2s1.poll_active() { self.dma_i2s_port(cycles, 1); }
    }

    #[inline(never)]
    fn dma_i2s_port(&mut self, cycles: u64, which: usize) {
        self.dma_i2s_one(cycles, which);
        let i2s = if which == 0 { &self.periph.i2s0 } else { &self.periph.i2s1 };
        if i2s.rx_running() { self.dma_i2s_rx(cycles, which as u32); }
    }

    /// Move I2S TX data for controller `which` (0 = I2S0 on GDMA trigger 3, 1 = I2S1 on trigger 4).
    fn dma_i2s_one(&mut self, cycles: u64, which: usize) {
        let (frames, bpf, sample_bytes) = { let i2s = if which == 0 { &mut self.periph.i2s0 } else { &mut self.periph.i2s1 }; (i2s.frames_due(cycles), i2s.bytes_per_frame as usize, i2s.sample_bytes()) };
        if frames == 0 || bpf == 0 { return; }
        let Some(ch) = self.periph.gdma.out_channel_for(if which == 0 { 3 } else { 4 }) else { return };
        let mut need = frames as usize * bpf;
        let mut samples: Vec<i16> = Vec::new();
        let mut walk = DescriptorWalk::new(GDMA_DESCRIPTOR_STEP_BUDGET);
        'transfer: while need > 0 {
            let c = self.periph.gdma.out[ch];
            if !c.running || c.desc == 0 { break; }
            let Ok((_, d)) = walk.read(self, c.desc) else { self.fail_dma_out(ch); break };
            let remaining = d.length.saturating_sub(c.buf_pos) as usize;
            if remaining == 0 {
                // descriptor complete: hand back to software, raise EOF/DONE, advance
                let ch_ref = &mut self.periph.gdma.out[ch];
                if ch_ref.conf0 & (1 << 2) != 0 { let dw0 = self.read32_unpriced(d.addr).unwrap_or(0) & !(1 << 31); let _ = self.write32_unpriced(d.addr, dw0); }   // AUTO_WRBACK: owner -> cpu
                let ch_ref = &mut self.periph.gdma.out[ch];
                self.irq_dirty = true;
                ch_ref.int_raw |= 1 << 0;                                                     // OUT_DONE
                if d.eof { ch_ref.int_raw |= 1 << 1; ch_ref.eof_desc = d.addr; }             // OUT_EOF
                if d.next == 0 { ch_ref.running = false; ch_ref.desc = 0; ch_ref.int_raw |= 1 << 3; break; }   // OUT_TOTAL_EOF
                ch_ref.desc = d.next; ch_ref.buf_pos = 0;
                continue;
            }
            let take = remaining.min(need);
            let start = d.buf.wrapping_add(c.buf_pos);
            // Keep the first DMA channel, scaling its signed PCM sample to the 16-bit host sink.
            let mut i = 0usize;
            while i + bpf <= take {
                let addr = start.wrapping_add(i as u32);
                let sample = if sample_bytes == 1 {
                    self.read8_unpriced(addr).map(|v| (v as i8 as i16) << 8)
                } else {
                    self.read16_unpriced(addr.wrapping_add((sample_bytes - 2) as u32)).map(|v| v as i16)
                };
                let Ok(sample) = sample else { self.fail_dma_out(ch); break 'transfer };
                samples.push(sample);
                i += bpf;
            }
            self.periph.gdma.out[ch].buf_pos += take as u32;
            need -= take;
        }
        if !samples.is_empty() { let i2s = if which == 0 { &mut self.periph.i2s0 } else { &mut self.periph.i2s1 }; i2s.frames_out += samples.len() as u64; i2s.pcm.extend_from_slice(&samples); }
    }

    fn fail_dma_out(&mut self, ch: usize) {
        self.periph.gdma.out[ch].running = false;
        self.periph.gdma.out[ch].int_raw |= 1 << 2; // OUT_DSCR_ERR
        self.irq_dirty = true;
    }

    /// Copy `n` guest bytes for the memory-to-memory engine, a word at a time where both ends are aligned.
    pub(super) fn dma_copy(&mut self, src: u32, dst: u32, n: u32) -> Result<(), M2mFault> {
        // EX170: whole mapping runs with one memmove while both ends are plain host-backed memory.
        // Anything else (MMIO, unmapped, read-only destination, forward-overlapping ranges) leaves
        // the rest to the word loop below, which continues from `i` with its own fault position.
        let mut i = self.dma_copy_runs(src, dst, n);
        if i == n { return Ok(()); }
        if (src | dst) & 3 == 0 {
            while i + 4 <= n {
                let v = self.read32_unpriced(src.wrapping_add(i)).map_err(|_| M2mFault::Source)?;
                self.write32_unpriced(dst.wrapping_add(i), v).map_err(|_| M2mFault::Destination)?;
                i += 4;
            }
        }
        while i < n {
            let v = self.read8_unpriced(src.wrapping_add(i)).map_err(|_| M2mFault::Source)?;
            self.write8_unpriced(dst.wrapping_add(i), v).map_err(|_| M2mFault::Destination)?;
            i += 1;
        }
        Ok(())
    }

    /// The plain per-word copy that `dma_copy` must stay indistinguishable from.
    #[cfg(test)]
    pub(super) fn dma_copy_reference(&mut self, src: u32, dst: u32, n: u32) -> Result<(), M2mFault> {
        let mut i = 0u32;
        if (src | dst) & 3 == 0 {
            while i + 4 <= n {
                let v = self.read32_unpriced(src.wrapping_add(i)).map_err(|_| M2mFault::Source)?;
                self.write32_unpriced(dst.wrapping_add(i), v).map_err(|_| M2mFault::Destination)?;
                i += 4;
            }
        }
        while i < n {
            let v = self.read8_unpriced(src.wrapping_add(i)).map_err(|_| M2mFault::Source)?;
            self.write8_unpriced(dst.wrapping_add(i), v).map_err(|_| M2mFault::Destination)?;
            i += 1;
        }
        Ok(())
    }

    /// Copy the leading part of an M2M span a mapping run at a time and return how many bytes were
    /// done. A run is taken only when the word loop would have produced the same bytes: both ends
    /// inside one mapping each, destination writable, and the host ranges either disjoint or with
    /// the destination below the source (where a forward copy equals memmove). Page versions get
    /// exactly the bumps the per-word (aligned) or per-byte writes would have made.
    fn dma_copy_runs(&mut self, src: u32, dst: u32, n: u32) -> u32 {
        let words = (src | dst) & 3 == 0;
        let mut i = 0u32;
        while i < n {
            let (s, d) = (src.wrapping_add(i), dst.wrapping_add(i));
            if Self::is_periph(s) || Self::is_periph(d) { break; }
            let Some(se) = self.lookup(s) else { break };
            let Some(de) = self.lookup(d) else { break };
            if de.writable == 0 { break; }
            let mut take = (n - i).min(se.hi - s).min(de.hi - d);
            // The word loop hands the unaligned tail to byte writes, which bump differently: keep
            // runs word-sized here and let the last 1..3 bytes go through the byte loop below.
            if words { take &= !3; }
            if take == 0 { break; }
            let (so, dof) = (se.off as usize + (s - se.lo) as usize, de.off as usize + (d - de.lo) as usize);
            let len = take as usize;
            if so + len > self.buf(se.src as u8).len() || dof + len > self.buf(de.src as u8).len() { break; }
            if se.src == de.src && dof > so && dof < so + len { break; }      // forward copy would re-read its own output
            let sp = self.buf(se.src as u8).as_ptr();
            let dp = self.buf_mut(de.src as u8).as_mut_ptr();
            // SAFETY: both ranges were bounds-checked against their buffers above; `copy` is memmove.
            unsafe { std::ptr::copy(sp.add(so), dp.add(dof), len); }
            // EX110: the per-word reference path bumps through `write*_access`, which skips a
            // destination no decoded code depends on; the run copy must skip exactly the same.
            if de.code != 0 { self.bump_run(de.vbase, (d - de.lo) as usize, len, words); }
            i += take;
        }
        i
    }

    /// The page-version bumps of `len` bytes written at `off` as aligned words or as single bytes:
    /// one per write on the write's page, and one on the previous page for each write that starts
    /// in the first three bytes of a page (see `bump`).
    fn bump_run(&mut self, vbase: u32, off: usize, len: usize, words: bool) {
        const SHIFT: usize = xtensa_lx7::bus::VPAGE_SHIFT as usize;
        const SIZE: usize = 1 << SHIFT;
        let (mut at, end) = (off, off + len);
        while at < end {
            let page_end = ((at >> SHIFT) + 1) << SHIFT;
            let stop = end.min(page_end);
            let p = vbase as usize + (at >> SHIFT);
            let in_page = at & (SIZE - 1);
            let (writes, early) = if words { ((stop - at) / 4, usize::from(in_page == 0)) }
                                  else { (stop - at, (emu_core::bus::PREV_PAGE_BYTES as usize).saturating_sub(in_page).min(stop - at)) };
            self.page_ver[p] = self.page_ver[p].wrapping_add(writes as u32);
            if early != 0 && p > 0 { self.page_ver[p - 1] = self.page_ver[p - 1].wrapping_add(early as u32); }
            self.touched(p.saturating_sub(1), p);
            at = stop;
        }
    }

    /// Memory-to-memory GDMA: a channel pair whose IN side has MEM_TRANS_EN set copies its OUT
    /// descriptor chain into its IN chain — the transaction-based `esp_async_memcpy` of IDF v5.4,
    /// which starts both channels for each copy. The copy lands in one scheduling round, no
    /// transfer timing, and the descriptors are written back the way the engine does it: the IN
    /// side gets each buffer's length, owner back to the CPU and SUC_EOF where the OUT chain's
    /// EOF fell, so the driver's EOF callback finds its transaction through IN_SUC_EOF_DES_ADDR.
    ///
    /// A descriptor the CPU still owns parks that side with its DSCR_ERR raised until software
    /// hands it over, and the copy resumes where it stopped. A fault reading a descriptor,
    /// copying or writing back stops that side with DSCR_ERR and writes nothing more back; so
    /// does an unproductive walk longer than `GDMA_DESCRIPTOR_STEP_BUDGET`. Productive copies
    /// yield at the budget and resume from their descriptor positions on the next pump.
    /// Interrupt inputs are marked for re-evaluation only when a channel's state changed.
    pub(super) fn dma_m2m_step(&mut self) {
        use crate::periph::{GdmaInCh, GdmaOutCh};
        const OUT_DONE: u32 = 1 << 0;
        const OUT_EOF: u32 = 1 << 1;
        const OUT_DSCR_ERR: u32 = 1 << 2;
        const OUT_TOTAL_EOF: u32 = 1 << 3;
        const IN_DSCR_ERR: u32 = 1 << 3;
        const IN_DSCR_EMPTY: u32 = 1 << 4;
        const AUTO_WRBACK: u32 = 1 << 2;
        const MEM_TRANS_EN: u32 = 1 << 4;
        let out_state = |c: &GdmaOutCh| (c.int_raw, c.desc, c.buf_pos, c.running, c.eof_desc);
        let in_state = |c: &GdmaInCh| (c.int_raw, c.desc, c.buf_pos, c.running, c.eof_desc);
        for ch in 0..crate::periph::GDMA_CHANNELS {
            let (mut r, mut o) = (self.periph.gdma.inp[ch], self.periph.gdma.out[ch]);
            if !(r.running && o.running && r.conf0 & MEM_TRANS_EN != 0 && r.desc != 0 && o.desc != 0) { continue; }
            let (in_before, out_before) = (in_state(&r), out_state(&o));
            let mut copied = false;
            let mut walk = DescriptorWalk::new(GDMA_DESCRIPTOR_STEP_BUDGET);
            loop {
                // Resume a legal long copy on the next pump instead of faulting at the work budget.
                if walk.remaining < 2 {
                    if !copied { o.int_raw |= OUT_DSCR_ERR; o.running = false; }
                    break;
                }
                let Ok((out_dw0, od)) = walk.read(self, o.desc) else { o.int_raw |= OUT_DSCR_ERR; o.running = false; break };
                if !od.owner_dma { o.int_raw |= OUT_DSCR_ERR; break; }                 // parked until software hands it over
                let remaining = od.length.saturating_sub(o.buf_pos);
                if remaining > 0 || (od.eof && r.desc != 0) {
                    if r.desc == 0 { r.int_raw |= IN_DSCR_EMPTY; break; }
                    let Ok((in_dw0, id)) = walk.read(self, r.desc) else { r.int_raw |= IN_DSCR_ERR; r.running = false; break };
                    if !id.owner_dma || (remaining > 0 && id.size <= r.buf_pos) { r.int_raw |= IN_DSCR_ERR; break; }
                    let n = remaining.min(id.size.saturating_sub(r.buf_pos));
                    if n > 0 {
                        match self.dma_copy(od.buf.wrapping_add(o.buf_pos), id.buf.wrapping_add(r.buf_pos), n) {
                            Ok(()) => {}
                            Err(M2mFault::Source) => { o.int_raw |= OUT_DSCR_ERR; o.running = false; break; }
                            Err(M2mFault::Destination) => { r.int_raw |= IN_DSCR_ERR; r.running = false; break; }
                        }
                        copied = true;
                        o.buf_pos += n;
                        r.buf_pos += n;
                    }
                    let eof_now = o.buf_pos == od.length && od.eof;
                    if (r.buf_pos == id.size || eof_now) && !r.close(self, in_dw0, id.next, eof_now) {
                        r.int_raw |= IN_DSCR_ERR; r.running = false; break;
                    }
                    if o.buf_pos < od.length { continue; }                                 // the IN buffer filled first
                }
                if o.conf0 & AUTO_WRBACK != 0 && self.write32_unpriced(od.addr, out_dw0 & !(1 << 31)).is_err() {
                    o.int_raw |= OUT_DSCR_ERR; o.running = false; break;
                }
                o.int_raw |= OUT_DONE;
                if od.eof { o.int_raw |= OUT_EOF; o.eof_desc = od.addr; }
                if od.next == 0 { o.int_raw |= OUT_TOTAL_EOF; o.running = false; o.desc = 0; o.buf_pos = 0; break; }
                o.desc = od.next;
                o.buf_pos = 0;
            }
            if r.desc == 0 { r.running = false; }
            let changed = in_state(&r) != in_before || out_state(&o) != out_before;
            self.periph.gdma.inp[ch] = r;
            self.periph.gdma.out[ch] = o;
            self.irq_dirty |= changed;
        }
    }

    /// Sensor VSYNC precedes capture, allowing the driver to arm GDMA during blanking.
    /// CAM_VSYNC_INT bit 2, CAM_START bit 29 and CAM_VS_EOF_EN bit 8:
    /// ESP-IDF v5.5.4 components/soc/esp32s3/register/soc/lcd_cam_reg.h,
    /// unchanged in v4.4.8 components/soc/esp32s3/include/soc/lcd_cam_reg.h.
    pub(super) fn dma_cam_step(&mut self, cycles: u64) {
        if !self.periph.lcd_cam.cam_clock_active() { return; }
        let before = self.periph.lcd_cam.acc;
        if self.periph.lcd_cam.frame_due(cycles) {
            if self.periph.lcd_cam.cam_ctrl() & (1 << 8) != 0 {
                if let Some(ch) = self.periph.gdma.in_channel_for(5) {
                    self.camera_dma_in(ch, &[], None, true);
                }
            }
            let frame = if self.periph.lcd_cam.cam_active() { self.board.camera_frame().map(|(_, _, frame)| frame) } else { None };
            let cam = &mut self.periph.lcd_cam;
            cam.set_cam_frame(frame);
            cam.frame_dropped = false;
            if cam.cam_frame().is_some() {
                cam.int_raw |= 1 << 2;
                cam.frames += 1;
                self.irq_dirty = true;
            }
            return;
        }
        let cam = &mut self.periph.lcd_cam;
        if !cam.running() { return; }
        let Some(frame) = cam.cam_frame().cloned() else { return };
        // Approximate DVP: 5% blanking, then half a period of active pixels.
        let position = |phase: u64| ((phase.saturating_sub(cam.frame_start.saturating_add(cam.frame_cycles / 20)) as u128 * frame.len() as u128 * 2) / cam.frame_cycles.max(1) as u128).min(frame.len() as u128) as usize;
        let end = position(cam.acc);
        // Belt-and-braces: u128 progress is monotonic, so start already cannot exceed end.
        let start = position(before).min(end);
        if start == end { return; }
        // IDF v5.5.4 soc/lcd_cam_reg.h: BIT_ORDER bit 6, 2BYTE_EN bit 24,
        // VH_DE_MODE_EN bit 28, REC_DATA_BYTELEN bits 0..15 (encoded length - 1).
        // These bits also match IDF v4.4.8 soc/lcd_cam_reg.h.
        let ch = self.periph.gdma.in_channel_for(5);
        if cam.cam_ctrl1 & ((1 << 24) | (1 << 28)) != 0 || ch.is_none() {
            if !cam.frame_dropped { cam.dropped += 1; cam.frame_dropped = true; }
            return;
        }
        let eof = (cam.cam_ctrl() & (1 << 8) == 0).then_some((cam.cam_ctrl1 & 0xffff) + 1);
        let reversed;
        let bytes = if cam.cam_ctrl() & (1 << 6) != 0 {
            reversed = frame[start..end].iter().map(|b| b.reverse_bits()).collect::<Vec<_>>();
            &reversed[..]
        } else { &frame[start..end] };
        self.camera_dma_in(ch.unwrap(), bytes, eof, false);
    }

    fn camera_dma_in(&mut self, ch: usize, bytes: &[u8], eof: Option<u32>, finish: bool) {
        let mut channel = self.periph.gdma.inp[ch];
        if !channel.scatter(self, bytes, eof, finish, Self::is_periph) {
            if channel.desc != 0 { channel.fail_receive(&mut self.irq_dirty); }
            channel.running = false;
        }
        self.periph.gdma.inp[ch] = channel;
        self.irq_dirty = true;
    }

    /// LCD RGB output: consume the GDMA out-channel bound to LCD (trigger 5) at the panel's pixel rate,
    /// assemble frames, publish each completed frame to the board and raise LCD_VSYNC.
    /// LCD RGB output. The LCD engine's async FIFO (16 words) is kept full ahead of the pixel clock,
    /// so a DMA link restart mid-frame (the RGB driver skips LCD_FIFO_PRESERVE_SIZE_PX pixels then)
    /// behaves as on silicon. Frames are published to the board and raise LCD_VSYNC.
    pub(super) fn dma_lcd_step(&mut self, cycles: u64) {
        if !self.periph.lcd_cam.lcd_running() { return; }
        let (ha, va, bpp, frame_cycles) = self.periph.lcd_cam.lcd_geometry();
        let frame_bytes = (ha * va * bpp) as usize;
        if frame_bytes == 0 { return; }
        const FIFO_BYTES: usize = 17 * 2;
        self.periph.lcd_cam.lcd_acc += cycles;
        let due = (self.periph.lcd_cam.lcd_acc as u128 * frame_bytes as u128 / frame_cycles as u128) as usize;
        if due < 512 { return; }
        self.periph.lcd_cam.lcd_acc = 0;
        let log = self.periph.lcd_cam.lcd_log;
        // 1) top the FIFO up from DMA so that it holds `due` + lookahead bytes
        if let Some(ch) = self.periph.gdma.out_channel_for(5) {
            let mut want = (due + FIFO_BYTES).saturating_sub(self.periph.lcd_cam.lcd_fifo.len());
            let mut walk = DescriptorWalk::new(GDMA_DESCRIPTOR_STEP_BUDGET);
            while want > 0 {
                let c = self.periph.gdma.out[ch];
                if !c.running || c.desc == 0 { break; }
                let Ok((_, d)) = walk.read(self, c.desc) else { self.fail_dma_out(ch); break };
                let (length, eof, buf, next) = (d.length, d.eof, d.buf, d.next);
                let remaining = length.saturating_sub(c.buf_pos) as usize;
                if remaining == 0 {
                    if log { eprintln!("[lcd] desc {:#010x} done (buf {:#010x} len {} eof {}) -> next {:#010x}", c.desc, buf, length, eof, next); }
                    let ch_ref = &mut self.periph.gdma.out[ch];
                    self.irq_dirty = true;
                    ch_ref.int_raw |= 1 << 0;
                    if eof { ch_ref.int_raw |= 1 << 1; ch_ref.eof_desc = c.desc; }
                    if next == 0 { ch_ref.running = false; ch_ref.desc = 0; ch_ref.int_raw |= 1 << 3; break; }
                    ch_ref.desc = next; ch_ref.buf_pos = 0;
                    continue;
                }
                let take = remaining.min(want);
                let mut bytes = Vec::with_capacity(take);
                if self.append_mapped_bytes(buf.wrapping_add(c.buf_pos), take, &mut bytes).is_err() {
                    self.fail_dma_out(ch); break;
                }
                self.periph.lcd_cam.lcd_fifo.extend(bytes);
                self.periph.gdma.out[ch].buf_pos += take as u32;
                want -= take;
            }
        }
        // 2) the panel consumes `due` bytes from the FIFO
        let n = due.min(self.periph.lcd_cam.lcd_fifo.len());
        for _ in 0..n { let b = self.periph.lcd_cam.lcd_fifo.pop_front().unwrap(); self.periph.lcd_cam.lcd_line.push(b); }
        while self.periph.lcd_cam.lcd_line.len() >= frame_bytes {
            let frame = std::mem::take(&mut self.periph.lcd_cam.lcd_line);
            self.board.lcd_frame(ha, va, &frame[..frame_bytes]);
            if frame.len() > frame_bytes { self.periph.lcd_cam.lcd_line.extend_from_slice(&frame[frame_bytes..]); }
            self.periph.lcd_cam.lcd_frames += 1;
            self.periph.lcd_cam.int_raw |= 1 << 0;                                    // LCD_VSYNC_INT
            self.irq_dirty = true;
        }
    }

    fn scatter_dma_in(&mut self, ch: usize, data: &[u8], eof_bytes: Option<u32>, finish: bool) -> Result<(), ()> {
        let mut channel = self.periph.gdma.inp[ch];
        let mut irq_changed = false;
        let result = channel.receive(self, data, eof_bytes, finish, Self::is_periph, &mut irq_changed);
        self.irq_dirty |= irq_changed;
        self.periph.gdma.inp[ch] = channel;
        result.then_some(()).ok_or(())
    }

    /// Feed SHA from its GDMA out channel (peripheral 7). Main's shape, so the step stays
    /// inlined in `tick_impl` without growing its frame; only the gather is shared.
    pub(super) fn sha_dma_step(&mut self) {
        self.periph.sha.dma_pending = false;
        let want = (self.periph.sha.block_num as usize).saturating_mul(self.periph.sha.block_bytes());
        let Some(ch) = self.periph.gdma.out_channel_for(7) else { self.periph.sha.busy = false; return };
        // Direct users can set block_num as well as firmware. Reject oversized work before allocating.
        if want > GDMA_DESCRIPTOR_STEP_BUDGET * 4095 {
            self.fail_dma_out(ch); self.periph.sha.busy = false; return;
        }
        let mut input = Vec::new();
        if esp_periph::gdma::gather_out(&mut CryptoDmaOut { bus: self, channel: ch }, want, &mut input).is_err() || input.len() != want {
            self.fail_dma_out(ch); self.periph.sha.busy = false; return;
        }
        self.periph.sha.hash_dma_input(&input);
        self.periph.sha.busy = false;
        self.irq_dirty = true;
    }

    pub(super) fn aes_dma_step(&mut self) {
        self.periph.aes.dma_pending = false;
        let (Some(out_ch), Some(in_ch)) = (self.periph.gdma.out_channel_for(6), self.periph.gdma.in_channel_for(6)) else {
            self.periph.aes.state = 2; self.periph.aes.int_raw |= 1; self.irq_dirty = true; return;
        };
        let mut input = Vec::new();
        if esp_periph::gdma::gather_out(&mut CryptoDmaOut { bus: self, channel: out_ch }, usize::MAX, &mut input).is_err() {
            self.fail_dma_out(out_ch); self.periph.aes.state = 0; return;
        }
        if self.debug.has("aes") {
            eprintln!("[aes] dma block_mode={} num_blocks={} mode={} bytes={}", self.periph.aes.block_mode, self.periph.aes.num_blocks, self.periph.aes.mode, input.len());
        }
        let output = self.periph.aes.transform_blocks(&input);
        if self.scatter_dma_in(in_ch, &output, Some(output.len() as u32), false).is_err() {
            self.periph.aes.state = 0; return;
        }
        self.periph.aes.state = 2;                                              // DONE
        self.periph.aes.int_raw |= 1;
        self.irq_dirty = true;
    }

}

/// The S3 bus as crypto DMA memory: main's unpriced bus accesses, so descriptors and write-backs
/// that land on registers keep their side effects. Descriptor words feed `DescriptorWalk::read_with`.
struct CryptoDmaOut<'a> { bus: &'a mut SocBus, channel: usize }
impl esp_periph::gdma::OutMemory for CryptoDmaOut<'_> {
    fn out_channel(&mut self) -> &mut esp_periph::GdmaOutCh { &mut self.bus.periph.gdma.out[self.channel] }
    fn word(&mut self, address: u32) -> Result<u32, Fault> { self.bus.read32_unpriced(address) }
    fn append(&mut self, address: u32, count: usize, out: &mut Vec<u8>) -> Result<(), (u32, Fault)> { self.bus.append_mapped_bytes(address, count, out) }
    fn writeback(&mut self, address: u32, value: u32) -> Result<(), Fault> { self.bus.write32_unpriced(address, value) }
}

/// RMT DMA memory: symbol buffers are word arrays in RAM. Descriptors in MMIO and misaligned
/// buffers or lengths are rejected; everything else is the crypto adapter's bus access.
struct RmtDmaOut<'a> { bus: &'a mut SocBus, channel: usize }
impl esp_periph::gdma::OutMemory for RmtDmaOut<'_> {
    fn out_channel(&mut self) -> &mut esp_periph::GdmaOutCh { &mut self.bus.periph.gdma.out[self.channel] }
    fn word(&mut self, address: u32) -> Result<u32, Fault> {
        if SocBus::is_periph(address) { return Err(Fault::Prohibited); }
        self.bus.read32_unpriced(address)
    }
    fn append(&mut self, address: u32, count: usize, out: &mut Vec<u8>) -> Result<(), (u32, Fault)> {
        if address & 3 != 0 || count & 3 != 0 { return Err((address, Fault::Misaligned)); }
        self.bus.append_mapped_bytes(address, count, out)
    }
    fn writeback(&mut self, address: u32, value: u32) -> Result<(), Fault> { self.bus.write32_unpriced(address, value) }
}
