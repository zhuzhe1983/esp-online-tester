//! Original diagnostics, not manufacturer firmware. Every peripheral register
//! access executes an LX7 S32I/L32I and checks the retired instruction's PC.
use emu_core::{Bus, Core};
use esp_soc::{SocBus, Stop};
use esp_periph::sdmmc::{self, reg, des0, int, CMD_START, CMD_RESP, CMD_RESP_LONG, CMD_DATA,
    CMD_WRITE, CTRL_USE_INTERNAL_DMA, CTRL_INT_ENABLE, BMOD_DE};
const PC: u32 = 0x4037_0000;
const GPIO: u32 = 0x6000_4000;
const SD: u32 = 0x6002_8000;
const RAM: u32 = esp32s3::bus::DRAM_LOW + 0x1000;
const BOARDS: [&str; 2] = ["waveshare-s3-touch-lcd4-v4", "waveshare-s3-lcd4-v4"];
const RGB: [u8; 16] = [5,45,48,47,21,14,13,12,11,10,9,46,3,8,18,17];
const CORNERS: [usize;4] = [0,479,479 * 480,480 * 480 - 1];
const CORNER_COLORS: [u16;4] = [0xf81f,0x07ff,0xffe0,0xffff];
const SAMPLES: [usize; 3] = [240 * 480 + 120, 240 * 480 + 240, 240 * 480 + 360];
struct Guest { m: esp32s3::Machine }
impl Guest {
    fn new(name: &str) -> Self {
        let mut m = esp32s3::machine([0; 6]); m.bus.board = esp32s3::board::make_board(name).unwrap();
        m.bus.attach_board_devices(); m.bus.set_flash_size(16 << 20); m.bus.set_psram_size(8 << 20).unwrap();
        m.bus.load_bytes(PC, &[0x32,0x64,0,0x52,0x24,0]).unwrap();
        m.cores[0].ps = 0; m.cores[0].set_jit(false); Self { m }
    }
    fn put(&mut self, address: u32, value: u32) {
        self.m.cores[0].pc = PC; self.m.cores[0].set_ar(4, address); self.m.cores[0].set_ar(3, value);
        assert!(matches!(self.m.run(1), Stop::MaxInsns)); assert_eq!(self.m.cores[0].pc, PC + 3);
    }
    fn get(&mut self, address: u32) -> u32 {
        self.m.cores[0].pc = PC + 3; self.m.cores[0].set_ar(4, address);
        assert!(matches!(self.m.run(1), Stop::MaxInsns)); assert_eq!(self.m.cores[0].pc, PC + 6);
        self.m.cores[0].get_ar(5)
    }
    fn route(&mut self, pin: u8, signal: u32) {
        self.put(0x6000_9004 + 4 * u32::from(pin), 1 << 12 | 1 << 9);
        self.put(GPIO + 0x554 + 4 * u32::from(pin), signal);
        if signal == 256 { self.put(GPIO + if pin < 32 { 0x24 } else { 0x30 }, 1 << (pin % 32)); }
    }
    fn high(&mut self, pin: u8, value: bool) {
        self.put(GPIO + if pin < 32 { if value { 8 } else { 12 } } else if value { 20 } else { 24 }, 1 << (pin % 32));
    }
    fn i2c_route(&mut self, bus: u32, sda: u8, scl: u8) {
        for (pin, signal) in [(sda,90 + bus * 2), (scl,89 + bus * 2)] {
            self.route(pin, signal); self.put(GPIO + 0x154 + 4 * signal, 0x80 | u32::from(pin));
        }
    }
    fn i2c(&mut self, bus: u32, address: u8, data: &[u8], n: u8) -> Option<Vec<u8>> {
        let base = [0x6001_3000,0x6002_7000][bus as usize];
        self.put(base + 0x24, u32::MAX); self.put(base + 0x18, 3 << 12);
        let mut commands = vec![];
        if !data.is_empty() || n == 0 {
            for v in [address << 1].into_iter().chain(data.iter().copied()) { self.put(base + 0x1c, u32::from(v)); }
            commands.extend([6 << 11, 1 << 11 | 1 << 8 | (data.len() + 1) as u32]);
        }
        if n > 0 {
            self.put(base + 0x1c, u32::from(address << 1 | 1));
            commands.extend([6 << 11, 1 << 11 | 1 << 8 | 1, 3 << 11 | u32::from(n)]);
        }
        commands.push(2 << 11);
        for (i, c) in commands.into_iter().enumerate() { self.put(base + 0x58 + 4 * i as u32, c); }
        self.put(base + 4, 1 << 5); self.m.bus.tick(100_000);
        let status = self.get(base + 0x20); if status & (1 << 10) != 0 { return None; }
        assert_ne!(status & (1 << 7), 0);
        Some((0..n).map(|_| self.get(base + 0x1c) as u8).collect())
    }
    fn ch32(&mut self, register: u8, value: u8) { self.i2c(0, 0x24, &[register,value], 0).unwrap(); }
    fn setup(&mut self) {
        self.i2c_route(0, 15, 7); self.ch32(2, 0xff); self.ch32(3, 0); self.ch32(3, 0x2a);
        self.ch32(5, 0); for p in [42,2,1] { self.route(p, 256); } self.high(42, true);
    }
    fn serial_bits(&mut self, cs: u8, dc: bool, value: u8, count: u8) {
        self.high(cs, true); self.high(2, false); self.high(cs, false);
        let word = u16::from(value) | u16::from(dc) << 8;
        for bit in (0..count).rev() {
            self.high(2, false); self.high(1, (word >> bit) & 1 != 0); self.high(2, true);
        }
        self.high(cs, true);
    }
    fn serial(&mut self, dc: bool, value: u8) { self.serial_bits(42, dc, value, 9); }
    fn command(&mut self, command: u8, args: &[u8]) { self.serial(false, command); for &a in args { self.serial(true, a); } }
    fn spi9(&mut self, dc: bool, value: u8, sample_unconnected_miso: bool) {
        const SPI: u32 = 0x6002_4000;
        self.put(SPI + 0x10, 1 << 31 | 1 << 30 | if sample_unconnected_miso { 1 << 28 | 1 } else { 0 });
        self.put(SPI + 0x14, 7 << 27); self.put(SPI + 0x18, if dc { 0x80 } else { 0 });
        self.put(SPI + 4, u32::from(value) << 24); self.put(SPI + 0x1c, 7); self.put(SPI, 1 << 24);
        self.m.bus.flush_ticks();
        if sample_unconnected_miso { assert_eq!(self.get(SPI + 0x98) & 255,255,"no fabricated serial readback"); }
    }
    fn panel_init(&mut self) { self.command(0x11, &[]); self.command(0x3a, &[0x66]); self.command(0x29, &[]); }
    fn rgb_routes(&mut self) {
        for (p,s) in [(41,154),(38,151),(39,152),(40,150)] { self.route(p,s); }
        for (i,p) in RGB.into_iter().enumerate() { self.route(p,133 + i as u32); }
    }
    fn scan(&mut self) {
        // Original fixture data; every descriptor and peripheral control write
        // is issued by CPU instructions. DMA reads actual emulated guest RAM.
        const DESC: u32 = 0x3fc8_8000; const DATA: u32 = 0x3fc8_a000;
        let total = 480 * 480 * 2; let mut data = vec![0;4092];
        for (index,color) in SAMPLES.into_iter().zip([0xf800u16,0x07e0,0x001f]).chain(CORNERS.into_iter().zip(CORNER_COLORS)) {
            let at = (2 * index) % 4092; data[at..at+2].copy_from_slice(&color.to_le_bytes());
        }
        self.m.bus.load_bytes(DATA,&data).unwrap(); let (mut pos, mut index) = (0,0);
        while pos < total {
            let len = (total - pos).min(4092); let last = pos + len == total;
            for (i,v) in [1 << 31 | u32::from(last) << 30 | len << 12 | len, DATA,
                if last { 0 } else { DESC + 12 * (index + 1) }].into_iter().enumerate() {
                self.put(DESC + 12 * index + 4 * i as u32,v);
            }
            pos += len; index += 1;
        }
        for (a,v) in [(0x60041014,0), (0x600c001c,1 << 8), (0x6003f0a8,5),
            (0x6003f080,(DESC & 0xfffff) | (1 << 21)), (0x60041000,(1 << 31) | (3 << 29) | (20 << 9) | (1 << 6)),
            (0x60041020,(479 << 8) | (501 << 20)), (0x6004101c,(1 << 31) | (479 << 11) | (518 << 21)),
            (0x60041014,(1 << 27) | (1 << 24) | (1 << 23))] { self.put(a,v); }
        for _ in 0..240 { self.m.bus.tick(100_000); } self.m.bus.refresh_board_outputs();
    }
    fn samples(&self) -> [u16;3] { let f = self.m.bus.board.display().unwrap().2; SAMPLES.map(|i| f[i]) }
    fn sd_wiring(&mut self) {
        for (p,s) in [(2,173),(1,179),(4,213)] { self.route(p,s); self.put(GPIO + 0x154 + 4 * s,0x80 | u32::from(p)); }
        let c = self.get(0x600c001c); self.put(0x600c001c,c | 1 << 7);
        let r = self.get(0x600c0024); self.put(0x600c0024,r & !(1 << 7));
    }
    fn cmd(&mut self, n: u8, argument: u32, flags: u32) -> u32 {
        self.put(SD + reg::RINTSTS,u32::MAX); self.put(SD + reg::CMDARG,argument);
        self.put(SD + reg::CMD,CMD_START | 1 << 16 | u32::from(n) | flags); self.get(SD + reg::RESP0)
    }
    fn card_ready(&mut self) {
        self.cmd(0,0,0); assert_eq!(self.cmd(8,0x1aa,CMD_RESP),0x1aa);
        for _ in 0..2 { self.cmd(55,0,CMD_RESP); self.cmd(41,0x40ff8000,CMD_RESP); }
        self.cmd(2,0,CMD_RESP | CMD_RESP_LONG); let rca = self.cmd(3,0,CMD_RESP) & 0xffff0000;
        self.cmd(7,rca,CMD_RESP); self.put(SD + reg::CTYPE,0);
    }
    fn sector(&mut self, n: u8, buffer: u32) {
        for (i,v) in [des0::OWN | des0::FS | des0::LD,512,buffer,0].into_iter().enumerate() { self.put(RAM + i as u32 * 4,v); }
        for (r,v) in [(reg::CTRL,CTRL_USE_INTERNAL_DMA | CTRL_INT_ENABLE),(reg::BMOD,BMOD_DE),
            (reg::DBADDR,RAM),(reg::BLKSIZ,512),(reg::BYTCNT,512)] { self.put(SD + r,v); }
        self.cmd(n,7,CMD_RESP | CMD_DATA | if n == 24 { CMD_WRITE } else { 0 });
        assert_ne!(self.get(SD + reg::RINTSTS) & int::DATA_OVER,0); assert_eq!(self.get(RAM) & des0::OWN,0);
    }
}

