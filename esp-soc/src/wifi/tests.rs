use super::*;

fn config() -> ApConfig {
    ApConfig { ssid: "esp32sim".into(), bssid: [2, 0x53, 0x49, 0x4d, 0, 1], channel: 6, psk: None, epoch_s: None }
}

fn station_frame(fc: u16, payload: &[u8]) -> Vec<u8> {
    let mut frame = vec![0; 24];
    frame[..2].copy_from_slice(&fc.to_le_bytes());
    frame[4..10].copy_from_slice(&config().bssid);
    frame[10..16].copy_from_slice(&[2, 3, 4, 5, 6, 7]);
    frame.extend_from_slice(payload);
    frame
}

#[test]
fn auth_rejects_every_truncated_fixed_body() {
    let valid = station_frame(11 << 4, &[0, 0, 1, 0, 0, 0]);
    for len in 0..valid.len() {
        let mut ap = VirtualAp::new(config(), true);
        assert!(ap.on_station_tx(&valid[..len], 0).is_none());
        assert_eq!(ap.state, StaState::Idle, "length {len}");
        assert!(ap.queue.is_empty(), "length {len}");
    }
    let mut ap = VirtualAp::new(config(), false);
    assert!(ap.on_station_tx(&valid, 0).is_none());
    assert_eq!(ap.state, StaState::Authenticated);
    assert_eq!(ap.queue.len(), 1);
}

fn eapol_message4() -> Vec<u8> {
    let mut body = vec![0; 99];
    body[0] = 2;
    body[1] = 3;
    body[2..4].copy_from_slice(&95u16.to_be_bytes());
    body[4] = 2;
    body[5..7].copy_from_slice(&0x0300u16.to_be_bytes());
    body
}

fn send_eapol(ap: &mut VirtualAp, body: &[u8]) {
    ap.state = StaState::Associated;
    let mut payload = vec![0xaa, 0xaa, 3, 0, 0, 0, 0x88, 0x8e];
    payload.extend_from_slice(body);
    assert!(ap.on_station_tx(&station_frame(0x0108, &payload), 0).is_none());
}

#[test]
fn eapol_rejects_declared_lengths_shorter_than_key_header() {
    for len in 0..95u16 {
        let mut body = eapol_message4();
        body[2..4].copy_from_slice(&len.to_be_bytes());
        // Exercise the message-2 slices as well as the message-4 state transition.
        for (state, key_info) in [(WpaState::AwaitingMessage2, 0x0100u16), (WpaState::AwaitingMessage4, 0x0300)] {
            body[5..7].copy_from_slice(&key_info.to_be_bytes());
            let mut ap = VirtualAp::new(config(), false);
            ap.wpa.state = state;
            send_eapol(&mut ap, &body);
            assert_eq!(ap.wpa.state, state, "declared length {len}");
            assert!(ap.queue.is_empty());
        }
    }
}

#[test]
fn eapol_rejects_truncated_declared_payload_and_key_data() {
    for (declared, key_data) in [(96u16, 0u16), (95, 1), (u16::MAX, 0)] {
        let mut body = eapol_message4();
        body[2..4].copy_from_slice(&declared.to_be_bytes());
        body[97..99].copy_from_slice(&key_data.to_be_bytes());
        let mut ap = VirtualAp::new(config(), false);
        ap.wpa.state = WpaState::AwaitingMessage4;
        send_eapol(&mut ap, &body);
        assert_eq!(ap.wpa.state, WpaState::AwaitingMessage4);
        assert!(ap.queue.is_empty());
    }
}

#[test]
fn eapol_allows_bytes_after_declared_payload() {
    let mut body = eapol_message4();
    body.extend_from_slice(&[0xff; 8]);
    let mut ap = VirtualAp::new(config(), false);
    ap.wpa.state = WpaState::AwaitingMessage4;
    send_eapol(&mut ap, &body);
    assert_eq!(ap.wpa.state, WpaState::Installed);
}

