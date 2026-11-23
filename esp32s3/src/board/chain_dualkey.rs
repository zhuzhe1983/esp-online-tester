//! M5Stack C147 Chain DualKey: a Chain-series main controller with two
//! hot-swappable mechanical keys and a two-LED WS2812B chain.
//!
//! Pin assignments are from the official PinMap, schematic and UserDemo:
//! - the official product page PinMap (`docs.m5stack.com/en/chain/Chain_DualKey`):
//!   KEY G0 = KEY_1 INPUT, G17 = KEY_2 INPUT; RGB LED G21 = WS2812 INPUT,
//!   G40 = WS2812_PWR PWR_EN; ADC G8 SWITCH_1, G7 SWITCH_2, G10 ADC_BAT,
//!   G2 ADC_VBUS, G9 ADC_CHARGE; USB G19 = USB_DN, G20 = USB_DP;
//!   HY2.0-4P_1 = GND VOUT G48 UART1_TX G47 UART1_RX; HY2.0-4P_2 = GND VOUT
//!   G5 UART2_RX G6 UART2_TX (PinMap order differs from the rear view).
//! - the factory image sources `M5DualKey-UserDemo` at 82d7fa60cf2716e6bea01ce30bd916479644140c:
//!   `bsp/keyboard.h` (`KBD_COL_IO_0 17`, `KBD_COL_IO_1 0`, `KBD_INPUT_IOS {0, 17}`,
//!   `KBD_WS2812_GPIO 21`, `KBD_WS2812_POWER_IO 40`), `esp_duo.c` and `chain_bus.h`
//!   (`UART_TEST_TXD1 48/RXD1 47/TXD2 6/RXD2 5`, 115200 baud).
//! - `Chain_DualKey_SCH_V1.5_20250725.pdf` page 1/3 for the net names.
//!
//! Both keys are `GPIO_MODE_INPUT` with `GPIO_PULLUP_ENABLE` because
//! `KBD_ATTIVE_LEVEL` is 0 (`kbd_gpio.c:21-31`), sampled by a 1 ms gptimer with
//! `debounce_ticks = 3` (`keyboard.h:14`, `esp_duo.c:30-33`); `enable_power_save`
//! arms a low-level GPIO wake source on the same two pins. A press pulls the pad
//! low and a release returns it to the board pull-up. G0 is also the BOOT-side key: the official download-mode recipe holds
//! "the button farther from the lanyard hole" while plugging USB-C in.
//!
//! The LEDs are one 2-LED strip: `bsp_ws2812_init` makes G40 a
//! `GPIO_MODE_OUTPUT_OD` pad with no pulls and creates a led-strip 2.5.5 RMT
//! WS2812 device (GRB, 10 MHz, `invert_out: false`, 64 symbols, no DMA) on G21;
//! `bsp_ws2812_enable(e)` writes `gpio_set_level(40, !e)`, so a driven *low* is the
//! lit state and the strip is dark until the guest enables it (`main.cpp:2183-2194`).
//! Battery/charge ADC (G10/G2/G9), the two slide-switch ADC monitors (G8/G7), the
//! Chain UART daisy-chain protocol, BLE/USB HID reports, light sleep and the factory
//! web configuration page are not modeled: firmware sees no device there, and the
//! unimplemented items stay in `report()` and the catalog `limitations`.
use esp_soc::board::{BoardModel,VirtualCycle};
use esp_soc::devices::Ws2812Chain;
use esp_soc::pins::PinRoutes;

// Official PinMap and `bsp/keyboard.h` net names → ESP32-S3 GPIO.
const KEY1:u8=0;const KEY2:u8=17;
const RGB_DATA:u8=21;const RGB_POWER:u8=40;
const CHAIN1_TX:u8=48;const CHAIN1_RX:u8=47;const CHAIN2_TX:u8=6;const CHAIN2_RX:u8=5;
const SWITCH1:u8=8;const SWITCH2:u8=7;const ADC_BAT:u8=10;const ADC_VBUS:u8=2;const ADC_CHARGE:u8=9;
const USB_DP:u8=20;const USB_DN:u8=19;
/// `LIGHTMAP_NUM` in `bsp/lightmap.h`; `g_led_config` maps key matrix {G0,G17} to
/// LED indices {1,0}, so the chain order is fixed by the wire, not by the keycaps.
const LEDS:usize=2;

pub struct ChainDualKey {
    rgb: Ws2812Chain,
    powered: bool,
    frames: u64,
    ignored: u64,
}

impl ChainDualKey {
    /// G40 starts high-Z, leaving the strip unpowered.
    pub fn new() -> Self {
        Self { rgb: Ws2812Chain::new(LEDS), powered: false, frames: 0, ignored: 0 }
    }

