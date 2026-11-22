//! Source-backed S2 MMIO and original guest probes. These tests do not claim
//! vendor firmware, ESP-IDF boot or physical timing/hardware certification.
use emu_core::{Bus,Core,Fault,Trap};
use esp_soc::{Soc,SocBus};
use esp32s2::{S2Bus,S2Core,bus::{IRAM_BASE,DRAM_BASE,SRAM_SIZE,GPIO_BASE,IO_MUX_BASE,UART0,UART1},board::{BASE,WOOR,DONGLE}};

const CONSOLE:&[u8]=include_bytes!("fixtures/s2-sram-console.bin");
const SCREEN:&[u8]=include_bytes!("fixtures/dongle-sram-diagnostic.bin");
const MARKER:u32=0x3fff_f000;
fn board(id:&str)->S2Bus {let mut b=S2Bus::new();b.attach_board(esp32s2::board::make_board(id).unwrap());b}
fn put(b:&mut S2Bus,a:u32,v:u32){b.write32(a,v).unwrap();}
fn mux(b:&mut S2Bus,pin:u8,function:u32){put(b,IO_MUX_BASE+4+4*u32::from(pin),(function<<12)|(1<<9)|(1<<8));}
fn level(b:&mut S2Bus,pin:u8,high:bool){let off=match(pin<32,high){(true,true)=>8,(true,false)=>12,(false,true)=>20,(false,false)=>24};put(b,GPIO_BASE+off,1<<(pin%32));}
fn output(b:&mut S2Bus,pin:u8,high:bool){mux(b,pin,1);put(b,GPIO_BASE+0x554+4*u32::from(pin),256);put(b,GPIO_BASE+if pin<32{0x24}else{0x30},1<<(pin%32));level(b,pin,high);}
fn spi(b:&mut S2Bus,dc:bool,data:&[u8]){
    level(b,34,false);level(b,37,dc);
    for &byte in data {for bit in(0..8).rev(){level(b,36,false);level(b,35,byte&(1<<bit)!=0);level(b,36,true);}}
    level(b,34,true);
}
fn cmd(b:&mut S2Bus,command:u8,data:&[u8]){spi(b,false,&[command]);if !data.is_empty(){spi(b,true,data);}}
fn init(b:&mut S2Bus){for(pin,high)in[(33,true),(34,true),(35,false),(36,false),(37,false),(38,true)]{output(b,pin,high);}cmd(b,0x11,&[]);cmd(b,0x29,&[]);cmd(b,0x3a,&[0x55]);cmd(b,0x36,&[8]);}
fn pixel(b:&mut S2Bus,color:u16){cmd(b,0x2a,&[0,52,0,52]);cmd(b,0x2b,&[0,40,0,40]);cmd(b,0x2c,&color.to_be_bytes());}

#[test]
fn shared_sram_aliases_have_one_code_version_and_strict_boundaries(){
    let mut b=S2Bus::new();put(&mut b,DRAM_BASE+16,0x12345678);assert_eq!(b.read32(IRAM_BASE+16),Ok(0x12345678));
    b.write8(IRAM_BASE+17,0xab).unwrap();assert_eq!(b.read32(DRAM_BASE+16),Ok(0x1234ab78));
    let last=DRAM_BASE+SRAM_SIZE as u32-4;put(&mut b,last,7);assert_eq!(b.read32(IRAM_BASE+SRAM_SIZE as u32-4),Ok(7));
    assert_eq!(b.read32(DRAM_BASE+1),Err(Fault::Misaligned));assert_eq!(b.write32(IRAM_BASE+SRAM_SIZE as u32,1),Err(Fault::Unmapped));
    assert_eq!(b.fetch(DRAM_BASE),Err(Fault::Prohibited));assert!(b.load_bytes(0x40000000,&[0;4]).is_err(),"ROM may not be populated");
    assert!(b.load_bytes(0x50000000,&[0;4]).is_err(),"RTC RAM is outside this contract");
    b.load_bytes(IRAM_BASE,&[0x22,0xa0,5,0]).unwrap();let mut c=S2Core::new();c.pc=IRAM_BASE;c.ps=0;
    assert_eq!(c.step(&mut b).result(),Ok(()));assert_eq!(c.get_ar(2),5);
    b.write8(DRAM_BASE+2,7).unwrap();c.pc=IRAM_BASE;
    assert_eq!(c.step(&mut b).result(),Ok(()));assert_eq!(c.get_ar(2),7,"write through DRAM invalidates an IRAM decode");
    b.load_bytes(IRAM_BASE+SRAM_SIZE as u32-2,&[0x0c,0x12]).unwrap();assert!(b.fetch(IRAM_BASE+SRAM_SIZE as u32-2).is_ok(),"a complete density instruction fits at the boundary");
    b.write8(DRAM_BASE+SRAM_SIZE as u32-2,0x22).unwrap();assert_eq!(b.fetch(IRAM_BASE+SRAM_SIZE as u32-2),Err(Fault::Prohibited));
}

