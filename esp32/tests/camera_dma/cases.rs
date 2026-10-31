use super::*;
use esp32::board::aithinker::{FRAME_CYCLES, VSYNC_CYCLES};
const I: u32 = 0x3ff4_f000;
const D: u32 = 0x3ffb_0000;
const B: u32 = 0x3ffb_1000;
const DONE: u32 = 1 << 8;
const EOF: u32 = 1 << 9;
const ERR: u32 = 1 << 13;
fn descriptor(b: &mut SocBus, index: u32, size: u32, next: u32) {
    put(b, D + index * 12, (1 << 31) | size);
    put(b, D + index * 12 + 4, B + index * 64);
    put(b, D + index * 12 + 8, next);
}
fn arm(b: &mut SocBus, words: u32) {
    put(b, I + 0x24, words);
    put(b, I + 0x60, 1 << 12);
    put(b, I + 0x14, EOF | ERR | DONE | (1 << 15));
    put(b, I + 0x34, (D & 0xfffff) | (1 << 29));
}
fn setup(mode: u32) -> SocBus {
    let mut b = running(); receiver(&mut b);
    b.board.set_camera_picture(picture([255,0,0])).unwrap();
    put(&mut b, I + 0x20, (mode << 16) | (1 << 12) | (1 << 20));
    b
}
fn begin(b: &mut SocBus) {
    if b.periph.gpio.gpio.level(25) {
        tick_bus(b,1); // newly armed during the physical pulse
        let next = b.board.next_deadline().unwrap();
        tick_bus(b,next-esp_soc::SocBus::cycles(b));
    } else {
        let now = esp_soc::SocBus::cycles(b);
        let next = b.board.next_deadline().unwrap();
        tick_bus(b, next - now); // physical VSYNC rising
        tick_bus(b, VSYNC_CYCLES); // falling: latch frame, not pulse time
    }
}
fn bytes(b: &mut SocBus, addr: u32, len: u32) -> Vec<u8> {
    (0..len).map(|n| b.read8(addr + n).unwrap()).collect()
}
fn raw(b: &mut SocBus) -> u32 { b.read32(I + 0xc).unwrap() }
#[test]
fn inverted_route_reuses_capture_cache_and_invalidates_source_and_mask() {
    let mut b = setup(1);
    descriptor(&mut b, 0, 64, D);
    arm(&mut b, 16);
    put(&mut b, 0x3ff4_4130 + 4 * 140, 0x80 | 5 | (1 << 6));
    let original = b.camera_frame().unwrap().2;
    let first = b.camera_routed_frame().unwrap().2;
    assert_eq!(*first, [249, 1].repeat(16));
    for _ in 0..3 {
        put(&mut b, 0x3ff4_4530 + 4 * 2, 256); // unrelated MMIO write
        assert!(Arc::ptr_eq(&first, &b.camera_routed_frame().unwrap().2));
    }
    assert_eq!(*original, [248, 0].repeat(16), "inversion must not alter the sensor cache");
    b.board.set_camera_picture(picture([0,255,0])).unwrap();
    let replacement = b.camera_routed_frame().unwrap().2;
    assert!(!Arc::ptr_eq(&first, &replacement));
    assert_eq!(*replacement, [6, 225].repeat(16));
    put(&mut b, 0x3ff4_4130 + 4 * 141, 0x80 | 18 | (1 << 6));
    let mask_changed = b.camera_routed_frame().unwrap().2;
    assert!(!Arc::ptr_eq(&replacement, &mask_changed));
    assert_eq!(*mask_changed, [4, 227].repeat(16));
}
#[test]
fn c7_exact_native_descriptors_two_eofs_and_classic_irq() {
    for mode in [1,3] {
        let mut b = setup(mode);
        // 32 sensor bytes: mode1 64 DMA bytes; mode3 128. Three
        // descriptors per EOF, including a short EOF-closed descriptor.
        let chunk = if mode == 1 {32} else {64};
        for j in 0..6 { descriptor(&mut b,j,chunk / 2 - 4,if j == 5 {0} else {D+(j+1)*12}); }
        arm(&mut b,chunk/4); begin(&mut b);
        assert_eq!(raw(&mut b),0,"VSYNC is GPIO, not I2S EOF");
        tick_bus(&mut b,(FRAME_CYCLES - VSYNC_CYCLES)/2);
        assert_eq!(raw(&mut b),DONE|EOF);
        assert_eq!(b.read32(I+0x10).unwrap(),DONE|EOF);
        assert_eq!(b.read32(I+0x3c).unwrap(),D+24);
        let mut actual=Vec::new();
        for j in 0..3 {
            let len=if j==2 {8} else {chunk/2-4};
            assert_eq!(b.read32(D+j*12).unwrap(),(chunk/2-4)|(len<<12)|if j==2 {1<<30}else{0});
            actual.extend(bytes(&mut b,B+j*64,len));
        }
        let pattern=if mode==1 {vec![0,0,248,0]}else{vec![0,0,248,0,0,0,0,0]};
        assert_eq!(actual,pattern.repeat(8));
        put(&mut b,I+0x18,DONE|EOF); assert_eq!(b.read32(I+0x10).unwrap(),0);
        tick_bus(&mut b,(FRAME_CYCLES - VSYNC_CYCLES)/2);
        assert_eq!(raw(&mut b),DONE|EOF);
        assert_eq!(b.read32(I+0x3c).unwrap(),D+60);
        let mut second=Vec::new();
        for j in 3..6 {
            let len=if j==5 {8} else {chunk/2-4};
            assert_eq!(b.read32(D+j*12).unwrap(),(chunk/2-4)|(len<<12)|if j==5 {1<<30}else{0});
            second.extend(bytes(&mut b,B+j*64,len));
        }
        assert_eq!(second,pattern.repeat(8));
    }
}
#[test]
fn c7_bad_owner_forbidden_descriptor_and_buffer_never_eof() {
    for bad in [0,1,2,3,4,5] {
        let mut b=setup(1); descriptor(&mut b,0,64,0); arm(&mut b,16);
        match bad {
            0=>put(&mut b,D,64),
            1=>put(&mut b,D+4,0x3f80_0000),
            2=>put(&mut b,I+0x34,(1<<29)|0x4f000), // I2S MMIO, not DRAM
            3=>put(&mut b,D+4,0x3fff_fff0), // crosses DRAM high
            4=>put(&mut b,D+4,0x4008_0000),
            _=>put(&mut b,D+4,0x3ffa_dafc), // CPU-only ROM integrity prefix
        }
        begin(&mut b); tick_bus(&mut b,FRAME_CYCLES - VSYNC_CYCLES);
        assert_eq!(raw(&mut b)&EOF,0,"bad {bad}");
        assert_ne!(raw(&mut b)&ERR,0,"bad {bad}");
        tick_bus(&mut b,FRAME_CYCLES); assert_eq!(raw(&mut b)&EOF,0);
    }
}
#[test]
fn c7_ring_starvation_and_finite_guest_service_reuse() {
    for service in [true,false] {
        let mut b=setup(1);
        descriptor(&mut b,0,16,D+12); descriptor(&mut b,1,16,D);
        arm(&mut b,8); begin(&mut b);
        let quarter=(FRAME_CYCLES - VSYNC_CYCLES)/4;
        for _ in 0..2 { tick_bus(&mut b,quarter); }
        assert_eq!(raw(&mut b),DONE|EOF);
        assert_eq!(b.read32(I+0x3c).unwrap(),D+12);
        if service {
            assert_eq!(bytes(&mut b,B,16),[0,0,248,0].repeat(4));
            descriptor(&mut b,0,16,D+12); descriptor(&mut b,1,16,D);
        }
        put(&mut b,I+0x18,u32::MAX);
        // Guest has one quarter-frame to refill the ring; no hidden host refill.
        tick_bus(&mut b,quarter); tick_bus(&mut b,quarter);
        if service {
            assert_eq!(raw(&mut b),DONE|EOF);
            assert_eq!(b.read32(I+0x3c).unwrap(),D+12);
            assert_eq!(bytes(&mut b,B+64,16),[0,0,248,0].repeat(4));
        } else { assert_eq!(raw(&mut b)&EOF,0); assert_ne!(raw(&mut b)&ERR,0); }
    }
    let mut b=setup(1); descriptor(&mut b,0,16,0); arm(&mut b,16); begin(&mut b);
    tick_bus(&mut b,FRAME_CYCLES-VSYNC_CYCLES);
    assert_eq!(raw(&mut b)&EOF,0); assert_ne!(raw(&mut b)&(ERR|(1<<15)),0);
}
#[test]
fn c7_stop_restart_and_every_rx_reset_discard_incomplete_frame() {
    for (off,value) in [(8,1<<7),(8,(1<<7)|(1<<5)|2),(8,(1<<7)|(1<<5)|8),
        (0x60,(1<<12)|1),(0x60,(1<<12)|4),(0x60,(1<<12)|8),(0x34,1<<28)] {
        let mut b=setup(1); descriptor(&mut b,0,64,0); arm(&mut b,16); begin(&mut b);
        tick_bus(&mut b,(FRAME_CYCLES-VSYNC_CYCLES)/4);
        assert_eq!(raw(&mut b)&EOF,0);
        put(&mut b,I+off,value);
        put(&mut b,I+8,(1<<7)|(1<<5)); put(&mut b,I+0x60,1<<12);
        tick_bus(&mut b,(FRAME_CYCLES-VSYNC_CYCLES)*3/4);
        assert_eq!(raw(&mut b)&EOF,0,"reset {off:x}:{value:x}");
        begin(&mut b); tick_bus(&mut b,FRAME_CYCLES-VSYNC_CYCLES);
        assert_eq!(raw(&mut b)&EOF,0,"RX reset/stop requires explicit inlink restart");
        // Explicit fresh inlink and fresh physical frame recover with no old bytes.
        descriptor(&mut b,0,64,0); arm(&mut b,16);
        begin(&mut b); tick_bus(&mut b,FRAME_CYCLES-VSYNC_CYCLES);
        assert_eq!(raw(&mut b)&EOF,EOF);
        assert_eq!(b.read32(D).unwrap(),64|(64<<12)|(1<<30));
        assert_eq!(bytes(&mut b,B,64),[0,0,248,0].repeat(16));
    }
}
#[test]
fn c7_tick_split_invariance_and_late_arm_waits_vsync() {
    for split in [1,7,64,257] {
        let mut b=setup(1); descriptor(&mut b,0,64,0); arm(&mut b,16);
        tick_bus(&mut b,1000); tick_bus(&mut b,(FRAME_CYCLES-VSYNC_CYCLES)/4);
        assert_eq!(bytes(&mut b,B,64),vec![0;64]);
        begin(&mut b);
        let span=FRAME_CYCLES-VSYNC_CYCLES;
        for _ in 0..split { tick_bus(&mut b,span/split); }
        tick_bus(&mut b,span%split);
        assert_eq!(b.read32(D).unwrap(),64|(64<<12)|(1<<30));
        assert_eq!(bytes(&mut b,B,64),[0,0,248,0].repeat(16));
    }
}

