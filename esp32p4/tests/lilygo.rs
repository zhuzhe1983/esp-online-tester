//! Exact T-Display-P4 V1.0 digital wiring through P4 instructions and MMIO.
//! These probes do not execute the vendor factory image or a MIPI-DSI driver.
use emu_core::Bus;
use esp_soc::SocBus as _;

fn machine()->esp32p4::Machine {
    let mut m=esp32p4::machine([0;6],16<<20);
    m.bus.board=esp32p4::board::make_board("lilygo-t-display-p4").unwrap();
    m.bus.attach_board_devices();m
}
fn route(b:&mut esp32p4::bus::SocBus) {
    for (pin,signal) in [(7,69),(8,68)] {
        b.write32(0x500e_1004+4*pin,1<<12|1<<9).unwrap();
        b.write32(0x500e_0558+4*pin,signal).unwrap();
        b.write32(0x500e_0158+4*signal,0x80|pin).unwrap();
    }
}
fn transfer(b:&mut esp32p4::bus::SocBus,base:u32,bytes:&[u8],reads:usize)->(Vec<u8>,u32) {
    b.write32(base+0x18,3<<12).unwrap();b.write32(base+0x24,u32::MAX).unwrap();
    for &byte in bytes {b.write32(base+0x1c,u32::from(byte)).unwrap();}
    let mut commands=vec![6<<11,1<<11|1<<8|bytes.len() as u32];
    if reads>0 {commands.push(3<<11|reads as u32);}
    commands.push(2<<11);
    for (index,command) in commands.into_iter().enumerate() {b.write32(base+0x58+4*index as u32,command).unwrap();}
    b.write32(base+4,1<<5).unwrap();b.tick(1_000_000);
    let status=b.read32(base+0x20).unwrap();
    ((0..reads).map(|_|b.read32(base+0x1c).unwrap() as u8).collect(),status)
}
fn read_register(b:&mut esp32p4::bus::SocBus,register:u8,count:usize)->Vec<u8> {
    assert_eq!(transfer(b,0x500c_4000,&[0x40,register],0).1 & (1<<10),0);
    let (data,status)=transfer(b,0x500c_4000,&[0x41],count);assert_eq!(status & (1<<10),0);data
}

#[test]
fn actual_riscv_guest_observes_boot_gpio35_press_release_and_reset() {
    let mut m=machine();let entry=esp32p4::bus::L2MEM_LOW+0x1000;
    // lui t0,0x500e0; lw t1,0x40(t0); andi t2,t1,8; jal zero,-8.
    let words=[0x500e_02b7u32,0x0402_a303,0x0083_7393,0xff9f_f06f];
    m.bus.load_bytes(entry,&words.into_iter().flat_map(u32::to_le_bytes).collect::<Vec<_>>()).unwrap();
    m.boot_rom();m.cores[0].pc=entry;m.run(30);assert_eq!(m.cores[0].x[7],8);
    m.bus.gpio_set_input(35,false);m.run(30);assert_eq!(m.cores[0].x[7],0);
    m.bus.gpio_set_input(35,true);m.run(30);assert_eq!(m.cores[0].x[7],8);
    assert_eq!(m.exceptions,0);assert!(m.bus.board.display().is_none());
    assert!(m.bus.board.leds().is_none());assert!(!m.bus.board.button_at(100,"touch",true));
    assert!(m.bus.board.button_at(100,"boot",true));m.bus.reboot([0;6]);
    assert_eq!(m.bus.read32(0x500e_0040).unwrap()&8,0,"persistent board button is still held across MCU reset");
}

#[test]
fn xl9535_uses_i2c0_exact_routes_register_pairs_and_persists_across_mcu_reset() {
    let mut m=machine();let b=&mut m.bus;
    assert!(b.periph.i2c0.has_device(0x20));assert!(!b.periph.i2c1.has_device(0x20));
    assert_ne!(transfer(b,0x500c_4000,&[0x40,2,0x40,0x21],0).1&(1<<10),0,"no physical GPIO route");
    route(b);assert_eq!(read_register(b,6,2),[0xff;2]);
    assert_eq!(transfer(b,0x500c_4000,&[0x40,2,0x40,0x21],0).1&(1<<10),0);
    assert_eq!(read_register(b,2,4),[0x40,0x21,0x40,0x21],"output register pair wrap");
    assert_eq!(transfer(b,0x500c_4000,&[0x40,6,0xbf,0xde],0).1&(1<<10),0);
    assert!(b.board.report().contains("driven_high=0x2140"));
    b.reboot([0;6]);route(b);assert_eq!(read_register(b,2,2),[0x40,0x21]);
    assert_eq!(read_register(b,6,2),[0xbf,0xde],"RST does not power-cycle XL9535");
    for (address,value) in [(0x500e_1020,0),(0x500e_0574,69|1<<9),(0x500e_026c,0x80|6)] {
        route(b);b.write32(address,value).unwrap();
        assert_ne!(transfer(b,0x500c_4000,&[0x40,2,0,0],0).1&(1<<10),0,"wrong pad, inverted output, or mismatched SDA");
    }
    route(b);assert_ne!(transfer(b,0x500c_5000,&[0x40,2,0,0],0).1&(1<<10),0,"wrong hardware controller");
    assert_eq!(read_register(b,2,2),[0x40,0x21]);
    assert_ne!(transfer(b,0x500c_4000,&[0x70],0).1&(1<<10),0,"no invented touch controller at 0x38");
}

