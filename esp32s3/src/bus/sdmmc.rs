//! ESP32-S3 native SDMMC wiring and IDMAC, reusing the shared host/card protocol.
//! Sources: IDF v5.5.1 ESP32-S3 sdmmc_struct.h, sdmmc_ll.h, gpio_sig_map.h,
//! system_reg.h; PaperMono factory 1.2.0 hal_tf_card.cpp (e55877dc).
use super::*;
use esp_periph::Device;
use esp_periph::sdmmc::{Sdmmc,SdmmcLayout,SdmmcMemory};

struct Memory<'a>(&'a mut SocBus);
impl Memory<'_> {
    // S3 SDMMC IDMAC uses internal DMA-capable SRAM; firmware must bounce PSRAM.
    // No peripheral recursion, wraparound or out-of-range guest descriptor is allowed.
    fn check(addr:u32,len:usize)->Result<(),Fault> {
        if addr>=DRAM_LOW && u64::from(addr).checked_add(len as u64).is_some_and(|end|end<=u64::from(DRAM_HIGH)) {Ok(())} else {Err(Fault::Unmapped)}
    }
}
impl SdmmcMemory for Memory<'_> {
    fn read(&mut self,addr:u32,buf:&mut [u8])->Result<(),Fault> {Self::check(addr,buf.len())?;for (i,b) in buf.iter_mut().enumerate(){*b=self.0.read8_unpriced(addr+i as u32)?;}Ok(())}
    fn write(&mut self,addr:u32,data:&[u8])->Result<(),Fault> {Self::check(addr,data.len())?;for (i,&b) in data.iter().enumerate(){self.0.write8_unpriced(addr+i as u32,b)?;}Ok(())}
}
impl SocBus {
    #[cold]
    #[inline(never)]
    pub(super) fn sdmmc_write(&mut self,address:u32,value:u32) {
        // SYSTEM.perip_rst_en1.sdio_host_rst, IDF v5.5.1 system_reg.h:580.
        if address==0x600c0024 && value & (1<<7)!=0 {self.periph.sdmmc.reset_registers();}
        if self.periph.sdmmc.pending() {
            let mut host=std::mem::replace(&mut self.periph.sdmmc,Sdmmc::new(SdmmcLayout::Esp32));
            host.now_ns=(u128::from(self.cycles)*1_000_000_000/u128::from(crate::periph::CPU_HZ)) as u64;
            host.route=[None;2];
            if let Some(([clk,cmd,d0,d1,d2,d3],true))=self.board.sd_card_slot() {
                let routes=esp_soc::pins::ChipPins::S3.routes(&self.periph.gpio,&self.periph.io_mux);
                // Slot 0 CMD=178 DATA=180..187; slot 1 CMD=179 DATA=213..220.
                // PaperMono uses GPIO matrix routing, not fixed slot-0 IO_MUX pins.
                for slot in 0..2 {let cmd_sig=178+slot;let data_sig=if slot==0 {180}else{213};
                    let ctype=host.read(0x18);let width=if ctype&(1<<slot)!=0 {4}else{1};
                    let routed=routes.matrix_output(clk as usize,172+slot as u32)
                        && routes.matrix_output(cmd as usize,cmd_sig as u32) && routes.matrix_input(cmd_sig)==Some(cmd)
                        && [d0,d1,d2,d3][..width].iter().enumerate().all(|(i,&p)|routes.matrix_output(p as usize,(data_sig+i) as u32)&&routes.matrix_input(data_sig+i)==Some(p));
                    if routed && self.periph.system.read(0x1c)&(1<<7)!=0 && self.periph.system.read(0x24)&(1<<7)==0 {host.route[slot]=Some(0);}
                }
            }
            host.run(&mut Memory(self));self.periph.sdmmc=host;
        }
        esp_periph::Dispatch::refresh_optional(&mut self.periph,0x28);self.irq_dirty=true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use esp_periph::sdmmc::{reg,des0,int,CMD_START,CMD_RESP,CMD_DATA,CMD_WRITE,CMD_RESP_LONG,CTRL_USE_INTERNAL_DMA,CTRL_INT_ENABLE,BMOD_DE};
    use esp_soc::SocBus as _;
    const BASE:u32=0x60028000;
    fn write(b:&mut SocBus,o:u32,v:u32){b.write32_unpriced(BASE+o,v).unwrap();}
    fn read(b:&mut SocBus,o:u32)->u32 {b.read32_unpriced(BASE+o).unwrap()}
    fn cmd(b:&mut SocBus,n:u8,arg:u32,flags:u32)->u32 {write(b,reg::RINTSTS,u32::MAX);write(b,reg::CMDARG,arg);write(b,reg::CMD,CMD_START|1<<16|u32::from(n)|flags);read(b,reg::RESP0)}
    fn board()->SocBus {
        let mut b=SocBus::new(16<<20,8<<20,[0;6]);b.board=Box::new(crate::board::PaperMono::new());b.attach_board_devices();
        b.attach_sd_card(Box::new(esp_periph::sdmmc::MemStore(vec![0;1<<20])),Default::default()).unwrap();
        wiring(&mut b);b
    }
    fn wiring(b:&mut SocBus) {
        let mut dev=b.board.i2c_devices();let ioe=dev.iter_mut().find(|(bus,address,_)|*bus==1&&*address==0x4f).unwrap();
        for (reg,value) in [(4,0x20),(6,0x20),(0x14,0)] {ioe.2.start(false);ioe.2.write(reg);ioe.2.write(value);ioe.2.stop();}
        for (pin,signal) in [(13,173),(12,179),(11,213),(10,214),(9,215),(8,216)] {b.periph.io_mux.write(4+pin*4,1<<12|1<<9);b.periph.gpio.func_out_sel[pin as usize]=signal;b.periph.gpio.func_in_sel[signal as usize]=0x80|pin;}
    }
    fn ready(b:&mut SocBus) {
        cmd(b,0,0,0);assert_eq!(cmd(b,8,0x1aa,CMD_RESP),0x1aa);
        for _ in 0..2 {cmd(b,55,0,CMD_RESP);cmd(b,41,0x40ff8000,CMD_RESP);}
        cmd(b,2,0,CMD_RESP|CMD_RESP_LONG);let rca=cmd(b,3,0,CMD_RESP)&0xffff0000;cmd(b,7,rca,CMD_RESP);
        cmd(b,55,rca,CMD_RESP);cmd(b,6,2,CMD_RESP);write(b,reg::CTYPE,2);
    }
    fn transfer(b:&mut SocBus,number:u8,buffer:u32) {
        let desc=DRAM_LOW+0x100;for (i,v) in [des0::OWN|des0::FS|des0::LD,512,buffer,0].into_iter().enumerate(){b.write32_unpriced(desc+i as u32*4,v).unwrap();}
        write(b,reg::CTRL,CTRL_USE_INTERNAL_DMA|CTRL_INT_ENABLE);write(b,reg::BMOD,BMOD_DE);write(b,reg::DBADDR,desc);write(b,reg::BLKSIZ,512);write(b,reg::BYTCNT,512);write(b,reg::INTMASK,u32::MAX);
        cmd(b,number,8,CMD_RESP|CMD_DATA|if number==24 {CMD_WRITE}else{0});
    }
    #[test]
    fn physical_sdmmc_dma_roundtrip_and_chip_reset_retains_media() {
        let mut b=board();ready(&mut b);let at=DRAM_LOW+0x400;
        for i in 0..512 {b.write8_unpriced(at+i,(i%251) as u8).unwrap();}
        transfer(&mut b,24,at);assert_ne!(read(&mut b,reg::RINTSTS)&int::DATA_OVER,0);assert_eq!(b.periph.sdmmc.stats.bytes_written,512);
        assert_ne!(b.periph.source_status()[0]&(1<<crate::periph::SRC_SDIO_HOST),0);
        b.reboot([0;6]);wiring(&mut b);ready(&mut b);
        transfer(&mut b,17,at+512);assert_ne!(read(&mut b,reg::RINTSTS)&int::DATA_OVER,0);
        for i in 0..512 {assert_eq!(b.read8_unpriced(at+512+i).unwrap(),(i%251) as u8);}
        assert_eq!(b.read32_unpriced(DRAM_LOW+0x100).unwrap()&des0::OWN,0);
        assert!(b.periph.sdmmc.stats.bytes_read>=512);
    }
    #[test]
    fn sdmmc_missing_route_and_invalid_dma_fail_without_guest_memory_recursion() {
        let mut b=board();b.periph.gpio.func_in_sel[179]=0x80|20;cmd(&mut b,8,0x1aa,CMD_RESP);assert_ne!(read(&mut b,reg::RINTSTS)&int::RTO,0);
        b.periph.gpio.func_in_sel[179]=0x80|12;ready(&mut b);transfer(&mut b,17,BASE);assert_ne!(read(&mut b,reg::IDSTS)&esp_periph::sdmmc::idsts::FBE,0);
        assert!(Memory::check(u32::MAX,512).is_err());assert!(Memory::check(DRAM_HIGH-1,512).is_err());
    }
}