#[test]
fn c7_mid_frame_route_or_power_loss_is_terminal_until_rearm() {
    for (addr,value) in [(0x3ff4_4130+4*140,0x80|18),(0x3ff4_4010,1),(0x3ff4_4530,256)] {
        let mut b=setup(1); descriptor(&mut b,0,64,0); arm(&mut b,16); begin(&mut b);
        tick_bus(&mut b,(FRAME_CYCLES-VSYNC_CYCLES)/4);
        put(&mut b,addr,value); tick_bus(&mut b,100);
        assert_eq!(raw(&mut b)&EOF,0); assert_ne!(raw(&mut b)&ERR,0);
        dvp(&mut b); clock(&mut b); put(&mut b,0x3ff4_4010,0); receiver(&mut b);
        begin(&mut b); tick_bus(&mut b,FRAME_CYCLES-VSYNC_CYCLES);
        assert_eq!(raw(&mut b)&EOF,0);
    }
}
#[test]
fn c7_reacquire_next_image_on_reused_ring() {
    let mut b=setup(1);
    for (frame,color) in [[255,0,0],[0,255,0]].into_iter().enumerate() {
        b.board.set_camera_picture(picture(color)).unwrap();
        descriptor(&mut b,0,32,D+12); descriptor(&mut b,1,32,D);
        put(&mut b,I+0x18,u32::MAX);
        if frame == 0 { arm(&mut b,8); }
        begin(&mut b);
        tick_bus(&mut b,FRAME_CYCLES-VSYNC_CYCLES);
        assert_eq!(raw(&mut b),DONE|EOF);
        let pattern=if color[0]!=0 {[0,0,248,0]}else{[224,0,7,0]};
        assert_eq!(bytes(&mut b,B,32),pattern.repeat(8));
        assert_eq!(bytes(&mut b,B+64,32),pattern.repeat(8));
    }
}

