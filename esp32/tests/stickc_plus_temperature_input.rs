//! Native classic I2C-controller MMIO/board/AXP path tests, NOT original-firmware acceptance.
//! No guest code/ROM is executed here. Original factory captures are separate evidence.
use esp_periph::i2c::axp192::Axp192State;
use std::sync::{Arc,Mutex};
fn fixture()->(esp32::Machine,Arc<Mutex<Axp192State>>) {
    let mut m=esp32::machine([2,0,0,0,0,1],4<<20);
    let board=esp32::board::stickc_plus::StickCPlus::new();let s=board.pmic.clone();
    m.bus.board=Box::new(board);m.bus.attach_board_devices();
    m.bus.periph.i2c[0].set_pins(Some((21,22)));(m,s)
}
fn sample(m:&mut esp32::Machine,us:u64) {m.bus.board.advance_to(us*240);}
fn transfer(m:&mut esp32::Machine,bytes:&[u32],commands:&[u32]) {
    let c=&mut m.bus.periph.i2c[0];c.write(0x18,3<<12);c.write(0x24,u32::MAX);
    for b in bytes {c.write(0x1c,*b);}
    for (i,v) in commands.iter().enumerate() {c.write(0x58+i as u32*4,*v);}
    c.write(4,1<<5);
}
fn write(m:&mut esp32::Machine,r:u8,v:u8) {
    transfer(m,&[0x68,u32::from(r),u32::from(v)],&[0,(1<<11)|3,3<<11,4<<11]);
}
fn read_temperature(m:&mut esp32::Machine)->u16 {
    // START, address-write+pointer, repeated START, address-read, ACK high, NACK low, STOP.
    transfer(m,&[0x68,0x5e,0x69],&[0,(1<<11)|2,0,(1<<11)|1,(2<<11)|1,(2<<11)|(1<<10)|1,3<<11,4<<11]);
    let c=&mut m.bus.periph.i2c[0];let high=c.read(0x1c);let low=c.read(0x1c);
    assert_eq!(low&!15,0);((high<<4)|low) as u16
}
#[test]
fn native_controller_reads_normal_cold_hot_and_zero_through_shared_board_device() {
    for (mc,expected) in [(-144700,0),(-10000,1347),(25000,1697),(110000,2547)] {
        let (mut m,s)=fixture();let pixels=m.bus.board.display().unwrap().2.to_vec();
        s.lock().unwrap().set_chip_temperature_mc(mc);assert_eq!(read_temperature(&mut m),0);
        sample(&mut m,39999);assert_eq!(read_temperature(&mut m),0);
        sample(&mut m,40000);assert_eq!(read_temperature(&mut m),expected);
        assert_eq!(m.bus.board.display().unwrap().2,pixels,"ADC stimulus never draws screen");
    }
}
#[test]
fn native_controller_enable_readonly_rate_and_live_change_use_guest_clock_not_read_count() {
    let (mut m,s)=fixture();sample(&mut m,40000);assert_eq!(read_temperature(&mut m),1697);
    write(&mut m,0x83,0);s.lock().unwrap().set_chip_temperature_mc(110000);
    sample(&mut m,120000);assert_eq!(read_temperature(&mut m),1697);
    write(&mut m,0x5e,0xff);write(&mut m,0x5f,0xff);assert_eq!(read_temperature(&mut m),1697);
    write(&mut m,0x84,0xf2);write(&mut m,0x83,0x80);
    sample(&mut m,124999);assert_eq!(read_temperature(&mut m),1697);
    sample(&mut m,125000);assert_eq!(read_temperature(&mut m),2547);
}
#[test]
fn cpu_reboot_preserves_external_pmic_temperature_but_pmic_reset_clears_only_latch() {
    let (mut m,s)=fixture();s.lock().unwrap().set_chip_temperature_mc(-10000);
    sample(&mut m,40000);assert_eq!(read_temperature(&mut m),1347);
    m.reboot();
    // CPU reboot resets I2C routing; the host test explicitly restores wires, not PMIC state.
    m.bus.periph.i2c[0].set_pins(Some((21,22)));assert_eq!(s.lock().unwrap().chip_temperature_mc(),-10000);
    assert_eq!(read_temperature(&mut m),1347);s.lock().unwrap().reset_powered_fixture();
    assert_eq!(read_temperature(&mut m),0);sample(&mut m,80000);assert_eq!(read_temperature(&mut m),1347);
}