#[test]
fn s2_instruction_options_do_not_inherit_s3_extensions(){
    use xtensa_lx7::decode::{decode,Op};
    for(bytes,op)in[([0x76,0x82,0,0],Op::Loop),([0x20,0x32,0x02,0],Op::Andb),([0x04,0,0,0],Op::Pie),([0x44,0x40,0x24,0],Op::Mac16),([0x20,0x32,0x0a,0],Op::AddS),([0x22,0xe3,0,0],Op::S32c1i),([0x20,2,0x13,0],Op::Wsr)]{
        assert_eq!(decode(IRAM_BASE,bytes).op,op);
        let mut b=S2Bus::new();b.load_bytes(IRAM_BASE,&bytes).unwrap();let mut c=S2Core::new();c.pc=IRAM_BASE;c.ps=0;c.set_ar(2,99);
        Core::set_jit(&mut c,true);
        let(steps,trap)=c.run(&mut b,100);assert_eq!(steps,1);assert!(matches!(trap,Some(Trap::Unimplemented(pc,_))if pc==IRAM_BASE));
        assert_eq!(c.get_ar(2),99);assert_eq!(c.pc,IRAM_BASE);assert_eq!(c.code_cache_stats().unwrap().2,0);
    }
    let c=S2Core::new();assert_eq!(c.configid,[0xc2ecfafe,0x22075f76]);
}

