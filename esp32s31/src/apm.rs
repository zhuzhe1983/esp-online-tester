//! S31 APM reset-TEE subset. ESP-IDF6.1 fff9895c82d744c7237be8847347bdd1b07c6643,
//! soc/esp32s31/register/soc/{lp_apm,hp_apm,hp_mem_apm,cpu_apm}_reg.h.
//! The public S31 TRM pre-release0.1 omits PMS and bus topology. No P4/C6 master
//! numbering is borrowed: restrictive bus tests require an explicit route input.
use emu_core::Fault;
pub const BASES: [u32; 4] = [0x2070_6c00, 0x2050_4400, 0x2050_4800, 0x2050_4c00];
const REGIONS: [usize; 4] = [8, 16, 8, 8];
const PATHS: [u32; 4] = [4, 7, 6, 4];
const FUNC: [u32; 4] = [0xbc, 0xc4, 0xc4, 0xc4];
const INT: [u32; 4] = [0x10c, 0x14c, 0x128, 0x118];
const STATUS: [&[u32]; 4] = [&[0xc0, 0xd4, 0xe8, 0xfc], &[0xc8, 0xd8, 0xe8, 0xf8, 0x108, 0x118, 0x12c], &[0xc8, 0xd8, 0xe8, 0xf8, 0x108, 0x118], &[0xc8, 0xd8, 0xe8, 0xf8]];
const ERROR: [u32; 4] = [0x110, 0x154, 0x12c, 0x11c];
const START: [u32; 4] = [0, 0, 0x2f00_0000, 0x4000_0000];
const END: [u32; 4] = [u32::MAX, u32::MAX, 0x2f07_ffff, 0x5fff_ffff];
const ADDR_MASK: [u32; 4] = [u32::MAX, u32::MAX, 0x7ff80, 0x1fffe000];
#[derive(Clone, Copy)]
struct Region { start: u32, end: u32, attr: u32 }
#[derive(Clone, Copy)]
struct Route { path: u32, start: u32, end: u32 }
struct Domain { regions: [Region; 16], func: u32, bus_error: bool, clock: bool, date: u32, status: [u32; 7], route: Option<Route> }
impl Domain {
    fn new(id: usize) -> Self { Self { regions: [Region { start: START[id], end: END[id], attr: 0x7000 }; 16], func: (1 << PATHS[id]) - 1, bus_error: false, clock: true, date: 38_822_496, status: [0; 7], route: None } }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Denial { pub domain: usize, pub path: u32, pub address: u32, pub access: u32 }
pub struct Apm { domains: [Domain; 4], routes_present: bool, pub writes: u64, pub last_denial: Option<Denial> }
impl Default for Apm { fn default() -> Self { Self { domains: std::array::from_fn(Domain::new), routes_present: false, writes: 0, last_denial: None } } }
impl Apm {
    fn register(address: u32) -> Option<(usize, u32)> { BASES.iter().enumerate().find_map(|(i, &base)| address.checked_sub(base).filter(|&off| off < 0x400).map(|off| (i, off))) }
    pub fn is_mmio(address: u32) -> bool { Self::register(address).is_some() }
    /// Testbench wiring, not identification of an S31 physical bus path. There is
    /// no CLI/product default. The range must lie inside this bank's address domain.
    pub fn set_test_route(&mut self, domain: usize, path: u32, start: u32, end: u32) -> Result<(), Fault> {
        if domain >= 4 || path >= PATHS[domain] || start > end || start < START[domain] || end > END[domain] { return Err(Fault::Prohibited); }
        self.domains[domain].route = Some(Route { path, start, end }); self.routes_present = true; Ok(())
    }
    fn path_register(id: usize, off: u32) -> Option<(usize, u32)> {
        STATUS[id].iter().enumerate().find_map(|(p, &base)| off.checked_sub(base).filter(|&field| field < 16).map(|field| (p, field)))
    }
    pub fn read(&self, address: u32) -> Result<u32, Fault> {
        let (id, off) = Self::register(address).ok_or(Fault::Unmapped)?; if off & 3 != 0 { return Err(Fault::Misaligned); }
        let d = &self.domains[id];
        if off == 0 { return Ok(1); } // Only the reset region0 filter is implemented.
        if (4..=REGIONS[id] as u32 * 12).contains(&off) {
            let r = d.regions[((off - 4) / 12) as usize]; return Ok(match (off - 4) % 12 { 0 => r.start, 4 => r.end, _ => r.attr });
        }
        if off == FUNC[id] { return Ok(d.func); }
        if off == INT[id] { return Ok(0); }
        if off == ERROR[id] { return Ok(u32::from(d.bus_error)); }
        if let Some((p, field)) = Self::path_register(id, off) {
            return match field { 0 => Ok(d.status[p]), 4 => Ok(0), _ => Err(Fault::Unmapped) };
            // Precise INFO0 region/transaction-ID encoding is not established;
            // INFO0/INFO1 reads are unsupported rather than fabricated records.
        }
        match off { 0x150 if id == 1 => Ok(1), 0x3f8 => Ok(u32::from(d.clock)), 0x3fc => Ok(d.date), _ => Err(Fault::Unmapped) }
    }
    pub fn write(&mut self, address: u32, value: u32) -> Result<(), Fault> {
        let (id, off) = Self::register(address).ok_or(Fault::Unmapped)?; if off & 3 != 0 { return Err(Fault::Misaligned); }
        let d = &mut self.domains[id];
        if off == 0 {
            if value & ((1 << REGIONS[id]) - 1) != 1 { return Err(Fault::Prohibited); }
        } else if (4..=REGIONS[id] as u32 * 12).contains(&off) {
            let index = ((off - 4) / 12) as usize; let r = &mut d.regions[index];
            if r.attr & (1 << 16) == 0 {
                match (off - 4) % 12 {
                    0 => {
                        let next = START[id] | (value & ADDR_MASK[id]);
                        if index == 0 && next != START[id] { return Err(Fault::Prohibited); }
                        r.start = next;
                    }
                    4 => {
                        let next = (END[id] & !ADDR_MASK[id]) | (value & ADDR_MASK[id]);
                        if index == 0 && next != END[id] { return Err(Fault::Prohibited); }
                        r.end = next;
                    }
                    _ => {
                        if index == 0 && value & 0x7000 != 0x7000 && (d.route.is_none() || !d.bus_error) { return Err(Fault::Prohibited); }
                        r.attr = value & 0x17777;
                    }
                }
            }
        } else if off == FUNC[id] { d.func = value & ((1 << PATHS[id]) - 1); }
        else if off == ERROR[id] {
            if value & 1 == 0 && d.regions[0].attr & 0x7000 != 0x7000 { return Err(Fault::Prohibited); }
            d.bus_error = value & 1 != 0;
        } else if let Some((p, field)) = Self::path_register(id, off) {
            if field != 4 { return Err(Fault::Prohibited); }
            if value & 1 != 0 { d.status[p] = 0; }
        } else {
            match off {
                o if o == INT[id] && value & ((1 << PATHS[id]) - 1) == 0 => {},
                0x150 if id == 1 && value & 1 != 0 => {}, // transparent-on-denial behavior unsupported
                0x3f8 => d.clock = value & 1 != 0, // automatic gating does not disable accesses
                0x3fc => d.date = value & 0x0fff_ffff,
                o if o == INT[id] || id == 1 && o == 0x150 => return Err(Fault::Prohibited),
                _ => return Err(Fault::Unmapped),
            }
        }
        self.writes = self.writes.wrapping_add(1); Ok(())
    }
    /// One HP CPU in reset TEE mode only. Restrictive configurations have an
    /// explicit host route, so permission failures cannot invent master mappings.
    pub fn check(&mut self, address: u32, bytes: usize, access: u32) -> Result<(), Fault> {
        if !self.routes_present { return Ok(()); }
        if !matches!(access, 0..=2) { return Err(Fault::Prohibited); }
        let end = address.checked_add(u32::try_from(bytes.checked_sub(1).ok_or(Fault::Unmapped)?).map_err(|_| Fault::Unmapped)?).ok_or(Fault::Unmapped)?;
        for (id, d) in self.domains.iter_mut().enumerate() {
            let Some(route) = d.route else { continue; };
            if end < route.start || address > route.end || d.func & (1 << route.path) == 0 { continue; }
            if d.regions[0].attr & (1 << (12 + access)) == 0 {
                d.status[route.path as usize] |= 1;
                self.last_denial = Some(Denial { domain: id, path: route.path, address, access }); return Err(Fault::Prohibited);
            }
        }
        Ok(())
    }
    pub fn function_controls(&self) -> [u32; 4] { std::array::from_fn(|i| self.domains[i].func) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bank_resets_field_masks_locks_and_unsupported_topology_are_distinct() {
        let mut a = Apm::default();
        for i in 0..4 {
            let b = BASES[i]; assert_eq!(a.read(b + 4), Ok(START[i])); assert_eq!(a.read(b + 8), Ok(END[i]));
            assert_eq!(a.read(b + FUNC[i]), Ok((1 << PATHS[i]) - 1));
            assert_eq!(a.write(b + 0xc, 0x6000), Err(Fault::Prohibited));
            assert_eq!(a.write(b, 3), Err(Fault::Prohibited)); assert_eq!(a.read(b), Ok(1));
            a.write(b + 16, 0xffffffff).unwrap(); assert_eq!(a.read(b + 16), Ok(START[i] | ADDR_MASK[i]));
            a.write(b + 24, 0x17000).unwrap(); a.write(b + 16, 0).unwrap(); a.write(b + 24, 0).unwrap();
            assert_eq!(a.read(b + 16), Ok(START[i] | ADDR_MASK[i])); assert_eq!(a.read(b + 24), Ok(0x17000));
            a.write(b + FUNC[i], 0).unwrap(); assert_eq!(a.read(b + FUNC[i]), Ok(0));
            assert_eq!(a.write(b + INT[i], 1), Err(Fault::Prohibited));
            assert_eq!(a.read(b + 0x3f0), Err(Fault::Unmapped));
        }
        assert_eq!(a.set_test_route(4, 0, 0, 0), Err(Fault::Prohibited));
        assert_eq!(a.set_test_route(3, 4, START[3], END[3]), Err(Fault::Prohibited));
        assert_eq!(a.set_test_route(3, 0, 0, 1), Err(Fault::Prohibited));
    }
    #[test]
    fn declared_route_enforces_rw_execute_and_function_disable_without_fake_info() {
        let mut a = Apm::default(); let b = BASES[2]; let addr = START[2];
        a.set_test_route(2, 2, addr, addr + 127).unwrap(); a.write(b + ERROR[2], 1).unwrap();
        for access in 0..3 {
            a.write(b + 12, 0x7000 & !(1 << (12 + access))).unwrap();
            assert_eq!(a.check(addr, 4, access), Err(Fault::Prohibited));
            assert_eq!(a.read(b + 0xe8), Ok(1));
            assert_eq!(a.read(b + 0xf0), Err(Fault::Unmapped));
            assert_eq!(a.check(addr + 128, 4, access), Ok(()));
            assert_eq!(a.check(addr, 4, (access + 1) % 3), Ok(()));
            a.write(b + FUNC[2], 0x3b).unwrap(); assert_eq!(a.check(addr, 4, access), Ok(()));
            a.write(b + FUNC[2], 0x3f).unwrap();
            a.write(b + 0xec, 1).unwrap(); assert_eq!(a.read(b + 0xe8), Ok(0));
        }
        assert_eq!(a.write(b + ERROR[2], 0), Err(Fault::Prohibited));
        a.write(b + 12, 0x7000).unwrap(); a.write(b + ERROR[2], 0).unwrap();
        assert_eq!(a.check(addr, 4, 2), Ok(()));
    }
}
