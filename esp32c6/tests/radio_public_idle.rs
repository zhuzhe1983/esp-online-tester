use esp32c6::radio::{Ieee802154, RadioState, EVENT_ED_DONE};
use esp_periph::Device;

#[test]
fn public_energy_countdown_runs_after_idle() {
    let mut radio = Ieee802154::new();
    radio.tick(1);
    radio.state = RadioState::Ed;
    radio.ed_left = Some(3);
    radio.tick(2);
    assert_eq!(radio.ed_left, Some(1));
    assert_eq!(radio.scans, 0);
    radio.tick(1);
    assert_eq!(radio.ed_left, None);
    assert_eq!(radio.scans, 1);
    assert_eq!(radio.events & EVENT_ED_DONE, EVENT_ED_DONE);
    assert_eq!(radio.state, RadioState::Idle);
}