#[test]
fn s2_uart_and_gpio_have_s2_addresses_real_inputs_and_output_gates(){
    for id in[WOOR,BASE,DONGLE]{
        let mut b=board(id);assert_eq!(b.board.named_pin("boot"),Some(0));assert!(!b.board.uses_gpio_waveform());
        assert_eq!(b.read32(GPIO_BASE+0x3c).unwrap()&1,0,"input buffer starts disabled");mux(&mut b,0,1);assert_ne!(b.read32(GPIO_BASE+0x3c).unwrap()&1,0);
        b.gpio_set_input(0,false);assert_eq!(b.read32(GPIO_BASE+0x3c).unwrap()&1,0);b.gpio_set_input(0,true);assert_ne!(b.read32(GPIO_BASE+0x3c).unwrap()&1,0);
        put(&mut b,IO_MUX_BASE+4,1<<12);assert_eq!(b.read32(GPIO_BASE+0x3c).unwrap()&1,0,"FUN_IE gates the sampled register");
        b.uart_input(0,b"AB");assert_eq!(b.read32(UART0+0x1c).unwrap()&0x3ff,2);assert_eq!(b.read8(UART0),Ok(b'A'));assert_eq!(b.read32(0x60000000),Ok(u32::from(b'B')),"documented S2 AHB FIFO alias");
        b.uart_input(1,b"C");assert_eq!(b.read32(UART1),Ok(u32::from(b'C')));b.write8(UART0,b'X').unwrap();put(&mut b,UART1,u32::from(b'Y'));
        assert_eq!(b.console_take(),[vec![],b"X".to_vec(),b"Y".to_vec(),vec![]]);
        b.serial_input(b"USB must not arrive on UART");assert_eq!(b.read32(UART0+0x1c).unwrap()&0x3ff,0);
        for addr in[0x60004000,0x60009000,0x3ff44000,0x3f424000,0x3f413000,0x60080000,0x40080000,0x3f000000]{assert_eq!(b.write32(addr,0),Err(Fault::Unmapped),"{id} {addr:#x}");}
        assert!(b.gpio_state(22).is_none());assert!(b.gpio_state(47).is_none());mux(&mut b,46,1);put(&mut b,GPIO_BASE+0x30,1<<14);assert!(!b.gpio_state(46).unwrap().output_enable);
        if id!=DONGLE{assert!(b.board.display().is_none());assert!(b.board.leds().is_none());}
    }
    let mut b=board(DONGLE);output(&mut b,39,true);assert_eq!(b.board.leds().unwrap().0,[[255,0,0]]);
    mux(&mut b,39,2);assert_eq!(b.board.leds().unwrap().0,[[0;3]],"wrong IO_MUX must disconnect LED");mux(&mut b,39,1);
    put(&mut b,GPIO_BASE+0x34,1<<7);assert_eq!(b.board.leds().unwrap().0,[[0;3]],"disabled output must disconnect LED");
    assert!(esp32s2::board::make_board("lilygo-t-display-s3").is_none());
}

#[test]
fn dongle_software_spi_uses_real_edges_and_loses_partial_bytes_on_disconnect(){
    let mut b=board(DONGLE);init(&mut b);pixel(&mut b,0xf800);let(w,h,pixels,version)=b.board.display().unwrap();assert_eq!((w,h),(135,240));assert_eq!(pixels[0],0xf800);assert_eq!(pixels.iter().filter(|&&p|p!=0).count(),1);
    level(&mut b,33,false);assert!(b.board.display().unwrap().2.iter().all(|&p|p==0));assert!(b.board.display_version()>version);level(&mut b,33,true);assert_eq!(b.board.display().unwrap().2[0],0xf800);
    mux(&mut b,35,2);pixel(&mut b,0x07e0);assert_eq!(b.board.display().unwrap().2[0],0xf800,"wrong MOSI route drops bytes");mux(&mut b,35,1);
    put(&mut b,GPIO_BASE+0x34,1<<4);pixel(&mut b,0x07e0);assert_eq!(b.board.display().unwrap().2[0],0xf800,"disabled CLK cannot transfer");put(&mut b,GPIO_BASE+0x30,1<<4);
    // Half of DISPOFF followed by CS release must never become a command.
    level(&mut b,34,false);level(&mut b,37,false);for bit in(4..8).rev(){level(&mut b,36,false);level(&mut b,35,0x28&(1<<bit)!=0);level(&mut b,36,true);}level(&mut b,34,true);
    pixel(&mut b,0x07e0);assert_eq!(b.board.display().unwrap().2[0],0x07e0);
    cmd(&mut b,0x3a,&[0x66]);pixel(&mut b,0x001f);assert_eq!(b.board.display().unwrap().2[0],0x07e0,"RGB666 is not falsely decoded as RGB565");
    level(&mut b,38,false);assert!(b.board.display().unwrap().2.iter().all(|&p|p==0));level(&mut b,38,true);assert!(b.board.display().unwrap().2.iter().all(|&p|p==0),"reset requires new sleep/display commands");
}