    // The pixel state is volatile across a loss of supply.
    fn set_gate(&mut self, powered: bool) {
        if self.powered && !powered {
            self.rgb.leds.fill([0; 3]);
            self.rgb.updates = self.rgb.updates.wrapping_add(1);
        }
        self.powered = powered;
    }
}

impl Default for ChainDualKey {
    fn default() -> Self { Self::new() }
}

impl BoardModel for ChainDualKey {
    fn name(&self) -> &'static str { "chain-dualkey" }

    fn uses_gpio_routes(&self) -> bool { true }

    fn gpio_waveform_at(&mut self, _cycle: VirtualCycle, routes: PinRoutes<'_>) {
        // Only a routed, driven G40 low closes the LED supply switch. The bus
        // also reports fresh GPIO routes when the guest reboots.
        self.set_gate(routes.software_output(RGB_POWER) == Some(false));
    }

    fn rmt_frame(&mut self, pin: u8, bits: &[bool]) {
        if pin != RGB_DATA || !self.powered {
            self.ignored = self.ignored.wrapping_add(1);
            return;
        }
        let before = self.rgb.updates;
        self.rgb.from_bits(bits);
        if self.rgb.updates != before {
            self.frames = self.frames.wrapping_add(1);
        }
    }

    fn input_levels(&self)->Vec<(u8,bool)> {vec![(KEY1,true),(KEY2,true)]}
    fn named_pin(&self,name:&str)->Option<u8> {
        match name {
            "key1"|"btn1"|"button1"|"g0"=>Some(KEY1),
            "key2"|"btn2"|"button2"|"g17"=>Some(KEY2),
            "rgb"|"ws2812"|"led-data"|"g21"=>Some(RGB_DATA),
            "rgb-power"|"ws2812-power"|"led-power"|"g40"=>Some(RGB_POWER),
            "chain1-tx"|"grove1-tx"|"uart1-tx"|"g48"=>Some(CHAIN1_TX),
            "chain1-rx"|"grove1-rx"|"uart1-rx"|"g47"=>Some(CHAIN1_RX),
            "chain2-tx"|"grove2-tx"|"uart2-tx"|"g6"=>Some(CHAIN2_TX),
            "chain2-rx"|"grove2-rx"|"uart2-rx"|"g5"=>Some(CHAIN2_RX),
            "switch1"|"ble-switch"|"g8"=>Some(SWITCH1),
            "switch2"|"rainmaker-switch"|"g7"=>Some(SWITCH2),
            "bat"|"adc-bat"|"g10"=>Some(ADC_BAT),
            "vbus"|"adc-vbus"|"g2"=>Some(ADC_VBUS),
            "chg"|"adc-charge"|"g9"=>Some(ADC_CHARGE),
            "usb-dp"|"g20"=>Some(USB_DP),
            "usb-dn"|"g19"=>Some(USB_DN),
            _=>None,
        }
    }
    fn leds(&self)->Option<(&[[u8;3]],u64)> {Some((&self.rgb.leds,self.rgb.updates))}
    fn report(&self)->String {
        format!("[chain-dualkey] keys G0/G17 active-low pull-up; WS2812B x2 data G21 power G40 driven_low={} updates={} accepted_frames={} ignored_frames={} rgb={:?}; battery/charge ADC, slide switches, Chain UART daisy-chain, BLE/USB HID, light sleep and the factory web configurator = unmodeled",
            self.powered,self.rgb.updates,self.frames,self.ignored,self.rgb.leds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::SocBus;
    use emu_core::Bus as _;
    use esp_soc::SocBus as _;

    // ESP-IDF v5.5.1 ESP32-S3 register/soc/{gpio_reg,io_mux_reg,gpio_sig_map}.h.
    // GPIO_IN +0x3c, FUNCn_OUT_SEL +0x554+4n, OEN_SEL bit 10, GPIO mux function 1.
    const GPIO:u32=0x6000_4000;const MUX:u32=0x6000_9000;
    /// GRB on the wire (MSB first, G then R then B) as an RMT bit stream.
    fn grb(bytes:&[u8])->Vec<bool> {bytes.iter().flat_map(|&b|(0..8).map(move|i|b&(0x80>>i)!=0)).collect()}

    #[test]
    fn pin_matrix_is_the_official_pinmap_not_a_default_mapping() {
        let mut b=ChainDualKey::new();
        assert_eq!(b.name(),"chain-dualkey");
        for (id,pin) in [("key1",0),("btn1",0),("g0",0),("key2",17),("btn2",17),("g17",17),("rgb",21),("ws2812",21),("rgb-power",40),("chain1-tx",48),("chain1-rx",47),("chain2-tx",6),("chain2-rx",5),("switch1",8),("switch2",7),("bat",10),("vbus",2),("chg",9),("usb-dp",20),("usb-dn",19)] {
            assert_eq!(b.named_pin(id),Some(pin),"{id} must resolve to the documented GPIO");
        }
        for id in ["key3","display","touch","encoder","buzzer","micro-sd"] {assert_eq!(b.named_pin(id),None,"{id} is not on C147");}
        assert_eq!(b.input_levels(),[(0,true),(17,true)],"both keys boot released-high on their internal pull-up");
        assert!(b.display().is_none());
        assert!(b.i2c_devices().is_empty(),"C147 has no I2C device: nothing may be attached");
        assert!(b.hardware_diagnostics().is_none(),"no diagnostics contract is claimed for this board");
    }

    #[test]
    fn g40_driven_low_powers_the_chain_and_other_pins_are_ignored() {
        let mut b=ChainDualKey::new();
        let frame=grb(&[0x10,0x20,0x30,0x04,0x05,0x06]);
        b.rmt_frame(RGB_DATA,&frame);
        assert_eq!(b.leds().unwrap(),(&[[0;3];LEDS][..],0),"the strip is dark while G40 is not driven low");
        assert_eq!(b.ignored,1);
        b.set_gate(true);
        assert!(b.powered);
        b.rmt_frame(RGB_DATA,&frame);
        assert_eq!(b.leds().unwrap().0,[[0x20,0x10,0x30],[0x05,0x04,0x06]],"GRB on the wire becomes RGB");
        assert_eq!(b.leds().unwrap().1,1);assert_eq!(b.frames,1);
        for pin in [20,22,40,48,5] {b.rmt_frame(pin,&frame);}
        assert_eq!(b.leds().unwrap().1,1,"only G21 drives the chain");
        assert_eq!(b.ignored,6);
        b.rmt_frame(RGB_DATA,&frame[..23]);
        assert_eq!((b.leds().unwrap().1,b.frames),(1,1),"a truncated pixel is not an update");
        b.rmt_frame(RGB_DATA,&[true;LEDS*24+24]);
        assert_eq!(b.frames,2,"the third wire LED falls off the 2-LED chain");
    }

    #[test]
    fn power_off_blanks_the_chain_and_power_on_needs_a_fresh_frame() {
        let mut b=ChainDualKey::new();
        let bits=grb(&[255,0,0,0,255,0]);
        b.set_gate(true);
        b.rmt_frame(RGB_DATA,&bits);
        assert_eq!(b.leds().unwrap().0,[[0,255,0],[255,0,0]]);
        b.set_gate(false);
        assert!(!b.powered);
        assert_eq!(b.leds().unwrap(),(&[[0;3];LEDS][..],2),"switching the supply off clears the colours");
        b.rmt_frame(RGB_DATA,&bits);
        assert_eq!(b.frames,1,"frames received while unpowered are dropped");
        b.set_gate(true);
        b.rmt_frame(RGB_DATA,&bits);
        assert_eq!((b.leds().unwrap().0[0],b.frames),([0,255,0],2),"the strip needs a new frame after re-powering");
    }

    #[test]
    fn real_registers_route_rmt_to_g21_and_keys_read_the_pull_ups() {
        let mut bus=SocBus::new(8*1024*1024,0,[0,1,2,3,4,5]);
        bus.board=Box::<ChainDualKey>::default();
        bus.attach_board_devices();
        // G0/G17 arrive as pulled-up inputs; a host press pulls the pad low and a
        // release gives the pad back to the board pull-up.
        assert_eq!(bus.read32(GPIO+0x3c).unwrap()&0x2_0001,0x2_0001,"G0 and G17 read high before any press");
        bus.gpio_set_input(KEY1,false);
        assert_eq!(bus.read32(GPIO+0x3c).unwrap()&1,0,"pressing KEY_1 pulls G0 low");
        bus.gpio_release_input(KEY1);
        assert_eq!(bus.read32(GPIO+0x3c).unwrap()&1,1,"releasing KEY_1 returns to the pull-up");
        // RMT TX channel 0 → GPIO21 through the GPIO matrix, exactly as led-strip configures it.
        bus.write32(MUX+4+21*4,1<<12).unwrap();
        bus.write32(GPIO+0x554+21*4,crate::bus::RMT_SIG_OUT0|1024).unwrap();
        bus.write32(GPIO+0x24,1<<21).unwrap();
        // A stale GPIO latch must not drive the LED gate while G40 is muxed away.
        bus.write32(GPIO+0x30,1<<8).unwrap();
        bus.periph.rmt.done.push((0,grb(&[1,2,3,4,5,6])));
        bus.tick(32_768);
        assert_eq!(bus.board.leds().unwrap(),(&[[0;3];LEDS][..],0));
        // G40 as an open-drain output driven low: enable set, out clear.
        bus.write32(MUX+4+40*4,1<<12).unwrap();
        bus.write32(GPIO+0x30,1<<8).unwrap();
        bus.tick(1);
        bus.write32(GPIO+0x18,1<<8).unwrap();
        bus.tick(1);
        bus.periph.rmt.done.push((0,grb(&[0,255,0,255,0,0])));
        bus.tick(32_768);
        {
            let (leds,updates)=bus.board.leds().unwrap();
            assert_eq!((leds[0],leds[1],updates),([255,0,0],[0,255,0],1),"RMT delivery on G21 lights both wire LEDs");
            assert!(bus.board.report().contains("driven_low=true"),"the GPIO write really reached the board's supply gate");
        }
        // Moving G40 high removes the supply, so the next frame must land on a dark chain.
        bus.write32(GPIO+0x14,1<<8).unwrap();
        bus.tick(1);
        assert_eq!(bus.board.leds().unwrap(),(&[[0;3];LEDS][..],2));
        bus.periph.rmt.done.push((0,grb(&[1,2,3,4,5,6])));
        bus.tick(32_768);
        assert_eq!(bus.board.leds().unwrap().1,2,"no light without a driven-low G40");
        // A wrong pad: G18 carries the frame instead of G21. Pressing G1 is also unrelated.
        bus.write32(GPIO+0x18,1<<8).unwrap();
        bus.write32(GPIO+0x554+21*4,0).unwrap();
        bus.write32(GPIO+0x28,1<<21).unwrap();
        bus.tick(1);
        bus.write32(MUX+4+18*4,1<<12).unwrap();
        bus.write32(GPIO+0x554+18*4,crate::bus::RMT_SIG_OUT0|1024).unwrap();
        bus.write32(GPIO+0x24,1<<18).unwrap();
        bus.tick(1);
        bus.periph.rmt.done.push((0,grb(&[9,9,9,9,9,9])));
        bus.tick(32_768);
        assert_eq!(bus.board.leds().unwrap(),(&[[0;3];LEDS][..],2),"G18 is not the C147 data pin");
        bus.gpio_set_input(1,false);bus.gpio_set_input(16,false);bus.tick(1);
        assert_eq!(bus.board.leds().unwrap().0,[[0;3];LEDS],"a press on an adjacent pad does nothing on this board");
        assert_eq!(bus.read32(GPIO+0x3c).unwrap()&(1<<1),0,"the pad itself still reads the hostile drive");
        bus.gpio_release_input(1);bus.gpio_release_input(16);
        assert_eq!(bus.read32(GPIO+0x3c).unwrap()&(1<<1),1<<1,"an unowned pad falls back to the default high");
    }

    #[test]
    fn a_guest_reboot_releases_g40_and_reapplies_the_key_pull_ups() {
        let mut bus=SocBus::new(8*1024*1024,0,[0,1,2,3,4,5]);
        bus.board=Box::<ChainDualKey>::default();
        bus.attach_board_devices();
        bus.write32(MUX+4+21*4,1<<12).unwrap();
        bus.write32(GPIO+0x554+21*4,crate::bus::RMT_SIG_OUT0|1024).unwrap();
        bus.write32(GPIO+0x24,1<<21).unwrap();
        bus.write32(MUX+4+40*4,1<<12).unwrap();
        bus.write32(GPIO+0x30,1<<8).unwrap();
        bus.write32(GPIO+0x18,1<<8).unwrap();
        bus.tick(1);
        bus.periph.rmt.done.push((0,grb(&[0,255,0,255,0,0])));
        bus.tick(32_768);
        assert_eq!(bus.board.leds().unwrap().0,[[255,0,0],[0,255,0]]);
        bus.gpio_set_input(KEY2,false);
        let cause=esp_soc::SocBus::reboot(&mut bus,[0,1,2,3,4,5]);
        assert_eq!(cause,esp_periph::RST_POWERON,"a cold reset reports the power-on cause");
        assert_eq!(bus.board.name(),"chain-dualkey","a reboot keeps the board identity");
        assert_eq!(bus.board.leds().unwrap(),(&[[0;3];LEDS][..],2),"fresh GPIO clears the strip supply");
        assert!(bus.board.report().contains("accepted_frames=1"),"the frame survives the reset as history");
        assert_eq!(bus.read32(GPIO+0x3c).unwrap()&0x2_0001,0x2_0001,"G0/G17 are pulled up again after the reset");
        assert!(bus.board.report().contains("unmodeled"));
    }
}