#[test]
fn wpa_handshake_installs_keys_only_after_message4() {
    let mut cfg = config();
    cfg.psk = Some("esp32sim-pass".into());
    let mut ap = VirtualAp::new(cfg, false);
    assert_eq!(ap.wpa.state, WpaState::Idle);
    ap.on_station_tx(&station_frame(11 << 4, &[0, 0, 1, 0, 0, 0]), 0);
    ap.on_station_tx(&station_frame(0, &[1, 0, 0, 0]), 0);
    assert_eq!(ap.wpa.state, WpaState::AwaitingMessage2);
    let m1 = &ap.queue.last().unwrap().frame[32..];
    assert_eq!(&m1[5..7], &0x008au16.to_be_bytes());
    assert_eq!(&m1[81..97], &[0; 16]);
    ap.queue.clear();

    let mut m2 = eapol_message4();
    m2[5..7].copy_from_slice(&0x010au16.to_be_bytes());
    m2[17..49].fill(0x5a);
    send_eapol(&mut ap, &m2);
    assert_eq!(ap.wpa.state, WpaState::AwaitingMessage4);
    let m3 = &ap.queue.last().unwrap().frame[32..];
    assert_eq!(&m3[5..7], &0x13cau16.to_be_bytes());
    assert_ne!(&m3[81..97], &[0; 16]);
    let ethernet = [0u8; 14];
    assert_eq!(ap.data_from_ds(&ethernet).unwrap()[1] & 0x40, 0);

    send_eapol(&mut ap, &eapol_message4());
    assert_eq!(ap.wpa.state, WpaState::Installed);
    let protected = ap.data_from_ds(&ethernet).unwrap();
    assert_eq!(protected[1] & 0x40, 0x40);
    assert_eq!(protected.len(), 24 + 8 + 8 + 8); // MAC, CCMP, LLC/SNAP and MIC
}

#[test]
fn data_to_eth_rejects_every_truncated_header() {
    let mut payload = vec![0xaa, 0xaa, 3, 0, 0, 0, 0x08, 0];
    payload.extend_from_slice(&[1, 2, 3]);
    for fc in [0x0108, 0x0188] {
        let mut frame = station_frame(fc, &[]);
        if fc == 0x0188 { frame.extend_from_slice(&[0; 2]); }
        let header_len = frame.len() + 8;
        frame.extend_from_slice(&payload);
        for len in 0..header_len {
            assert!(data_to_eth(&frame[..len]).is_none(), "length {len}");
        }
        assert_eq!(data_to_eth(&frame).unwrap()[12..], [0x08, 0, 1, 2, 3]);
    }
}

#[test]
fn ap_configuration_keeps_defaults_and_recognizes_aliases() {
    let cfg = ApConfig::parse("").unwrap();
    assert_eq!(cfg.ssid, "esp32sim");
    assert_eq!(cfg.bssid, [2, 0x53, 0x49, 0x4d, 0, 1]);
    assert_eq!(cfg.channel, 6);
    assert!(cfg.psk.is_none());
    for channel in ["chan", "channel", "ch"] {
        for psk in ["psk", "password", "pass"] {
            let spec = format!("ssid=test,{channel}=11,{psk}=s=ecret,bssid=02:ab:CD:00:12:ff");
            let cfg = ApConfig::parse(&spec).unwrap();
            assert_eq!(cfg.ssid, "test");
            assert_eq!(cfg.channel, 11);
            assert_eq!(cfg.psk.as_deref(), Some("s=ecret"));
            assert_eq!(cfg.bssid, [2, 0xab, 0xcd, 0, 0x12, 0xff]);
        }
    }
}

#[test]
fn ap_configuration_rejects_unknown_options_and_invalid_values() {
    for spec in [
        "passwd=secret", "pass", "ssid=x,", "=x", "channel=0", "channel=15",
        "channel=256", "channel=-1", "channel=no", "bssid=02:53:49:4d:00",
        "bssid=02:53:49:4d:00:01:02", "bssid=02:xx:53:49:4d:00:01",
        "bssid=02:53:49:4d:00:GG", "bssid=2:53:49:4d:00:01", "bssid=+2:53:49:4d:00:01",
    ] {
        assert!(ApConfig::parse(spec).is_err(), "accepted {spec}");
    }
}