fn raw_image(entry:u32,segments:&[(u32,Vec<u8>)])->Vec<u8>{
    let mut data=vec![0;24];data[0]=0xe9;data[1]=segments.len()as u8;data[4..8].copy_from_slice(&entry.to_le_bytes());data[12]=2;let mut check=0xef;
    for(addr,bytes)in segments{data.extend_from_slice(&addr.to_le_bytes());data.extend_from_slice(&(bytes.len()as u32).to_le_bytes());data.extend_from_slice(bytes);for &b in bytes{check^=b;}}
    let at=data.len()/16*16+15;data.resize(at,0);data.push(check);data
}
#[test]
fn image_validation_rejects_alias_overlap_revision_and_integrity_transactionally(){
    for image in[CONSOLE,SCREEN]{assert!(esp32s2::S2::check_image(image).is_ok());}
    let simple=vec![0x22,0xa0,5,0];let valid=raw_image(IRAM_BASE,&[(IRAM_BASE,simple.clone())]);assert!(esp32s2::image::parse(&valid).is_ok());
    for (addr,len) in[(IRAM_BASE+2,4),(IRAM_BASE,2),(IRAM_BASE,0),(IRAM_BASE+SRAM_SIZE as u32,4),(0x40000000,4),(0x50000000,4),(0x3ff9e000,4),(0x40080000,4),(GPIO_BASE,4)]{
        assert!(esp32s2::image::parse(&raw_image(IRAM_BASE,&[(addr,vec![0;len])])).is_err(),"{addr:#x}+{len}");
    }
    for entry in[IRAM_BASE+2,IRAM_BASE+4,DRAM_BASE,0x40070000]{assert!(esp32s2::image::parse(&raw_image(entry,&[(IRAM_BASE,simple.clone())])).is_err());}
    let overlap=raw_image(IRAM_BASE,&[(IRAM_BASE,simple.clone()),(DRAM_BASE,vec![1;4])]);assert!(esp32s2::image::parse(&overlap).err().unwrap().contains("overlapping physical"));
    assert!(esp32s2::image::parse(&raw_image(IRAM_BASE,&[(IRAM_BASE,simple),(DRAM_BASE+4,vec![1;4])])).is_ok());
    for chip in[0,9,0x17]{let mut bad=CONSOLE.to_vec();bad[12]=chip;assert!(esp32s2::image::parse(&bad).err().unwrap().contains("chip ID 2"));}
    for revision_byte in[14,15,16]{let mut bad=valid.clone();bad[revision_byte]=1;assert!(esp32s2::image::parse(&bad).err().unwrap().contains("revision"));}
    let mut max=valid.clone();max[17]=1;assert!(esp32s2::image::parse(&max).is_ok(),"positive maximum still includes diagnostic revision 0");
    for count in[0,17,255]{let mut bad=CONSOLE.to_vec();bad[1]=count;assert!(esp32s2::image::parse(&bad).is_err());}
    let p=esp_soc::image::parse(CONSOLE).unwrap();let end=p.segments.last().map(|s|(s.file_off+s.len)as usize).unwrap();let checksum=end/16*16+15;
    let mut bad=CONSOLE.to_vec();bad[32]^=1;assert!(esp32s2::image::parse(&bad).err().unwrap().contains("checksum"));
    let mut bad=CONSOLE.to_vec();bad[end]=1;assert!(esp32s2::image::parse(&bad).err().unwrap().contains("padding"));
    let mut bad=CONSOLE.to_vec();*bad.last_mut().unwrap()^=1;assert!(esp32s2::image::parse(&bad).err().unwrap().contains("SHA-256"));
    let mut bad=CONSOLE.to_vec();bad[23]=2;assert!(esp32s2::image::parse(&bad).err().unwrap().contains("flag"));
    for len in[0,23,24,end,checksum,CONSOLE.len()-1]{assert!(esp32s2::image::parse(&CONSOLE[..len]).is_err());}
    let mut tail=CONSOLE.to_vec();tail.extend([0xff;16]);assert!(esp32s2::image::parse(&tail).is_ok());*tail.last_mut().unwrap()=0;assert!(esp32s2::image::parse(&tail).err().unwrap().contains("footer"));
    let mut b=S2Bus::new();b.set_flash_size(4<<20);b.sram.fill(0xa5);b.write_flash(0x10000,&overlap).unwrap();assert!(b.boot_app(0x10000).is_err());assert!(b.sram.iter().all(|&v|v==0xa5));
    assert!(b.write_flash(usize::MAX,&[1]).is_err());assert!(b.set_psram_size(1).is_err());
}

