use super::*;

const DESC: u32 = DRAM_LOW;
const INPUT: u32 = DRAM_LOW + 0x100;
const RX_DESC: u32 = DRAM_LOW + 0x200;
const OUTPUT: u32 = DRAM_LOW + 0x300;

fn bus_with_out(peripheral: u32, length: u32, next: u32) -> SocBus {
    let mut bus = SocBus::new(1024, 1024, [0; 6]);
    bus.write32(DESC, (1 << 31) | (length << 12) | length).unwrap();
    bus.write32(DESC + 4, INPUT).unwrap();
    bus.write32(DESC + 8, next).unwrap();
    let ch = &mut bus.periph.gdma.out[0];
    ch.running = true; ch.peri_sel = peripheral; ch.desc = DESC;
    bus
}

fn arm_aes_input(bus: &mut SocBus, capacity: u32, next: u32) {
    bus.write32(RX_DESC, (1 << 31) | capacity).unwrap();
    bus.write32(RX_DESC + 4, OUTPUT).unwrap();
    bus.write32(RX_DESC + 8, next).unwrap();
    let ch = &mut bus.periph.gdma.inp[0];
    ch.running = true; ch.peri_sel = 6; ch.desc = RX_DESC;
}

#[test]
fn empty_streaming_descriptor_cycles_raise_error_and_stop() {
    for peripheral in [3, 4, 5] {
        let mut bus = bus_with_out(peripheral, 0, DESC);
        match peripheral {
            3 | 4 => {
                let i2s = if peripheral == 3 { &mut bus.periph.i2s0 } else { &mut bus.periph.i2s1 };
                i2s.write(0x2c, 15 << 13); i2s.write(0x54, (1 << 16) | 3); i2s.write(0x24, 4); i2s.sample_rate = crate::periph::CPU_HZ as u32;
                bus.dma_i2s_step(1);
            }
            _ => {
                bus.periph.lcd_cam.lcd_user = 1 << 27;
                bus.periph.lcd_cam.lcd_ctrl = 1 << 31;
                bus.periph.lcd_cam.lcd_ctrl1 = 511 << 8;
                bus.dma_lcd_step(1);
            }
        }
        assert!(!bus.periph.gdma.out[0].running, "peripheral {peripheral}");
        assert_ne!(bus.periph.gdma.out[0].int_raw & (1 << 2), 0);
        assert!(bus.irq_dirty);
    }
}

#[test]
fn finite_i2s_work_can_read_a_nonempty_ring() {
    let mut bus = bus_with_out(3, 4, DESC);
    bus.write32(INPUT, 0x1234).unwrap();
    bus.periph.i2s0.write(0x2c, 15 << 13);
    bus.periph.i2s0.write(0x54, (1 << 16) | 3);
    bus.periph.i2s0.write(0x24, 4);
    bus.periph.i2s0.sample_rate = crate::periph::CPU_HZ as u32;
    bus.dma_i2s_step(2);
    assert!(bus.periph.gdma.out[0].running);
    assert_eq!(bus.periph.gdma.out[0].int_raw & (1 << 2), 0);
    assert_eq!(bus.periph.i2s0.pcm, [0x1234; 2]);
}

#[test]
fn crypto_cycles_do_not_hash_or_encrypt_partial_input() {
    for peripheral in [6, 7] {
        for length in [0, 16] {
            let mut bus = bus_with_out(peripheral, length, DESC);
            if peripheral == 6 {
                arm_aes_input(&mut bus, 64, 0);
                bus.aes_dma_step();
                assert_eq!(bus.periph.aes.blocks, 0);
                assert_eq!(bus.periph.aes.int_raw, 0);
            } else {
                bus.periph.sha.block_num = 1;
                bus.sha_dma_step();
                assert_eq!(bus.periph.sha.blocks, 0);
                assert!(!bus.periph.sha.busy);
            }
            assert!(!bus.periph.gdma.out[0].running);
            assert_ne!(bus.periph.gdma.out[0].int_raw & (1 << 2), 0);
        }
    }
}

#[test]
fn aes_empty_destination_cycle_is_not_reported_as_success() {
    let mut bus = bus_with_out(6, 16, 0);
    arm_aes_input(&mut bus, 0, RX_DESC);
    bus.aes_dma_step();
    assert!(!bus.periph.gdma.inp[0].running);
    assert_eq!(bus.periph.gdma.inp[0].int_raw, 1 << 3);
    assert_eq!(bus.periph.aes.int_raw, 0);
}

#[test]
fn sha_rejects_oversized_and_short_input_without_allocating_a_padded_message() {
    for blocks in [u32::MAX, 1] {
        let mut bus = bus_with_out(7, 0, 0);
        bus.periph.sha.block_num = blocks;
        bus.sha_dma_step();
        assert_eq!(bus.periph.sha.blocks, 0);
        assert!(!bus.periph.gdma.out[0].running);
        assert_eq!(bus.periph.gdma.out[0].int_raw & (1 << 2), 1 << 2);
    }
}

