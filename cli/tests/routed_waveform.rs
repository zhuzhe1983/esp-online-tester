use esp_soc::{board::BoardModel, pins::PinRoutes, SocBus};
use std::sync::{Arc, Mutex};

type Samples = Arc<Mutex<Vec<(u64, Option<bool>)>>>;
struct Capture { samples: Samples, enabled: bool }
impl BoardModel for Capture {
    fn name(&self) -> &'static str { "routed-waveform" }
    fn uses_gpio_edges(&self) -> bool { false }
    fn uses_gpio_waveform(&self) -> bool { self.enabled }
    fn gpio_waveform_at(&mut self, cycle: u64, routes: PinRoutes<'_>) {
        self.samples.lock().unwrap().push((cycle, routes.software_output(4)));
    }
}
fn check<B: SocBus>(mut bus: B, gpio: u32, mux: u32, selector: u32, signal: u32, classic: bool, install: impl Fn(&mut B, Box<dyn BoardModel>)) {
    let samples = Arc::new(Mutex::new(Vec::new()));
    install(&mut bus, Box::new(Capture { samples: samples.clone(), enabled: true }));
    let write = |bus: &mut B, addr, value, expected| {
        samples.lock().unwrap().clear();
        bus.write32(addr, value).unwrap();
        let s = samples.lock().unwrap();
        assert_eq!(s.len(), 1, "one routed callback per register write at {addr:x}");
        assert_eq!(s[0], (bus.cycles(), expected));
    };
    write(&mut bus, mux, (if classic { 2 } else { 1 }) << 12, None);
    write(&mut bus, gpio + 4, 1 << 4, None);
    write(&mut bus, gpio + 0x20, 1 << 4, Some(true));
    write(&mut bus, selector, signal | (signal * 2), Some(false)); // inversion only
    write(&mut bus, selector, signal | (signal * 8), None); // enable inversion only
    write(&mut bus, gpio + 0x20, 0, Some(true)); // inverted enable
    write(&mut bus, selector, 12, None); // peripheral output, not software drive
    write(&mut bus, selector, signal, None);
    write(&mut bus, gpio + 0x20, 1 << 4, Some(true));
    write(&mut bus, mux, if classic { 1 << 12 } else { 0 }, None); // mux-only disconnect; classic function 0 is GPIO here
    write(&mut bus, mux, (if classic { 2 } else { 1 }) << 12, Some(true));
    // Non-pin writes do not notify. Existing raw GPIO notifications remain independent.
    samples.lock().unwrap().clear();
    bus.write32(if classic { 0x3ff4_0000 } else { 0x6000_0000 }, 0).unwrap();
    assert!(samples.lock().unwrap().is_empty());
    bus.reboot([0; 6]);
    assert_eq!(samples.lock().unwrap().last().unwrap().1, None, "reset releases routed drive");
    write(&mut bus, mux, (if classic { 2 } else { 1 }) << 12, None);
    install(&mut bus, Box::new(Capture { samples: samples.clone(), enabled: false }));
    samples.lock().unwrap().clear();
    bus.write32(gpio + 0x20, 1 << 4).unwrap();
    bus.write32(selector, signal).unwrap();
    bus.write32(mux, 0).unwrap();
    assert!(samples.lock().unwrap().is_empty(), "opt-out must not notify");
}
#[test]
fn s3_routed_waveform() { check(esp32s3::machine([0; 6]).bus, 0x6000_4000, 0x6000_9014, 0x6000_4564, 256, false, |b, board| { b.board = board; b.attach_board_devices(); }); }
#[test]
fn c3_routed_waveform() { check(esp32c3::machine([0; 6], 4 << 20).bus, 0x6000_4000, 0x6000_9014, 0x6000_4564, 128, false, |b, board| { b.board = board; b.attach_board_devices(); }); }
#[test]
fn c6_routed_waveform() { check(esp32c6::machine([0; 6], 4 << 20).bus, 0x6009_1000, 0x6009_0014, 0x6009_1564, 128, false, |b, board| { b.board = board; b.attach_board_devices(); }); }
#[test]
fn classic_routed_waveform_and_alias() {
    check(esp32::machine([0; 6], 4 << 20).bus, 0x3ff4_4000, 0x3ff4_9048, 0x3ff4_4540, 256, true, |b, board| { b.board = board; b.attach_board_devices(); });
    check(esp32::machine([0; 6], 4 << 20).bus, 0x6000_4000, 0x6000_9048, 0x6000_4540, 256, true, |b, board| { b.board = board; b.attach_board_devices(); });
}