#[test]
fn c7_qqvga_pattern_twenty_descriptors_ten_serviced_eof_chunks() {
    let mut b=setup(1);
    write(&mut b,0x5a,40); write(&mut b,0x5b,30);
    b.board.set_camera_picture(Picture {w:2,h:2,rgb:vec![255,0,0,0,255,0,0,0,255,255,255,255]}).unwrap();
    let (w,h,wire)=b.camera_routed_frame().unwrap();
    assert_eq!((w,h,wire.len()),(160,120,38400));
    let expected:Vec<_>=wire.as_chunks::<2>().0.iter().flat_map(|v| [v[1],0,v[0],0]).collect();
    for j in 0..20 {
        put(&mut b,D+j*12,(1<<31)|3840);
        put(&mut b,D+j*12+4,B+j*3840);
        put(&mut b,D+j*12+8,if j==19 {0}else{D+(j+1)*12});
    }
    arm(&mut b,1920); begin(&mut b);
    for chunk in 0..10 {
        tick_bus(&mut b,(FRAME_CYCLES-VSYNC_CYCLES)/10);
        assert_eq!(raw(&mut b),DONE|EOF);
        assert_eq!(b.read32(I+0x3c).unwrap(),D+(chunk*2+1)*12);
        for j in chunk*2..chunk*2+2 {
            assert_eq!(b.read32(D+j*12).unwrap(),3840|(3840<<12)|if j%2==1 {1<<30}else{0});
        }
        if chunk < 9 { assert_eq!(bytes(&mut b,B+(chunk+1)*7680,8),[0;8],"no pixels from the next interval"); }
        put(&mut b,I+0x18,DONE|EOF);
    }
    assert_eq!(bytes(&mut b,B,76800),expected);
}

