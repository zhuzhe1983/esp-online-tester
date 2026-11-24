//! Exact Waveshare inputs through instructions, GPIO routing and the I2C controller.
//! The host supplies physical button/touch events only. These probes do not run
//! a factory image, synthesize DSI pixels, or claim complete controller emulation.
use emu_core::Bus;

const I2C:u32=0x500c_4000;
const ENTRY:u32=esp32p4::bus::L2MEM_LOW+0x1000;
const RESULT:u32=esp32p4::bus::L2MEM_LOW+0x8000;
type Machine=esp_soc::Machine<esp32p4::P4Rev3>;
fn li(code:&mut Vec<u32>,r:u32,v:u32){code.push((v.wrapping_add(0x800)&0xffff_f000)|(r<<7)|0x37);code.push(((v&0xfff)<<20)|(r<<15)|(r<<7)|0x13);}
fn write(code:&mut Vec<u32>,a:u32,v:u32){li(code,1,a);li(code,2,v);code.push((2<<20)|(1<<15)|(2<<12)|0x23);}
fn copy(code:&mut Vec<u32>,from:u32,to:u32){li(code,1,from);code.push((1<<15)|(2<<12)|(2<<7)|3);li(code,1,to);code.push((2<<20)|(1<<15)|(2<<12)|0x23);}
fn wait(code:&mut Vec<u32>){
    li(code,3,100_000);code.push(0xfff1_8193); // addi x3,x3,-1
    code.push(0xfe01_9ee3); // bne x3,x0,-4
}
fn run(m:&mut Machine,mut code:Vec<u32>){
    code.push(0x0000_006f); // jal zero,0
    m.bus.load_bytes(ENTRY,&code.into_iter().flat_map(u32::to_le_bytes).collect::<Vec<_>>()).unwrap();
    m.cores[0].pc=ENTRY;m.run(350_000);assert_eq!(m.exceptions,0);
}
fn machine(name:&str)->Machine{
    let mut m=esp32p4::machine_rev::<esp32p4::RevGe3>([0;6],32<<20);
    m.bus.board=esp32p4::board::make_board(name).unwrap();m.bus.attach_board_devices();m.boot_rom();m
}
fn route(code:&mut Vec<u32>,sda:u32,reset:Option<u32>){
    for(pin,signal)in[(sda,69),(8,68)]{
        write(code,0x500e_1004+4*pin,1<<12|1<<9);
        write(code,0x500e_0558+4*pin,signal);
        write(code,0x500e_0158+4*signal,0x80|pin);
    }
    if let Some(pin)=reset{
        write(code,0x500e_1004+4*pin,1<<12|1<<9);
        write(code,0x500e_0024,1<<pin);write(code,0x500e_0008,1<<pin);
    }
    write(code,I2C,19);write(code,I2C+0x38,20);
}
fn read_register(code:&mut Vec<u32>,address:u8,reg:u16,count:usize){
    write(code,I2C+0x18,3<<12);write(code,I2C+0x24,u32::MAX);
    for b in [address<<1,(reg>>8)as u8,reg as u8,address<<1|1]{write(code,I2C+0x1c,u32::from(b));}
    for(i,c)in[6<<11,1<<11|1<<8|3,6<<11,1<<11|1<<8|1,3<<11|count as u32,2<<11].into_iter().enumerate(){write(code,I2C+0x58+4*i as u32,c);}
    write(code,I2C+4,1<<5);wait(code);copy(code,I2C+0x20,RESULT);
    for n in 0..count{copy(code,I2C+0x1c,RESULT+4+4*n as u32);}
}

#[test]
fn waveshare_knob_guest_reads_cst9217_and_physical_switches_with_a_wrong_route_control(){
    for sda in [7,6]{
        let mut m=machine("waveshare-p4-knob-amoled175");let mut setup=Vec::new();route(&mut setup,sda,Some(27));run(&mut m,setup);
        m.bus.board.touch(100,200,true);
        for id in ["boot","cw","power"]{assert!(m.bus.board.button_at(m.bus.cycles,id,true));}
        let mut code=Vec::new();read_register(&mut code,0x5a,0xd000,10);copy(&mut code,0x500e_003c,RESULT+0x80);copy(&mut code,0x500e_0040,RESULT+0x84);run(&mut m,code);
        let status=m.bus.read32(RESULT).unwrap();
        if sda==7{
            assert_eq!(status&(1<<10),0,"the exact SDA7/SCL8 physical route acknowledges");
            let bytes:Vec<u8>=(0..10).map(|n|m.bus.read32(RESULT+4+4*n).unwrap()as u8).collect();
            assert_eq!((bytes[0],bytes[5],bytes[6]),(6,1,0xab));
            assert_eq!(((u16::from(bytes[1])<<4)|u16::from(bytes[3]>>4),(u16::from(bytes[2])<<4)|u16::from(bytes[3]&15)),(365,265));
        }else{assert_ne!(status&(1<<10),0,"a host touch event cannot bypass wrong SDA6");}
        let pins=m.bus.read32(RESULT+0x80).unwrap();assert_eq!(pins&((1<<20)|(1<<21)|(1<<2)),(1<<21)|(1<<2));
        assert_eq!(m.bus.read32(RESULT+0x84).unwrap()&8,0);assert!(m.bus.board.display().is_none());
        for id in ["boot","cw","power"]{assert!(m.bus.board.button_at(m.bus.cycles,id,false));}
        let mut release=Vec::new();wait(&mut release);copy(&mut release,0x500e_003c,RESULT+0x80);copy(&mut release,0x500e_0040,RESULT+0x84);run(&mut m,release);
        assert_eq!(m.bus.read32(RESULT+0x80).unwrap()&((1<<20)|(1<<21)|(1<<2)),(1<<20)|(1<<21));assert_eq!(m.bus.read32(RESULT+0x84).unwrap()&8,8);
    }
}

#[test]
fn waveshare_exact_panel_kit_guest_reads_gt9271_identity_while_bare_carrier_nacks(){
    for(name,panel)in[("waveshare-p4-wifi6-db-kit-101",true),("waveshare-p4-wifi6-db",false)]{
        let mut m=machine(name);let mut code=Vec::new();route(&mut code,7,None);read_register(&mut code,0x5d,0x8140,4);run(&mut m,code);
        let status=m.bus.read32(RESULT).unwrap();
        if panel{assert_eq!(status&(1<<10),0);let bytes:Vec<u8>=(0..4).map(|n|m.bus.read32(RESULT+4+4*n).unwrap()as u8).collect();assert_eq!(bytes,b"9271");}
        else{assert_ne!(status&(1<<10),0,"the base SKU does not inherit an external screen");}
        assert!(m.bus.board.display().is_none(),"MIPI-DSI output is not implemented by a touch attachment");
    }
}
