//! Classic EMAC attachment. ESP-IDF v5.4.2 emac_*_struct.h, dport_reg.h,
//! gpio_sig_map.h and io_mux_reg.h provide register/pad identities.
use crate::bus::{SocBus,dma_range};
use esp_periph::{Device,emac::Memory};
impl Memory for SocBus {
    fn read(&mut self,address:u32,bytes:usize)->Option<Vec<u8>> {let range=dma_range(address,bytes)?;Some(self.dram.get(range)?.to_vec())}
    fn write(&mut self,address:u32,bytes:&[u8])->bool {
        let Some(range)=dma_range(address,bytes.len()) else{return false;};
        let Some(dst)=self.dram.get_mut(range) else{return false;};dst.copy_from_slice(bytes);self.written(address,bytes.len());true
    }
}
impl SocBus {
    pub(crate) fn attach_emac(&mut self) {
        self.emac_config=self.board.emac_config();
        self.periph.emac=Default::default();
        if self.emac_config.is_none(){self.wired_link=None;}
        if let Some(cfg)=self.emac_config {
            let id=match cfg.phy {esp_soc::board::EthernetPhy::Ip101g=>0x0243_0c54,esp_soc::board::EthernetPhy::Rtl8201=>0x001c_c816,esp_soc::board::EthernetPhy::Lan87xx=>0x0007_c0f1};
            self.periph.emac.configure(cfg.phy_addr,id);
            if let Some(link)=self.wired_link.as_mut(){link.reset();}else{self.wired_link=Some(esp_soc::ethernet::EthernetLink::new());}
        }
    }
    pub(crate) fn emac_routes(&mut self) {
        let Some(cfg)=self.emac_config else{return;};
        let routes=self.periph.gpio.routes();
        let reset=cfg.reset_pin.is_some_and(|p|routes.software_output(p)==Some(false));
        let mdio=routes.matrix_output(cfg.mdc as usize,200)&&routes.i2c_pin(201)==Some(cfg.mdio);
        let rmii=cfg.rmii.iter().enumerate().all(|(i,&p)|if i<3{routes.function(p as usize,5)}else{routes.input_function(p as usize,5)})
            &&cfg.ref_clk.is_none_or(|p|routes.input_function(p as usize,5))
            &&self.periph.emac.read(0x200c)&(7<<13)==4<<13
            &&self.periph.emac.read(0x2008)&0x21==0x21;
        let clock=self.periph.dport.ram.read(0xcc)&(1<<14)!=0&&self.periph.dport.ram.read(0xd0)&(1<<7)==0;
        self.periph.emac.set_routes(clock,mdio,rmii,reset);
        self.periph.emac.poll(self.cycles.saturating_mul(1_000_000_000)/crate::periph::CPU_HZ);
    }
    pub(crate) fn emac_step(&mut self) {
        self.emac_routes();if !self.periph.emac.clocked(){return;}
        let before=self.periph.emac.irq_sources();
        // Taking a lazy empty Emac is allocation-free. Guest memory access goes
        // through the SoC's DMA window and invalidates translated code pages.
        let mut emac=std::mem::take(&mut self.periph.emac);emac.step(self);
        if let Some(link)=self.wired_link.as_mut(){
            let now=self.cycles.saturating_mul(1_000_000)/crate::periph::CPU_HZ;
            for frame in emac.take_frames(){let _=link.transmit(&frame,now);}link.poll(now);
            for _ in 0..64 {let Some(frame)=link.next_frame()else{break;};let _=emac.receive(&frame);}
        }
        self.irq_dirty|=before!=emac.irq_sources();self.periph.emac=emac;
    }
}