#[test]
fn crypto_descriptor_read_fault_stops_channel() {
    for peripheral in [6, 7] {
        let mut bus = bus_with_out(peripheral, 0, 0);
        bus.periph.gdma.out[0].desc = DRAM_HIGH;
        if peripheral == 6 { arm_aes_input(&mut bus, 16, 0); bus.aes_dma_step(); }
        else { bus.periph.sha.block_num = 1; bus.sha_dma_step(); }
        assert!(!bus.periph.gdma.out[0].running);
        assert_eq!(bus.periph.gdma.out[0].int_raw, 1 << 2);
    }
}

#[test]
fn aes_dma_scatters_across_descriptors_and_marks_only_final_eof() {
    let mut bus = bus_with_out(6, 16, 0);
    arm_aes_input(&mut bus, 8, RX_DESC + 16);
    bus.write32(RX_DESC + 16, (1 << 31) | 8).unwrap();
    bus.write32(RX_DESC + 20, OUTPUT + 8).unwrap();
    bus.write32(RX_DESC + 24, 0).unwrap();
    bus.aes_dma_step();
    let actual: Vec<_> = (0..16).map(|i| bus.read8(OUTPUT + i).unwrap()).collect();
    assert_eq!(actual, [0x66, 0xe9, 0x4b, 0xd4, 0xef, 0x8a, 0x2c, 0x3b, 0x88, 0x4c, 0xfa, 0x59, 0xca, 0x34, 0x2b, 0x2e]);
    assert_eq!(bus.read32(RX_DESC).unwrap() >> 30, 0);
    assert_eq!(bus.read32(RX_DESC + 16).unwrap() >> 30, 1);
    assert_eq!(bus.periph.gdma.inp[0].eof_desc, RX_DESC + 16);
    assert!(!bus.periph.gdma.inp[0].running);
    assert_eq!(bus.periph.aes.state, 2);
    assert_eq!(bus.periph.aes.int_raw, 1);
    assert_eq!(bus.periph.gdma.inp[0].int_raw, 0x3);
}

#[test]
fn sha_dma_hashes_exact_requested_message() {
    let mut bus = bus_with_out(7, 64, 0);
    let mut padded = [0; 64];
    padded[..4].copy_from_slice(b"abc\x80");
    padded[63] = 24;
    bus.load_bytes(INPUT, &padded).unwrap();
    bus.periph.sha.block_num = 1;
    bus.periph.sha.dma_first = true;
    bus.sha_dma_step();
    assert_eq!(&bus.periph.sha.h[..8], &[0xba7816bf, 0x8f01cfea, 0x414140de, 0x5dae2223, 0xb00361a3, 0x96177a9c, 0xb410ff61, 0xf20015ad]);
    assert_eq!(bus.periph.sha.blocks, 1);
    assert_eq!(bus.periph.gdma.out[0].int_raw & (1 << 2), 0);
}

#[test]
fn i2s_32_bit_stereo_consumes_eight_bytes_and_keeps_the_high_sample_bits() {
    let mut bus = bus_with_out(3, 16, 0);
    bus.write32(INPUT, 0x1234_5678).unwrap();
    bus.write32(INPUT + 4, 0xaaaa_bbbb).unwrap();
    bus.write32(INPUT + 8, 0xfedc_ba98).unwrap();
    bus.write32(INPUT + 12, 0xcccc_dddd).unwrap();
    bus.periph.i2s0.write(0x2c, (31 << 13) | (31 << 18) | (31 << 24));
    bus.periph.i2s0.write(0x54, (1 << 16) | 3);
    bus.periph.i2s0.write(0x24, 4);
    bus.periph.i2s0.sample_rate = crate::periph::CPU_HZ as u32;
    bus.dma_i2s_step(1);
    assert_eq!(bus.periph.gdma.out[0].buf_pos, 8);
    assert_eq!(bus.periph.i2s0.pcm, [0x1234]);
    bus.dma_i2s_step(1);
    assert_eq!(bus.periph.gdma.out[0].buf_pos, 16);
    assert_eq!(bus.periph.i2s0.pcm, [0x1234, 0xfedcu16 as i16]);
}

#[test]
fn aes_dma_rejects_mmio_destination_before_writing_any_register() {
    let mut bus = bus_with_out(6, 16, 0);
    arm_aes_input(&mut bus, 16, 0);
    bus.write32(RX_DESC + 4, 0x6000_4004).unwrap(); // GPIO_OUT
    bus.aes_dma_step();
    assert_eq!(bus.periph.gpio.out, 0);
    assert_eq!(bus.periph.gpio.enable, 0);
    assert!(!bus.periph.gdma.inp[0].running);
    assert_eq!(bus.periph.gdma.inp[0].int_raw, 1 << 3);
    assert_eq!(bus.periph.aes.int_raw, 0);
}