/// What `next_rx` may change: the access point's timers, queue and counts, and the link's queues
/// and pacing. `nothing_due` must only say so when none of them would change.
fn link_state(l: &StationLink) -> (u64, usize, (u64, u64, u64), u16, usize, u64) {
    let ap = l.ap().unwrap();
    (ap.next_beacon_us, ap.queue.len(), ap.stats, ap.seq, l.eth_rx.len(), l.last_rx_us)
}

#[test]
fn nothing_due_means_next_rx_delivers_and_changes_nothing() {
    for (case, relay) in [("network", false), ("relay", true)] {
        for t in (0..400_000).step_by(997) {
            // fresh, a frame queued for later, a frame from the network, and a frame just delivered
            for setup in 0..4 {
                let mut l = StationLink::default();
                l.attach(Some(VirtualAp::new(config(), false)), None);
                l.set_relay(relay);
                match setup {
                    1 => l.ap_mut().unwrap().queue.push(AirFrame { at_us: t + 1_000, frame: vec![0x40, 0, 0, 0] }),
                    2 => l.eth_rx.push(vec![0; 60]),
                    3 => { l.last_rx_us = t.saturating_sub(100); l.ap_mut().unwrap().queue.push(AirFrame { at_us: 0, frame: vec![0x40, 0, 0, 0] }); }
                    _ => {}
                }
                if !l.nothing_due(t) { continue; }
                let before = link_state(&l);
                assert_eq!(l.next_rx(t, false), None, "{case}, setup {setup}, t {t}: nothing_due but a frame came");
                assert_eq!(link_state(&l), before, "{case}, setup {setup}, t {t}: nothing_due but next_rx changed state");
            }
        }
    }
    // and it does not hide what is due: the first beacon, a queued frame, a frame from the network
    let mut l = StationLink::default();
    l.attach(Some(VirtualAp::new(config(), false)), None);
    let first_beacon = l.ap().unwrap().next_beacon_us;
    assert!(l.nothing_due(first_beacon - 1) && !l.nothing_due(first_beacon));
    l.ap_mut().unwrap().queue.push(AirFrame { at_us: 0, frame: vec![0x40, 0, 0, 0] });
    assert!(!l.nothing_due(1_000));
    let mut l = StationLink::default();
    l.attach(Some(VirtualAp::new(config(), false)), None);
    l.eth_rx.push(vec![0; 60]);
    assert!(!l.nothing_due(1_000));
}

/// A link with an access point and frames queued at it: `frames` are (at_us, frame type byte).
fn link_with_queue(frames: &[(u64, u8)]) -> StationLink {
    let mut l = StationLink::default();
    let mut ap = VirtualAp::new(config(), false);
    ap.next_beacon_us = u64::MAX;   // only the queued frames
    for &(at_us, fc) in frames { ap.queue.push(AirFrame { at_us, frame: vec![fc, 0, 0, 0] }); }
    l.attach(Some(ap), None);
    l
}

#[test]
fn next_rx_delivers_every_due_frame_one_gap_apart_management_first() {
    // two beacons (0x80) and a probe response (0x50), all due at once
    let mut l = link_with_queue(&[(10, 0x80), (20, 0x80), (30, 0x50)]);
    let mut got = Vec::new();
    let mut t = 1_000;
    while got.len() < 3 && t < 10_000 {
        if let Some(f) = l.next_rx(t, false) { got.push((t, f[0])); l.rx_delivered(0x1000); }
        t += 100;
    }
    assert_eq!(got.iter().map(|g| g.1).collect::<Vec<_>>(), [0x50, 0x80, 0x80], "management first, then beacons in time order, none lost");
    assert!(got.windows(2).all(|w| w[1].0 - w[0].0 >= RX_GAP_US), "one airtime gap apart: {got:?}");
    assert_eq!(l.rx_frames(), 3);
}

