//! Original RV32 guest probes. All control, DCS commands, DMA descriptors and
//! test pixels are written by executed instructions; host input is touch only.
//! This is not a factory-image or physical panel acceptance claim.
use emu_core::Bus;
use esp_soc::SocBus as _;
const ENTRY:u32=0x4ff1_0000;
const RESULT:u32=0x4ff0_2000;
const DESC:u32=0x4ff0_3000;
const FB:u32=0x4801_0000;
const DRAW:u32=0x4820_0000;
const I2C:u32=0x500c_4000;
const DSI:u32=0x500a_0000;
type Machine=esp32p4::Machine;
fn li(c:&mut Vec<u32>,r:u32,v:u32){c.push((v.wrapping_add(0x800)&0xffff_f000)|(r<<7)|0x37);c.push(((v&0xfff)<<20)|(r<<15)|(r<<7)|0x13);}
fn write(c:&mut Vec<u32>,a:u32,v:u32){li(c,1,a);li(c,2,v);c.push(2<<20|1<<15|2<<12|0x23);}
fn half(c:&mut Vec<u32>,a:u32,v:u16){li(c,1,a);li(c,2,v.into());c.push(2<<20|1<<15|1<<12|0x23);}
fn copy(c:&mut Vec<u32>,a:u32,b:u32){li(c,1,a);c.push(1<<15|2<<12|2<<7|3);li(c,1,b);c.push(2<<20|1<<15|2<<12|0x23);}
fn delay(c:&mut Vec<u32>,cycles:u32){li(c,3,cycles/2);c.extend([0xfff1_8193,0xfe01_9ee3]);}
fn run(m:&mut Machine,mut c:Vec<u32>,cycles:u64){write(&mut c,RESULT+0x100,0xabc);c.push(0x0000_006f);m.bus.load_bytes(ENTRY,&c.into_iter().flat_map(u32::to_le_bytes).collect::<Vec<_>>()).unwrap();m.cores[0].pc=ENTRY;m.run(cycles);assert_eq!(m.exceptions,0);assert_eq!(m.bus.read32(RESULT+0x100).unwrap(),0xabc);}
fn machine()->Machine{let mut m=esp32p4::machine([0;6],16<<20);m.bus.psram=vec![0;32<<20];m.bus.board=esp32p4::board::make_board("m5stack-tab5").unwrap();m.bus.attach_board_devices();m.boot_rom();m}
fn route(c:&mut Vec<u32>,sda:u32){for(pin,signal)in[(sda,69),(32,68)]{write(c,0x500e_1004+4*pin,1<<12|1<<9);write(c,0x500e_0558+4*pin,signal);write(c,0x500e_0158+4*signal,0x80|pin);}write(c,I2C,19);write(c,I2C+0x38,20);}
fn transfer(c:&mut Vec<u32>,bytes:&[u8],reads:usize){write(c,I2C+0x18,3<<12);write(c,I2C+0x24,u32::MAX);for &v in bytes{write(c,I2C+0x1c,u32::from(v));}let mut cmds=vec![6<<11,1<<11|1<<8|bytes.len()as u32];if reads!=0{cmds.push(3<<11|reads as u32);}cmds.push(2<<11);for(i,v)in cmds.into_iter().enumerate(){write(c,I2C+0x58+4*i as u32,v);}write(c,I2C+4,1<<5);li(c,1,I2C+0x20);c.extend([0x0000_a183,0x4801_f193,0xfe01_8ce3]);copy(c,I2C+0x20,RESULT);for i in 0..reads{copy(c,I2C+0x1c,RESULT+4+4*i as u32);}}
fn io(c:&mut Vec<u32>,reg:u8,v:u8){transfer(c,&[0x86,reg,v],0);}
fn read_touch(c:&mut Vec<u32>,reg:u16,n:usize){transfer(c,&[0x28,(reg>>8)as u8,reg as u8],0);transfer(c,&[0x29],n);}
fn board_init(c:&mut Vec<u32>){route(c,31);io(c,1,1);io(c,13,0x7f);io(c,11,0x7f);io(c,5,0x66);io(c,7,0);io(c,3,0x7f);io(c,3,0x6f);write(c,0x500e_105c,1<<12|1<<9);write(c,0x500e_05b0,256);write(c,0x500e_0024,1<<22);write(c,0x500e_0008,1<<22);}
fn phy_reg(c:&mut Vec<u32>,a:u8,v:u8){for(o,v)in[(0xb4,0),(0xb8,1<<16|u32::from(a)),(0xb4,2),(0xb4,0),(0xb8,u32::from(v)),(0xb4,2),(0xb4,0)]{write(c,DSI+o,v);}}
fn dcs(c:&mut Vec<u32>,cmd:u8,args:&[u8]){if args.is_empty(){write(c,DSI+0x6c,5|u32::from(cmd)<<8);}else if args.len()==1{write(c,DSI+0x6c,0x15|u32::from(cmd)<<8|u32::from(args[0])<<16);}else{let mut p=vec![cmd];p.extend_from_slice(args);for chunk in p.chunks(4){let mut b=[0;4];b[..chunk.len()].copy_from_slice(chunk);write(c,DSI+0x70,u32::from_le_bytes(b));}write(c,DSI+0x6c,0x39|(p.len()as u32)<<8);}}
fn dsi_init(c:&mut Vec<u32>){
    // LDO3 = software controlled, enabled; nominal 2.5 V (DREF9 1 V,
    // MUL6 => 2.5 V). This selector check is not analog calibration.
    write(c,0x5011_51c4,9<<28|6<<23);write(c,0x5011_51c0,1<<7|1<<8);
    write(c,0x500e_6014,0xe6df_b7af);write(c,0x500e_6018,1<<12|1<<5);write(c,0x500e_603c,3|1<<5|1<<7|3<<8);
    write(c,DSI+4,1);write(c,DSI+0xa4,1);write(c,DSI+0xa0,15);
    phy_reg(c,0x17,0);phy_reg(c,0x18,29);phy_reg(c,0x18,0x80);delay(c,100);
    copy(c,DSI+0xb0,RESULT+0x80);dcs(c,0xff,&[0x98,0x81,0]);dcs(c,0x3a,&[0x55]);dcs(c,0x11,&[]);dcs(c,0x29,&[]);
    write(c,DSI+0x804,1);write(c,DSI+0x818,2);write(c,DSI+0x838,720<<16|940);write(c,DSI+0x830,1280<<16|1324);write(c,DSI+0x34,0);write(c,DSI+0x840,1);write(c,DSI+0x844,1);
}
fn fill(c:&mut Vec<u32>){li(c,4,DRAW);li(c,5,720*1280);li(c,6,0x1234);c.extend([6<<20|4<<15|1<<12|0x23,0x0022_0213,0xfff2_8293,0xfe02_9ae3]);for(i,p)in[(0,0xf800),(719,0x07e0),(720*1279,0x001f),(720*1280-1,0xffff)]{half(c,DRAW+2*i,p);}}
fn copy2d(c:&mut Vec<u32>){
    write(c,0x500e_6018,1<<6);
    for(at,buf)in[(DESC+0x100,DRAW),(DESC+0x140,FB)]{for(i,w)in[1280|720<<14|1<<29|1<<30|1<<31,1280|720<<14|3<<28,0,buf,0,0].into_iter().enumerate(){write(c,at+4*i as u32,w);}}
    for(o,v)in[(0,17),(8,3),(0x38,4),(0x20,DESC+0x100),(0x1c,1<<21),(0x500,17),(0x508,3),(0x53c,3),(0x520,DESC+0x140),(0x51c,1<<22)]{write(c,0x5008_8000+o,v);}
}
fn dma(c:&mut Vec<u32>,src:u32,items:u32,valid:bool){let ctl=1|1<<6|3<<8|3<<11;for(i,w)in[src,0,0x5010_5000,0,items-1,0,0,0,ctl,1<<30|if valid{1<<31}else{0},0,0,0,0,0,0].into_iter().enumerate(){write(c,DESC+4*i as u32,w);}for(o,v)in[(0x10,3),(0x120,15),(0x124,1),(0x128,DESC|1),(0x180,u32::MAX),(0x190,u32::MAX),(0x18,0x101)]{write(c,0x5008_1000+o,v);}}
fn fnv(data:&[u16])->u64{data.iter().flat_map(|p|p.to_le_bytes()).fold(0xcbf2_9ce4_8422_2325,|h,b|(h^u64::from(b)).wrapping_mul(0x100_0000_01b3))}

