//! Optional host-debounced input through the actual board/I2C controller, leaving raw scans unchanged.
use emu_core::Bus;
use esp_soc::board::{BoardModel,KeyEventAcceptance as A,KeyEventDiagnostics};
type S3=esp32s3::bus::SocBus;
const BASE:u32=0x6001_3000;
fn put(b:&mut S3,a:u32,v:u32) {b.write32(a,v).unwrap();}
fn route(b:&mut S3) {
    // IDF v5.4.2 S3 gpio_sig_map.h: I2C0 SDA90/SCL89, real SDA8/SCL9.
    for (pin,signal) in [(8,90),(9,89)] {
        put(b,0x6000_9004+4*pin,(1<<12)|(1<<9));put(b,0x6000_4554+4*pin,signal);put(b,0x6000_4154+4*signal,0x80|pin);
    }
    put(b,0x6000_9004+4*11,(1<<12)|(1<<9));
}
fn transfer(b:&mut S3,bytes:&[u8],commands:&[u32]) {
    put(b,BASE+0x24,u32::MAX);put(b,BASE+0x18,3<<12);
    for byte in bytes {put(b,BASE+0x1c,u32::from(*byte));}
    for (index,command) in commands.iter().enumerate() {put(b,BASE+0x58+index as u32*4,*command);}
    put(b,BASE+4,1<<5);b.tick(100_000);b.flush_ticks();
    let status=b.read32(BASE+0x20).unwrap();assert_eq!(status&(1<<10),0,"TCA NACK");assert_ne!(status&(1<<7),0);
}
fn write(b:&mut S3,r:u8,v:u8) {transfer(b,&[0x68,r,v],&[6<<11,1<<11|1<<8|3,2<<11]);}
fn read(b:&mut S3,r:u8)->u8 {
    transfer(b,&[0x68,r,0x69],&[6<<11,1<<11|1<<8|2,6<<11,1<<11|1<<8|1,3<<11|1<<10|1,2<<11]);
    b.read32(BASE+0x1c).unwrap() as u8
}
fn fixture()->S3 {
    let mut b=esp32s3::machine([0;6]).bus;b.board=esp32s3::board::make_board("cardputer-adv").unwrap();b.attach_board_devices();route(&mut b);b
}
fn configure(b:&mut S3) {write(b,0x1d,0x7f);write(b,0x1e,0xff);write(b,1,1);}
fn inject(b:&mut S3,row:u8,col:u8,down:bool)->A {b.board.matrix_key_debounced_at(b.cycles,row,col,down)}
#[test]
fn debounced_logical_matrix_mapping_irq_and_real_event_register_consumption() {
    let mut b=fixture();assert_eq!(inject(&mut b,0,0,true),A::Disabled);configure(&mut b);
    assert_ne!(b.read32(0x6000_403c).unwrap()&(1<<11),0);
    for row in 0..4 {for column in 0..14 {
        assert_eq!(inject(&mut b,row,column,true),A::Accepted);assert_eq!(inject(&mut b,row,column,true),A::Duplicate);
        assert_eq!(b.read32(0x6000_403c).unwrap()&(1<<11),0,"actual GPIO11 IRQ");
        let expected=(column/2*10+row+4*(column&1)+1)|0x80;
        assert_eq!(read(&mut b,3),1);assert_eq!(read(&mut b,4),expected);write(&mut b,2,1);
    }}
    assert_eq!(b.board.matrix_key_diagnostics(),Some(KeyEventDiagnostics{accepted:56,consumed:56,pending:0}));
    assert_eq!(inject(&mut b,4,0,true),A::Invalid);assert_eq!(inject(&mut b,0,14,true),A::Invalid);
}
#[test]
fn ordinary_matrix_input_keeps_two_scans_and_reattachment_keeps_external_fifo() {
    let mut b=fixture();configure(&mut b);let at=b.cycles;
    assert!(b.board.matrix_key_at(at,3,13,true));b.tick((esp32s3::periph::CPU_HZ/40) as u32);b.flush_ticks();assert_eq!(read(&mut b,3),0);
    b.tick((esp32s3::periph::CPU_HZ/40) as u32);b.flush_ticks();assert_eq!(read(&mut b,3),1);
    let before=b.board.matrix_key_diagnostics();b.attach_board_devices();route(&mut b);
    assert_eq!(b.board.matrix_key_diagnostics(),before,"MCU-side reattachment does not power-cycle the external keyboard");
    assert_eq!(read(&mut b,4),0xc4);assert_eq!(b.board.matrix_key_diagnostics().unwrap().consumed,1);
    let fresh=esp32s3::board::make_board("cardputer-adv").unwrap();assert_eq!(fresh.matrix_key_diagnostics(),Some(KeyEventDiagnostics::default()));
}
#[test]
fn unsupported_board_defaults_refuse_without_changing_legacy_source_contract() {
    let mut b=esp_soc::board::NoBoard;assert_eq!(b.matrix_key_debounced_at(10,0,0,true),A::Disabled);assert_eq!(b.matrix_key_diagnostics(),None);
}
#[test]
fn guest_ack_irq_edge_is_published_before_the_next_debounced_key() {
    use esp_periph::i2c::I2cDevice;
    use esp_soc::board::BoardEdge;
    fn reg(d:&mut dyn I2cDevice,r:u8,v:u8) {assert!(d.start(false));assert!(d.write(r));assert!(d.write(v));}
    fn event(d:&mut dyn I2cDevice)->u8 {assert!(d.start(false));assert!(d.write(4));assert!(d.start(true));d.read()}
    let mut board=esp32s3::board::CardputerAdv::new();let mut devices=board.i2c_devices();
    let d=devices.iter_mut().find(|(bus,address,_)|*bus==0&&*address==0x34).unwrap().2.as_mut();
    for (r,v) in [(0x1d,0x7f),(0x1e,0xff),(1,1)] {reg(d,r,v);}
    assert_eq!(board.matrix_key_debounced_at(0,3,0,true),A::Accepted);
    assert_eq!(board.take_edges(),vec![BoardEdge{cycle:0,pin:11,level:false}]);
    assert_eq!(event(d),0x84);reg(d,2,1);
    // No board.advance_to/take_edges here: the slave has deasserted IRQ, but the
    // board's cached pad is still low. A new host event must not erase that ACK edge.
    assert_eq!(board.matrix_key_debounced_at(10,3,0,false),A::Accepted);
    assert_eq!(board.take_edges(),vec![BoardEdge{cycle:0,pin:11,level:true},BoardEdge{cycle:10,pin:11,level:false}]);
    assert_eq!(event(d),4);reg(d,2,1);assert_eq!(board.matrix_key_diagnostics(),Some(KeyEventDiagnostics{accepted:2,consumed:2,pending:0}));
}
