//! Digital codec controls plus actual S3 I2S/GDMA memory and PCM, without a host sound device.
use emu_core::Bus;
use esp_soc::SocBus as _;
use esp_periph::i2s::{PcmPins,PcmSource};
type S3=esp32s3::bus::SocBus;
const DESC:u32=0x3fc9_0000;
const BUFFER:u32=0x3fc9_1000;

fn put(b:&mut S3,a:u32,v:u32) {b.write32(a,v).unwrap();}
fn route(b:&mut S3,pin:u32,signal:u32,input:bool) {
    put(b,0x6000_9004+4*pin,(1<<12)|if input{1<<9}else{0});
    put(b,0x6000_4554+4*pin,signal);
}
fn codec(b:&mut S3,controller:u32,reg:u8,value:u8) {
    // Real controller transaction on the board's SDA8/SCL9, including ACK checking.
    for (pin,signal) in [(8,90+controller*2),(9,89+controller*2)] {
        route(b,pin,signal,true);put(b,0x6000_4154+4*signal,0x80|pin);
    }
    let base=if controller==0{0x6001_3000}else{0x6002_7000};
    put(b,base+0x24,u32::MAX);put(b,base+0x18,3<<12);
    for v in [0x30,reg,value] {put(b,base+0x1c,u32::from(v));}
    for (off,v) in [(0x58,6<<11),(0x5c,1<<11|1<<8|3),(0x60,2<<11),(4,1<<5)] {put(b,base+off,v);}
    b.tick(100_000);b.flush_ticks();let status=b.read32(base+0x20).unwrap();
    assert_eq!(status&(1<<10),0,"codec NACK reg {reg:#x}");assert_ne!(status&(1<<7),0);
}
fn fixture()->S3 {
    let mut m=esp32s3::machine([0;6]);m.bus.board=esp32s3::board::make_board("cardputer-adv").unwrap();m.bus.attach_board_devices();
    for (r,v) in [(0,0x80),(1,0xbf),(9,0x0c),(10,0x0c),(0x0d,1),(0x0e,2),(0x12,0),
        (0x14,0x10),(0x16,0),(0x17,0xbf),(0x32,0xbf)] {codec(&mut m.bus,0,r,v);}
    m.bus
}
fn descriptor(b:&mut S3,size:u32,tx:bool) {
    for (off,v) in [(0,(1<<31)|if tx{(1<<30)|(size<<12)|size}else{size}),(4,BUFFER),(8,0)] {
        b.load_bytes(DESC+off,&v.to_le_bytes()).unwrap();
    }
}
fn arm_rx(b:&mut S3,port:usize,bits:u32,fill:bool,size:u32) {
    let data=if port==0{25}else{30};let bck=if port==0{26}else{31};
    route(b,46,256,true);route(b,41,bck,false);route(b,43,bck+1,false);
    put(b,0x6000_4154+4*data,0x80|46);
    let mut source=PcmSource::stream(16000,Some(PcmPins::I2s{bclk:41,ws:43,data:46})).unwrap();
    source.push(&[[1000;2];8]);b.attach_pcm_sources().unwrap().inputs=vec![Some(source)];
    descriptor(b,size,false);b.load_bytes(BUFFER,&[0x55;64]).unwrap();
    let p=&mut b.periph;
    let rx=if port==0{&mut p.i2s0}else{&mut p.i2s1};
    // IDF v5.4.2 S3 i2s_reg.h: PLL160/(39+1/16)/4/64 = 16kHz.
    rx.write(0x28,((bits-1)<<13)|(31<<18)|(3<<7));rx.write(0x30,(1<<26)|(2<<27)|39);
    rx.write(0x38,1|(15<<9));rx.write(0x50,(1<<16)|1);rx.write(0x64,64);
    rx.write(0x20,(1<<5)|4|if fill{1<<16}else{0});assert_eq!(rx.rx_rate(),Some(16000));
    p.gdma.inp[0].desc=DESC;p.gdma.inp[0].peri_sel=3+port as u32;p.gdma.inp[0].running=true;
    b.refresh_tick_budget();
}
fn advance(b:&mut S3) {b.tick(120_000);b.flush_ticks();}
fn memory(b:&mut S3,n:usize)->Vec<u8> {(0..n).map(|i|b.read8_unpriced(BUFFER+i as u32).unwrap()).collect()}