#[test]
fn actual_lx7_v4_gpio_serial_and_gdma_rgb_require_physical_routes_reset_and_inverse_pwm() {
    for name in BOARDS {
        let mut g = Guest::new(name); g.setup(); g.rgb_routes();
        g.route(43,256); for c in [0x11,0x29] { g.serial_bits(43,false,c,9); }
        g.scan(); assert_eq!(g.samples(),[0;3],"unrelated CS cannot initialize glass");
        g.panel_init(); g.scan(); assert!(g.m.bus.periph.lcd_cam.lcd_frames > 0);
        assert_eq!(g.samples(),[0xf800,0x07e0,0x001f],"{name}: {}",g.m.bus.board.report());
        let pixels = g.m.bus.board.display().unwrap().2;
        assert_eq!(CORNERS.map(|i| pixels[i]),CORNER_COLORS,"V4 is square: preserve all four distant corners");
        let count = g.m.bus.board.display_frames(); assert!(count > 0);
        g.route(RGB[8],132); g.scan(); assert_eq!(g.m.bus.board.display_frames(),count,"wrong data lane");
        g.rgb_routes(); g.route(41,153); g.scan(); assert_eq!(g.m.bus.board.display_frames(),count,"wrong PCLK");
        g.rgb_routes(); g.route(RGB[0],134); g.route(RGB[1],133); g.scan();
        assert_eq!(g.samples(),[0xf800,0x07e0,0x001f],"both low blue bits initially set");
        g.route(RGB[0],148); g.route(RGB[15],133); g.scan();
        assert_eq!(g.samples(),[0x7801,0x07e0,0x801e],"DMA word is remapped through actual wires");
        g.rgb_routes(); g.scan(); g.ch32(5,255); assert_eq!(g.samples(),[0;3],"BSP off=255");
        g.ch32(5,127); assert_eq!(g.samples(),[0x7800,0x03e0,0x000f],"explicit linear digital policy");
        g.ch32(5,0); assert_eq!(g.samples(),[0xf800,0x07e0,0x001f]);
        g.command(0xff,&[0x77,1,0,0,0x11]); g.command(0x10,&[1]); assert_eq!(g.samples(),[0xf800,0x07e0,0x001f],"vendor bank10 is not sleep");
        g.command(0xff,&[0x77,1,0,0,0]); g.serial_bits(42,false,0x10,8);
        assert_eq!(g.samples(),[0xf800,0x07e0,0x001f],"truncated word dropped at CS release");
        g.command(0x10,&[]); assert_eq!(g.samples(),[0;3]); g.command(0x11,&[]);
        g.ch32(3,0x22); let frames = g.m.bus.board.display_frames(); g.scan();
        assert_eq!(g.m.bus.board.display_frames(),frames); assert_eq!(g.samples(),[0;3]);
        g.ch32(3,0x2a); g.panel_init(); assert_eq!(g.samples(),[0;3],"reset must not resurrect a stale frame");
        g.scan(); assert_eq!(g.samples(),[0xf800,0x07e0,0x001f]);
        g.command(1,&[]); g.panel_init(); assert_eq!(g.samples(),[0;3],"software reset also clears pixels");
        // The alternative hardware engine still drives the same nine wires
        // per word. This checks the MSB-aligned one-bit D/C prefix through
        // actual CPU/SPI2 registers, including simultaneous MISO sampling.
        for (p,s) in [(2,101),(1,103),(42,110)] { g.route(p,s); } g.put(0x60024020,0x3e);
        g.spi9(false,0x28,false); g.scan(); assert_eq!(g.samples(),[0;3]);
        g.spi9(false,0x29,false); g.spi9(false,0x36,false); g.spi9(true,8,true);
        assert_eq!(g.samples(),[0x001f,0x07e0,0xf800],"SPI2 D/C prefix and BGR byte survive actual transport");
        g.route(42,256); g.high(42,true); g.route(43,110); g.spi9(false,0x28,false);
        assert_eq!(g.samples(),[0x001f,0x07e0,0xf800],"wrong hardware chip select cannot turn display off");
    }
}