#[test]
fn actual_guest_initializes_phy_and_scans_a_full_tab5_frame_then_redraws(){
    let mut m=machine();let mut c=Vec::new();board_init(&mut c);
    for page in 1..61{write(&mut c,0x5008_e380,page);write(&mut c,0x5008_e37c,0xc00|page);}
    fill(&mut c);copy2d(&mut c);dsi_init(&mut c);dma(&mut c,FB,720*1280/4,true);delay(&mut c,9_000_000);run(&mut m,c,14_000_000);
    assert_eq!(m.bus.read32(RESULT+0x80).unwrap()&0x95,0x95,"PLL and both stop lanes arise from supplied clocks/PLL programming");
    let(_,_,px,version)=m.bus.board.display().unwrap();assert_eq!([px[0],px[719],px[720*1279],px[720*1280-1]],[0xf800,0x07e0,0x001f,0xffff]);
    let mut expected=vec![0x1234;720*1280];for(i,p)in[(0,0xf800),(719,0x07e0),(720*1279,0x001f),(720*1280-1,0xffff)]{expected[i]=p;}
    assert_eq!(fnv(&px),fnv(&expected));assert_eq!(m.bus.periph.dw_dma.transfers,1);assert_eq!(m.bus.periph.dma2d.transfers.0,1);
    m.bus.board.touch(123,456,true);
    let mut c=Vec::new();transfer(&mut c,&[0x28,0x81,0x4e,0],0);delay(&mut c,4_000_000);read_touch(&mut c,0x8150,4);li(&mut c,1,RESULT+4);c.push(1<<15|2<<12|2<<7|3);li(&mut c,1,FB+2*721);c.push(2<<20|1<<15|1<<12|0x23);dma(&mut c,FB,720*1280/4,true);delay(&mut c,9_000_000);run(&mut m,c,15_000_000);let(_,_,new,newver)=m.bus.board.display().unwrap();assert_eq!(new[721],123,"guest-drawn pixel uses the actual GT911 I2C report");assert_ne!(fnv(&new),fnv(&px));assert!(newver>version);
    // Display sleep/off and the physical backlight each gate visible pixels.
    let mut c=Vec::new();dcs(&mut c,0x10,&[]);run(&mut m,c,2000);assert!(m.bus.board.display().unwrap().2.iter().all(|p|*p==0));
    let mut c=Vec::new();dcs(&mut c,0x11,&[]);write(&mut c,0x500e_000c,1<<22);delay(&mut c,100);run(&mut m,c,2000);assert!(m.bus.board.display().unwrap().2.iter().all(|p|*p==0));
    let mut c=Vec::new();write(&mut c,0x500e_0008,1<<22);delay(&mut c,100);run(&mut m,c,2000);assert_eq!(m.bus.board.display().unwrap().2[721],123);
    m.bus.reboot([0;6]);assert!(m.bus.board.display().unwrap().2.iter().all(|p|*p==0),"MCU reset shuts down streaming DSI instead of publishing a stale frame");
}