#[test]
fn a_busy_ring_holds_the_air_for_50_ms_only() {
    let mut l = link_with_queue(&[(0, 0x80), (0, 0x80)]);
    assert!(l.next_rx(1_000, false).is_some());
    l.rx_delivered(0x1000);
    assert_eq!(l.next_rx(1_000 + RX_GAP_US, true), None, "software has not recycled the last descriptor");
    assert_eq!(l.next_rx(1_000 + 50_000 - 1, true), None, "still within the stall time");
    assert!(l.next_rx(1_000 + 50_000, true).is_some(), "after 50 ms the frame goes anyway");
}

#[test]
fn a_reboot_keeps_the_host_side_only() {
    let mut l = link_with_queue(&[(0, 0x80)]);
    let ap = l.aps.pop();
    l.attach(ap, Some(crate::net::VirtualNet::new(false)));
    l.set_relay(true);
    l.eth_tx.push(vec![1]); l.eth_rx.push(vec![2]);
    assert!(l.next_rx(1_000, false).is_some());
    l.rx_delivered(0x1000); l.drop_rx(); l.station_tx(&[0; 24], 2_000);
    let l = l.surviving_reboot();
    assert!(l.ap().is_some() && l.net().is_some() && l.relay(), "the access point, network and relay mode stay");
    assert_eq!((l.eth_tx.len(), l.eth_rx.len(), l.last_rx_us, l.last_rx_desc, l.net_polled_us), (0, 0, 0, 0, 0), "queues and pacing start again");
    assert_eq!((l.tx_frames(), l.rx_frames(), l.rx_dropped(), l.tx_dropped()), (0, 0, 0, 0), "counts start again");
}

// ------------------------------------------------------------------ networks around the station

fn nearby() -> Vec<BeaconAp> {
    vec![
        BeaconAp { ssid: "Home".into(), bssid: [0x02, 1, 2, 3, 4, 5], channel: 1, rssi: -60, secure: true },
        BeaconAp { ssid: "Cafe".into(), bssid: [0x02, 9, 8, 7, 6, 5], channel: 11, rssi: -75, secure: false },
    ]
}

/// A probe request from the station to the broadcast address and BSSID, with the given elements.
fn probe_request(elements: &[u8]) -> Vec<u8> {
    let mut f = station_frame(4 << 4, elements);
    f[4..10].copy_from_slice(&[0xff; 6]);
    f[16..22].copy_from_slice(&[0xff; 6]);
    f
}

/// (BSSID, capability, the DSSS element's channel, an RSN element present) of a probe response.
fn response_summary(r: &[u8]) -> ([u8; 6], u16, u8, bool) {
    let mut bssid = [0u8; 6];
    bssid.copy_from_slice(&r[16..22]);
    let cap = u16::from_le_bytes([r[34], r[35]]);
    let elements = ies(r, 36);
    let chan = elements.iter().find(|(id, _)| *id == 3).map(|(_, d)| d[0]).expect("DSSS Parameter Set");
    (bssid, cap, chan, elements.iter().any(|(id, _)| *id == 48))
}

#[test]
fn nearby_networks_answer_a_wildcard_probe_after_the_access_point() {
    let mut ap = VirtualAp::new(config(), false);
    ap.beacons = nearby();
    assert!(ap.on_station_tx(&probe_request(&[0, 0]), 1000).is_none());
    assert_eq!(ap.queue.len(), 3);
    let (own, cap, chan, rsn) = response_summary(&ap.queue[0].frame);
    assert_eq!((own, cap & 0x11, chan, rsn), (config().bssid, 0x01, 6, false), "the open access point first: ESS, no Privacy");
    let (b, cap, chan, rsn) = response_summary(&ap.queue[1].frame);
    assert_eq!((b, cap & 0x11, chan, rsn), ([0x02, 1, 2, 3, 4, 5], 0x11, 1, true), "Home: Privacy and RSN, channel 1");
    let (b, cap, chan, rsn) = response_summary(&ap.queue[2].frame);
    assert_eq!((b, cap & 0x11, chan, rsn), ([0x02, 9, 8, 7, 6, 5], 0x01, 11, false), "Cafe: open, channel 11");
    assert!(ap.queue[0].at_us < ap.queue[1].at_us && ap.queue[1].at_us < ap.queue[2].at_us, "one after the other");
    for q in &ap.queue {
        assert_eq!(&q.frame[4..10], &[2, 3, 4, 5, 6, 7], "to the station");
        assert_eq!(&q.frame[10..16], &q.frame[16..22], "from its own BSSID");
    }
}