#[test]
fn actual_lx7_v4_ch32_touch_reset_address_irq_direction_rtc_and_absent_devices() {
    for name in BOARDS {
        let mut g = Guest::new(name); g.i2c_route(0,8,9); assert!(g.i2c(0,0x24,&[],0).is_none());
        g.setup();
        for at in [0x20,0x6b,0x18,0x40] { assert!(g.i2c(0,at,&[],0).is_none(),"no inherited TCA/IMU/codecs"); }
        for reg in [0,1,6,7,255] { assert!(g.i2c(0,0x24,&[reg],0).is_none(),"no fake ADC/unknown helper command"); }
        assert!(g.i2c(0,0x24,&[2],1).is_none(),"direction readback is not the input register");
        assert!(g.i2c(0,0x24,&[3,0x2a,255],0).is_none(),"8-bit V003 is not the V006 word protocol");
        assert_eq!(g.m.bus.board.named_pin("boot"),Some(0)); assert_eq!(g.m.bus.board.named_pin("power"),None);
        assert!(g.m.bus.board.input_levels().iter().all(|(p,_)|*p != 16));
        if name.contains("touch") {
            assert_eq!(g.i2c(0,0x5d,&[0x81,0x40],4).unwrap(),b"911\0");
            assert!(g.i2c(0,0x14,&[],0).is_none());
            assert_eq!(g.i2c(0,0x5d,&[0x81,0x46],4).unwrap(),[0xe0,1,0xe0,1]);
            assert!(g.i2c(0,0x5d,&[0x81,0x46,0],0).is_none(),"read-only resolution");
            assert!(g.i2c(0,0x5d,&[0x80,0x00],1).is_none(),"unknown register does not invent data");
            g.i2c(0,0x5d,&[0x81,0x4e,0],0).unwrap();
            g.m.bus.board.touch(111,333,true); g.m.bus.tick(2_500_000); g.m.bus.refresh_board_outputs();
            assert_eq!(g.i2c(0,0x5d,&[0x81,0x4e],8).unwrap(),[0x81,0,111,0,77,1,20,0]);
            g.ch32(2,0x7b); // release EX2 to input; RTC EX7 also input
            assert_eq!(g.i2c(0,0x24,&[4],1).unwrap()[0] & 0x84,0x80,"IRQ low on EX2, RTC inactive high");
            g.i2c(0,0x5d,&[0x81,0x4e,0],0).unwrap();
            assert_eq!(g.i2c(0,0x24,&[4],1).unwrap()[0] & 4,4,"ACK releases EX2");
            g.ch32(2,0xff); assert_eq!(g.i2c(0,0x24,&[4],1).unwrap()[0] & 4,0,"output-low remains output-low");
            g.ch32(3,0x28); assert!(g.i2c(0,0x5d,&[],0).is_none(),"EX1 holds touch reset");
            g.ch32(3,0x2c); g.ch32(3,0x2e); // high INT strap followed by reset release
            assert!(g.i2c(0,0x5d,&[],0).is_none()); assert_eq!(g.i2c(0,0x14,&[0x81,0x40],4).unwrap(),b"911\0");
            g.ch32(2,0x7b); g.i2c(0,0x14,&[0x81,0x4e,0],0).unwrap();
            g.m.bus.board.touch(999,999,true); g.m.bus.tick(2_500_000); g.m.bus.refresh_board_outputs();
            assert_eq!(g.i2c(0,0x14,&[0x81,0x50],4).unwrap(),[0xdf,1,0xdf,1]);
        } else {
            for at in [0x5d,0x14] { assert!(g.i2c(0,at,&[],0).is_none()); }
            g.m.bus.board.touch(111,333,true); assert!(g.m.bus.board.report().contains("injections=0"));
        }
        g.i2c(0,0x51,&[4,0],0).unwrap(); let start = g.i2c(0,0x51,&[4],1).unwrap()[0];
        g.m.bus.tick(240_000_000); g.m.bus.refresh_board_outputs();
        g.i2c_route(1,15,7); assert_eq!(g.i2c(1,0x51,&[4],1).unwrap()[0],start + 1,"shared RTC and STOP forwarding");
        assert!(g.i2c(1,0x51,&[1,0x80],0).is_none(),"unimplemented alarm IRQ is not acknowledged");
        assert!(g.m.bus.board.take_edges().iter().all(|e|e.pin != 16));
    }
}

