//! Host messages are untrusted even when a browser normally sends small inputs.
use esp_soc::{board::BoardModel, picture::Picture, web::WebServer, ScriptAction, Stop};
use std::sync::{Arc, Mutex};

struct InputBoard(Arc<Mutex<Vec<Picture>>>);

impl BoardModel for InputBoard {
    fn name(&self) -> &'static str { "input-test" }
    fn encoder(&self) -> Option<(u8, u8)> { Some((1, 2)) }
    fn set_camera_picture(&mut self, picture: Picture) -> Result<(), &'static str> { self.0.lock().unwrap().push(picture); Ok(()) }
}

fn fixture() -> (esp32s3::Machine, WebServer, Arc<Mutex<Vec<Picture>>>) {
    let mut machine = esp32s3::machine([1, 2, 3, 4, 5, 6]);
    let pictures = Arc::new(Mutex::new(Vec::new()));
    machine.bus.board = Box::new(InputBoard(pictures.clone()));
    let web = WebServer::queued();
    machine.web = Some(web.clone());
    (machine, web, pictures)
}

fn camera_message(width: u16, height: u16, rgba: &[u8]) -> Vec<u8> {
    let mut message = vec![3];
    message.extend_from_slice(&width.to_le_bytes());
    message.extend_from_slice(&height.to_le_bytes());
    message.extend_from_slice(rgba);
    message
}

#[test]
fn camera_input_rejects_invalid_sizes_and_preserves_valid_rgb() {
    let (mut machine, web, pictures) = fixture();
    for message in [
        vec![],
        vec![3],
        camera_message(0, 1, &[1, 2, 3, 4]),
        camera_message(1, 0, &[1, 2, 3, 4]),
        camera_message(2, 1, &[1, 2, 3, 4]),
        camera_message(32768, 32768, &[]),
        camera_message(u16::MAX, u16::MAX, &[]),
    ] {
        web.push_incoming_bin(message);
    }
    web.push_incoming_bin(camera_message(2, 1, &[10, 20, 30, 0, 40, 50, 60, 255]));
    assert!(matches!(machine.run(0), Stop::MaxInsns));
    let pictures = pictures.lock().unwrap();
    assert_eq!(pictures.len(), 1);
    assert_eq!((pictures[0].w, pictures[0].h), (2, 1));
    assert_eq!(pictures[0].rgb, [10, 20, 30, 40, 50, 60]);
    assert_eq!(machine.bus.cycles, 0);
    assert!(web.poll_incoming_bin().is_empty());
}

#[test]
fn knob_extreme_deltas_have_bounded_work_and_keep_their_direction() {
    for delta in [i32::MIN, i32::MAX] {
        let (mut machine, web, _) = fixture();
        web.push_incoming(format!(r#"{{"t":"knob","d":{delta}}}"#));
        assert!(matches!(machine.run(0), Stop::MaxInsns));
        assert_eq!(machine.script.events.len(), 64 * 4);
        let first_pin = if delta > 0 { 1 } else { 2 };
        assert!(matches!(machine.script.events[0].1, ScriptAction::Gpio(pin, false) if pin == first_pin));
        assert_eq!(machine.bus.cycles, 0);
        assert!(web.poll_incoming().is_empty());
    }
}

#[test]
fn ordinary_knob_deltas_preserve_edges_spacing_and_queue_order() {
    let (mut machine, web, _) = fixture();
    for delta in [2, 0, -1] {
        web.push_incoming(format!(r#"{{"t":"knob","d":{delta}}}"#));
    }
    assert!(matches!(machine.run(0), Stop::MaxInsns));
    let step = 240_000_000 / 500;
    let expected = [
        (1, 1, false), (2, 2, false), (3, 1, true), (4, 2, true),
        (9, 1, false), (10, 2, false), (11, 1, true), (12, 2, true),
        (17, 2, false), (18, 1, false), (19, 2, true), (20, 1, true),
    ];
    assert_eq!(machine.script.events.len(), expected.len());
    for ((cycle, action), (phase, expected_pin, expected_level)) in machine.script.events.iter().zip(expected) {
        assert_eq!(*cycle, phase * step);
        assert!(matches!(action, ScriptAction::Gpio(pin, level) if *pin == expected_pin && *level == expected_level));
    }
}

#[test]
fn watcher_polled_knob_uses_slow_phases_in_both_host_paths() {
    let (mut machine, web, _) = fixture();
    machine.bus.board = esp32s3::board::make_board("seeed-sensecap-watcher").unwrap();
    for delta in [1, -1] { web.push_incoming(format!(r#"{{"t":"knob","d":{delta}}}"#)); }
    assert!(matches!(machine.run(0), Stop::MaxInsns));
    let step = 240_000_000 / 50; // 20ms: comfortably beyond two 3ms polls.
    let expected = [(1,41,false),(2,42,false),(3,41,true),(4,42,true),
                    (9,42,false),(10,41,false),(11,42,true),(12,41,true)];
    assert_eq!(machine.script.events.len(), expected.len());
    for ((cycle, action), (phase, pin, level)) in machine.script.events.iter().zip(expected) {
        assert_eq!(*cycle, phase * step);
        assert!(matches!(action, ScriptAction::Gpio(p,l) if *p==pin&&*l==level));
    }
    // Native scripts emit the same GPIO transitions from their explicit start
    // time. They must not silently retain the short browser-incompatible pulse.
    machine.load_script("1 knob cw 1\n2 knob ccw 1\n").unwrap();
    for (i, (cycle, action)) in machine.script.events.iter().enumerate() {
        let start = if i<4 {240_000_000} else {480_000_000};
        assert_eq!(*cycle, start + (i as u64 % 4) * step);
        let (_,pin,level)=expected[i];
        assert!(matches!(action, ScriptAction::Gpio(p,l) if *p==pin&&*l==level));
    }
}

#[test]
fn camera_plain_input_channel_needs_no_web_server() {
    let (mut machine, _, pictures) = fixture();
    machine.web = None;
    machine.camera_input = Some(Arc::new(Mutex::new(Some(Picture { w: 1, h: 1, rgb: vec![1, 2, 3] }))));
    machine.run(0);
    assert_eq!(pictures.lock().unwrap()[0].rgb, [1, 2, 3]);
    assert!(machine.camera_input.as_ref().unwrap().lock().unwrap().is_none());
    assert!(machine.web.is_none());
}