#[test]
fn a_directed_probe_gets_only_the_named_nearby_network() {
    let mut ap = VirtualAp::new(config(), false);
    ap.beacons = nearby();
    assert!(ap.on_station_tx(&probe_request(&[0, 4, b'C', b'a', b'f', b'e']), 1000).is_none());
    assert_eq!(ap.queue.len(), 1, "not the access point, not Home");
    assert_eq!(response_summary(&ap.queue[0].frame).0, [0x02, 9, 8, 7, 6, 5]);
    ap.queue.clear();
    assert!(ap.on_station_tx(&probe_request(&[0, 5, b'O', b't', b'h', b'e', b'r']), 2000).is_none());
    assert!(ap.queue.is_empty(), "nobody answers for an unknown SSID");
}

#[test]
fn frames_from_a_nearby_network_carry_its_channel_and_signal() {
    let mut ap = VirtualAp::new(config(), false);
    ap.beacons = nearby();
    let mut from_home = vec![0u8; 24];
    from_home[10..16].copy_from_slice(&[0x02, 1, 2, 3, 4, 5]);
    let mut from_ap = vec![0u8; 24];
    from_ap[10..16].copy_from_slice(&config().bssid);
    assert_eq!(ap.rx_meta(&from_home), (1, -60));
    assert_eq!(ap.rx_meta(&from_ap), (6, -40));
    assert_eq!(ap.rx_meta(&[0; 8]), (6, -40), "too short for an address: the access point's");
}

#[test]
fn authentication_to_a_nearby_network_goes_unanswered() {
    let mut ap = VirtualAp::new(config(), false);
    ap.beacons = nearby();
    let mut auth = station_frame(11 << 4, &[0, 0, 1, 0, 0, 0]);
    auth[4..10].copy_from_slice(&[0x02, 1, 2, 3, 4, 5]);
    auth[16..22].copy_from_slice(&[0x02, 1, 2, 3, 4, 5]);
    assert!(ap.on_station_tx(&auth, 0).is_none());
    assert_eq!(ap.state, StaState::Idle);
    assert!(ap.queue.is_empty(), "nothing answers: the network is out of reach");
}

#[test]
fn nearby_specs_parse_with_defaults_and_the_access_points_own_ssid_or_bssid_is_left_out() {
    let home = BeaconAp::parse("ssid=Home").unwrap();
    assert_eq!((home.channel, home.rssi, home.secure, home.bssid[0]), (1, -60, true, 0x02), "defaults; a locally administered unicast BSSID");
    assert_eq!(home.bssid, BeaconAp::parse("ssid=Home,chan=3").unwrap().bssid, "the same BSSID every time");
    assert_ne!(home.bssid, BeaconAp::parse("ssid=Cafe").unwrap().bssid);
    let cafe = BeaconAp::parse("ssid=Cafe,ch=11,rssi=-75,secure=0,bssid=02:09:08:07:06:05").unwrap();
    assert_eq!(cafe, BeaconAp { ssid: "Cafe".into(), bssid: [0x02, 9, 8, 7, 6, 5], channel: 11, rssi: -75, secure: false });
    for bad in ["chan=1", "ssid=", "ssid=Home,chan=15", "ssid=Home,rssi=low", "ssid=Home,secure=maybe", "ssid=Home,bssid=02:09", "ssid=Home,psk=x", "ssid=Home,chan"] {
        assert!(BeaconAp::parse(bad).is_err(), "{bad}");
    }
    assert!(BeaconAp::parse_all(&vec!["ssid=x".to_string(); 33]).is_err(), "more than 32");
    let mut link = StationLink::default();
    assert!(link.set_beacons(nearby()).is_none(), "no access point yet");
    link.attach(Some(VirtualAp::new(config(), false)), None);
    let mut list = nearby();
    list.push(BeaconAp { ssid: "esp32sim".into(), ..home.clone() });
    list.push(BeaconAp { ssid: "Office".into(), bssid: config().bssid, ..home });
    let left_out = link.set_beacons(list).unwrap();
    assert_eq!(link.ap().unwrap().beacons, nearby(), "the access point's own SSID or BSSID is that access point");
    assert_eq!(left_out.iter().map(|(b, why)| (b.ssid.as_str(), *why)).collect::<Vec<_>>(),
        [("esp32sim", "the access point's SSID"), ("Office", "the access point's BSSID")]);
}