#[test]
fn actual_lx7_v4_native_one_bit_tf_requires_ex4_high_and_exact_cmd_data_routes() {
    for name in BOARDS {
        let mut g = Guest::new(name); g.setup();
        g.m.bus.attach_sd_card(Box::new(sdmmc::MemStore(vec![0;1 << 20])),Default::default()).unwrap();
        g.sd_wiring(); g.cmd(8,0x1aa,CMD_RESP);
        assert_ne!(g.get(SD + reg::RINTSTS) & int::RTO,0,"selected BSP 0x2A leaves EX4 low: retain failure");
        g.ch32(3,0x3a); g.card_ready();
        for i in 0u32..128 { g.put(RAM + 512 + 4 * i,i.wrapping_mul(0x1020_4081)); }
        g.sector(24,RAM + 512); g.sector(17,RAM + 1024);
        for i in 0u32..128 { assert_eq!(g.get(RAM + 1024 + 4 * i),i.wrapping_mul(0x1020_4081)); }
        assert_eq!(g.m.bus.periph.sdmmc.stats.bytes_written,512); assert_eq!(g.m.bus.periph.sdmmc.stats.bytes_read,512);
        for (signal,pin) in [(179,42),(213,42)] {
            g.put(GPIO + 0x154 + 4 * signal,0x80 | pin); g.cmd(8,0x1aa,CMD_RESP);
            assert_ne!(g.get(SD + reg::RINTSTS) & int::RTO,0); g.sd_wiring();
        }
        g.put(SD + reg::CTYPE,2); g.cmd(8,0x1aa,CMD_RESP); assert_ne!(g.get(SD + reg::RINTSTS) & int::RTO,0);
        g.put(SD + reg::CTYPE,0); g.ch32(3,0x2a); g.cmd(8,0x1aa,CMD_RESP); assert_ne!(g.get(SD + reg::RINTSTS) & int::RTO,0);
        g.ch32(3,0x3a); g.m.bus.board.set_sd_card_present(false); g.cmd(8,0x1aa,CMD_RESP);
        assert_ne!(g.get(SD + reg::RINTSTS) & int::RTO,0);
    }
}