#[test]
fn c7_camera_free_active_pcm_does_not_poll_board_camera() {
    use std::sync::atomic::{AtomicUsize,Ordering};
    struct AudioOnly(Arc<AtomicUsize>);
    impl esp_soc::board::BoardModel for AudioOnly {
        fn name(&self)->&'static str {"audio-only"}
        fn camera_dvp_pins(&self)->Option<[u8;11]> {self.0.fetch_add(1,Ordering::Relaxed); None}
    }
    let count=Arc::new(AtomicUsize::new(0));
    let mut b=SocBus::new(0,[0;6]); b.board=Box::new(AudioOnly(count.clone())); b.attach_board_devices();
    put(&mut b,0x3ff0_00c0,1<<4);
    put(&mut b,I+0xac,25|(1<<8)|(2<<14)); put(&mut b,I+0xb0,(25<<6)|(16<<18));
    put(&mut b,I+8,1<<5); descriptor(&mut b,0,64,0); arm(&mut b,16);
    for _ in 0..100 {tick_bus(&mut b,240);}
    assert_eq!(count.load(Ordering::Relaxed),0);
}

#[test]
fn c7_dport_reset_drops_capture_and_requires_fresh_inlink() {
    let mut b=setup(1); descriptor(&mut b,0,64,0); arm(&mut b,16); begin(&mut b);
    tick_bus(&mut b,(FRAME_CYCLES-VSYNC_CYCLES)/4);
    put(&mut b,0x3ff0_00c4,1<<4); put(&mut b,0x3ff0_00c4,0);
    assert_eq!(b.read32(I+0xa8).unwrap(),0);
    receiver(&mut b); begin(&mut b); tick_bus(&mut b,FRAME_CYCLES-VSYNC_CYCLES);
    assert_eq!(raw(&mut b)&EOF,0);
    descriptor(&mut b,0,64,0); arm(&mut b,16); begin(&mut b);
    tick_bus(&mut b,FRAME_CYCLES-VSYNC_CYCLES);
    assert_eq!(raw(&mut b)&EOF,EOF);
}