#[test]
fn streaming_dma_buffer_faults_raise_error_instead_of_emitting_zeros() {
    for peripheral in [3, 5] {
        let mut bus = bus_with_out(peripheral, 16, 0);
        bus.write32(DESC + 4, DRAM_HIGH).unwrap();
        if peripheral == 3 {
            bus.periph.i2s0.write(0x2c, 15 << 13);
            bus.periph.i2s0.write(0x54, (1 << 16) | 3);
            bus.periph.i2s0.write(0x24, 4);
            bus.periph.i2s0.sample_rate = crate::periph::CPU_HZ as u32;
            bus.dma_i2s_step(1);
            assert!(bus.periph.i2s0.pcm.is_empty());
        } else {
            bus.periph.lcd_cam.lcd_user = 1 << 27;
            bus.periph.lcd_cam.lcd_ctrl = 1 << 31;
            bus.periph.lcd_cam.lcd_ctrl1 = 511 << 8;
            bus.dma_lcd_step(1);
            assert_eq!(bus.periph.lcd_cam.lcd_frames, 0);
        }
        assert!(!bus.periph.gdma.out[0].running);
        assert_eq!(bus.periph.gdma.out[0].int_raw, 1 << 2);
    }
}

#[test]
fn crypto_owner_check_is_controlled_by_conf1() {
    for check_owner in [false, true] {
        for input_side in [false, true] {
            let mut bus = bus_with_out(6, 16, 0);
            arm_aes_input(&mut bus, 16, 0);
            let (desc, conf1) = if input_side {
                (RX_DESC, &mut bus.periph.gdma.inp[0].conf1)
            } else { (DESC, &mut bus.periph.gdma.out[0].conf1) };
            *conf1 = if check_owner { 1 << 12 } else { 0 };
            let control = bus.read32(desc).unwrap();
            bus.write32(desc, control & !(1 << 31)).unwrap();
            bus.aes_dma_step();
            assert_eq!(bus.periph.aes.state == 2, !check_owner);
        }
    }
}

use std::sync::Arc;
const FIRST_DESC: u32 = DRAM_LOW + 0x1000;
const M2M_DST: u32 = DRAM_LOW + 0x2000;
fn m2m_desc(bus: &mut SocBus, addr: u32, control: u32, buffer: u32, next: u32) {
    for (offset, word) in [(0, control), (4, buffer), (8, next)] { bus.write32(addr + offset, word).unwrap(); }
}

struct CameraBoard(Option<Arc<Vec<u8>>>);
impl crate::board::BoardModel for CameraBoard {
    fn name(&self) -> &'static str { "camera-test" }
    fn camera_frame(&mut self) -> Option<(u32, u32, Arc<Vec<u8>>)> {
        self.0.clone().map(|frame| (4, 1, frame))
    }
}

#[test]
fn camera_untouched_lcd_cam_does_no_work() {
    struct NoCameraWork;
    impl crate::board::BoardModel for NoCameraWork {
        fn name(&self) -> &'static str { "no-camera-work" }
        fn camera_frame(&mut self) -> Option<(u32, u32, Arc<Vec<u8>>)> {
            panic!("untouched LCD_CAM must not query the board");
        }
    }
    let mut bus = SocBus::new(4 << 20, 0, [1, 2, 3, 4, 5, 6]);
    bus.board = Box::new(NoCameraWork);
    for cycles in [1, 1000, bus.periph.lcd_cam.frame_cycles, u64::MAX] {
        bus.dma_cam_step(cycles);
        assert_eq!(bus.periph.lcd_cam.acc, 0);
        assert_eq!(bus.periph.lcd_cam.frames, 0);
        assert_eq!(bus.periph.lcd_cam.int_raw, 0);
        assert!(bus.periph.lcd_cam.cam_frame().is_none());
    }
    bus.periph.lcd_cam.write(0x64, 1 << 2);
    bus.periph.lcd_cam.write(0x64, 0);
    bus.dma_cam_step(u64::MAX);
    assert_eq!(bus.periph.lcd_cam.acc, 0, "disabling VSYNC before a frame returns to idle");
}

