//! S3 GP-SPI master RX through the existing bounded GDMA descriptor walker.
//! IDF v5.5.1 esp32s3 spi_reg.h: DMA_RX_ENA bit 27, RX_EOF_EN bit 21;
//! gdma_channel.h: SPI2 trigger 0, SPI3 trigger 1. spi_master.c uses internal
//! MALLOC_CAP_DMA bounce buffers for non-DMA-capable RX memory on this target.
use super::*;

// Do not let descriptor or buffer addresses recurse into peripheral MMIO. This
// subset accepts internal SRAM, not cache/PSRAM DMA or segmented/slave transfers.
struct Memory<'a>(&'a mut SocBus);
impl Memory<'_> {
    fn check(addr:u32,len:u32)->Result<(),Fault> {
        if addr>=DRAM_LOW && addr.checked_add(len).is_some_and(|end|end<=DRAM_HIGH) {Ok(())} else {Err(Fault::Unmapped)}
    }
}
impl Bus for Memory<'_> {
    fn read8(&mut self,a:u32)->Result<u8,Fault> {Self::check(a,1)?;self.0.read8_unpriced(a)}
    fn read16(&mut self,a:u32)->Result<u16,Fault> {Self::check(a,2)?;self.0.read16_unpriced(a)}
    fn read32(&mut self,a:u32)->Result<u32,Fault> {Self::check(a,4)?;self.0.read32_unpriced(a)}
    fn write8(&mut self,a:u32,v:u8)->Result<(),Fault> {Self::check(a,1)?;self.0.write8_unpriced(a,v)}
    fn write16(&mut self,a:u32,v:u16)->Result<(),Fault> {Self::check(a,2)?;self.0.write16_unpriced(a,v)}
    fn write32(&mut self,a:u32,v:u32)->Result<(),Fault> {Self::check(a,4)?;self.0.write32_unpriced(a,v)}
    fn fetch(&mut self,_:u32)->Result<[u8;4],Fault> {Err(Fault::Prohibited)}
    fn note_code_page(&mut self,page:u32) {self.0.note_code_page(page);}
}