/// Two networks around on one BSSID would both answer from it: `parse_all` refuses them, whether
/// the BSSID is derived from one SSID twice or given twice.
#[test]
fn nearby_networks_need_a_bssid_each() {
    let specs = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert!(BeaconAp::parse_all(&specs(&["ssid=Home", "ssid=Home,chan=6"])).unwrap_err().contains("share the BSSID"));
    assert!(BeaconAp::parse_all(&specs(&["ssid=Home,bssid=02:00:00:00:00:42", "ssid=Cafe,bssid=02:00:00:00:00:42"])).unwrap_err()
        .contains("'Home' and 'Cafe' share the BSSID 02:00:00:00:00:42"));
    assert_eq!(BeaconAp::parse_all(&specs(&["ssid=Home,bssid=02:00:00:00:00:42", "ssid=Home,bssid=02:00:00:00:00:43"])).unwrap().len(), 2,
        "one SSID on two BSSIDs is two networks");
}

/// A network around answers a probe request sent to the broadcast address or its own, with a
/// wildcard BSSID or its own (IEEE 802.11-2020 11.1.4.3.4), and no other.
#[test]
fn a_nearby_network_answers_only_probes_addressed_to_it() {
    let home = [0x02, 1, 2, 3, 4, 5];
    for (a1, a3, answers) in [([0xff; 6], [0xff; 6], true), (home, [0xff; 6], true), ([0xff; 6], home, true), (home, home, true),
        ([0x02, 9, 8, 7, 6, 5], [0xff; 6], false), ([0xff; 6], [0x02, 9, 8, 7, 6, 5], false)] {
        let mut ap = VirtualAp::new(config(), false);
        ap.beacons = vec![nearby().remove(0)];
        let mut probe = probe_request(&[0, 4, b'H', b'o', b'm', b'e']);
        probe[4..10].copy_from_slice(&a1);
        probe[16..22].copy_from_slice(&a3);
        assert!(ap.on_station_tx(&probe, 1000).is_none());
        assert_eq!(ap.queue.len(), usize::from(answers), "a1 {a1:02x?} a3 {a3:02x?}");
    }
}

// ------------------------------------------------------------------ several joinable access points

const STA: [u8; 6] = [2, 3, 4, 5, 6, 7];

/// An open network and a WPA2-PSK one, as `--wifi ssid=esp32sim-open --wifi ssid=esp32sim,psk=…`,
/// with the network behind them.
fn two_ap_link() -> StationLink {
    let cfgs = ApConfig::parse_all(&["ssid=esp32sim-open".into(), "ssid=esp32sim,psk=esp32sim-pass".into()]).unwrap();
    let mut l = StationLink::default();
    let mut it = cfgs.into_iter();
    l.attach(Some(VirtualAp::new(it.next().unwrap(), false)), Some(crate::net::VirtualNet::new(false)));
    for cfg in it { l.add_ap(cfg, &crate::DebugFlags::default()).unwrap(); }
    l
}

/// A management or data frame from the station to `bssid`.
fn frame_to(fc: u16, bssid: [u8; 6], payload: &[u8]) -> Vec<u8> {
    let mut f = vec![0; 24];
    f[..2].copy_from_slice(&fc.to_le_bytes());
    f[4..10].copy_from_slice(&bssid);
    f[10..16].copy_from_slice(&STA);
    f[16..22].copy_from_slice(&bssid);
    f.extend_from_slice(payload);
    f
}