#[test]
fn camera_vsync_starts_capture_and_streams_partial_descriptors() {
    for (reverse, vsync_eof) in [(false, false), (true, true)] {
        let mut bus = SocBus::new(4 << 20, 0, [1, 2, 3, 4, 5, 6]);
        bus.board = Box::new(CameraBoard(Some(Arc::new(vec![1, 2, 3, 4, 5, 6, 7, 8]))));
        bus.periph.lcd_cam.set_clock_enabled(true);
    bus.periph.lcd_cam.write(0x04, 1 << 29);
    bus.periph.lcd_cam.frame_cycles = 1000;
        bus.periph.lcd_cam.write(0x64, 1 << 2);
        assert!(bus.cadence_active());
        for _ in 0..3 {
            bus.periph.lcd_cam.write(0x08, 0);
            bus.periph.lcd_cam.write(0x70, u32::MAX);
            bus.dma_cam_step(1000 - bus.periph.lcd_cam.acc);
            assert!(bus.periph.lcd_cam.irq(), "VSYNC before CAM_START or GDMA");
            bus.periph.lcd_cam.write(0x70, 1 << 2);
            m2m_desc(&mut bus, FIRST_DESC, (1 << 31) | 3, M2M_DST, FIRST_DESC + 12);
            m2m_desc(&mut bus, FIRST_DESC + 12, (1 << 31) | 5, M2M_DST + 3, 0);
            bus.periph.gdma.write(0x48, 5);
            bus.periph.gdma.write(0x20, (FIRST_DESC & 0xfffff) | (1 << 22));
            bus.periph.lcd_cam.write(0x04, (1 << 29) | (u32::from(reverse) << 6) | (u32::from(vsync_eof) << 8));
            bus.periph.lcd_cam.write(0x08, (1 << 29) | 7);
            bus.dma_cam_step(50);
            assert_eq!(bus.periph.gdma.inp[0].buf_pos, 0, "blanking");
            bus.dma_cam_step(125);
            assert_eq!(bus.periph.gdma.inp[0].buf_pos, 2);
            assert_eq!(bus.read32(FIRST_DESC).unwrap() >> 31, 1);
            bus.dma_cam_step(375);
            if vsync_eof { bus.dma_cam_step(450); }
            assert_eq!(bus.read32(FIRST_DESC).unwrap(), 3 | (3 << 12));
            assert_eq!(bus.read32(FIRST_DESC + 12).unwrap(), 5 | (5 << 12) | (1 << 30));
            assert_eq!(bus.periph.gdma.inp[0].int_raw & 3, 3);
            assert_eq!(bus.periph.gdma.inp[0].eof_desc, FIRST_DESC + 12);
            for i in 0..8 {
                let byte = (i + 1) as u8;
                assert_eq!(bus.read8(M2M_DST + i).unwrap(), if reverse { byte.reverse_bits() } else { byte });
            }
        }
    }
}

#[test]
fn camera_sensor_clock_survives_stop_reset_and_dma_failure() {
    let mut bus = SocBus::new(4 << 20, 0, [1, 2, 3, 4, 5, 6]);
    bus.periph.lcd_cam.set_clock_enabled(true);
    bus.periph.lcd_cam.write(0x04, 1 << 29);
    bus.periph.lcd_cam.frame_cycles = 1000;
    bus.periph.lcd_cam.write(0x64, 1 << 2);
    bus.dma_cam_step(1000);
    assert_eq!(bus.periph.lcd_cam.int_raw, 0, "no sensor frame");
    bus.board = Box::new(CameraBoard(Some(Arc::new(vec![42; 8]))));
    bus.dma_cam_step(500);
    bus.periph.lcd_cam.write(0x08, 1 << 30);
    bus.dma_cam_step(500);
    assert_eq!(bus.periph.lcd_cam.int_raw, 1 << 2);
    bus.periph.lcd_cam.write(0x70, 1 << 2);
    bus.periph.lcd_cam.write(0x08, (1 << 29) | 7);
    bus.periph.gdma.write(0x48, 5);
    m2m_desc(&mut bus, FIRST_DESC, 1 << 31, M2M_DST, FIRST_DESC);
    bus.periph.gdma.write(0x20, (FIRST_DESC & 0xfffff) | (1 << 22));
    bus.dma_cam_step(550);
    assert_eq!(bus.periph.gdma.inp[0].int_raw & (1 << 3), 1 << 3);
    bus.dma_cam_step(450);
    assert_eq!(bus.periph.lcd_cam.int_raw, 1 << 2, "DMA fault must not stop VSYNC");
    bus.periph.lcd_cam.write(0x08, 0);
    bus.periph.lcd_cam.write(0x70, 1 << 2);
    bus.dma_cam_step(1000);
    assert_eq!(bus.periph.lcd_cam.int_raw, 1 << 2);
}

#[test]
fn camera_stopped_capture_does_not_pause_or_replay_sensor_bytes() {
    let mut bus = SocBus::new(4 << 20, 0, [1, 2, 3, 4, 5, 6]);
    bus.board = Box::new(CameraBoard(Some(Arc::new(vec![1, 2, 3, 4, 5, 6, 7, 8]))));
    bus.periph.lcd_cam.set_clock_enabled(true);
    bus.periph.lcd_cam.write(0x04, 1 << 29);
    bus.periph.lcd_cam.frame_cycles = 1000;
    bus.periph.lcd_cam.write(0x64, 1 << 2);
    m2m_desc(&mut bus, FIRST_DESC, (1 << 31) | 8, M2M_DST, 0);
    bus.periph.gdma.write(0x48, 5);
    bus.periph.gdma.write(0x20, (FIRST_DESC & 0xfffff) | (1 << 22));
    bus.dma_cam_step(1000);
    bus.periph.lcd_cam.write(0x64, 0); // The published frame still advances with capture stopped.
    bus.dma_cam_step(175);
    assert_eq!(bus.periph.gdma.inp[0].buf_pos, 0);
    bus.periph.lcd_cam.write(0x08, (1 << 29) | 7);
    bus.dma_cam_step(125);
    assert_eq!(bus.read16(M2M_DST).unwrap(), 0x0403);
    bus.periph.lcd_cam.write(0x08, 0);
    bus.dma_cam_step(250);
    assert_eq!(bus.periph.gdma.inp[0].buf_pos, 2);
    bus.board = Box::new(CameraBoard(None));
    bus.periph.lcd_cam.write(0x70, 1 << 2);
    bus.dma_cam_step(450);
    assert_eq!(bus.periph.lcd_cam.int_raw, 0);
    assert!(bus.periph.lcd_cam.cam_frame().is_none());
}