fn machine(id:&str,image:&[u8])->esp32s2::Machine{
    let mut m=esp32s2::machine([0;6],4<<20);m.console.capture=true;m.console.mask=2;m.bus.attach_board(esp32s2::board::make_board(id).unwrap());
    m.bus.write_flash(0x10000,image).unwrap();assert_eq!(m.boot_app(0x10000).unwrap(),0x40021400);m
}
fn run(m:&mut esp32s2::Machine,cycles:u64){
    m.max_cycles=m.bus.cycles()+cycles;let stop=m.run(u64::MAX);assert!(matches!(stop,esp_soc::Stop::MaxInsns|esp_soc::Stop::Halted),"{stop:?} at {:#x} fault={:?}",m.cores[0].pc,m.bus.first_fault);
    assert_eq!(m.exceptions,0);assert!(m.bus.first_fault.is_none());assert_eq!(m.interrupts,0);
}
#[test]
fn original_s2_guest_runs_on_each_distinct_board_and_restarts_without_rom(){
    for id in[WOOR,BASE,DONGLE]{
        let mut m=machine(id,CONSOLE);run(&mut m,3000);assert!(m.console.uart0.ends_with(b"S2 SRAM READY\nBUTTON UP\n"));assert_eq!(m.bus.read32(MARKER),Ok(1));
        m.bus.gpio_set_input(0,false);run(&mut m,3000);assert!(m.console.uart0.ends_with(b"BUTTON DOWN\n"));assert_eq!(m.bus.read32(MARKER),Ok(0));
        m.bus.uart_input(0,b"echo\n");run(&mut m,3000);assert!(m.console.uart0.ends_with(b"echo\n"));
        m.bus.request_reset(3);m.reboot();run(&mut m,3000);assert!(m.console.uart0.ends_with(b"S2 SRAM READY\nBUTTON DOWN\n"),"held BOOT remains physically low across host restart");
        m.bus.gpio_set_input(0,true);run(&mut m,3000);assert!(m.console.uart0.ends_with(b"BUTTON UP\n"));assert_eq!(m.reboots,1);assert_eq!(m.cores[0].code_cache_stats().unwrap().2,0);
    }
}
#[test]
fn original_dongle_guest_draws_entire_red_green_frames_by_gpio_software_spi(){
    let mut m=machine(DONGLE,SCREEN);
    for(pressed,color,message)in[(false,0xf800,b"FRAME RED\n".as_slice()),(true,0x07e0,b"FRAME GREEN\n".as_slice()),(false,0xf800,b"FRAME RED\n".as_slice())]{
        m.bus.gpio_set_input(0,!pressed);run(&mut m,6_000_000);
        assert!(m.console.uart0.ends_with(message),"{}",String::from_utf8_lossy(&m.console.uart0));
        let(w,h,pixels,_)=m.bus.board_ref().display().unwrap();assert_eq!((w,h),(135,240));assert!(pixels.iter().all(|&p|p==color));assert_eq!(m.bus.read32(MARKER),Ok(u32::from(!pressed)));
        assert_eq!(m.bus.board_ref().leds().unwrap().0,if pressed{[[255,0,0]]}else{[[0;3]]});
    }
    m.bus.uart_input(0,b"S2 echo\n");run(&mut m,3000);assert!(m.console.uart0.ends_with(b"S2 echo\n"));
    m.bus.request_reset(3);m.reboot();assert!(m.bus.board_ref().display().unwrap().2.iter().all(|&p|p==0));run(&mut m,6_000_000);assert!(m.console.uart0.ends_with(b"FRAME RED\n"));
}