/// Every frame the air delivers from `from_us` to `to_us`, one per airtime gap.
fn drain(l: &mut StationLink, from_us: u64, to_us: u64) -> Vec<Vec<u8>> {
    let mut got = Vec::new();
    let mut t = from_us;
    while t < to_us {
        l.net_step(t);
        if let Some(f) = l.next_rx(t, false) { got.push(f); l.rx_delivered(0x1000); }
        t += 100;
    }
    got
}

/// Open-system authentication and association with `bssid`, then what the air brings for 50 ms.
fn join(l: &mut StationLink, bssid: [u8; 6], t: u64) -> Vec<Vec<u8>> {
    l.station_tx(&frame_to(11 << 4, bssid, &[0, 0, 1, 0, 0, 0]), t);
    l.station_tx(&frame_to(0, bssid, &[1, 0, 0, 0]), t + 1_000);
    drain(l, t + 1_000, t + 50_000)
}

fn dhcp_discover() -> Vec<u8> {
    let mut d = vec![0u8; 240];
    d[0] = 1; d[1] = 1; d[2] = 6;
    d[4..8].copy_from_slice(&[1, 2, 3, 4]);
    d[28..34].copy_from_slice(&STA);
    d[236..240].copy_from_slice(&[0x63, 0x82, 0x53, 0x63]);
    d.extend_from_slice(&[53, 1, 1, 255]);
    let udp = crate::net::packet::udp_packet(&[0; 4], &[255; 4], 68, 67, &d);
    let ip = crate::net::packet::ip_packet(17, &[0; 4], &[255; 4], &udp);
    let mut payload = vec![0xaa, 0xaa, 3, 0, 0, 0, 0x08, 0x00];
    payload.extend_from_slice(&ip);
    payload
}

#[test]
fn several_access_points_parse_with_distinct_bssids_and_no_duplicates() {
    let cfgs = ApConfig::parse_all(&["ssid=a".into(), "ssid=b,psk=12345678".into(), "ssid=c,bssid=02:00:00:00:00:09".into()]).unwrap();
    assert_eq!(cfgs[0].bssid, [0x02, 0x53, 0x49, 0x4d, 0x00, 0x01], "the first keeps today's default");
    assert_eq!(cfgs[1].bssid, derived_bssid("b"), "a later one derives its own from the SSID");
    assert_eq!(cfgs[2].bssid, [2, 0, 0, 0, 0, 9]);
    assert_eq!(ApConfig::parse_all(&["ssid=a".into()]).unwrap()[0].bssid, ApConfig::parse("ssid=a").unwrap().bssid, "one spec parses as before");
    assert!(ApConfig::parse_all(&["ssid=a".into(), "ssid=a,psk=12345678".into()]).is_err(), "one SSID twice");
    assert!(ApConfig::parse_all(&["ssid=a".into(), "ssid=b,bssid=02:53:49:4d:00:01".into()]).is_err(), "one BSSID twice");
    let mut l = two_ap_link();
    assert!(l.add_ap(ApConfig::parse("ssid=esp32sim").unwrap(), &crate::DebugFlags::default()).is_err());
    assert!(StationLink::default().add_ap(ApConfig::parse("").unwrap(), &crate::DebugFlags::default()).is_err(), "needs the first one");
}

#[test]
fn a_scan_hears_both_access_points() {
    let mut l = two_ap_link();
    let (open, psk) = (l.aps()[0].cfg.bssid, l.aps()[1].cfg.bssid);
    // beacons from both within the first 300 ms
    let beacons: Vec<[u8; 6]> = drain(&mut l, 0, 300_000).iter().filter(|f| is_beacon(f)).map(|f| f[10..16].try_into().unwrap()).collect();
    assert!(beacons.contains(&open) && beacons.contains(&psk), "{beacons:x?}");
    // and a wildcard probe gets a response from each, with its own security
    l.station_tx(&probe_request(&[0, 0]), 400_000);
    let resp: Vec<_> = drain(&mut l, 400_000, 420_000).into_iter().filter(|f| f[0] == 0x50).map(|f| response_summary(&f)).collect();
    assert_eq!(resp.len(), 2, "{resp:x?}");
    assert!(resp.iter().any(|r| r.0 == open && !r.3) && resp.iter().any(|r| r.0 == psk && r.3));
    // a frame from the second carries its channel
    let mut f = vec![0x80, 0, 0, 0]; f.resize(24, 0); f[10..16].copy_from_slice(&psk);
    assert_eq!(l.rx_meta(&f), (6, -40));
}