#[test]
fn c7_inlink_rmw_stop_must_remain_stopped_until_fresh_start() {
    rmw_stop(false);
}
#[test]
fn c7_rx_disable_rmw_stop_and_reenable_requires_fresh_start() {
    rmw_stop(true);
}
fn rmw_stop(disable_rx: bool) {
    for mode in [1,3] {
        let mut b=setup(mode);
        let size=if mode==1 {64}else{128};
        descriptor(&mut b,0,size,0); arm(&mut b,size/4); begin(&mut b);
        let active=FRAME_CYCLES-VSYNC_CYCLES;
        tick_bus(&mut b,active/4);
        assert_eq!(raw(&mut b),0);
        assert_ne!(bytes(&mut b,B,16),[0;16],"capture actually started");
        if disable_rx {
            let conf=b.read32(I+8).unwrap();
            put(&mut b,I+8,conf & !(1<<5));
        }
        // Volatile bitfield STOP, as in IDF and esp32-camera ll_cam_stop.
        let inlink=b.read32(I+0x34).unwrap();
        put(&mut b,I+0x34,inlink | (1<<28));
        let stopped=bytes(&mut b,B,size);
        let desc=b.read32(D).unwrap();
        let eof_desc=b.read32(I+0x3c).unwrap();
        // Re-enabling RX alone must not re-arm the stopped inlink.
        if disable_rx {
            let conf=b.read32(I+8).unwrap();
            put(&mut b,I+8,conf | (1<<5));
        }
        tick_bus(&mut b,active*3/4);
        begin(&mut b); tick_bus(&mut b,active);
        assert_eq!(raw(&mut b),0,"RMW STOP requires explicit new START");
        assert_eq!(b.read32(I+0x10).unwrap(),0);
        assert_eq!(b.read32(I+0x3c).unwrap(),eof_desc);
        assert_eq!(b.read32(D).unwrap(),desc);
        assert_eq!(bytes(&mut b,B,size),stopped);
        // Fresh START, after explicitly clearing other commands, and a new
        // physical VSYNC recover the full frame, never a partial old frame.
        descriptor(&mut b,0,size,0);
        let inlink=b.read32(I+0x34).unwrap();
        put(&mut b,I+0x34,(inlink & !(7<<28)) | (1<<29));
        begin(&mut b); tick_bus(&mut b,active);
        assert_eq!(raw(&mut b),DONE|EOF);
        assert_eq!(b.read32(D).unwrap(),size|(size<<12)|(1<<30));
        let pattern=if mode==1 {vec![0,0,248,0]}else{vec![0,0,248,0,0,0,0,0]};
        assert_eq!(bytes(&mut b,B,size),pattern.repeat(16));
    }
}