#[test]
fn both_p4_i2c_controllers_preserve_eight_bit_fractional_clock_fields() {
    use esp_periph::i2c::I2cDevice;
    use std::sync::{Arc,Mutex};
    struct ClockProbe(Arc<Mutex<Vec<Option<u32>>>>);
    impl I2cDevice for ClockProbe {
        fn start_address_with_clock(&mut self,configured:u8,address:u8,_read:bool,hz:Option<u32>)->bool {
            if configured!=address {return false;}self.0.lock().unwrap().push(hz);true
        }
        fn write(&mut self,_byte:u8)->bool {true}
        fn read(&mut self)->u8 {0}
    }
    let mut m=machine();let b=&mut m.bus;
    let seen=Arc::new(Mutex::new(Vec::new()));
    b.periph.i2c0.attach(0x6e,Box::new(ClockProbe(seen.clone())));
    b.periph.i2c1.attach(0x6e,Box::new(ClockProbe(seen.clone())));
    // 40 MHz / (1 + 64/128), then 100 source periods per SCL cycle.
    // The old 6-bit packing silently discarded both of these legal fields.
    b.write32(0x500e_6040,128<<10|64<<18).unwrap();
    b.write32(0x500e_6044,128<<8|64<<16).unwrap();
    for base in [0x500c_4000,0x500c_5000] {
        b.write32(base,49).unwrap();b.write32(base+0x38,50).unwrap();
        assert_eq!(transfer(b,base,&[0xdc],0).1&(1<<10),0);
    }
    assert_eq!(*seen.lock().unwrap(),[Some(266_666),Some(266_666)]);
}

#[test]
fn p4_sdio_socket_matches_manufacturer_pins_and_rejects_a_disconnected_clock() {
    let mut m=machine();let b=&mut m.bus;
    b.attach_sd_card(Box::new(esp_periph::sdmmc::MemStore(vec![0;1<<20])),Default::default()).unwrap();
    assert_eq!(b.periph.sd_pins,[esp32p4::periph::SdPins{clk:43,cmd:44,data:[39,40,41,42]}]);
    b.write32(0x5008_3028,0x1aa).unwrap();b.write32(0x5008_302c,1<<31|1<<6|8).unwrap();
    assert_eq!(b.read32(0x5008_3030).unwrap(),0x1aa);
    b.write32(0x500e_10b0,1<<12).unwrap(); // CLK43 moved from SDMMC IO_MUX to GPIO.
    b.write32(0x5008_3044,u32::MAX).unwrap();b.write32(0x5008_302c,1<<31|1<<6|8).unwrap();
    assert_ne!(b.read32(0x5008_3044).unwrap()&(1<<8),0,"no response on a disconnected clock");
}

#[test]
fn p4_sdio_transfers_a_written_block_through_real_guest_ram_dma_descriptors() {
    use esp_periph::sdmmc::{des0,reg,CMD_START,CMD_RESP,CMD_RESP_LONG,CMD_DATA,CMD_WRITE};
    fn cmd(b:&mut esp32p4::bus::SocBus,index:u32,arg:u32,flags:u32)->u32 {
        b.write32(0x5008_3000+reg::RINTSTS,u32::MAX).unwrap();
        b.write32(0x5008_3000+reg::CMDARG,arg).unwrap();b.write32(0x5008_3000+reg::CMD,CMD_START|index|flags).unwrap();
        b.read32(0x5008_3000+reg::RESP0).unwrap()
    }
    let mut m=machine();let b=&mut m.bus;
    b.attach_sd_card(Box::new(esp_periph::sdmmc::MemStore(vec![0;1<<20])),Default::default()).unwrap();
    cmd(b,0,0,0);assert_eq!(cmd(b,8,0x1aa,CMD_RESP),0x1aa);
    for _ in 0..2 {cmd(b,55,0,CMD_RESP);cmd(b,41,0x40ff_8000,CMD_RESP);}
    cmd(b,2,0,CMD_RESP|CMD_RESP_LONG);let rca=cmd(b,3,0,CMD_RESP)&0xffff_0000;cmd(b,7,rca,CMD_RESP);
    let descriptor=esp32p4::bus::L2MEM_LOW+0x1000;let buffer=descriptor+0x1000;
    let payload:Vec<u8>=(0..512).map(|n|(n%251) as u8).collect();b.load_bytes(buffer,&payload).unwrap();
    for (command,flags) in [(24,CMD_WRITE),(17,0)] {
        if command==17 {b.tick(2_000_000);b.load_bytes(buffer,&[0;512]).unwrap();}
        for (i,word) in [des0::OWN|des0::CH|des0::FS|des0::LD,512,buffer,0].into_iter().enumerate() {b.write32(descriptor+4*i as u32,word).unwrap();}
        for (offset,value) in [(reg::CTRL,1<<4|1<<25|1<<5),(reg::BMOD,1<<7|1<<1),(reg::BYTCNT,512),(reg::BLKSIZ,512),(reg::DBADDR,descriptor),(reg::PLDMND,1)] {b.write32(0x5008_3000+offset,value).unwrap();}
        cmd(b,command,2,CMD_RESP|CMD_DATA|flags);
        assert_eq!(b.read32(0x5008_3000+reg::RINTSTS).unwrap(),(1<<2)|(1<<3));
        assert_eq!(b.read32(descriptor).unwrap()&des0::OWN,0);
    }
    let actual:Vec<u8>=(0..512).map(|n|b.read8(buffer+n).unwrap()).collect();assert_eq!(actual,payload);
}