#[test]
fn camera_host_frames_follow_sccb_configuration_into_dma_memory() {
    use esp_soc::{board::BoardModel, web::WebServer};
    let mut machine = crate::machine([1, 2, 3, 4, 5, 6]);
    let mut board = crate::board::waveshare_cam::WaveshareCam::new();
    let mut devices = board.i2c_devices();
    let sensor = &mut devices.iter_mut().find(|(_, addr, _)| *addr == 0x3c).unwrap().2;
    let write = |sensor: &mut Box<dyn crate::i2c::I2cDevice>, reg: u16, value| {
        sensor.start(false);
        for b in reg.to_be_bytes().into_iter().chain([value]) { sensor.write(b); }
        sensor.stop();
    };
    for (r, v) in [(0x3808, 0), (0x3809, 2), (0x380a, 0), (0x380b, 1), (0x3008, 2)] { write(sensor, r, v); }
    write(sensor, 0x4300, 0x61);
    assert_eq!(*board.camera_frame().unwrap().2, [0; 4], "black frames allow initialization before a host connects");
    machine.bus.board = Box::new(board);
    let web = WebServer::queued();
    machine.web = Some(web.clone());
    machine.bus.periph.lcd_cam.set_clock_enabled(true);
    machine.bus.periph.lcd_cam.write(0x04, 1 << 29);
    machine.bus.periph.lcd_cam.frame_cycles = 1000;
    machine.bus.periph.lcd_cam.write(0x64, 1 << 2);
    machine.bus.dma_cam_step(1000);
    assert_ne!(machine.bus.periph.lcd_cam.int_raw & (1 << 2), 0, "VSYNC before host input");
    // Two live inputs, then format changes at identical dimensions must invalidate the cache.
    for (index, (format, rgba, expected)) in [
        (0x61, [255, 0, 0, 255, 0, 255, 0, 255], vec![248, 0, 7, 224]),
        (0x61, [0, 0, 255, 0, 255, 255, 255, 0], vec![0, 31, 255, 255]),
        (0x10, [0, 0, 255, 0, 255, 255, 255, 0], vec![29, 255]),
        (0x30, [0, 0, 255, 0, 255, 255, 255, 0], vec![29, 191, 255, 117]),
    ].into_iter().enumerate() {
        write(sensor, 0x4300, format);
        let mut message = vec![3, 2, 0, 1, 0];
        message.extend_from_slice(&rgba);
        if index < 2 { web.push_incoming_bin(message); }
        machine.run(0);
        let bus = &mut machine.bus;
        bus.dma_cam_step(1000 - bus.periph.lcd_cam.acc);
        m2m_desc(bus, FIRST_DESC, (1 << 31) | expected.len() as u32, M2M_DST, 0);
        bus.periph.gdma.write(0x48, 5);
        bus.periph.gdma.write(0x20, (FIRST_DESC & 0xfffff) | (1 << 22));
        bus.periph.lcd_cam.write(0x08, (1 << 29) | (expected.len() as u32 - 1));
        bus.dma_cam_step(550);
        let actual: Vec<_> = (0..expected.len()).map(|i| bus.read8(M2M_DST + i as u32).unwrap()).collect();
        assert_eq!(actual, expected, "format {format:#x}");
        assert_eq!(bus.periph.gdma.inp[0].int_raw & 3, 3);
    }
    // Crop the right half of the array: only the white source pixel remains.
    for (reg, value) in [(0x3800, 5), (0x3801, 32), (0x3802, 0), (0x3803, 0),
        (0x3804, 10), (0x3805, 63), (0x3806, 7), (0x3807, 159), (0x4300, 0x61)] {
        write(sensor, reg, value);
    }
    assert_eq!(*machine.bus.board.camera_frame().unwrap().2, [255; 4]);
    write(sensor, 0x3801, 0);
    write(sensor, 0x3800, 0); // change only the window, keeping size and format
    assert_eq!(*machine.bus.board.camera_frame().unwrap().2, [0, 31, 255, 255]);
    write(sensor, 0x3800, 5); write(sensor, 0x3801, 32);
    write(sensor, 0x3809, 1);
    assert_eq!(*machine.bus.board.camera_frame().unwrap().2, [255; 2]);
    write(sensor, 0x3008, 0x42);
    assert!(machine.bus.board.camera_frame().is_none());
    write(sensor, 0x3008, 2);
    assert!(machine.bus.board.camera_frame().is_some());
    write(sensor, 0x3821, 0x20);
    assert!(machine.bus.board.camera_frame().is_none(), "unsupported JPEG must not emit YUV bytes");
    write(sensor, 0x3821, 0);
    assert!(machine.bus.board.camera_frame().is_some());
    write(sensor, 0x3008, 0x82);
    assert!(machine.bus.board.camera_frame().is_none(), "reset clears output geometry");
    for (r, v) in [(0x3809, 2), (0x380b, 1), (0x4300, 0x61), (0x3008, 2)] { write(sensor, r, v); }
    assert!(machine.bus.board.camera_frame().is_some());
    machine.bus.board.i2c_devices();
    assert!(machine.bus.board.camera_frame().is_none(), "reboot resets the shared sensor state too");
}