#[test]
fn the_open_network_associates_and_leases_without_a_handshake() {
    let mut l = two_ap_link();
    let open = l.aps()[0].cfg.bssid;
    let got = join(&mut l, open, 0);
    assert_eq!(l.aps()[0].state, StaState::Associated);
    assert_eq!(l.aps()[1].state, StaState::Idle);
    assert_eq!(l.aps()[0].wpa.state, WpaState::Idle, "no four-way handshake on an open network");
    assert!(!got.iter().any(|f| describe(f).contains("888e")), "no EAPOL frames");
    l.station_tx(&frame_to(0x0108, open, &dhcp_discover()), 60_000);
    let offer = drain(&mut l, 60_000, 70_000).into_iter().find(|f| f[0] == 0x08 && f[10..16] == open);
    assert!(offer.is_some(), "a DHCP offer from the open access point");
}

#[test]
fn the_psk_network_runs_the_wpa2_handshake() {
    let mut l = two_ap_link();
    let psk = l.aps()[1].cfg.bssid;
    let got = join(&mut l, psk, 0);
    assert_eq!(l.aps()[1].state, StaState::Associated);
    assert_eq!(l.aps()[1].wpa.state, WpaState::AwaitingMessage2);
    assert!(got.iter().any(|f| f[10..16] == psk && describe(f).contains("888e")), "message 1 from the PSK access point");
    let mut m2 = eapol_message4();
    m2[5..7].copy_from_slice(&0x010au16.to_be_bytes());
    let mut payload = vec![0xaa, 0xaa, 3, 0, 0, 0, 0x88, 0x8e];
    payload.extend_from_slice(&m2);
    l.station_tx(&frame_to(0x0108, psk, &payload), 60_000);
    assert_eq!(l.aps()[1].wpa.state, WpaState::AwaitingMessage4);
    let mut payload = vec![0xaa, 0xaa, 3, 0, 0, 0, 0x88, 0x8e];
    payload.extend_from_slice(&eapol_message4());
    l.station_tx(&frame_to(0x0108, psk, &payload), 70_000);
    assert_eq!(l.aps()[1].wpa.state, WpaState::Installed);
    assert_eq!(l.aps()[0].state, StaState::Idle, "the open one never heard the station");
}

#[test]
fn associating_with_another_access_point_leaves_the_first() {
    let mut l = two_ap_link();
    let (open, psk) = (l.aps()[0].cfg.bssid, l.aps()[1].cfg.bssid);
    join(&mut l, psk, 0);
    assert_eq!(l.aps()[1].state, StaState::Associated);
    join(&mut l, open, 100_000);
    assert_eq!((l.aps()[0].state, l.aps()[1].state), (StaState::Associated, StaState::Idle));
    assert_eq!(l.aps()[1].wpa.state, WpaState::Idle);
    // data to the one left is not passed on; to the joined one it reaches the network
    l.station_tx(&frame_to(0x0108, psk, &dhcp_discover()), 200_000);
    assert_eq!(l.eth_tx_pending(), 0);
    l.station_tx(&frame_to(0x0108, open, &dhcp_discover()), 200_100);
    assert_eq!(l.eth_tx_pending(), 1);
    // and back again
    join(&mut l, psk, 300_000);
    assert_eq!((l.aps()[0].state, l.aps()[1].state), (StaState::Idle, StaState::Associated));
    // both survive a reboot
    let l = l.surviving_reboot();
    assert_eq!(l.aps().len(), 2);
}
