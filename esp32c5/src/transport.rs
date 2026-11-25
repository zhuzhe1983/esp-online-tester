//! C5 SPI2 / AHB-GDMA / LEDC, independently mapped from fixed C5 IDF headers.
//! Functional byte transport with a serial-clock time floor. DMA setup/FIFO,
//! bus contention, and sub-byte/DDR/slave transactions are not modeled.
use esp_periph::{Device, GpSpi, Ledc, LedcLayout};
use esp_soc::{board::{SpiPins,SpiQspiPins},pins::SpiQspiNative};
use crate::{bus::{C5Bus,CPU_HZ,PINS},transport_dma::C5Dma,transport_regs};

#[derive(Clone,Copy)]
struct Pending { pins:SpiQspiPins, dma:u32, source_hz:u32, duration:u64, due:Option<u64> }
pub(crate) struct Transport {
    spi:GpSpi, dma:C5Dma, ledc:Ledc, spi_conf:u32,spi_clock:u32,dma_conf:u32,
    ledc_conf:u32,ledc_clock:u32,ledc_power:u32,last_ledc:u64,ledc_remainder:u64,
    xtal_hz:u32,rc_fast_hz:Option<u32>,pll_ready:bool,pll_div_gate_mask:u16,precise_ledc:bool,ledc_fraction:u64,
    pending:Option<Pending>, pub deadline:Option<u64>, pwm_deadline:Option<u64>,pub rejected:u64,
}
impl Transport {
    fn new_spi()->GpSpi {
        let mut s=GpSpi::new().diverting_dma_rx();
        for off in (0..=0xf0).step_by(4){if let Some((reset,_))=transport_regs::config(0,off){s.write(off,reset);}}
        s
    }
    fn new_ledc()->Ledc {
        let mut l=Ledc::new(LedcLayout::C5);
        for off in (0..=0x174).step_by(4){if let Some((reset,_))=transport_regs::config(2,off){l.write(off,reset);}}
        l.clock_enabled=false;l
    }
    pub fn new(cycle:u64)->Self{Self{spi:Self::new_spi(),dma:C5Dma::new(),ledc:Self::new_ledc(),spi_conf:1,spi_clock:1<<22,dma_conf:1,ledc_conf:0,ledc_clock:0,ledc_power:4,last_ledc:cycle,ledc_remainder:0,xtal_hz:48_000_000,rc_fast_hz:None,pll_ready:false,pll_div_gate_mask:0x1ff,precise_ledc:false,ledc_fraction:0,pending:None,deadline:None,pwm_deadline:None,rejected:0}}
    pub fn contains(a:u32)->bool{matches!(a,0x6000_7000..=0x6000_757c|0x6008_0000..=0x6008_0424|0x6008_1000..=0x6008_10f0|0x6009_6048..=0x6009_6050|0x6009_60c0..=0x6009_60c8)}
    // C5 IDF v5.5.3@2c211b236707889e8400c4dc5644dd5c4ee071e0 and
    // v5.5.4@735507283d5b2f9fb363a1901172dbd9e847945d: soc/esp32c5/register/soc/pcr_reg.h
    // SPI2_CLKM_SEL[21:20]={XTAL,PLL160,RC_FAST,PLL120}; LEDC_SCLK_SEL={XTAL,RC_FAST,PLL80}.
    // EN is bit22; SPI2 DIV_NUM[19:12]+1 is confirmed by hal/esp32c5/include/hal/spi_ll.h.
    fn ledc_on(&self)->bool{self.ledc_conf&3==1&&self.ledc_clock&(1<<22)!=0&&self.ledc_power&4==0}
    fn spi_source_hz(&self,xtal:u32)->Option<u32>{match (self.spi_clock>>20)&3{0=>Some(xtal),1=>(self.pll_ready&&self.pll_div_gate_mask&2!=0).then_some(160_000_000),2=>self.rc_fast_hz,3=>(self.pll_ready&&self.pll_div_gate_mask&4!=0).then_some(120_000_000),_=>None}}
    fn ledc_source_hz(&self)->Option<u32>{match (self.ledc_clock>>20)&3{0=>Some(self.xtal_hz),1=>self.rc_fast_hz,2=>(self.pll_ready&&self.pll_div_gate_mask&8!=0).then_some(80_000_000),_=>None}}
    fn spi_hz(&self,xtal:u32)->Option<u32>{
        if self.spi_conf&3!=1||self.spi_clock&(1<<22)==0{return None;}
        self.spi_source_hz(xtal).map(|hz|hz/(((self.spi_clock>>12)&255)+1)).filter(|&hz|hz!=0)
    }
    fn check_spi_clock(&mut self,xtal:u32){
        if self.pending.is_some_and(|p|self.spi_source_hz(xtal)!=Some(p.source_hz)||self.spi_hz(xtal).is_none()){self.cancel_spi();}
    }
    fn set_ledc_clock(&mut self){self.ledc.external_clock_hz=u64::from(self.ledc_source_hz().unwrap_or(0));self.ledc.clock_enabled=self.ledc_on();}
    fn ledc_hz(&self)->u64{if self.ledc.clock_enabled{self.ledc.external_clock_hz}else{0}}
    /// Observations belong to the independently configured analog/clock engines.
    /// No profile, cookie, PCR mux selection or elapsed host time grants readiness here.
    pub fn set_clock_sources(&mut self,cycle:u64,rc_fast_hz:Option<u32>,pll_ready:bool,pll_div_gate_mask:u16){
        self.tick_ledc(cycle);let rc_fast_hz=rc_fast_hz.filter(|&hz|hz!=0);let pll_div_gate_mask=pll_div_gate_mask&0x1ff;
        if self.rc_fast_hz!=rc_fast_hz||self.pll_ready!=pll_ready||self.pll_div_gate_mask!=pll_div_gate_mask{
            if !self.precise_ledc{
                // Flush the old source's 0..2 CPU cycles before changing the source.
                let phase=self.ledc_remainder*self.ledc_hz();self.ledc_remainder=0;
                self.ledc.tick_with_source(1,|_,_|phase/3);self.ledc_fraction=phase%3;self.precise_ledc=true;
            }
            self.rc_fast_hz=rc_fast_hz;self.pll_ready=pll_ready;self.pll_div_gate_mask=pll_div_gate_mask;self.check_spi_clock(self.xtal_hz);
        }
        self.set_ledc_clock();self.plan(cycle);
    }
    fn cancel_spi(&mut self){self.spi.abort_transfer();self.spi.dma_rx=None;self.pending=None;}
    fn tick_ledc(&mut self,now:u64){
        let delta=now.wrapping_sub(self.last_ledc);self.last_ledc=now;
        if !self.precise_ledc{let delta=delta+self.ledc_remainder;self.ledc_remainder=delta%3;self.ledc.tick(delta/3);return;}
        // Fixed 240MHz CPU / 80MHz LEDC reference. Keep fractional phase across
        // source/gate boundaries; large elapsed intervals use Ledc's u128 fallback.
        let hz=self.ledc_hz();self.ledc.tick_with_source(delta/3,|_,_|hz);
        let phase=(delta%3)*hz+self.ledc_fraction;self.ledc_fraction=phase%3;
        self.ledc.tick_with_source(1,|_,_|phase/3);
    }
    fn plan(&mut self,now:u64){
        self.pwm_deadline=self.ledc.next_output_change().map(|n|{
            // The shared API rounds to APB ticks. In precise mode wake at the
            // earliest possible CPU tick, then recheck up to two ticks later.
            let delta=if self.precise_ledc{n.saturating_sub(1).saturating_mul(3).saturating_add(1)}else{n.saturating_mul(3).saturating_sub(self.ledc_remainder)};
            now.saturating_add(delta)
        });
        self.deadline=self.pending.and_then(|p|p.due).into_iter().chain(self.pwm_deadline).min();
    }
    pub fn publish_pwm(&mut self,bus:&mut C5Bus){
        self.tick_ledc(bus.cycles);self.xtal_hz=bus.xtal_hz;self.check_spi_clock(bus.xtal_hz);self.set_ledc_clock();
        let (ledc,gpio,mux)=(&self.ledc,&bus.gpio,&bus.io_mux);
        let clock=|pin:u8|((mux.read(4+4*u32::from(pin))>>12)&7==1).then(||ledc.output(gpio,pin)).flatten();
        bus.board.advance_inputs_with_clock(bus.cycles,&clock);self.plan(bus.cycles);
    }
    pub fn sources(&self)->[(usize,bool);8]{[(49,self.ledc.irq_sources()!=0),(71,self.dma.channels.inp[0].irq()),(72,self.dma.channels.inp[1].irq()),(73,self.dma.channels.inp[2].irq()),(74,self.dma.channels.out[0].irq()),(75,self.dma.channels.out[1].irq()),(76,self.dma.channels.out[2].irq()),(77,self.spi.irq())]}
    pub fn read(&mut self,a:u32,bus:&mut C5Bus)->Option<u32>{
        self.tick_ledc(bus.cycles);
        let off=a&0xfff;
        match a {
            0x6009_6048=>Some(self.ledc_conf|(u32::from(self.ledc_conf&2==0)*4)),0x6009_604c=>Some(self.ledc_clock),0x6009_6050=>Some(self.ledc_power),
            0x6009_60c0=>Some(self.dma_conf),0x6009_60c4=>Some(self.spi_conf|(u32::from(self.spi_conf&2==0)*4)),0x6009_60c8=>Some(self.spi_clock),
            0x6008_1000..=0x6008_10f0 if self.spi_conf&3==1=>{
                transport_regs::config(0,off)?;
                Some(if off==0xf0{self.spi.regs.read(off)}else{self.spi.read(off)})
            },
            0x6008_0000..=0x6008_0424 if self.dma_conf&3==1=>{
                if [0xd8,0x198,0x258].contains(&off){Some(self.dma.fifo(((off-0xd8)/0xc0) as usize,bus))}else{self.dma.read(off)}
            },
            0x6000_7000..=0x6000_757c if self.ledc_conf&3==1=>{
                Self::ledc_register(off)?;Some(self.ledc.read(off))
            },_=>None,
        }
    }
    fn ledc_register(off:u32)->Option<(u32,u32)>{
        // Six sixteen-word gamma ranges are real RAM. ETM/capture are not implemented.
        if (0x400..0x580).contains(&off)&&off.is_multiple_of(4){return Some((0,u32::MAX));}
        if matches!(off,0x120..=0x15c){return None;}transport_regs::config(2,off)
    }
    pub fn write(&mut self,a:u32,value:u32,bus:&mut C5Bus)->bool{
        self.tick_ledc(bus.cycles);let off=a&0xfff;
        match a {
            0x6009_6048=>{if value&2!=0{self.ledc=Self::new_ledc();if self.precise_ledc{self.ledc_fraction=0;}}self.ledc_conf=value&3;},
            0x6009_604c=>self.ledc_clock=value&(7<<20),0x6009_6050=>self.ledc_power=value&6,
            0x6009_60c0=>{if value&2!=0{self.dma=C5Dma::new();if self.pending.is_some_and(|p|p.dma&((1<<27)|(1<<28))!=0){self.cancel_spi();}}self.dma_conf=value&3;},
            0x6009_60c4=>{if value&2!=0{self.spi=Self::new_spi();self.pending=None;}self.spi_conf=value&3;if self.spi_conf&3!=1{self.cancel_spi();}},
            0x6009_60c8=>{let v=value&0x7ff000;if self.spi_clock!=v{self.cancel_spi();}self.spi_clock=v;},
            0x6000_7000..=0x6000_757c if self.ledc_conf&3==1=>{
                let Some((_,mask))=Self::ledc_register(off)else{return false;};if off>=0x400&&self.ledc_power&4!=0{return false;}
                if off==0xc8&&value&mask&!0x3f0!=0{return false;} // only static-duty-end IRQs currently have an autonomous deadline
                self.ledc.write(off,value&mask);
            },
            0x6008_0000..=0x6008_0424 if self.dma_conf&3==1=>{if !self.dma.write(off,value){return false;}},
            0x6008_1000..=0x6008_10f0 if self.spi_conf&3==1=>{
                let Some((reset,mask))=transport_regs::config(0,off)else{return false;};let mut v=(value&mask)|(reset&!mask);
                if off==0x3c {self.spi.int_raw&=!v;return true;} // C5 RAW is R/WTC/SS
                if off==0x30&&v&(7<<29)!=0 {self.cancel_spi();v&=!(7<<29);}
                if off==0&&v&(1<<24)!=0&&!self.spi.has_pending_transfer(){
                    let Some((source_hz,hz))=self.spi_source_hz(bus.xtal_hz).zip(self.spi_hz(bus.xtal_hz))else{self.rejected+=1;return false;};
                    let mut pins=PINS.routes(&bus.gpio,&bus.io_mux).spi_qspi_pins(&self.spi,[56,58,57,60,59,61,101,102,103,104,105],&[SpiQspiNative{function:2,sclk:6,data:[7,2,5,4],cs:&[10]}]);
                    pins.unsupported_mode|=self.spi.read(0x10)&8!=0; // C5 QPI_MODE bit3, not the S3 field
                    let user=self.spi.read(0x10);let dma=self.spi.read(0x30);
                    let cpu_overflow=(pins.data_bits>if user&(1<<25)!=0{256}else{512}&&dma&(1<<28)==0)
                        ||(pins.read_bits>if user&(1<<24)!=0{256}else{512}&&dma&(1<<27)==0);
                    // C5 IDF5.5.3/5.5.4 spi_reg.h: bits19/20 only select
                    // slave-segment clear policies. master_init sets both
                    // with segment enable18 clear. Slave mode stays rejected.
                    let unsupported=pins.unsupported_mode||!pins.command_bits.is_multiple_of(8)||!pins.address_bits.is_multiple_of(8)||!pins.data_bits.is_multiple_of(8)||!pins.read_bits.is_multiple_of(8)
                        ||dma&(1<<18)!=0||v&0x3ffff!=0||cpu_overflow;
                    if unsupported{self.rejected+=1;return false;}
                    let duration=(self.spi.wire_source_cycles()*CPU_HZ).div_ceil(u64::from(hz));
                    self.pending=Some(Pending{pins,dma,source_hz,duration,due:None});
                }
                self.spi.write(off,v);
            },_=>return false,
        }
        self.publish_pwm(bus);true
    }
    pub fn pump(&mut self,bus:&mut C5Bus){
        self.check_spi_clock(bus.xtal_hz);
        let Some(mut pending)=self.pending else{self.plan(bus.cycles);return;};
        if let Some(bits)=self.spi.dma_tx_pending {
            if self.dma_conf&3!=1{return;}
            match self.dma.transmit(bus,(bits as usize).div_ceil(8)){
                Some(Ok(data))=>self.spi.complete_dma_tx(&data),
                Some(Err(()))=>{self.spi.fail_dma_tx();self.pending=None;self.plan(bus.cycles);bus.irq_dirty=true;return;},
                None=>return,
            }
        }
        // A receive DMA channel must exist before data clocks are presented.
        if pending.dma&(1<<27)!=0&&pending.pins.read_bits!=0&&(self.dma_conf&3!=1||self.dma.input().is_none()){return;}
        if pending.due.is_none(){pending.due=Some(bus.cycles.saturating_add(pending.duration));self.pending=Some(pending);}
        if pending.due.is_some_and(|d|bus.cycles<d){self.plan(bus.cycles);return;}
        if let Some(transfer)=self.spi.take_transfer(){
            let p=pending.pins;
            let rx=if bus.board.uses_qspi_pins(){bus.board.spi_transfer_qspi_pins(2,p,&transfer.tx,transfer.rx_len)}
                else if bus.board.uses_spi_pins(){bus.board.spi_transfer_pins(2,SpiPins{sclk:p.sclk,mosi:p.data[0],miso:p.input[1],cs:p.cs,keep_cs:p.keep_cs},&transfer.tx,transfer.rx_len)}
                else{bus.board.spi_transfer(2,&transfer.tx,transfer.rx_len)};
            // Completion must use DMA selection accepted with USR, not a later live write.
            let live=self.spi.regs.read(0x30);self.spi.regs.write(0x30,pending.dma);self.spi.finish_transfer(transfer,&rx);self.spi.regs.write(0x30,live);
            if let Some(data)=self.spi.dma_rx.take(){self.dma.receive(bus,&data);}
            self.pending=None;bus.irq_dirty=true;
        }
        self.plan(bus.cycles);
    }
    pub fn report(&self)->String{
        let clocks=match(self.rc_fast_hz.is_some(),self.pll_ready){(false,false)=>"XTAL-only",(true,false)=>"XTAL/RC_FAST",(false,true)=>"XTAL/PLL",(true,true)=>"XTAL/RC_FAST/PLL"};
        let report=format!("SPI2={} transfers rejected={} serial-time-floor; GDMA3/full32-bit; LEDC6/static-duty {}",self.spi.transfers,self.rejected,clocks);
        if self.pll_div_gate_mask==0x1ff{report}else{format!("{report} pll_div_gates={:#x}",self.pll_div_gate_mask)}
    }
    /// Cold native snapshot only: no MMIO, event advancement or device call.
    pub fn spi_debug(&self,bus:&C5Bus)->String{
        let mut pins=PINS.routes(&bus.gpio,&bus.io_mux).spi_qspi_pins(&self.spi,[56,58,57,60,59,61,101,102,103,104,105],&[SpiQspiNative{function:2,sclk:6,data:[7,2,5,4],cs:&[10]}]);
        pins.unsupported_mode|=self.spi.read(0x10)&8!=0;
        let regs=[0,8,12,16,20,24,28,32,48,224,232].map(|off|(off,self.spi.read(off)));
        format!("PCRconf={:#x} PCRclock={:#x} sourceHz={:?} controllerHz={:?} regs={regs:08x?} pins={pins:?} pending={} rejected={}",
            self.spi_conf,self.spi_clock,self.spi_source_hz(bus.xtal_hz),self.spi_hz(bus.xtal_hz),self.pending.is_some(),self.rejected)
    }
}

#[cfg(test)]
#[path="transport_clock_tests.rs"]
mod clock_tests;