fn camera_bus(bytes: Vec<u8>) -> SocBus {
    let mut bus = SocBus::new(4 << 20, 0, [0; 6]);
    bus.board = Box::new(CameraBoard(Some(Arc::new(bytes))));
    bus.write32(0x600c_001c, 1 << 8).unwrap();
    bus.write32(0x600c_0024, 0).unwrap();
    bus.periph.lcd_cam.write(0x04, 1 << 29);
    bus.periph.lcd_cam.write(0x64, 1 << 2);
    bus.periph.lcd_cam.frame_cycles = 1000;
    bus.periph.gdma.write(0x48, 5);
    bus
}

#[test]
fn camera_vsync_eof_realigns_a_mid_frame_start() {
    let mut bus = camera_bus((1..=8).collect());
    bus.periph.lcd_cam.write(0x04, (1 << 29) | (1 << 8));
    bus.periph.lcd_cam.write(0x08, (1 << 29) | 1); // byte count must be ignored
    for i in 0..3 {
        m2m_desc(&mut bus, FIRST_DESC + i * 12, (1 << 31) | 16, M2M_DST + i * 16, FIRST_DESC + ((i + 1) % 3) * 12);
    }
    bus.dma_cam_step(1000);
    bus.dma_cam_step(300); // blanking + four bytes before DMA is armed
    bus.periph.gdma.write(0x20, (FIRST_DESC & 0xfffff) | (1 << 22));
    bus.dma_cam_step(250);
    assert_eq!(bus.periph.gdma.inp[0].rx_eof_pos, 4);
    assert_eq!(bus.periph.gdma.inp[0].int_raw & 2, 0, "VS_EOF waits for VSYNC");
    bus.dma_cam_step(450);
    assert_eq!(bus.read32(FIRST_DESC).unwrap(), 16 | (4 << 12) | (1 << 30));
    assert_eq!(bus.periph.gdma.inp[0].rx_eof_pos, 0);
    assert_eq!(bus.read32(M2M_DST).unwrap(), 0x08070605);
    for i in 1..3 {
        bus.dma_cam_step(550);
        bus.dma_cam_step(450);
        assert_eq!(bus.read32(FIRST_DESC + i * 12).unwrap(), 16 | (8 << 12) | (1 << 30));
        assert_eq!(bus.read32(M2M_DST + i * 16).unwrap(), 0x04030201);
        assert_eq!(bus.read32(M2M_DST + i * 16 + 4).unwrap(), 0x08070605);
    }
}

#[test]
fn camera_ring_has_multiple_eofs_per_frame_and_resets_each_counter() {
    let mut bus = camera_bus((1..=12).collect());
    bus.periph.lcd_cam.write(0x08, (1 << 29) | 2);
    for i in 0..4 { m2m_desc(&mut bus, FIRST_DESC + i * 12, (1 << 31) | 8, M2M_DST + i * 8, FIRST_DESC + ((i + 1) % 4) * 12); }
    bus.periph.gdma.write(0x20, (FIRST_DESC & 0xfffff) | (1 << 22));
    for _ in 0..2 {
        bus.dma_cam_step(1000 - bus.periph.lcd_cam.acc);
        bus.dma_cam_step(50);
        for i in 0..4 {
            bus.periph.gdma.write(0x14, u32::MAX);
            bus.dma_cam_step(125);
            assert_eq!(bus.periph.gdma.inp[0].int_raw & 3, 3);
            assert_eq!(bus.periph.gdma.inp[0].eof_desc, FIRST_DESC + i * 12);
            assert_eq!(bus.periph.gdma.inp[0].rx_eof_pos, 0);
            assert_eq!(bus.read32(FIRST_DESC + i * 12).unwrap(), 8 | (3 << 12) | (1 << 30));
            for j in 0..3 { assert_eq!(bus.read8(M2M_DST + i * 8 + j).unwrap(), (i * 3 + j + 1) as u8); }
        }
    }
    for reset in [false, true] {
        bus.dma_cam_step(1000 - bus.periph.lcd_cam.acc);
        bus.dma_cam_step(100); // one received byte, no EOF
        assert_eq!(bus.periph.gdma.inp[0].rx_eof_pos, 1);
        if reset { bus.periph.gdma.write(0, 1); }
        else { bus.periph.gdma.write(0x20, (FIRST_DESC & 0xfffff) | (1 << 22)); }
        assert_eq!(bus.periph.gdma.inp[0].rx_eof_pos, 0);
        assert_eq!(bus.periph.gdma.inp[0].buf_pos, 0);
    }
}

