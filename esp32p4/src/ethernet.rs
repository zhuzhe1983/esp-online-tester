//! P4 wired GMAC attachment, ESP-IDF v5.4.2 emac_ll.h, gpio_sig_map.h,
//! io_mux_reg.h, hp_sys_clkrst_reg.h, hp_system_reg.h and lp_clkrst_reg.h.
use crate::bus::SocBus;
use esp_periph::{Device,Dispatch};
impl SocBus {
    pub(crate) fn attach_emac(&mut self) {
        let config=self.board.emac_config();let same=config==self.emac_config;self.emac_config=config;
        if let Some(cfg)=self.emac_config {
            let id=match cfg.phy {esp_soc::board::EthernetPhy::Ip101g=>0x0243_0c54,esp_soc::board::EthernetPhy::Rtl8201=>0x001c_c816,esp_soc::board::EthernetPhy::Lan87xx=>0x0007_c0f1};
            if same&&self.periph.emac.attached(){self.periph.emac.reset();}else{self.periph.emac.configure(cfg.phy_addr,id);}
            if let Some(link)=self.wired_link.as_mut(){link.reset();}else{self.wired_link=Some(esp_soc::ethernet::EthernetLink::new());}
        }else{self.wired_link=None;self.periph.emac=Default::default();}
        Dispatch::refresh_optional(&mut self.periph,0x50098);
    }
    pub(crate) fn emac_routes(&mut self) {
        let Some(cfg)=self.emac_config else{return;};
        let routes=crate::periph::GPIO_PINS.routes(&self.periph.gpio.g,&self.periph.io_mux);
        // MDC=108, MDO=109, MDI=107: bidirectional MDIO has different input
        // and output matrix indices and must not use i2c_pin's same-index rule.
        let mdio=routes.matrix_output(usize::from(cfg.mdc),108)&&routes.matrix_output(usize::from(cfg.mdio),109)&&routes.matrix_input(107)==Some(cfg.mdio);
        let reset=cfg.reset_pin.is_some_and(|p|routes.software_output(p)==Some(false));
        let output=cfg.rmii[..3].iter().enumerate().all(|(i,&p)|routes.matrix_output(usize::from(p),178+i as u32)||routes.function(usize::from(p),3));
        let input=cfg.rmii[3..].iter().zip([179,180,178]).all(|(&p,s)|routes.matrix_input(s)==Some(p)||routes.input_function(usize::from(p),3));
        let refclk=cfg.ref_clk.is_some_and(|p|matches!(p,32|44|50)&&routes.input_function(usize::from(p),3));
        let clk0=self.periph.hp_clkrst.read(0x30);let clk1=self.periph.hp_clkrst.read(0x34);
        let rmii_clocks=(clk0&((1<<27)|(1<<29)))==((1<<27)|(1<<29))&&clk0&((3<<25)|(1<<28))==0&&clk1&(1<<9)!=0&&clk1&(1<<8)==0
            &&self.periph.lp.ram[1].read(0x40)&(1<<15)!=0;
        let rmii=output&&input&&refclk&&rmii_clocks&&self.periph.hp_sys.read(0x14c)>>2&7==4;
        let clock=self.periph.hp_clkrst.read(0x18)&(1<<13)!=0&&self.periph.lp.ram[1].read(0x4c)&(1<<30)==0;
        self.periph.emac.set_routes(clock,mdio,rmii,reset);
        self.periph.emac.poll(self.periph.now_ns());
    }
    pub(crate) fn emac_step(&mut self) {
        let before=self.periph.emac.irq_sources();self.emac_routes();
        if self.periph.emac.clocked(){
            let mut emac=std::mem::take(&mut self.periph.emac);emac.step(self);
            if let Some(link)=self.wired_link.as_mut(){
                let now=self.cycles.saturating_mul(1_000_000)/crate::periph::CPU_HZ;
                for frame in emac.take_frames(){let _=link.transmit(&frame,now);}link.poll(now);
                for _ in 0..64{let Some(frame)=link.next_frame()else{break;};let _=emac.receive(&frame);}
            }
            self.periph.emac=emac;
        }
        Dispatch::refresh_optional(&mut self.periph,0x50098);self.irq_dirty|=before!=self.periph.emac.irq_sources();
    }
}
