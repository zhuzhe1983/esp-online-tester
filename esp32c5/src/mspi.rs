//! C5 SPI0 cache/MMU and SPI1 memory transactions, C5 IDF v5.5.4
//! spi_mem_c_reg.h, spi1_mem_reg.h, ext_mem_defs.h and pcr_reg.h.
//! Register headers and spimem_flash_ll.h match IDF v5.5.3 (LCD273 factory).
//! mspi_ll.h differs; the shared PLL divider contract is retained separately.
//! The functional model has no cache-line latency: mapped reads reach the
//! backing array. It does not bypass MMU validity, chip select, reset or gates.
//! Source-observation contract: docs/evidence/c5-mspi-clock-2026-11-24.
//! Idle NOR Resume: docs/evidence/c5-mspi-usr-2026-11-24.
//! PLL divider correction: docs/evidence/c5-mspi-divider-2026-11-24.
use esp_periph::SpiMem;
// XM25QH256C Rev2.1, SFDP tables1/9 and2/9 (printedpp85/86), Advanced Information.
// Only this published DWORD1/density window is qualified, for JEDEC20/40/19.
// In byte32, DTR bit3 is0: do not impersonate the XMC-D capability probe.
const XMC_C_SFDP_BASIC:[u8;8]=[0xe5,0x20,0xf3,0xff,0xff,0xff,0xff,0x0f];
pub(crate) struct Mspi {
    port:[SpiMem;2],config:u32,clock:u32,raw:[u32;2],enable:[u32;2],rd_status:[u32;2],
    pub mmu:[u32;512],index:u32,power:u32,power_down:bool,reset_enabled:bool,
    quad:crate::quad_psram::QuadPsram,native_pads:u32,xtal_hz:u32,rc_fast_hz:Option<u32>,pll_ready:bool,
    mp_rom_compat:bool,opaque_spi0:[u32;2],
}
impl Mspi {
    // Metadata exists for the complete header, but only the implemented
    // controller/configuration subset is exposed. PMS/ECC engines, XTS and
    // NAND do not become register RAM merely because a header describes them.
    fn plain_config(port:usize,off:u32)->Option<(u32,u32)>{
        let allowed=if port==0{matches!(off,0x08|0x0c|0x10|0x14|0x18|0x1c|0x20|0x34|0x3c|0x40|0x44|0x48|0x4c|0x50|0x54|0xd4|0xd8|0x174|0x180|0x184|0x188|0x18c|0x190|0x194|0x198|0x19c|0x1a0|0x200|0x3fc)}
            else{matches!(off,0x04|0x08|0x0c|0x14|0x18|0x1c|0x20|0x24|0x28|0x34|0x3c|0x98|0xa0|0xac|0xd4|0x180|0x200|0x3fc)};
        if allowed{crate::mspi_regs::config(port,off)}else{None}
    }
    pub fn new()->Self{
        let mut port=[SpiMem::new(false),SpiMem::new(true)];
        for (n,p) in port.iter_mut().enumerate(){p.has_psram=false;p.status=0;for off in (0..0x400).step_by(4){if let Some((reset,_))=crate::mspi_regs::config(n,off){p.regs.write(off,reset);}}}
        port[1].regs.write(0xa4,0x7a7a_0000);port[1].regs.write(0x9c,0x0800_2000);
        Self{port,config:5,clock:0x400,raw:[0;2],enable:[0;2],rd_status:[0;2],mmu:[0x37c;512],index:0,power:0x1320_0004,power_down:false,reset_enabled:false,quad:Default::default(),native_pads:0,xtal_hz:48_000_000,rc_fast_hz:None,pll_ready:false,mp_rom_compat:false,opaque_spi0:[0;2]}
    }
    /// After validating the MP rev100 ROM, retain its two witnessed opaque
    /// bootstrap words. Their virtual initial zero is not a silicon reset
    /// claim: the fixed C5 header explicitly reserves SPI0 +24/+28. They do
    /// not configure an engine, alias SPI1 lengths, or produce an interrupt.
    pub fn enable_mp_rom_compat(&mut self){self.mp_rom_compat=true;}
    pub fn disable_mp_rom_compat(&mut self){self.mp_rom_compat=false;}
    pub fn mp_rom_compat(&self)->bool{self.mp_rom_compat}
    pub fn set_xtal_hz(&mut self,hz:u32){self.xtal_hz=hz;}
    /// Observations from the separate oscillator/PLL model, not an enable
    /// cookie or a requested PCR mux value. Withdrawal immediately gates work.
    pub fn set_clock_sources(&mut self,rc_fast_hz:Option<u32>,pll_ready:bool){self.rc_fast_hz=rc_fast_hz.filter(|&hz|hz!=0);self.pll_ready=pll_ready;}
    pub fn configure_psram(&mut self,bytes:usize)->Result<(),String>{self.quad.configure(bytes)}
    pub fn psram_size(&self)->usize{self.quad.bytes()}
    /// Explicit diagnostic snapshot only: no guest MMIO, engine progress or
    /// readiness mutation. SPI1 offsets are paired with their existing words.
    pub fn psram_command_debug(&self)->String{
        let p=&self.port[1];let regs=[0x04,0x08,0x0c,0x14,0x18,0x1c,0x20,0x24,0x28,0x34,0x98,0xd4].map(|off|(off,p.regs.read(off)));
        format!("mspi_config={:#010x} mspi_clock={:#010x} native_pads={:#010x} qpi={} functional_hz={:?} psram_hz={:?} spi1={:08x?} W0={:#010x} W8={:#010x}",self.config,self.clock,self.native_pads,self.quad.qpi(),self.functional_clock_hz(),self.psram_clock_hz(p.regs.read(0x14)),regs,p.w[0],p.w[8])
    }
    pub fn preserve_psram(&mut self,old:&Self){self.quad=old.quad.clone();}
    pub fn set_pad(&mut self,pin:u8,function:u32){let bit=1u32<<pin;if bit&crate::quad_psram::NATIVE_PADS!=0{if function==0{self.native_pads|=bit;}else{self.native_pads&=!bit;}}}
    fn functional_clock_hz(&self)->Option<u32>{
        // C5 IDF5.5.3/5.5.4 PCR+1c: FUNC_EN10, AXI_RST11, source9:8.
        // The PCR header's low-speed-only comment omits the PLL divider:
        // IDF5.5.3/5.5.4 C5 mspi_ll.h sets NUM5 for PLL480/6=80MHz, and
        // spimem_flash_ll_get_source_freq_mhz divides that tap by NUM+1.
        // Low sources retain the header-based 0/1/2 subset. NUM2's /4
        // conflicts with the LL getter's /3 and has no independent validation.
        if self.config&3!=1||self.clock&0xc00!=0x400{return None;}
        let source=(self.clock>>8)&3;
        let hz=match source{0=>self.xtal_hz,1=>self.rc_fast_hz?,2 if self.pll_ready&&self.config&4!=0=>480_000_000,_=>return None};
        if hz==0{return None;}
        if source==2{Some(hz/((self.clock&255)+1))}else{match self.clock&255{0=>Some(hz),1=>Some(hz/2),2=>Some(hz/4),_=>None}.filter(|&hz|hz!=0)}
    }
    fn psram_clock_hz(&self,register:u32)->Option<u32>{
        let source=self.functional_clock_hz()?;
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
    fn enabled(&self)->bool{self.functional_clock_hz().is_some()}
    pub fn irq(&self)->bool{self.enabled()&&(self.raw[0]&self.enable[0]!=0||self.raw[1]&self.enable[1]!=0)}
    pub fn translate(&self,address:u32)->Option<(bool,usize)>{
        if !self.enabled()||self.power&2!=0||self.port[0].regs.read(0x3c)&(1<<31)!=0||!(0x4200_0000..0x4400_0000).contains(&address){return None;}
        let shift=16-((self.power>>3)&3);let n=((address-0x4200_0000)>>shift) as usize;let entry=*self.mmu.get(n)?;
        if entry&(1<<10)==0||entry&(1<<11)!=0{return None;}
        Some((entry&(1<<9)!=0,(((entry&0x1ff) as usize)<<shift)+(address&((1<<shift)-1)) as usize))
    }
    /// Ordinary scalar data only: stay within one current MMU page and prove
    /// each byte reaches the same backing at consecutive physical offsets.
    /// C5 IDF5.5.3/5.5.4 spi_mem_c_reg.h MMU_PAGE_SIZE controls8..64KiB.
    /// Cross-page split transactions and their error timing remain unmodeled.
    pub fn translate_data_span(&self,address:u32,width:u8)->Option<(bool,usize)>{
        if !matches!(width,2|4){return None;}
        let last=address.checked_add(u32::from(width)-1)?;let shift=16-((self.power>>3)&3);
        if address>>shift!=last>>shift{return None;}
        let first=self.translate(address)?;
        for i in 1..width{if self.translate(address+u32::from(i))!=Some((first.0,first.1.checked_add(usize::from(i))?)){return None;}}
        Some(first)
    }
    /// Pure diagnostic: no register read, transfer, clock advance or cache fill.
    pub fn mapping_debug(&self,address:u32)->String{
        let shift=16-((self.power>>3)&3);let index=address.checked_sub(0x4200_0000).map(|a|(a>>shift)as usize);
        let entry=index.and_then(|i|self.mmu.get(i)).copied();
        format!("address={address:#010x} page_bytes={} index={index:?} entry={entry:#x?} power={:#010x} config={:#x} clock={:#x} AXI={:#010x} translated={:?} psram_read={} psram_write={}",1u32<<shift,self.power,self.config,self.clock,self.port[0].regs.read(0x3c),self.translate(address),self.psram_access(false),self.psram_access(true))
    }
    pub fn read(&mut self,address:u32)->Option<u32>{
        match address{0x6009_6018=>return Some(self.config|if self.config&2==0{8}else{0}),0x6009_601c=>return Some(self.clock),_=>{}}
        let n=match address&!0xfff{0x6000_2000=>0,0x6000_3000=>1,_=>return None};let off=address&0xfff;if !off.is_multiple_of(4)||self.config&3!=1{return None;}
        Some(match off{
            0=>0,0x2c=>self.rd_status[n],
            0x24|0x28 if n==0&&self.mp_rom_compat=>self.opaque_spi0[((off-0x24)/4) as usize],
            // SMEM_AXI_ADDR_CTRL: this functional engine has no queued AXI
            // transfers. Mapped array accesses complete before the next MMIO
            // read; SPI0 software commands remain unsupported below. Thus all
            // five FIFOs are empty and the actual SPI0 engine is idle. This
            // is quiescence, not an invented completion of a pending request.
            0x178 if n==0=>0xfc00_0000,
            0x10 if n==1=>0,0x9c|0xa4 if n==1=>self.port[n].regs.read(off),
            0x58..=0x94 if n==1=>self.port[n].read(off),0x38 if n==1=>u32::MAX,
            0xc0=>self.enable[n],0xc4=>0,0xc8=>self.raw[n],0xcc=>self.raw[n]&self.enable[n],
            0x37c if n==0=>*self.mmu.get(self.index as usize)?,0x380 if n==0=>self.index,0x384 if n==0=>self.power,
            _=>{Self::plain_config(n,off)?;self.port[n].regs.read(off)}
        })
    }
    pub fn write(&mut self,address:u32,value:u32,flash:&mut[u8],psram:&mut[u8],trace:bool)->bool{
        match address{
            0x6009_6018=>{if value&2!=0{let status=self.port[1].status;let down=self.power_down;let reset=self.reset_enabled;let quad=self.quad.clone();let pads=self.native_pads;let xtal=self.xtal_hz;let compat=self.mp_rom_compat;let sources=(self.rc_fast_hz,self.pll_ready);*self=Self::new();self.port[1].status=status;self.power_down=down;self.reset_enabled=reset;self.quad=quad;self.native_pads=pads;self.xtal_hz=xtal;self.mp_rom_compat=compat;self.set_clock_sources(sources.0,sources.1);}self.config=value&7;return true;}
            0x6009_601c=>{self.clock=value&0xfff;return true;}_=>{}
        }
        let n=match address&!0xfff{0x6000_2000=>0,0x6000_3000=>1,_=>return false};let off=address&0xfff;if !off.is_multiple_of(4)||self.config&3!=1{return false;}
        match off{
            0 if n==0=>value==0, // No SPI0 USR/PE/unknown software engine.
            0=>{if !self.enabled(){return false;}self.command(value,flash,psram,trace)}
            0x2c=>{let (reset,mask)=crate::mspi_regs::config(n,off).unwrap();self.rd_status[n]=(value&mask)|(reset&!mask);true}
            0x24|0x28 if n==0&&self.mp_rom_compat=>{let word=&mut self.opaque_spi0[((off-0x24)/4) as usize];if value&1023!=255||value&!1023!=*word&!1023{return false;}*word=value;true}
            0x178 if n==0=>true, // All documented fields are RO.
            0x174 if n==0=>{
                // C5 IDF5.5.3/5.5.4 spi_mem_c_reg.h SPI_SMEM_ECC_CTRL:
                // PAGE_SIZE[19:18] retains the configured burst page size.
                // ECC counting/address conversion bits17/20 have no engine.
                if value&((1<<17)|(1<<20))!=0{return false;}self.port[0].regs.write(off,value&(3<<18));true
            }
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
        if cmd==0{return true;}
        let usr=cmd&(1<<18)!=0;
        // One command trigger, optionally with FLASH_PE for a USR operation.
        // Unknown/multiple triggers must not become successful idle commands.
        if cmd&!0xfffe_0000!=0||(cmd&0xfffc_0000).count_ones()!=1||!usr&&cmd&(1<<17)!=0{return false;}
        // HPM is not modeled. C5 dedicated BE means 32KB, while the shared
        // older engine erases 64KB; reject it rather than use wrong geometry.
        if cmd&((1<<19)|(1<<23))!=0{return false;}
        if cmd&(1<<21)!=0{self.power_down=true;self.raw[1]|=24;return true;}
        if cmd&(1<<20)!=0{self.power_down=false;self.port[1].w.fill(u32::MAX);self.raw[1]|=24;return true;} // release; no fabricated legacy electronic ID
        let psram_clock=self.psram_clock_hz(self.port[1].regs.read(0x14));
        let p=&mut self.port[1];p.jedec=esp_periph::spi_mem::flash_jedec(false,flash.len());p.has_psram=false;p.log=trace;
        if p.regs.read(8)&((1<<10)|(1<<11))!=0{return false;} // unmodeled CRC engine
        let user=p.regs.read(0x18);let misc=p.regs.read(0x34);let opcode=if user&(1<<31)!=0{p.regs.read(0x20) as u8}else{0};
        if usr&&(misc&(1<<10)!=0||(misc&3==0&&!psram.is_empty())){return false;} // unmodeled held-CS sequence / two active devices
        if usr&&misc&3==1{
            let Some(hz)=psram_clock else{return false;};
            let handled=self.quad.transfer(p,psram,self.native_pads,hz);if handled{self.raw[1]|=24;}return handled;
        }
        if usr{
            let rx=if user&(1<<28)!=0{(p.regs.read(0x28)&1023)+1}else{0};let tx=if user&(1<<27)!=0{(p.regs.read(0x24)&1023)+1}else{0};
            // Shared array execution is audited only for byte transfers in
            // W0..W15, ordinary SPI command phase and STR. Quad PSRAM has its
            // own high-buffer and protocol validation above.
            if user&(1<<31)==0||p.regs.read(0x20)>>28!=7||rx!=0&&tx!=0||rx>512||tx>512||!rx.is_multiple_of(8)||!tx.is_multiple_of(8)
                ||user&((3<<24)|(1<<26))!=0||p.regs.read(8)&((7<<4)|(3<<8))!=0||p.regs.read(0xd4)&1!=0{return false;}
        }
        if usr&&misc&1!=0&&misc&2!=0{if user&(1<<28)!=0{p.w.fill(u32::MAX);}self.raw[1]|=24;return true;}
        let flash_selected=!usr||misc&1==0;
        if cmd==1<<31{
            // C5 ADDR[31:24] is the dedicated transfer byte count, not
            // MISO_DLEN. The shared engine uses MISO_DLEN on older paths.
            // Bound this dedicated path to the actual 64-byte data buffer.
            let addr=p.regs.read(4);let len=(addr>>24) as usize;if len==0||len>64{return false;}
            for i in 0..len{let byte=if self.power_down{255}else{*flash.get((addr&0xff_ffff) as usize+i).unwrap_or(&255)};let shift=(i&3)*8;p.w[i/4]=(p.w[i/4]&!(255<<shift))|u32::from(byte)<<shift;}
            self.raw[1]|=24;return true;
        }
        if flash_selected&&usr {
            if opcode==0x5a{
                // Active SPI Mode0 Read SFDP:8-bit command,24-bit address,
                // 8 dummy clocks, byte MISO in the existing low buffer. The
                // 32MiB virtual die matches the earlier actual JEDEC reply.
                // Unsupported windows/states reject before data/end events.
                if cmd!=1<<18||misc!=2||user!=0xf000_0000||p.regs.read(0x1c)&0xfc00_003f!=((23<<26)|7)
                    ||flash.len()!=32<<20||p.jedec!=[0x20,0x40,0x19]||self.power_down||p.pending_cmd!=0||p.status&0x8001!=0
                    ||psram_clock.is_none_or(|hz|hz>80_000_000){return false;}
                let address=p.regs.read(4);if !(0x30..0x38).contains(&address){return false;}
                let start=(address-0x30)as usize;let count=((p.regs.read(0x28)&1023)+1)as usize/8;
                let Some(data)=XMC_C_SFDP_BASIC.get(start..start+count)else{return false;};
                for(i,&byte)in data.iter().enumerate(){let shift=(i&3)*8;p.w[i/4]=(p.w[i/4]&!(255<<shift))|u32::from(byte)<<shift;}
                self.reset_enabled=false;self.raw[1]|=24;return true;
            }
            if opcode==0x7a{
                // IDF5.5.3 C5 bootloader sends command-only Resume before
                // waiting idle. XM25QH256C Rev2.1 sections7.2.36/7.2.52:
                // an idle, non-suspended die ignores it, and an intervening
                // command cancels Reset Enable. The shared array engine has
                // only synchronous jobs; no suspended operation is invented.
                if cmd!=1<<18||misc&3!=2||user&0x7800_0000!=0||self.power_down||p.pending_cmd!=0||p.status&0x8001!=0{return false;}
                self.reset_enabled=false;self.raw[1]|=24;return true;
            }
            if !matches!(opcode,0x03|0x0b|0x3b|0x6b|0xbb|0xeb|0x9f|0x05|0x35|0x06|0x04|0x01|0x31|0x02|0x32|0x38|0x20|0x52|0xd8|0xc7|0x60|0x66|0x99|0xb9|0xab){return false;}
            if matches!(opcode,0x66|0x99|0xb9|0xab)&&user&0x7800_0000!=0{return false;}
            if opcode==0x66{self.reset_enabled=true;self.raw[1]|=24;return true;}
            if opcode==0x99{if self.reset_enabled{p.status=0;self.power_down=false;}self.reset_enabled=false;self.raw[1]|=24;return true;}
            self.reset_enabled=false;
            if opcode==0xb9{self.power_down=true;self.raw[1]|=24;return true;}
            if opcode==0xab{self.power_down=false;self.raw[1]|=24;return true;}
            if self.power_down{if user&(1<<28)!=0{p.w.fill(u32::MAX);}self.raw[1]|=24;return true;}
        }
        let writes=if usr{matches!(opcode,0x01|0x31|0x11|0x02|0x32|0x38|0x20|0x52|0xd8|0xc7|0x60)}else{cmd&0x07c0_0000!=0};
        if flash_selected&&writes&&p.status&2==0{self.raw[1]|=24;return true;}
        let status=if flash_selected&&(usr&&matches!(opcode,0x01|0x31|0x11)||!usr&&cmd&(1<<26)!=0){Some(if usr{match opcode{0x31=>(p.status&255)|(p.w[0]&255)<<8,0x11=>p.status,_=>p.w[0]&65535}}else if p.regs.read(8)&(1<<22)!=0{self.rd_status[1]&65535}else{(p.status&0xff00)|(self.rd_status[1]&255)})}else{None};
        p.write(0,cmd);p.execute(flash,psram);if let Some(status)=status{p.status=status&!3;}
        // RD_STATUS is a software-writable controller latch, independently
        // of the die's live status and WB_MODE configuration (C5 header).
        if cmd==1<<27{self.rd_status[1]=(self.rd_status[1]&!255)|(p.status&255);}
        p.dirty.clear();self.raw[1]|=24;true
    }
}

#[cfg(test)]
mod tests{
    use super::Mspi;
    const SPI0:u32=0x6000_2000;
    const SPI1:u32=0x6000_3000;
    const PCR:u32=0x6009_6018;
    fn put(m:&mut Mspi,address:u32,value:u32){assert!(m.write(address,value,&mut[],&mut[],false),"write {address:#x}={value:#x}");}
    fn command(m:&mut Mspi,value:u32,flash:&mut[u8])->bool{m.write(SPI1,value,flash,&mut[],false)}
    #[test]
    fn fifo_empty_is_actual_synchronous_quiescence_and_ro(){
        let mut m=Mspi::new();assert_eq!(m.read(SPI0+0x178),Some(0xfc00_0000));
        put(&mut m,SPI0+0x178,0);assert_eq!(m.read(SPI0+0x178),Some(0xfc00_0000));
        assert!(!m.write(SPI0,1<<18,&mut[],&mut[],false));assert!(!m.write(SPI0,1<<17,&mut[],&mut[],false));
        assert_eq!(m.read(SPI0+0xc8),Some(0));assert_eq!(m.read(SPI1+0xc8),Some(0));
        put(&mut m,PCR,0);assert_eq!(m.read(SPI0+0x178),None);
        put(&mut m,PCR,3);assert_eq!(m.read(SPI0+0x178),None);
        put(&mut m,PCR,1);assert_eq!(m.read(SPI0+0x178),Some(0xfc00_0000));
    }
    #[test]
    fn mp_rom_words_are_opt_in_independent_and_transactional(){
        let mut m=Mspi::new();for off in [0x24,0x28]{assert_eq!(m.read(SPI0+off),None);assert!(!m.write(SPI0+off,255,&mut[],&mut[],false));}
        m.enable_mp_rom_compat();assert_eq!(m.read(SPI0+0x24),Some(0));assert_eq!(m.read(SPI0+0x28),Some(0));
        put(&mut m,SPI0+0x28,255);assert_eq!(m.read(SPI0+0x24),Some(0));assert_eq!(m.read(SPI0+0x28),Some(255));
        for value in [0,1,254,256,1023,0x4ff,u32::MAX]{assert!(!m.write(SPI0+0x28,value,&mut[],&mut[],false));assert_eq!(m.read(SPI0+0x28),Some(255));}
        put(&mut m,SPI0+0x24,255);assert_eq!(m.read(SPI1+0x24),Some(0));assert_eq!(m.read(SPI1+0x28),Some(0));
        assert_eq!(m.read(SPI0+0xc8),Some(0));assert_eq!(m.read(SPI1+0xc8),Some(0));
        put(&mut m,PCR,0);assert_eq!(m.read(SPI0+0x24),None);assert!(!m.write(SPI0+0x24,255,&mut[],&mut[],false));
        put(&mut m,PCR,3);put(&mut m,PCR,1);assert_eq!(m.read(SPI0+0x24),Some(0));assert_eq!(m.read(SPI0+0x28),Some(0));
        put(&mut m,SPI0+0x24,255);assert!(!m.write(SPI0,1<<18,&mut[],&mut[],false));
    }
    #[test]
    fn dedicated_read_uses_real_address_count_and_then_raises_interrupt(){
        let mut m=Mspi::new();let mut flash=vec![255;0x2100];flash[0x2000..0x2005].copy_from_slice(&[0xe9,6,2,0x20,0x55]);
        put(&mut m,SPI1+0x04,(5<<24)|0x2000);put(&mut m,SPI1+0x28,7);put(&mut m,SPI1+0x5c,0xaaaa_aaaa);put(&mut m,SPI1+0xc0,24);
        assert!(!m.irq());assert!(command(&mut m,1<<31,&mut flash));
        assert_eq!(m.read(SPI1+0x58),Some(0x2002_06e9));assert_eq!(m.read(SPI1+0x5c),Some(0xaaaa_aa55));assert_eq!(m.read(SPI1),Some(0));assert!(m.irq());
        put(&mut m,SPI1+0xc4,24);assert!(!m.irq());
        for len in [0,65,255]{put(&mut m,SPI1+4,(len<<24)|0x2000);assert!(!command(&mut m,1<<31,&mut flash));assert_eq!(m.read(SPI1+0xc8),Some(0));assert_eq!(m.read(SPI1+0x58),Some(0x2002_06e9));}
    }
    #[test]
    fn rd_status_latch_mode_bits_and_physical_status_are_separate(){
        let mut m=Mspi::new();put(&mut m,SPI1+0x2c,0xfa55_1234);assert_eq!(m.read(SPI1+0x2c),Some(0x0a55_1234));assert_eq!(m.port[1].status,0);
        assert!(command(&mut m,1<<27,&mut[]));assert_eq!(m.read(SPI1+0x2c),Some(0x0a55_1200));
        assert!(command(&mut m,1<<30,&mut[]));assert_eq!(m.port[1].status,2);assert_eq!(m.read(SPI1+0x2c),Some(0x0a55_1200));
        assert!(command(&mut m,1<<27,&mut[]));assert_eq!(m.read(SPI1+0x2c),Some(0x0a55_1202));
        let ctrl=m.read(SPI1+8).unwrap();put(&mut m,SPI1+8,ctrl|(1<<22));put(&mut m,SPI1+0x2c,0x0f5a_0200);assert!(command(&mut m,1<<26,&mut[]));assert_eq!(m.port[1].status,0x0200);
        put(&mut m,SPI0+0x2c,u32::MAX);assert_eq!(m.read(SPI0+0x2c),Some(0x0fff_0000));assert_eq!(m.port[1].status,0x0200);
    }
    #[test]
    fn unknown_commands_and_unsupported_host_encodings_do_not_complete(){
        let mut m=Mspi::new();let mut flash=vec![0xa5;64];
        for cmd in [1,1<<17,1<<19,1<<23,(1<<28)|(1<<30),u32::MAX]{assert!(!command(&mut m,cmd,&mut flash));assert_eq!(m.read(SPI1+0xc8),Some(0));assert_eq!(m.port[1].status,0);}
        assert!(command(&mut m,0,&mut flash));assert_eq!(m.read(SPI1+0xc8),Some(0));
        put(&mut m,SPI1+0x18,0x9000_0000);put(&mut m,SPI1+0x20,0x7000_0005);
        for bits in [0,8,512,1023]{put(&mut m,SPI1+0x28,bits);assert!(!command(&mut m,1<<18,&mut flash));assert_eq!(m.read(SPI1+0xc8),Some(0));}
        put(&mut m,SPI1+0x28,7);put(&mut m,SPI1+0x20,0x7000_0071);assert!(!command(&mut m,1<<18,&mut flash));assert_eq!(m.read(SPI1+0xc8),Some(0));
        put(&mut m,SPI1+0x20,0x7000_0005);put(&mut m,SPI1+0x18,0x9100_0000);assert!(!command(&mut m,1<<18,&mut flash));assert_eq!(m.read(SPI1+0xc8),Some(0));
        assert_eq!(flash,[0xa5;64]);
    }
    #[test]
    fn rom_style_user_read_moves_flash_bytes_without_touching_psram(){
        let mut m=Mspi::new();let mut flash=vec![255;0x2100];let expected:Vec<u8>=(0..64).map(|i|i^0x5a).collect();flash[0x2000..0x2040].copy_from_slice(&expected);
        put(&mut m,SPI1+4,0x2000);put(&mut m,SPI1+0x18,0xd000_0000);put(&mut m,SPI1+0x1c,23<<26);put(&mut m,SPI1+0x20,0x7000_0003);put(&mut m,SPI1+0x28,511);
        let mut psram=vec![0xc3;8<<20];m.configure_psram(psram.len()).unwrap();assert!(m.write(SPI1,1<<18,&mut flash,&mut psram,false));
        let actual:Vec<u8>=(0..64).map(|i|(m.read(SPI1+0x58+(i/4)*4).unwrap()>>((i&3)*8)) as u8).collect();assert_eq!(actual,expected);assert!(psram.iter().all(|&v|v==0xc3));
        assert_eq!(m.read(SPI1+0xc8),Some(24));assert_eq!(m.read(SPI0+0x178),Some(0xfc00_0000));
    }
    fn idle_resume_config(m:&mut Mspi){
        // Exact original LCD273 bootloader request at PC4084e7ac.
        m.set_clock_sources(Some(17_500_000),true);put(m,PCR,5);put(m,PCR+4,0x605);
        for(off,value)in[(8,0x0020_0000),(0x18,0x8000_0000),(0x20,0x7000_007a),(0x24,0),(0x28,0),(0x34,2),(0xd4,0x20)]{put(m,SPI1+off,value);}
    }
    #[test]
    fn original_bootloader_idle_resume_preserves_arrays_status_and_buffers(){
        let mut m=Mspi::new();idle_resume_config(&mut m);m.configure_psram(8<<20).unwrap();
        let mut flash=vec![0xa5;32<<20];let mut psram=vec![0xc3;8<<20];flash[0x2000]=0xe9;flash[(32<<20)-1]=0x5a;
        let flash_before=flash.clone();let psram_before=psram.clone();map_flash(&mut m);let mmu=m.mmu;
        for i in 0..16{put(&mut m,SPI1+0x58+i*4,0x1234_0000+i);}let words=m.port[1].w;
        m.port[1].status=0x202;put(&mut m,SPI1+0x2c,0x055a_1234);let latch=m.rd_status;put(&mut m,SPI1+0xc0,31);
        assert_eq!(m.read(PCR),Some(13));assert!(!m.irq());assert!(m.write(SPI1,1<<18,&mut flash,&mut psram,false));assert!(m.irq());
        for _ in 0..3{assert_eq!(m.read(SPI1),Some(0));assert_eq!(m.read(SPI1+0xc8),Some(24));assert_eq!(m.read(SPI1+0xcc),Some(24));}
        assert_eq!(m.port[1].status,0x202);assert_eq!(m.port[1].w,words);assert_eq!(m.rd_status,latch);assert_eq!(m.mmu,mmu);
        assert_eq!(m.port[1].pending_cmd,0);assert_eq!(flash,flash_before);assert_eq!(psram,psram_before);assert!(!m.power_down);
        put(&mut m,SPI1+0xc4,24);assert!(!m.irq());assert_eq!(m.raw,[0;2]);
        for(opcode,expected)in[(5,2),(0x35,2)]{
            put(&mut m,SPI1+0x18,0x9000_0000);put(&mut m,SPI1+0x20,0x7000_0000|opcode);put(&mut m,SPI1+0x28,7);
            assert!(m.write(SPI1,1<<18,&mut flash,&mut psram,false));assert_eq!(m.port[1].w[0]&255,expected);assert_eq!(m.port[1].status,0x202);
        }
    }
    #[test]
    fn resume_after_real_synchronous_program_has_no_pending_job_to_complete(){
        let mut m=Mspi::new();idle_resume_config(&mut m);let mut flash=vec![255;64];
        assert!(command(&mut m,1<<30,&mut flash));assert_eq!(m.port[1].status,2);
        put(&mut m,SPI1+4,4);put(&mut m,SPI1+0x18,0xc800_0000);put(&mut m,SPI1+0x1c,23<<26);
        put(&mut m,SPI1+0x20,0x7000_0002);put(&mut m,SPI1+0x24,31);put(&mut m,SPI1+0x58,0x7856_3412);
        assert!(command(&mut m,1<<18,&mut flash));assert_eq!(&flash[4..8],&[0x12,0x34,0x56,0x78]);assert_eq!(m.port[1].status&3,0);assert_eq!(m.port[1].pending_cmd,0);
        let expected=flash.clone();let words=m.port[1].w;idle_resume_config(&mut m);put(&mut m,SPI1+0xc4,u32::MAX);
        assert!(command(&mut m,1<<18,&mut flash));assert_eq!(flash,expected);assert_eq!(m.port[1].w,words);assert_eq!(m.port[1].status&3,0);assert_eq!(m.raw[1],24);
    }
    #[test]
    fn unsupported_resume_states_and_phases_reject_without_completion_or_state_changes(){
        for case in 0..22{
            let mut m=Mspi::new();idle_resume_config(&mut m);let mut flash=vec![0xa5;64];let mut psram=vec![0xc3;64];let mut cmd=1<<18;
            m.reset_enabled=true;m.port[1].status=0x202;m.port[1].w.fill(0x7654_3210);
            match case{
                0=>m.port[1].status|=1,1=>m.port[1].status|=0x8000,2=>m.port[1].pending_cmd=1<<30,3=>m.power_down=true,
                4..=7=>put(&mut m,SPI1+0x18,(1<<31)|(1<<(case+23))),8=>cmd|=1<<17,9=>put(&mut m,SPI1+0x20,0x6000_007a),
                10=>put(&mut m,SPI1+0xd4,0x21),11=>put(&mut m,SPI1+8,(1<<21)|(1<<8)),12=>put(&mut m,SPI1+0x34,2|(1<<10)),
                13=>put(&mut m,SPI1+0x34,0),14=>m.set_clock_sources(Some(17_500_000),false),15=>put(&mut m,PCR,1),
                16=>put(&mut m,PCR+4,0xe05),17=>put(&mut m,PCR+4,0x705),18=>put(&mut m,SPI1+0x18,0x8100_0000),
                19..=21=>put(&mut m,SPI1+0x20,0x7000_0000|[0x75,0x7b,0x30][case-19]),_=>unreachable!(),
            }
            let state=(m.port[1].status,m.port[1].pending_cmd,m.port[1].w,m.raw,m.power_down,m.reset_enabled,m.rd_status);
            assert!(!m.write(SPI1,cmd,&mut flash,&mut psram,false),"case {case}");
            assert_eq!((m.port[1].status,m.port[1].pending_cmd,m.port[1].w,m.raw,m.power_down,m.reset_enabled,m.rd_status),state,"case {case}");
            assert_eq!(flash,[0xa5;64]);assert_eq!(psram,[0xc3;64]);
        }
    }
    #[test]
    fn idle_resume_reset_enable_and_chip_select_follow_delivered_commands(){
        let mut m=Mspi::new();idle_resume_config(&mut m);m.port[1].status=0x202;let mut flash=vec![0xa5;64];
        put(&mut m,SPI1+0x20,0x7000_0066);assert!(command(&mut m,1<<18,&mut flash));assert!(m.reset_enabled);
        put(&mut m,SPI1+0x20,0x7000_007a);put(&mut m,SPI1+0x34,3);assert!(command(&mut m,1<<18,&mut flash));assert!(m.reset_enabled,"no flash selected");
        put(&mut m,SPI1+0x34,1);assert!(command(&mut m,1<<18,&mut flash));assert!(m.reset_enabled,"existing CS1 no-response path must not reach flash");
        put(&mut m,SPI1+0x34,2);assert!(command(&mut m,1<<18,&mut flash));assert!(!m.reset_enabled);assert_eq!(m.port[1].status,0x202);
        put(&mut m,SPI1+0x20,0x7000_0099);assert!(command(&mut m,1<<18,&mut flash));assert_eq!(m.port[1].status,0x202);assert_eq!(flash,[0xa5;64]);
        put(&mut m,SPI0+0xc4,u32::MAX);assert!(!m.write(SPI0,1<<18,&mut flash,&mut[],false));assert_eq!(m.raw[0],0);
    }
    #[test]
    fn virtual_jedec_capacity_and_mmu_cover_32_mib_without_aliasing(){
        let mut m=Mspi::new();let mut flash=vec![255;32<<20];assert!(command(&mut m,1<<28,&mut flash));assert_eq!(m.read(SPI1+0x58).unwrap()&0xff_ffff,0x0019_4020);
        put(&mut m,SPI0+0x3c,1);put(&mut m,SPI0+0x384,4);put(&mut m,SPI0+0x380,511);put(&mut m,SPI0+0x37c,0x5ff);
        assert_eq!(m.translate(0x43ff_fffc),Some((false,(32<<20)-4)));assert_eq!(m.translate(0x4400_0000),None);
        m.configure_psram(8<<20).unwrap();put(&mut m,SPI0+0x380,0);put(&mut m,SPI0+0x37c,0x600);assert_eq!(m.translate(0x4200_0000),Some((true,0)));
        assert!(command(&mut m,1<<28,&mut flash));assert_eq!(m.read(SPI1+0x58).unwrap()&0xff_ffff,0x0019_4020);
    }
    fn map_flash(m:&mut Mspi){put(m,SPI0+0x3c,0);put(m,SPI0+0x384,4);put(m,SPI0+0x380,0);put(m,SPI0+0x37c,0x400);}
    fn mapped_flash(m:&Mspi,flash:&[u8],address:u32)->Option<u8>{let(psram,off)=m.translate(address)?;if psram{None}else{flash.get(off).copied()}}
    #[test]
    fn source_observations_gate_real_flash_reads_commands_and_other_boundaries(){
        let mut m=Mspi::new();map_flash(&mut m);let mut flash=vec![0xe9,0x13,0x52,0xa7];
        put(&mut m,PCR+4,0x600);assert_eq!(mapped_flash(&m,&flash,0x4200_0002),None);assert!(!command(&mut m,1<<28,&mut flash));assert_eq!(m.raw,[0;2]);
        m.enable_mp_rom_compat();assert!(m.mp_rom_compat());assert!(!m.enabled(),"ROM compatibility cannot invent a PLL");
        m.set_clock_sources(None,true);assert_eq!(m.functional_clock_hz(),Some(480_000_000));assert_eq!(mapped_flash(&m,&flash,0x4200_0002),Some(0x52));
        flash[2]=0xbc;assert_eq!(mapped_flash(&m,&flash,0x4200_0002),Some(0xbc));assert!(command(&mut m,1<<28,&mut flash));
        let data=m.port[1].w;let raw=m.raw;m.set_clock_sources(None,false);assert!(!command(&mut m,1<<28,&mut flash));assert_eq!(m.port[1].w,data);assert_eq!(m.raw,raw);assert_eq!(mapped_flash(&m,&flash,0x4200_0002),None);
        m.set_clock_sources(None,true);m.disable_mp_rom_compat();assert!(!m.mp_rom_compat());assert!(m.enabled());
        for conf in[0,1,3]{put(&mut m,PCR,conf);put(&mut m,PCR+4,0x600);assert!(!m.enabled());assert!(!command(&mut m,1<<28,&mut flash));}
        put(&mut m,PCR,5);map_flash(&mut m);
        for clock in[0x200,0xe00,0x700]{put(&mut m,PCR+4,clock);assert_eq!(mapped_flash(&m,&flash,0x4200_0002),None);assert!(!command(&mut m,1<<28,&mut flash));}
        put(&mut m,PCR+4,0x600);put(&mut m,SPI0+0x384,6);assert_eq!(mapped_flash(&m,&flash,0x4200_0002),None);
        put(&mut m,SPI0+0x384,4);put(&mut m,SPI0+0x3c,1<<31);assert_eq!(mapped_flash(&m,&flash,0x4200_0002),None);
        put(&mut m,SPI0+0x3c,0);put(&mut m,SPI0+0x37c,0xc00);assert_eq!(mapped_flash(&m,&flash,0x4200_0002),None);
    }
    #[test]
    fn low_speed_dividers_and_pll_source_are_distinct_and_never_assume_readiness(){
        let mut m=Mspi::new();m.set_xtal_hz(48_000_000);
        for(div,hz)in[(0,48_000_000),(1,24_000_000),(2,12_000_000)]{put(&mut m,PCR+4,0x400|div);assert_eq!(m.functional_clock_hz(),Some(hz));}
        put(&mut m,PCR+4,0x500);assert_eq!(m.functional_clock_hz(),None);m.set_clock_sources(Some(17_500_000),false);
        for(div,hz)in[(0,17_500_000),(1,8_750_000),(2,4_375_000)]{put(&mut m,PCR+4,0x500|div);assert_eq!(m.functional_clock_hz(),Some(hz));}
        let mut flash=vec![0xe9,0x13,0x52,0xa7];map_flash(&mut m);assert_eq!(mapped_flash(&m,&flash,0x4200_0001),Some(0x13));
        put(&mut m,SPI1+4,2<<24);assert!(command(&mut m,1<<31,&mut flash));assert_eq!(m.read(SPI1+0x58).unwrap()&0xffff,0x13e9);
        m.set_clock_sources(None,false);assert_eq!(mapped_flash(&m,&flash,0x4200_0001),None);assert!(!command(&mut m,1<<31,&mut flash));
        put(&mut m,PCR+4,0x503);assert!(!m.enabled());m.set_clock_sources(Some(0),true);put(&mut m,PCR+4,0x500);assert!(!m.enabled());
        for(div,hz)in[(0,480_000_000),(1,240_000_000),(2,160_000_000),(3,120_000_000),(5,80_000_000),(255,1_875_000)]{put(&mut m,PCR+4,0x600|div);assert_eq!(m.functional_clock_hz(),Some(hz));}
        put(&mut m,PCR+4,0x700);assert!(!m.enabled());m.set_clock_sources(None,false);put(&mut m,PCR+4,0x600);assert!(!m.enabled());
    }
    fn quad_command(m:&mut Mspi,psram:&mut[u8],opcode:u8,qpi:bool,phases:(Option<u32>,Option<u32>,bool,u32))->bool{
        let(address,tx,rx,dummy)=phases;
        put(m,SPI1+8,if qpi{(1<<8)|(1<<13)|(1<<24)}else{0});put(m,SPI1+4,address.unwrap_or(0));
        put(m,SPI1+0x18,(1<<31)|if address.is_some(){1<<30}else{0}|if tx.is_some(){1<<27}else{0}|if rx{1<<28}else{0}|if dummy!=0{1<<29}else{0}|if qpi{1<<15}else{0});
        put(m,SPI1+0x1c,(23<<26)|dummy.saturating_sub(1));put(m,SPI1+0x20,(7<<28)|opcode as u32);put(m,SPI1+0x24,31);put(m,SPI1+0x28,31);
        if let Some(word)=tx{put(m,SPI1+0x58,word);}m.write(SPI1,1<<18,&mut[],psram,false)
    }
    #[test]
    fn factory_connection_check_uses_the_pcr_and_serial_dividers_without_weakening_die_checks(){
        // Original LCD273 F5/02/03 snapshots: PCR605, SPI1 CLOCK30103,
        // 24-bit address0, 32-bit payload5a6b7c8d. Each source observation is
        // supplied at this component boundary; native analog readiness is separate.
        for(clock,pads,address_bits,ready,responds)in[(0x605,true,23,true,true),(0x603,true,23,true,true),(0x600,true,23,true,false),(0x605,false,23,true,false),(0x605,true,22,true,false),(0x605,true,23,false,false)]{
            let mut m=Mspi::new();m.configure_psram(8<<20).unwrap();let mut memory=vec![0;8<<20];
            for pin in[15,17,18,20,21,22]{m.set_pad(pin,0);}if !pads{m.set_pad(15,1);}m.set_clock_sources(None,ready);put(&mut m,PCR+4,clock);
            for(off,value)in[(0x14,0x0003_0103),(0x34,1),(0x08,0x012c_210c),(0x18,0x8000_8000),(0x1c,0),(0x20,0x7000_00f5),(0xd4,0x20)]{put(&mut m,SPI1+off,value);}
            assert_eq!(m.write(SPI1,1<<18,&mut[],&mut memory,false),ready);assert!(!m.quad.qpi());
            if !ready{assert_eq!(m.raw,[0;2]);assert_eq!(&memory[..4],&[0;4]);continue;}
            let expected_hz=if clock==0x605{20_000_000}else if clock==0x603{30_000_000}else{120_000_000};assert_eq!(m.psram_clock_hz(m.port[1].regs.read(0x14)),Some(expected_hz));
            for(off,value)in[(0x04,0),(0x08,0x002c_000c),(0x18,0xc800_0000),(0x1c,address_bits<<26),(0x20,0x7000_0002),(0x24,31),(0x28,0),(0x58,0x5a6b_7c8d)]{put(&mut m,SPI1+off,value);}
            assert!(m.write(SPI1,1<<18,&mut[],&mut memory,false));assert_eq!(&memory[..4],if responds{&[0x8d,0x7c,0x6b,0x5a]}else{&[0;4]});
            for(off,value)in[(0x18,0xd000_0000),(0x20,0x7000_0003),(0x24,0),(0x28,31)]{put(&mut m,SPI1+off,value);}
            assert!(m.write(SPI1,1<<18,&mut[],&mut memory,false));assert_eq!(m.read(SPI1+0x58),Some(if responds{0x5a6b_7c8d}else{u32::MAX}));
        }
    }
    #[test]
    fn psram_page_word_accepts_disabled_ecc_configuration_and_rejects_engines_transactionally(){
        let mut m=Mspi::new();let mut flash=vec![0x5a;64];let mut memory=vec![0xa5;64];let word=SPI0+0x174;
        assert_eq!(m.read(word),Some(2<<18));let old=m.read(word).unwrap();
        assert!(m.write(word,(old&!(3<<18))|(2<<18),&mut flash,&mut memory,false));assert_eq!(m.read(word),Some(0x0008_0000));
        for page in 0..4{
            assert!(m.write(word,(page<<18)|(1<<31),&mut flash,&mut memory,false));assert_eq!(m.read(word),Some(page<<18));
            assert!(!m.psram_access(false)&&!m.psram_access(true),"a page setting is not QPI/phase initialization");
            for ecc in[1<<17,1<<20,(1<<17)|(1<<20)]{
                let raw=m.raw;let before=m.read(word);let ports=[m.port[0].w,m.port[1].w];
                assert!(!m.write(word,ecc|((3-page)<<18),&mut flash,&mut memory,false));assert_eq!(m.read(word),before);assert_eq!(m.raw,raw);assert_eq!([m.port[0].w,m.port[1].w],ports);
                assert_eq!(flash,[0x5a;64]);assert_eq!(memory,[0xa5;64]);
            }
        }
        for address in[SPI1+0x174,SPI0+0x168,SPI0+0x16c,SPI0+0x170,SPI0+0x17c]{assert_eq!(m.read(address),None);assert!(!m.write(address,0,&mut flash,&mut memory,false));}
        put(&mut m,PCR,0);assert_eq!(m.read(word),None);assert!(!m.write(word,0,&mut flash,&mut memory,false));put(&mut m,PCR,7);assert_eq!(m.read(word),None);put(&mut m,PCR,5);assert_eq!(m.read(word),Some(2<<18));
    }
    #[test]
    fn pll_serial_frequency_and_quad_wire_phases_control_actual_psram_cells(){
        let mut m=Mspi::new();m.configure_psram(8<<20).unwrap();assert_eq!(m.psram_size(),8<<20);let mut psram=vec![0;8<<20];
        for pin in[15,17,18,20,21,22]{m.set_pad(pin,0);}put(&mut m,SPI1+0x34,1);put(&mut m,PCR+4,0x600);m.set_clock_sources(None,true);
        put(&mut m,SPI1+0x14,(23<<16)|(11<<8)|23);assert_eq!(m.psram_clock_hz(m.port[1].regs.read(0x14)),Some(20_000_000));
        assert!(quad_command(&mut m,&mut psram,0x35,false,(None,None,false,0)));assert!(m.quad.qpi());
        put(&mut m,SPI1+0x14,(5<<16)|(2<<8)|5);assert_eq!(m.psram_clock_hz(m.port[1].regs.read(0x14)),Some(80_000_000));
        assert!(quad_command(&mut m,&mut psram,0x38,true,(Some(4),Some(0x7856_3412),false,0)));assert_eq!(&psram[4..8],&[0x12,0x34,0x56,0x78]);
        assert!(quad_command(&mut m,&mut psram,0xeb,true,(Some(4),None,true,6)));assert_eq!(m.read(SPI1+0x58),Some(0x7856_3412));
        put(&mut m,SPI0+0x40,(1<<20)|(23<<14)|(5<<6)|0x34);put(&mut m,SPI0+0x48,(7<<28)|0xeb);put(&mut m,SPI0+0x4c,(7<<28)|0x38);put(&mut m,SPI0+0x50,(5<<16)|(2<<8)|5);
        for page in 0..4{put(&mut m,SPI0+0x174,page<<18);assert!(m.psram_access(false)&&m.psram_access(true));}
        assert!(m.psram_access(false)&&m.psram_access(true));m.set_clock_sources(None,false);assert!(!m.psram_access(false)&&!m.psram_access(true));
        let cells=psram[4..8].to_vec();let raw=m.raw;assert!(!m.write(SPI1,1<<18,&mut[],&mut psram,false));assert_eq!(&psram[4..8],cells);assert_eq!(m.raw,raw);
        m.set_clock_sources(None,true);put(&mut m,SPI0+0x50,0x8000_0000);assert!(!m.psram_access(false),"480MHz exceeds the die's supported wire rate");
        put(&mut m,SPI0+0x50,(5<<16)|(2<<8)|5);m.set_pad(18,1);assert!(!m.psram_access(false));m.set_pad(18,0);
        put(&mut m,SPI0+0x48,(7<<28)|3);assert!(!m.psram_access(false),"QPI still needs its real read opcode");
    }
    #[test]
    fn controller_reset_preserves_supplied_clock_observations_without_copying_them_from_a_die(){
        let mut m=Mspi::new();m.set_clock_sources(Some(17_500_000),true);m.configure_psram(8<<20).unwrap();put(&mut m,PCR+4,0x600);
        put(&mut m,PCR,7);assert!(!m.enabled());put(&mut m,PCR,5);put(&mut m,PCR+4,0x600);assert!(m.enabled());
        put(&mut m,PCR+4,0x500);assert_eq!(m.functional_clock_hz(),Some(17_500_000));
        let mut fresh=Mspi::new();fresh.preserve_psram(&m);assert_eq!(fresh.psram_size(),8<<20);put(&mut fresh,PCR+4,0x600);assert!(!fresh.enabled());
        m.set_clock_sources(None,false);put(&mut m,PCR,7);put(&mut m,PCR,5);put(&mut m,PCR+4,0x600);assert!(!m.enabled());
    }
    fn sfdp_config(m:&mut Mspi,address:u32,bytes:u32){
        m.set_clock_sources(None,true);put(m,PCR+4,0x605);
        // Original final SPI1 snapshot at4080f804, except parameterized ADDR/RX.
        for(off,value)in[(4,address),(8,0),(0xc,0x003f_f7fc),(0x14,0x8000_0000),
            (0x18,0xf000_0000),(0x1c,0x5c00_0007),(0x20,0x7000_005a),(0x24,0),(0x28,bytes*8-1),(0x34,2),(0x98,0x0005_0001),(0xd4,0x20)]{put(m,SPI1+off,value);}
        m.port[1].jedec=esp_periph::spi_mem::flash_jedec(false,32<<20);
    }
    #[test]
    fn factory_sfdp_probe_reads_xmc_c_byte_without_claiming_d_series_caps(){
        let mut m=Mspi::new();sfdp_config(&mut m,0x32,1);m.port[1].w[0]=0x0019_4020;put(&mut m,SPI1+0xc0,24);
        let mut flash=vec![0xa5;32<<20];let mut psram=[0xc3;64];
        assert!(m.write(SPI1,1<<18,&mut flash,&mut psram,false));
        assert_eq!(m.port[1].jedec,[0x20,0x40,0x19]);assert_eq!(m.port[1].w[0],0x0019_40f3);
        assert_eq!(m.port[1].w[0]&8,0,"IDF553/554 generic_get_caps must not classify the C part as D");
        assert_eq!(m.read(SPI1),Some(0));assert_eq!(m.read(SPI1+0xc8),Some(24));assert!(m.irq());
        assert!(flash.iter().all(|&b|b==0xa5));assert_eq!(psram,[0xc3;64]);assert_eq!(m.port[1].status,0);
    }
    #[test]
    fn published_sfdp_window_reads_preserve_tail_status_protection_and_arrays(){
        let expected=[0xe5,0x20,0xf3,0xff,0xff,0xff,0xff,0x0f];let mut flash=vec![0x5a;32<<20];
        for(address,length)in[(0x30,8),(0x30,4),(0x32,1),(0x32,2),(0x34,4),(0x37,1)]{
            let mut m=Mspi::new();sfdp_config(&mut m,address,length);m.port[1].status=0x027e;
            m.rd_status=[0x0a55_1234,0x055a_5678];m.port[1].w.fill(0x9988_7766);let prior=m.port[1].w;
            assert!(command(&mut m,1<<18,&mut flash));
            for byte in 0..64{let value=(m.port[1].w[byte/4]>>((byte&3)*8))as u8;
                assert_eq!(value,if byte<length as usize{expected[address as usize-0x30+byte]}else{(prior[byte/4]>>((byte&3)*8))as u8});}
            assert_eq!(m.port[1].status,0x027e);assert_eq!(m.rd_status,[0x0a55_1234,0x055a_5678]);assert_eq!(m.port[1].pending_cmd,0);
        }
        assert!(flash.iter().all(|&b|b==0x5a));
    }
    #[test]
    fn sfdp_unsupported_ranges_phases_clocks_and_states_never_complete(){
        let mut flash=vec![0xa5;32<<20];let mut psram=[0xc3;64];
        for case in 0..26{
            let mut m=Mspi::new();sfdp_config(&mut m,0x32,1);m.port[1].status=0x202;m.reset_enabled=true;m.port[1].w.fill(0x7654_3210);
            let mut cmd=1<<18;
            match case{
                0=>put(&mut m,SPI1+4,0x2f),1=>put(&mut m,SPI1+4,0x38),2=>put(&mut m,SPI1+0x28,55),
                3=>put(&mut m,SPI1+4,0x0100_0032),4=>put(&mut m,SPI1+0x1c,(31<<26)|7),5=>put(&mut m,SPI1+0x1c,(23<<26)|6),
                6..=9=>put(&mut m,SPI1+0x18,0xf000_0000&!(1<<(case+22))),
                10=>put(&mut m,SPI1+0x18,0xf800_0000),11=>put(&mut m,SPI1+0x18,0xf100_0000),12=>put(&mut m,SPI1+0x20,0x6000_005a),
                13=>put(&mut m,SPI1+8,1<<8),14=>put(&mut m,SPI1+0xd4,0x21),15=>put(&mut m,SPI1+0x34,0x402),
                16=>{m.power_down=true;},17=>{m.port[1].status|=1;},18=>{m.port[1].status|=0x8000;},19=>{m.port[1].pending_cmd=1;},
                20=>{cmd|=1<<17;},21=>put(&mut m,PCR+4,0x603),22=>m.set_clock_sources(None,false),
                23=>put(&mut m,SPI1+0x14,0x0002_0003),24=>put(&mut m,SPI1+0x28,0),25=>put(&mut m,SPI1+0x34,0x202),_=>unreachable!(),
            }
            let words=m.port[1].w;let state=(m.port[1].status,m.port[1].pending_cmd,m.raw,m.rd_status,m.reset_enabled,m.power_down);let mmu=m.mmu;
            assert!(!m.write(SPI1,cmd,&mut flash,&mut psram,false),"case{case}");assert_eq!(m.port[1].w,words);
            assert_eq!((m.port[1].status,m.port[1].pending_cmd,m.raw,m.rd_status,m.reset_enabled,m.power_down),state);assert_eq!(m.mmu,mmu);
        }
        assert!(flash.iter().all(|&b|b==0xa5));assert_eq!(psram,[0xc3;64]);
        let mut m=Mspi::new();sfdp_config(&mut m,0x32,1);assert!(!command(&mut m,1<<18,&mut flash[..16<<20]));assert_eq!(m.raw,[0;2]);
    }
    #[test]
    fn sfdp_command_cancels_reset_enable_only_when_delivered_to_active_flash(){
        let mut m=Mspi::new();let mut flash=vec![0xa5;32<<20];sfdp_config(&mut m,0x32,1);m.port[1].status=0x202;
        // Real digital command sequence66,5a,99; no register-status write occurs.
        put(&mut m,SPI1+0x18,0x8000_0000);put(&mut m,SPI1+0x20,0x7000_0066);assert!(command(&mut m,1<<18,&mut flash));assert!(m.reset_enabled);
        sfdp_config(&mut m,0x32,1);assert!(command(&mut m,1<<18,&mut flash));assert!(!m.reset_enabled);
        put(&mut m,SPI1+0x18,0x8000_0000);put(&mut m,SPI1+0x20,0x7000_0099);assert!(command(&mut m,1<<18,&mut flash));assert_eq!(m.port[1].status,0x202);
        sfdp_config(&mut m,0x32,1);put(&mut m,SPI1+0x34,3);m.reset_enabled=true;
        assert!(command(&mut m,1<<18,&mut flash));assert_eq!(m.port[1].w,[u32::MAX;16]);assert!(m.reset_enabled,"no CS: no SFDP command delivered to flash");
    }
}