#[test]
fn camera_blanking_owner_modes_clock_reset_and_idle() {
    let mut bus = camera_bus(vec![42; 40]);
    m2m_desc(&mut bus, FIRST_DESC, 40, M2M_DST, 0); // CPU-owned
    bus.periph.gdma.write(4, 1 << 12);
    bus.periph.gdma.write(0x20, (FIRST_DESC & 0xfffff) | (1 << 22));
    bus.periph.lcd_cam.write(0x08, (1 << 29) | 39);
    bus.dma_cam_step(1000);
    bus.dma_cam_step(50); // would produce four bytes without blanking
    assert_eq!(bus.periph.gdma.inp[0].int_raw, 0);
    bus.dma_cam_step(50);
    assert_eq!(bus.periph.gdma.inp[0].int_raw & (1 << 3), 1 << 3);
    assert_eq!(bus.read32(M2M_DST).unwrap(), 0);
    for mode in [24, 28] {
        m2m_desc(&mut bus, FIRST_DESC, (1 << 31) | 40, M2M_DST, 0);
        bus.periph.gdma.write(0x20, (FIRST_DESC & 0xfffff) | (1 << 22));
        bus.periph.lcd_cam.write(0x08, (1 << 29) | (1 << mode) | 39);
        bus.dma_cam_step(1000 - bus.periph.lcd_cam.acc);
        let dropped = bus.periph.lcd_cam.dropped;
        bus.dma_cam_step(300); bus.dma_cam_step(250);
        assert_eq!(bus.periph.lcd_cam.dropped, dropped + 1);
        assert_eq!(bus.read32(M2M_DST).unwrap(), 0);
    }
    bus.periph.lcd_cam.write(0x08, (1 << 30) | (1 << 29));
    assert!(!bus.periph.lcd_cam.running(), "CAM_RESET stops capture without resetting the sensor clock");
    assert_eq!(bus.periph.lcd_cam.read(8) & (1 << 29), 0);
    for (addr, value) in [(0x6004_1004, 0), (0x600c_001c, 0)] {
        bus.write32(addr, value).unwrap();
        let before = bus.periph.lcd_cam.acc;
        bus.dma_cam_step(333);
        assert_eq!(bus.periph.lcd_cam.acc, before, "gated sensor clock");
        bus.write32(addr, if addr == 0x6004_1004 { 1 << 29 } else { 1 << 8 }).unwrap();
    }
    bus.periph.lcd_cam.write(0x64, 0);
    bus.dma_cam_step(1000);
    assert!(!bus.periph.lcd_cam.cam_clock_active());
    let before = bus.periph.lcd_cam.acc;
    bus.dma_cam_step(999);
    assert_eq!(bus.periph.lcd_cam.acc, before, "stopped sensor returns to idle");
}

#[test]
fn camera_partial_slices_resume_unaligned_without_false_ring_exhaustion() {
    let mut bus = camera_bus((0..40).collect());
    m2m_desc(&mut bus, FIRST_DESC, (1 << 31) | 40, M2M_DST, 0);
    bus.periph.gdma.write(0x20, (FIRST_DESC & 0xfffff) | (1 << 22));
    bus.periph.lcd_cam.write(0x08, (1 << 29) | 39);
    bus.dma_cam_step(1007); bus.dma_cam_step(50); bus.dma_cam_step(63);
    assert_eq!(bus.periph.gdma.inp[0].buf_pos, 5);
    bus.dma_cam_step(437);
    for i in 0..40 { assert_eq!(bus.read8(M2M_DST + i).unwrap(), i as u8); }
    assert_eq!(bus.periph.gdma.inp[0].int_raw, 0x3);
    assert!(!bus.periph.gdma.inp[0].running);
}

#[test]
fn camera_receive_leftover_data_reports_ring_exhaustion() {
    let mut bus = camera_bus((0..40).collect());
    m2m_desc(&mut bus, FIRST_DESC, (1 << 31) | 39, M2M_DST, 0);
    bus.periph.gdma.write(0x20, (FIRST_DESC & 0xfffff) | (1 << 22));
    bus.periph.lcd_cam.write(0x08, (1 << 29) | 39);
    bus.dma_cam_step(1000); bus.dma_cam_step(550);
    assert_eq!(bus.periph.gdma.inp[0].int_raw, 0x11);
    assert!(!bus.periph.gdma.inp[0].running);
    assert_eq!((bus.read32(FIRST_DESC).unwrap() >> 12) & 0xfff, 39);
}

#[test]
fn camera_large_frame_slow_clock_progress_does_not_overflow() {
    let mut bus = camera_bus(vec![0x5a; 16_000_000]);
    let period = 1u64 << 40;
    bus.periph.lcd_cam.frame_cycles = period;
    m2m_desc(&mut bus, FIRST_DESC, (1 << 31) | 8, M2M_DST, 0);
    bus.periph.gdma.write(0x20, (FIRST_DESC & 0xfffff) | (1 << 22));
    bus.dma_cam_step(period);
    // Arm capture after 90% of the pixels; the next tick crosses the old u64 product limit.
    bus.periph.lcd_cam.acc = period / 20 + period * 9 / 20;
    bus.periph.lcd_cam.write(0x08, (1 << 29) | 7);
    bus.dma_cam_step(period / 10);
    assert_eq!(bus.read32(M2M_DST).unwrap(), 0x5a5a5a5a);
    assert_eq!(bus.periph.gdma.inp[0].int_raw, 0x13);
}