#[test]
fn tab5_guest_touch_uses_exact_i2c_pins_report_ack_and_reset(){
    let mut m=machine();let mut c=Vec::new();board_init(&mut c);read_touch(&mut c,0x8140,4);run(&mut m,c,1_000_000);assert_eq!(m.bus.read32(RESULT).unwrap()&1<<10,0);assert_eq!((0..4).map(|i|m.bus.read32(RESULT+4+4*i).unwrap()as u8).collect::<Vec<_>>(),b"911\0");
    m.bus.board.touch(123,456,true);let mut c=Vec::new();transfer(&mut c,&[0x28,0x81,0x4e,0],0);delay(&mut c,4_000_000);read_touch(&mut c,0x814e,6);run(&mut m,c,5_000_000);assert_eq!((0..6).map(|i|m.bus.read32(RESULT+4+4*i).unwrap()as u8).collect::<Vec<_>>(),[0x81,0,123,0,200,1]);
    let mut c=Vec::new();transfer(&mut c,&[0x28,0x81,0x4e,0],0);read_touch(&mut c,0x814e,1);run(&mut m,c,1_000_000);assert_eq!(m.bus.read32(RESULT+4).unwrap(),0);
    let mut c=Vec::new();route(&mut c,30);read_touch(&mut c,0x8140,3);run(&mut m,c,1_000_000);assert_ne!(m.bus.read32(RESULT).unwrap()&1<<10,0,"wrong SDA cannot read a host-injected touch");
    let mut c=Vec::new();route(&mut c,31);io(&mut c,5,0x46);read_touch(&mut c,0x8140,3);run(&mut m,c,1_000_000);assert_ne!(m.bus.read32(RESULT).unwrap()&1<<10,0,"TP_RST low removes GT911 ACK");
}

#[test]
fn malformed_dw_dma_and_unpowered_phy_never_publish_pixels(){
    let mut m=machine();let mut c=Vec::new();board_init(&mut c);dsi_init(&mut c);dma(&mut c,0xffff_fff0,720*1280/4,true);delay(&mut c,9_000_000);run(&mut m,c,11_000_000);assert_eq!(m.bus.periph.dw_dma.transfers,0);assert!(m.bus.periph.dw_dma.errors>0);assert_eq!(m.bus.board.display_frames(),0);assert_ne!(m.bus.read32(0x5008_1188).unwrap()&1<<5,0);
    let mut c=Vec::new();write(&mut c,0x5011_51c0,0);copy(&mut c,DSI+0xb0,RESULT+0x80);run(&mut m,c,2000);assert_eq!(m.bus.read32(RESULT+0x80).unwrap(),0,"LDO3 off loses PLL and lane readiness");
    let mut c=Vec::new();write(&mut c,0x5011_51c0,1<<7|1<<8);delay(&mut c,100);dma(&mut c,FB,1,false);delay(&mut c,9_000_000);run(&mut m,c,10_000_000);assert_ne!(m.bus.read32(0x5008_1188).unwrap()&1<<13,0);assert_eq!(m.bus.board.display_frames(),0);
}
