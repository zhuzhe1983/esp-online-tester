//! C5 SPI0 cache/MMU and SPI1 memory transactions, C5 IDF v5.5.4
//! spi_mem_c_reg.h, spi1_mem_reg.h, ext_mem_defs.h and pcr_reg.h.
//! The functional model has no cache-line latency: mapped reads reach the
//! backing array. It does not bypass MMU validity, chip select, reset or gates.
use esp_periph::SpiMem;
pub(crate) struct Mspi {
    port:[SpiMem;2],config:u32,clock:u32,raw:[u32;2],enable:[u32;2],status_write:u32,
    pub mmu:[u32;512],index:u32,power:u32,power_down:bool,reset_enabled:bool,
    quad:crate::quad_psram::QuadPsram,native_pads:u32,xtal_hz:u32,
}
impl Mspi {
    // Metadata exists for the complete header, but only the implemented
    // controller/configuration subset is exposed. PMS/ECC engines, XTS and
    // NAND do not become register RAM merely because a header describes them.
    fn plain_config(port:usize,off:u32)->Option<(u32,u32)>{
        let allowed=if port==0{matches!(off,0x08|0x0c|0x10|0x14|0x18|0x1c|0x20|0x34|0x3c|0x40|0x44|0x48|0x4c|0x50|0x54|0xd4|0xd8|0x180|0x184|0x188|0x18c|0x190|0x194|0x198|0x19c|0x1a0|0x200|0x3fc)}
            else{matches!(off,0x04|0x08|0x0c|0x14|0x18|0x1c|0x20|0x24|0x28|0x34|0x3c|0x98|0xa0|0xac|0xd4|0x180|0x200|0x3fc)};
        if allowed{crate::mspi_regs::config(port,off)}else{None}
    }
    pub fn new()->Self{
        let mut port=[SpiMem::new(false),SpiMem::new(true)];
        for (n,p) in port.iter_mut().enumerate(){p.has_psram=false;p.status=0;for off in (0..0x400).step_by(4){if let Some((reset,_))=crate::mspi_regs::config(n,off){p.regs.write(off,reset);}}}
        port[1].regs.write(0xa4,0x7a7a_0000);port[1].regs.write(0x9c,0x0800_2000);
        Self{port,config:5,clock:0x400,raw:[0;2],enable:[0;2],status_write:0,mmu:[0x37c;512],index:0,power:0x1320_0004,power_down:false,reset_enabled:false,quad:Default::default(),native_pads:0,xtal_hz:48_000_000}
    }
    pub fn set_xtal_hz(&mut self,hz:u32){self.xtal_hz=hz;}
    pub fn configure_psram(&mut self,bytes:usize)->Result<(),String>{self.quad.configure(bytes)}
    pub fn psram_size(&self)->usize{self.quad.bytes()}
    pub fn preserve_psram(&mut self,old:&Self){self.quad=old.quad.clone();}
    pub fn set_pad(&mut self,pin:u8,function:u32){let bit=1u32<<pin;if bit&crate::quad_psram::NATIVE_PADS!=0{if function==0{self.native_pads|=bit;}else{self.native_pads&=!bit;}}}
    fn psram_clock_hz(&self,register:u32)->Option<u32>{
        // PCR_MSPI_FUNC_CLK_SEL: 0 XTAL, 1 RC_FAST, 2 PLL_F480M. Only
        // the explicit crystal has a modeled rate in this subset.
        if !self.enabled()||self.clock&0x300!=0||self.clock&255>2{return None;}
        let source=self.xtal_hz/(1<<(self.clock&255));
        if register&(1<<31)!=0{return Some(source);}
        let n=(register>>16)&255;let h=(register>>8)&255;let l=register&255;
        (n==l&&h<=n).then_some(source/(n+1))
    }
    pub fn psram_access(&self,write:bool)->bool{
        let p=&self.port[0];let s=p.regs.read(0x40);let phases=p.regs.read(0x44);
        let command=p.regs.read(if write{0x4c}else{0x48});
        let dummy_ok=if write{s&8==0}else{s&16!=0&&(s>>6)&63==5};
        self.quad.bytes()==crate::quad_psram::CAPACITY&&self.quad.qpi()
            &&self.native_pads==crate::quad_psram::NATIVE_PADS
            &&s&7==4&&s&(1<<21)==0&&(s>>14)&63==23&&dummy_ok
            &&s&if write{1<<20}else{1<<5}!=0
            &&phases&((7<<10)|(15<<14)|(15<<18))==15<<14
            &&command>>28==7&&command&65535==if write{0x38}else{0xeb}
            &&p.regs.read(0xd8)&1==0&&p.regs.read(0x1a0)&(1<<16)==0
            &&p.regs.read(0x34)&(1<<9)==0
            &&self.psram_clock_hz(p.regs.read(0x50)).is_some_and(|hz|hz<=if self.quad.wrap32(){109_000_000}else{84_000_000})
    }
    fn enabled(&self)->bool{self.config&3==1&&self.clock&0xf00==0x400&&self.clock&255<=2}
    pub fn irq(&self)->bool{self.enabled()&&(self.raw[0]&self.enable[0]!=0||self.raw[1]&self.enable[1]!=0)}
    pub fn translate(&self,address:u32)->Option<(bool,usize)>{
        if !self.enabled()||self.power&2!=0||self.port[0].regs.read(0x3c)&(1<<31)!=0||!(0x4200_0000..0x4400_0000).contains(&address){return None;}
        let shift=16-((self.power>>3)&3);let n=((address-0x4200_0000)>>shift) as usize;let entry=*self.mmu.get(n)?;
        if entry&(1<<10)==0||entry&(1<<11)!=0{return None;}
        Some((entry&(1<<9)!=0,(((entry&0x1ff) as usize)<<shift)+(address&((1<<shift)-1)) as usize))
    }
    pub fn read(&mut self,address:u32)->Option<u32>{
        match address{0x6009_6018=>return Some(self.config|if self.config&2==0{8}else{0}),0x6009_601c=>return Some(self.clock),_=>{}}
        let n=match address&!0xfff{0x6000_2000=>0,0x6000_3000=>1,_=>return None};let off=address&0xfff;if !off.is_multiple_of(4)||self.config&3!=1{return None;}
        Some(match off{
            0=>0,0x2c=>self.port[n].status,
            0x10 if n==1=>0,0x9c|0xa4 if n==1=>self.port[n].regs.read(off),
            0x58..=0x94 if n==1=>self.port[n].read(off),0x38 if n==1=>u32::MAX,
            0xc0=>self.enable[n],0xc4=>0,0xc8=>self.raw[n],0xcc=>self.raw[n]&self.enable[n],
            0x37c if n==0=>*self.mmu.get(self.index as usize)?,0x380 if n==0=>self.index,0x384 if n==0=>self.power,
            _=>{Self::plain_config(n,off)?;self.port[n].regs.read(off)}
        })
    }
    pub fn write(&mut self,address:u32,value:u32,flash:&mut[u8],psram:&mut[u8],trace:bool)->bool{
        match address{
            0x6009_6018=>{if value&2!=0{let status=self.port[1].status;let down=self.power_down;let reset=self.reset_enabled;let quad=self.quad.clone();let pads=self.native_pads;let xtal=self.xtal_hz;*self=Self::new();self.port[1].status=status;self.power_down=down;self.reset_enabled=reset;self.quad=quad;self.native_pads=pads;self.xtal_hz=xtal;}self.config=value&7;return true;}
            0x6009_601c=>{self.clock=value&0xfff;return true;}_=>{}
        }
        let n=match address&!0xfff{0x6000_2000=>0,0x6000_3000=>1,_=>return false};let off=address&0xfff;if !off.is_multiple_of(4)||self.config&3!=1{return false;}
        match off{
            0 if n==0=>true, // SPI0 CMD is hardware status; no guest command engine
            0=>{if !self.enabled(){return false;}self.command(value,flash,psram,trace)}
            0x2c=>{self.status_write=value;true}
            0x10 if n==1=>{if value&(1<<31)!=0{self.port[n].pending_cmd=0;}true}
            0x9c if n==1&&value&3==0=>{self.port[n].regs.write(off,value&0xffff_fffc);true}
            0xa4 if n==1&&value&1==0=>{self.port[n].regs.write(off,value&0xffff_80fe);true}
            0x58..=0x94 if n==1=>{self.port[n].write(off,value);true}
            0xc0=>{self.enable[n]=value&if n==0{0xf000_03f8}else{0x41f};true}
            0xc4|0xc8=>{self.raw[n]&=!value;true}0xcc=>true,
            0x37c if n==0=>{if let Some(entry)=self.mmu.get_mut(self.index as usize){*entry=value&0xfff;true}else{false}}
            0x380 if n==0=>{self.index=value;true}
            0x384 if n==0=>{self.power=value&0x7fff_001f;true}
            0x40|0x44 if n==0=>{
                let (reset,mask)=crate::mspi_regs::config(n,off).unwrap();let value=(value&mask)|(reset&!mask);self.port[0].regs.write(off,value);
                // C5 SPI0 SRAM_CMD describes these phase bits as the same
                // controls as CACHE_SCTRL.USR_SRAM_{DIO,QIO}.
                if off==0x40{let old=self.port[0].regs.read(0x44);self.port[0].regs.write(0x44,(old&!((7<<10)|(15<<14)))|if value&2!=0{7<<10}else{0}|if value&4!=0{15<<14}else{0});}
                else{let old=self.port[0].regs.read(0x40);self.port[0].regs.write(0x40,(old&!6)|if value&(7<<10)==7<<10{2}else{0}|if value&(15<<14)==15<<14{4}else{0});}true
            }
            _=>{let Some((reset,mask))=Self::plain_config(n,off)else{return false;};self.port[n].regs.write(off,(value&mask)|(reset&!mask));true}
        }
    }
    fn command(&mut self,cmd:u32,flash:&mut[u8],psram:&mut[u8],trace:bool)->bool{
        // Dedicated BE is ambiguous in the C5 published header (32KB) versus
        // the shared older IP's 64KB operation. Do not erase guessed geometry.
        if cmd&((1<<19)|(1<<23))!=0{return false;}
        if cmd&(1<<21)!=0{self.power_down=true;self.raw[1]|=24;return true;}
        if cmd&(1<<20)!=0{self.power_down=false;self.port[1].w.fill(u32::MAX);self.raw[1]|=24;return true;} // release; no fabricated legacy electronic ID
        let psram_clock=self.psram_clock_hz(self.port[1].regs.read(0x14));
        let p=&mut self.port[1];p.jedec=esp_periph::spi_mem::flash_jedec(false,flash.len());p.has_psram=false;p.log=trace;
        if p.regs.read(8)&((1<<10)|(1<<11))!=0{return false;} // unmodeled CRC engine
        let user=p.regs.read(0x18);let misc=p.regs.read(0x34);let opcode=if user&(1<<31)!=0{p.regs.read(0x20) as u8}else{0};let usr=cmd&(1<<18)!=0;
        if usr&&(misc&(1<<10)!=0||(misc&3==0&&!psram.is_empty())){return false;} // unmodeled held-CS sequence / two active devices
        if usr&&misc&3==1{
            let Some(hz)=psram_clock else{return false;};
            let handled=self.quad.transfer(p,psram,self.native_pads,hz);if handled{self.raw[1]|=24;}return handled;
        }
        if usr&&misc&1!=0&&misc&2!=0{if user&(1<<28)!=0{p.w.fill(u32::MAX);}self.raw[1]|=24;return true;}
        let flash_selected=!usr||misc&1==0;
        if flash_selected&&usr {
            if opcode==0x66{self.reset_enabled=true;self.raw[1]|=24;return true;}
            if opcode==0x99{if self.reset_enabled{p.status=0;self.power_down=false;}self.reset_enabled=false;self.raw[1]|=24;return true;}
            self.reset_enabled=false;
            if opcode==0xb9{self.power_down=true;self.raw[1]|=24;return true;}
            if opcode==0xab{self.power_down=false;self.raw[1]|=24;return true;}
            if self.power_down{if user&(1<<28)!=0{p.w.fill(u32::MAX);}self.raw[1]|=24;return true;}
            if !matches!(opcode,0x03|0x0b|0x3b|0x6b|0xbb|0xeb|0x9f|0x05|0x35|0x06|0x04|0x01|0x31|0x02|0x32|0x38|0x20|0x52|0xd8|0xc7|0x60){if user&(1<<28)!=0{p.w.fill(u32::MAX);}self.raw[1]|=24;return true;}
        }
        let writes=if usr{matches!(opcode,0x01|0x31|0x11|0x02|0x32|0x38|0x20|0x52|0xd8|0xc7|0x60)}else{cmd&0x07c0_0000!=0};
        if flash_selected&&writes&&p.status&2==0{self.raw[1]|=24;return true;}
        let status=if flash_selected&&(usr&&matches!(opcode,0x01|0x31|0x11)||!usr&&cmd&(1<<26)!=0){Some(if usr{match opcode{0x31=>(p.status&255)|(p.w[0]&255)<<8,0x11=>p.status,_=>p.w[0]&65535}}else{self.status_write&65535})}else{None};
        p.write(0,cmd);p.execute(flash,psram);if let Some(status)=status{p.status=status&!3;}p.dirty.clear();self.raw[1]|=24;true
    }
}