#[path = "arming.rs"]
mod arming;
#[path = "cancellation.rs"]
mod cancellation;

#[test]
fn jpeg_frame_is_host_bytes_then_zeros_to_one_word_before_an_eof() {
    // An odd-length JPEG leaves a mode-1 half word: it is flushed, not dropped.
    for (mode, eof, tag, odd) in [(3, 8, 1, false), (3, 13, 2, false), (1, 8, 3, false), (1, 8, 4, true), (3, 8, 5, true)] {
        let mut b=setup(mode);
        write(&mut b,0xda,0x12);
        let mut host=super::jpeg(4,4,tag);
        if odd { let end=host.len()-2; host.insert(end,0x55); }
        assert_eq!(host.len()%2==1,odd);
        b.board.set_camera_jpeg(Jpeg::parse(host.clone()).unwrap()).unwrap();
        for j in 0..8 { descriptor(&mut b,j,eof*4,if j==7 {0}else{D+(j+1)*12}); }
        for a in (B..B+8*64).step_by(4) { put(&mut b,a,0xaaaa_aaaa); }
        arm(&mut b,eof); begin(&mut b);
        tick_bus(&mut b,FRAME_CYCLES-VSYNC_CYCLES);
        let mut stream:Vec<u8>=if mode==3 {host.iter().flat_map(|&x| [0,0,x,0]).collect()}
            else {let (pairs,tail)=host.as_chunks::<2>(); pairs.iter().flat_map(|p| [p[1],0,p[0],0]).chain(tail.iter().flat_map(|&x| [0,0,x,0])).collect()};
        let words=stream.len() as u32/4;
        // At least one EOF block; the last block stops one word short of its EOF.
        let mut total=words.max(eof);
        while total%eof!=eof-1 { total+=1; }
        stream.resize(total as usize*4,0);
        let blocks=total/eof;
        let mut actual=Vec::new();
        for j in 0..=blocks { actual.extend(bytes(&mut b,B+j*64,eof*4)); }
        assert_eq!(&actual[..stream.len()],&stream[..],"mode {mode}, EOF {eof}");
        assert_eq!(&actual[stream.len()..],&[0xaa;4],"the final word is never written");
        for j in 0..blocks { assert_eq!(b.read32(D+j*12).unwrap(),(eof*4)|((eof*4)<<12)|(1<<30)); }
        assert_ne!(b.read32(D+blocks*12).unwrap()&(1<<31),0,"the last block raises no EOF");
        assert_eq!(raw(&mut b)&EOF,EOF);
    }
}
#[test]
fn jpeg_mode_without_matching_host_jpeg_delivers_nothing() {
    let mut b=setup(3);
    write(&mut b,0xda,0x12);
    for j in 0..2 { descriptor(&mut b,j,32,if j==1 {0}else{D+12}); }
    arm(&mut b,8); begin(&mut b);
    tick_bus(&mut b,FRAME_CYCLES);
    assert_eq!(raw(&mut b)&EOF,0);
    assert_eq!(bytes(&mut b,B,32),vec![0;32]);
    // A JPEG of the configured size starts at the next frame, not mid-frame.
    b.board.set_camera_jpeg(Jpeg::parse(super::jpeg(4,4,9)).unwrap()).unwrap();
    begin(&mut b); tick_bus(&mut b,FRAME_CYCLES-VSYNC_CYCLES);
    assert_eq!(raw(&mut b)&EOF,EOF);
    assert_eq!(bytes(&mut b,B,8),[0,0,0xff,0,0,0,0xd8,0]);
}