impl SocBus {
    #[cold]
    #[inline(never)]
    pub(super) fn complete_gpspi_rx(&mut self,host:u8,len:usize,rx:&[u8]) {
        let Some(ch)=self.periph.gdma.in_channel_for(u32::from(host-2)) else {return};
        let mut channel=self.periph.gdma.inp[ch];
        // RX_EOF_EN=0 ends the inlink at TRANS_DONE. Byte-count EOF mode and
        // slave/segmented transactions are outside this master subset.
        if self.periph.gpspi(host).regs.read(0x30)&(1<<21)!=0 {
            channel.fail_receive(&mut self.irq_dirty);
        } else {
            let mut data=vec![0xff;len];let copy=len.min(rx.len());data[..copy].copy_from_slice(&rx[..copy]);
            let mut changed=false;
            channel.receive(&mut Memory(self),&data,Some(len as u32),false,Self::is_periph,&mut changed);
            self.irq_dirty|=changed;
        }
        self.periph.gdma.inp[ch]=channel;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esp_soc::board::BoardModel;
    struct Reply;
    impl BoardModel for Reply {
        fn name(&self)->&'static str {"spi-rx-probe"}
        fn spi_transfer(&mut self,_:u8,_:&[u8],_:usize)->Vec<u8> {(0..100).collect()}
    }
    const DESC:u32=DRAM_LOW+0x200;
    const DATA:u32=DRAM_LOW+0x500;
    fn descriptor(bus:&mut SocBus,addr:u32,size:u32,buffer:u32,next:u32) {
        for (i,word) in [size|(1<<31),buffer,next].into_iter().enumerate() {bus.write32_unpriced(addr+4*i as u32,word).unwrap();}
    }
    fn setup(host:u8)->SocBus {
        let mut bus=SocBus::new(1024,1024,[0;6]);bus.board=Box::new(Reply);
        descriptor(&mut bus,DESC,64,DATA,DESC+12);descriptor(&mut bus,DESC+12,128,DATA+64,0);
        let ch=&mut bus.periph.gdma.inp[2];ch.peri_sel=u32::from(host-2);ch.desc=DESC;ch.running=true;ch.conf1=1<<12;ch.int_ena=0x1f;
        bus
    }
    fn run(bus:&mut SocBus,host:u8,dma:bool) {
        let base=if host==2 {0x60024000}else{0x60025000};
        bus.write32_unpriced(base+0x30,if dma {1<<27}else{0}).unwrap();
        bus.write32_unpriced(base+0x10,1<<28).unwrap();bus.write32_unpriced(base+0x1c,103*8-1).unwrap();bus.write32_unpriced(base,1<<24).unwrap();
    }
    #[test]
    fn both_hosts_write_bounded_miso_and_only_last_descriptor_gets_eof() {
        for host in [2,3] {let mut bus=setup(host);run(&mut bus,host,true);
            for i in 0..103 {assert_eq!(bus.read8_unpriced(DATA+i).unwrap(),if i<100 {i as u8}else{0xff});}
            assert_eq!(bus.read32_unpriced(DESC).unwrap(),64|(64<<12));
            assert_eq!(bus.read32_unpriced(DESC+12).unwrap(),128|(39<<12)|(1<<30));
            let ch=&bus.periph.gdma.inp[2];assert_eq!(ch.int_raw,3);assert_eq!(ch.eof_desc,DESC+12);assert!(!ch.running);
            assert!(bus.periph.gpspi(host).int_raw&(1<<12)!=0);
            assert_eq!(bus.periph.gdma.inp[0].int_raw,0);
        }
    }
    #[test]
    fn fifo_mode_and_wrong_peripheral_do_not_write_rx_memory() {
        for (enabled,trigger) in [(false,1),(true,0)] {let mut bus=setup(3);bus.periph.gdma.inp[2].peri_sel=trigger;run(&mut bus,3,enabled);
            assert_eq!(bus.read32_unpriced(DATA).unwrap(),0);assert_eq!(bus.read32_unpriced(DESC).unwrap(),64|(1<<31));assert_eq!(bus.periph.gdma.inp[2].int_raw,0);
        }
    }
    #[test]
    fn bad_descriptors_stop_without_successful_eof_or_mmio_side_effects() {
        for (desc,buffer,size,owner) in [(0x60025000,DATA,103,true),(DRAM_HIGH-4,DATA,103,true),(DESC,0x60025000,103,true),(DESC,u32::MAX,103,true),(DESC,DRAM_HIGH-1,103,true),(DESC,DATA,103,false),(DESC,DATA,0,true)] {
            let mut bus=setup(3);descriptor(&mut bus,DESC,size,buffer,0);if !owner {bus.write32_unpriced(DESC,size).unwrap();}bus.periph.gdma.inp[2].desc=desc;run(&mut bus,3,true);
            let ch=&bus.periph.gdma.inp[2];assert!(ch.int_raw&(1<<3)!=0);assert_eq!(ch.int_raw&3,0);assert!(!ch.running);assert_eq!(bus.periph.spi3.transfers,1);
        }
    }
    #[test]
    fn unsupported_eof_mode_fails_and_stopped_channel_is_not_restarted() {
        let mut bus=setup(3);bus.periph.spi3.regs.write(0x30,(1<<27)|(1<<21));bus.complete_gpspi_rx(3,103,&[1;103]);
        assert_eq!(bus.periph.gdma.inp[2].int_raw,1<<3);assert!(!bus.periph.gdma.inp[2].running);
        assert_eq!(bus.read32_unpriced(DATA).unwrap(),0);
        bus.periph.gdma.inp[2].int_raw=0;run(&mut bus,3,true);
        assert_eq!(bus.periph.gdma.inp[2].int_raw,0);assert_eq!(bus.read32_unpriced(DATA).unwrap(),0);
    }
    #[test]
    fn exhausted_or_cyclic_chain_is_bounded_and_can_be_restarted() {
        for next in [0,DESC] {let mut bus=setup(3);descriptor(&mut bus,DESC,4,DATA,next);run(&mut bus,3,true);
            assert!(bus.periph.gdma.inp[2].int_raw&(1<<3)!=0);assert_eq!(bus.periph.gdma.inp[2].int_raw&(1<<1),0);
            descriptor(&mut bus,DESC,103,DATA,0);let ch=&mut bus.periph.gdma.inp[2];ch.desc=DESC;ch.running=true;ch.buf_pos=0;ch.rx_eof_pos=0;ch.int_raw=0;
            run(&mut bus,3,true);assert_eq!(bus.periph.gdma.inp[2].int_raw,3);
        }
    }
}
