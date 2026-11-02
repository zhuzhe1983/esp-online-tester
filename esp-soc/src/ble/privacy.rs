//! Resolving-list configuration used by NimBLE startup, including IDF v5.5.4.
//! This is not LL Privacy: no RPA generation/resolution, RF, pairing or encryption
//! sessions. Keep the LE Privacy feature clear. The virtual peer uses public addresses.
use super::opcodes::*;

pub const CAPACITY: usize = 8;
#[derive(Default)]
pub struct ResolvingList {
    enabled: bool,
    entries: Vec<Entry>,
}

struct Entry { parameters: [u8; 39], mode: u8 }

impl ResolvingList {
    // Parameter lengths are checked by the HCI dispatcher before entry.
    pub fn command(&mut self, op: u16, p: &[u8], active: bool) -> u8 {
        if op == LE_SET_ADDRESS_RESOLUTION_ENABLE {
            if p[0] > 1 { return 0x12; }
            if active { return 0x0c; }
            self.enabled = p[0] != 0;
            return 0;
        }
        if self.enabled && active { return 0x0c; }
        if op == LE_CLEAR_RESOLVING_LIST { self.entries.clear(); return 0; }
        if p[0] > 1 { return 0x12; }
        let existing = self.entries.iter().position(|entry| entry.parameters[..7] == p[..7]);
        match op {
            LE_ADD_RESOLVING_LIST => {
                let invalid_static = p[0] == 1 && (p[6] & 0xc0 != 0xc0
                    || (p[6] == 0xc0 && p[1..6].iter().all(|v| *v == 0))
                    || p[1..7].iter().all(|v| *v == 0xff));
                if invalid_static || existing.is_some()
                    || (p[7..23].iter().any(|v| *v != 0) && self.entries.iter().any(|entry| entry.parameters[7..23] == p[7..23])) { return 0x12; }
                if self.entries.len() == CAPACITY { return 0x07; }
                self.entries.push(Entry { parameters: p.try_into().unwrap(), mode: 0 });
                0
            }
            LE_REMOVE_RESOLVING_LIST => {
                let Some(index) = existing else { return 0x02; };
                self.entries.remove(index);
                0
            }
            LE_SET_PRIVACY_MODE => {
                if p[7] > 1 { return 0x12; }
                let Some(index) = existing else { return 0x02; };
                self.entries[index].mode = p[7];
                0
            }
            _ => unreachable!("only resolving-list commands are dispatched here"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::Controller;

    fn command(c: &mut Controller, op: u16, parameters: &[u8]) -> Vec<u8> {
        let mut packet = vec![1, op as u8, (op >> 8) as u8, parameters.len() as u8];
        packet.extend_from_slice(parameters);
        c.send(&packet);
        let response = c.pop_packet().expect("HCI Command Complete");
        assert_eq!(&response[..6], &[4, 0x0e, (response.len() - 3) as u8, 1, op as u8, (op >> 8) as u8]);
        assert!(c.pop_packet().is_none());
        response[6..].to_vec()
    }
    fn status(c: &mut Controller, op: u16, p: &[u8], expected: u8) { assert_eq!(command(c, op, p), [expected], "opcode {op:04x}"); }
    fn entry(id: u8) -> [u8; 39] { let mut entry = [0; 39]; entry[1] = id; entry[23..].fill(0x42); entry }

    #[test]
    fn nimble_irk_initialization_and_reset_keep_privacy_capability_truthful() {
        let mut c = Controller::default();
        // ESP-IDF v5.5.4 ble_hs_pvcy_set_our_irk(), followed by public advertising.
        status(&mut c, 0x202d, &[0], 0);
        status(&mut c, 0x2029, &[], 0);
        status(&mut c, 0x202d, &[1], 0);
        status(&mut c, 0x2027, &entry(0), 0);
        status(&mut c, 0x204e, &[0,0,0,0,0,0,0,1], 0);
        assert!(c.privacy.enabled);
        assert_eq!(c.privacy.entries[0].parameters, entry(0));
        assert_eq!(c.privacy.entries[0].mode, 1);
        assert_eq!(command(&mut c, 0x2003, &[]), [0; 9], "no LL Privacy feature without RPA resolution");
        let supported = command(&mut c, 0x1002, &[]);
        assert_eq!(supported[35] & 0x78, 0, "C.9 commands cannot be advertised without LL Privacy");
        assert_eq!(supported[36] & 0x02, 0);
        assert_eq!(supported[40] & 0x04, 0);
        status(&mut c, 0x200a, &[1], 0);
        assert!(c.advertising);
        status(&mut c, 0x0c03, &[], 0);
        assert!(!c.privacy.enabled);
        status(&mut c, 0x2028, &entry(0)[..7], 0x02);
        status(&mut c, 0x2027, &entry(0), 0);
        assert_eq!(c.privacy.entries[0].mode, 0);
    }

    #[test]
    fn resolving_list_capacity_duplicates_remove_clear_and_invalid_parameters() {
        let mut c = Controller::default();
        assert_eq!(command(&mut c, 0x202a, &[]), [0, 8]);
        status(&mut c, 0x2027, &entry(0), 0);
        status(&mut c, 0x2027, &entry(0), 0x12);
        for id in 1..8 { status(&mut c, 0x2027, &entry(id), 0); }
        status(&mut c, 0x2027, &entry(8), 0x07);
        status(&mut c, 0x2028, &entry(3)[..7], 0);
        status(&mut c, 0x2028, &entry(3)[..7], 0x02);
        status(&mut c, 0x2027, &entry(8), 0);
        assert_eq!(command(&mut c, 0x202a, &[]), [0, 8], "capacity, not free entries");
        status(&mut c, 0x2029, &[], 0);
        for address in [[0,0,0,0,0,0xc0], [0xff;6]] {
            let mut invalid = entry(0); invalid[0] = 1; invalid[1..7].copy_from_slice(&address);
            status(&mut c, 0x2027, &invalid, 0x12);
        }
        let mut random = entry(9); random[0] = 1;
        status(&mut c, 0x2027, &random, 0x12);
        random[6] = 0xc0; random[7] = 42;
        status(&mut c, 0x2027, &random, 0);
        random[1] = 10;
        status(&mut c, 0x2027, &random, 0x12); // same nonzero peer IRK
        status(&mut c, 0x204e, &[0,0,0,0,0,0,0,1], 0x02);
        status(&mut c, 0x204e, &[1,9,0,0,0,0,0xc0,2], 0x12);
        status(&mut c, 0x202d, &[2], 0x12);
        for (op, len) in [(0x2027,39), (0x2028,7), (0x2029,0), (0x202a,0), (0x202d,1), (0x204e,8)] {
            for n in 0..=40 { if n != len { status(&mut c, op, &vec![0; n], 0x12); } }
        }
    }

    #[test]
    fn address_resolution_configuration_obeys_advertising_and_scan_lifecycle() {
        for scanning in [false, true] {
            let mut c = Controller::default();
            status(&mut c, 0x202d, &[1], 0);
            let op = if scanning { 0x200c } else { 0x200a };
            let params: &[u8] = if scanning { &[1,0] } else { &[1] };
            status(&mut c, op, params, 0);
            status(&mut c, 0x202d, &[0], 0x0c);
            status(&mut c, 0x2029, &[], 0x0c);
            status(&mut c, 0x2027, &entry(1), 0x0c);
            status(&mut c, op, &vec![0; params.len()], 0);
            status(&mut c, 0x2027, &entry(1), 0);
            status(&mut c, 0x202d, &[0], 0);
            status(&mut c, op, params, 0);
            status(&mut c, 0x2029, &[], 0); // editing is allowed while resolution is disabled
            status(&mut c, 0x202d, &[1], 0x0c);
            status(&mut c, 0x0c03, &[], 0);
            status(&mut c, 0x202d, &[1], 0); // reset ends scanning too
        }
    }

    #[test]
    fn data_length_suggestion_validates_connection_and_keeps_legacy_peer_limits() {
        let mut c = Controller::default();
        assert_eq!(command(&mut c, 0x2022, &[1,0,251,0,0x48,8]), [2,1,0]);
        c.connect(false);
        c.pop_packet().unwrap();
        for (octets, time, expected) in [(27u16,328u16,0), (251,17040,0), (26,328,0x12), (252,328,0x12), (27,327,0x12), (27,17041,0x12)] {
            let mut p = vec![1,0]; p.extend(octets.to_le_bytes()); p.extend(time.to_le_bytes());
            assert_eq!(command(&mut c, 0x2022, &p), [expected,1,0]);
        }
        assert_eq!(command(&mut c, 0x2022, &[2,0,251,0,0x48,8]), [2,2,0]);
        for len in [0,1,5,7] { status(&mut c, 0x2022, &vec![0; len], 0x12); }
        assert_eq!(command(&mut c, 0x2003, &[]), [0;9]);
        assert_eq!(command(&mut c, 0x1002, &[])[34] & 0x40, 0);
        // A successful suggestion does not consume/change the ACL link.
        c.send(&[1,6,4,3,1,0,0x13]);
        assert_eq!(c.pop_packet().unwrap(), [4,0x0f,4,0,1,6,4]);
        assert_eq!(c.pop_packet().unwrap(), [4,5,4,0,1,0,0x13]);
        assert_eq!(command(&mut c, 0x2022, &[1,0,251,0,0x48,8]), [2,1,0]);
    }
}