#[test]
fn crypto_descriptor_writeback_preserves_mmio_side_effects() {
    let mut bus = bus_with_out(7, 64, 0);
    let channel = &mut bus.periph.gdma.out[0];
    // Existing S3 DMA accesses use the bus even for register-backed descriptors.
    // Returning ownership to OUT_CONF0 must retain its OUT_RST side effect. OUT_CONF0 reads
    // back as the descriptor word: OUT_AUTO_WRBACK (bit 2) and SIZE >= LENGTH.
    channel.desc = 0x6003_f060;
    channel.conf0 = (3 << 30) | (64 << 12) | 64 | (1 << 2) | 1;
    channel.conf1 = INPUT;
    bus.periph.sha.block_num = 1;
    bus.sha_dma_step();
    assert!(!bus.periph.gdma.out[0].running);
    assert_eq!(bus.periph.gdma.out[0].desc, 0);
}

#[test]
fn i80_dma_reaches_routed_bus_words_and_completes_irq() {
    use std::sync::{Arc,Mutex};
    use esp_soc::board::{BoardModel,LcdI80Pins};
    type Transfer = (LcdI80Pins,bool,Vec<u16>);
    struct Capture(Arc<Mutex<Vec<Transfer>>>);
    impl BoardModel for Capture {fn name(&self)->&'static str {"i80-capture"} fn lcd_i80(&mut self,p:LcdI80Pins,d:bool,w:&[u16]) {self.0.lock().unwrap().push((p,d,w.to_vec()));}}
    let frames=Arc::new(Mutex::new(Vec::new()));let mut bus=bus_with_out(5,4,0);bus.board=Box::new(Capture(frames.clone()));
    bus.write32(DESC,(1<<31)|(1<<30)|(4<<12)|4).unwrap();bus.write32(INPUT,0x10208001).unwrap();
    for (pin,signal) in [(6,133),(16,154),(13,132)] {bus.periph.io_mux.write(4+pin*4,1<<12);bus.periph.gpio.func_out_sel[pin as usize]=signal;}
    bus.periph.lcd_cam.set_clock_enabled(true);bus.periph.lcd_cam.lcd_clock=3<<29;
    bus.periph.lcd_cam.write(0x64,2);bus.periph.lcd_cam.ram.write(0x28,0x11ab);
    bus.periph.lcd_cam.lcd_user=(1<<27)|(1<<26)|(1<<31)|(1<<24)|(1<<13)|(1<<19)|(1<<21);
    bus.lcd_i80_start();let data=frames.lock().unwrap();assert_eq!(data.len(),2);
    assert_eq!(data[0].2,[0xab,0x11]);assert!(!data[0].1);assert_eq!(data[1].2,[1,128,8,4]);assert!(data[1].1);
    assert_eq!(data[1].0.data[0],1<<6);assert_eq!(data[1].0.data[1],0);assert_eq!(data[1].0.clock,1<<16);assert_eq!(data[1].0.select,1<<13);
    assert_eq!(bus.periph.lcd_cam.lcd_user&(1<<27),0);assert!(bus.periph.lcd_cam.irq());assert_ne!(bus.periph.gdma.out[0].int_raw&3,0);
    bus.periph.lcd_cam.write(0x70,2);assert!(!bus.periph.lcd_cam.irq());
}

#[test]
fn i80_dummy_priming_clock_rgb_and_bad_dma_do_not_fake_data() {
    let mut bus=SocBus::new(1024,1024,[0;6]);bus.periph.lcd_cam.lcd_clock=3<<29;
    bus.periph.lcd_cam.lcd_user=(1<<27)|(1<<25);bus.lcd_i80_start();assert_eq!(bus.periph.lcd_cam.int_raw,0,"clock disabled");
    bus.periph.lcd_cam.set_clock_enabled(true);bus.periph.lcd_cam.lcd_ctrl=1<<31;bus.lcd_i80_start();assert_eq!(bus.periph.lcd_cam.int_raw,0,"RGB is not i80");
    bus.periph.lcd_cam.lcd_ctrl=0;bus.lcd_i80_start();assert_eq!(bus.periph.lcd_cam.int_raw,2,"IDF dummy phase primes ISR without a DMA channel");
    for (length,desc,owner) in [(0,DESC,true),(4,0x60041014,true),(4,DESC,false)] {
        let mut bus=bus_with_out(5,length,desc);bus.periph.gdma.out[0].conf1=1<<12;
        if !owner {bus.write32(DESC,(length<<12)|length).unwrap();}
        bus.periph.lcd_cam.set_clock_enabled(true);bus.periph.lcd_cam.lcd_clock=3<<29;
        bus.periph.lcd_cam.lcd_user=(1<<27)|(1<<24)|(1<<13);bus.lcd_i80_start();
        assert_eq!(bus.periph.lcd_cam.int_raw&2,0);assert_eq!(bus.periph.gdma.out[0].int_raw&4,4);assert!(!bus.periph.gdma.out[0].running);
    }
}