#[test]
fn rx_dma_packing_gain_mute_power_and_physical_routing() {
    for port in 0..2 {
        for (bits,fill,bytes) in [(16,false,2),(24,false,3),(24,true,4),(32,false,4)] {
            let mut b=fixture();arm_rx(&mut b,port,bits,fill,8*bytes);advance(&mut b);
            let word=if bytes==2{1000i16.to_le_bytes().to_vec()}else{(1000i32<<if bytes==3{8}else{16}).to_le_bytes()[..bytes as usize].to_vec()};
            assert_eq!(memory(&mut b,8*bytes as usize),word.repeat(8));
            let rx=if port==0{&b.periph.i2s0}else{&b.periph.i2s1};assert_eq!(rx.bytes_in,u64::from(8*bytes));
            assert!(!b.periph.gdma.inp[0].running);assert_ne!(b.periph.gdma.inp[0].int_raw&1,0);
        }
        for (reg,val,expected) in [(0x17,0xb3,501),(0x14,0x12,1995),(0x16,1,1995),(0x16,16,-1000),
            (0x17,0xff,32767),(10,0x4c,0),(0x0d,0xfc,0),(0x0e,0x62,0),(0,0x1f,0),(0,0,0),(0xfa,1,0)] {
            let mut b=fixture();codec(&mut b,1,reg,val);arm_rx(&mut b,port,16,false,16);advance(&mut b);
            assert_eq!(memory(&mut b,16),(expected as i16).to_le_bytes().repeat(8),"port {port} reg {reg:#x}");
            assert_eq!(if port==0{b.periph.i2s0.bytes_in}else{b.periph.i2s1.bytes_in},16,"silence still reaches DMA");
        }
        for bad in 0..7 {
            let mut b=fixture();arm_rx(&mut b,port,16,false,16);
            match bad {
                0=>b.periph.gpio.func_in_sel[if port==0{25}else{30}]=45|0x80,
                1=>b.periph.gpio.func_out_sel[41]^=1,
                2=>b.periph.gpio.func_out_sel[43]^=1<<9,
                3=>b.periph.io_mux.write(4+4*46,1<<12), // input buffer disabled
                4=>b.periph.io_mux.write(4+4*41,0),
                5=>b.periph.gpio.func_out_sel[41]|=1<<10, // GPIO-controlled OE disabled
                _=>b.periph.gpio.func_in_sel[if port==0{25}else{30}]^=1<<6,
            }
            advance(&mut b);assert_eq!(memory(&mut b,16),[0;16],"port {port} bad wire {bad}");
        }
    }
}

#[test]
fn rx_dma_count_is_written_payload_for_short_bad_stopped_and_disabled_channels() {
    for port in 0..2 {
        for (size,owner_check,stopped,disabled,expected) in [(8,false,false,false,8),(16,true,false,false,0),
            (16,false,true,false,0),(16,false,false,true,0)] {
            let mut b=fixture();arm_rx(&mut b,port,16,false,size);
            if owner_check {put(&mut b,DESC,size);b.periph.gdma.inp[0].conf1=1<<12;}
            if stopped {b.periph.gdma.inp[0].running=false;}
            if disabled {if port==0{b.periph.i2s0.write(0x20,0)}else{b.periph.i2s1.write(0x20,0)}}
            advance(&mut b);let count=if port==0{b.periph.i2s0.bytes_in}else{b.periph.i2s1.bytes_in};assert_eq!(count,expected);
            let actual=memory(&mut b,16);assert_eq!(&actual[expected as usize..],&vec![0x55;16-expected as usize]);
            if expected>0 {assert_eq!(&actual[..expected as usize],&1000i16.to_le_bytes().repeat(expected as usize/2));}
        }
    }
}

#[test]
fn tx_dma_codec_gain_mute_power_and_wires_change_real_pcm() {
    for port in 0..2 {
        for bad in 0..7 {
            let mut b=fixture();
            let (reg,val,expected)=match bad {0=>(0x32,0xbf,1000),1=>(0x32,0xb3,501),2=>(9,0x4c,0),3=>(0x12,2,0),4=>(0,0,0),_ =>(0x32,0xbf,0)};
            codec(&mut b,1,reg,val);
            let (data,bck,ws)=if port==0{(25,22,24)}else{(30,28,29)};
            route(&mut b,42,data,false);route(&mut b,41,bck,false);route(&mut b,43,ws,false);
            if bad==5 {b.periph.io_mux.write(4+4*42,0);}
            if bad==6 {b.periph.gpio.func_out_sel[43]^=1<<9;}
            descriptor(&mut b,16,true);b.load_bytes(BUFFER,&1000i16.to_le_bytes().repeat(8)).unwrap();
            let p=&mut b.periph;let tx=if port==0{&mut p.i2s0}else{&mut p.i2s1};
            tx.write(0x2c,(15<<13)|(31<<18)|(3<<7));tx.write(0x34,(1<<26)|(2<<27)|39);tx.write(0x3c,1|(15<<9));
            tx.write(0x54,(1<<16)|1);tx.write(0x24,(1<<5)|4);assert_eq!(tx.sample_rate,16000);
            p.gdma.out[0].desc=DESC;p.gdma.out[0].peri_sel=3+port as u32;p.gdma.out[0].running=true;
            b.refresh_tick_budget();advance(&mut b);
            let tx=if port==0{&b.periph.i2s0}else{&b.periph.i2s1};
            assert_eq!(tx.frames_out,8);assert_eq!(tx.pcm,[expected;8],"port {port} control {bad}");
            // A later interval observes the terminal descriptor and real EOF interrupt.
            advance(&mut b);assert_ne!(b.periph.gdma.out[0].int_raw&2,0);
        }
    }
}
