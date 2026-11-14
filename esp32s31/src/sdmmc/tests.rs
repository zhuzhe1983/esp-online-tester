use super::*;
use esp_periph::sdmmc::{des0, idsts, int, BMOD_DE, CMD_DATA, CMD_RESP, CMD_RESP_LONG, CMD_START, CMD_WRITE, CTRL_INT_ENABLE, CTRL_USE_INTERNAL_DMA};
use std::cell::RefCell;
use std::rc::Rc;

const SRAM: u32 = 0x4ff0_0000;
const PSRAM: u32 = 0x4800_0000;
const MPLL: Option<u64> = Some(500_000_000);
const SLOT: Option<([u8; 6], bool)> = Some((SLOT0_PINS, true));
struct Memory { sram: Vec<u8>, psram: Vec<u8> }
impl Default for Memory { fn default() -> Self { Self { sram: vec![0; 4096], psram: vec![0; 4096] } } }
impl Memory {
    fn range(&mut self, address: u32, len: usize) -> Result<&mut [u8], Fault> {
        for (base, bytes) in [(SRAM, &mut self.sram), (PSRAM, &mut self.psram)] {
            if let Some((start, end)) = address.checked_sub(base).map(|v| v as usize)
                .and_then(|start| start.checked_add(len).map(|end| (start, end))).filter(|(_, end)| *end <= bytes.len()) {
                return Ok(&mut bytes[start..end]);
            }
        }
        Err(Fault::Unmapped)
    }
}
impl SdmmcMemory for Memory {
    fn read(&mut self, address: u32, buf: &mut [u8]) -> Result<(), Fault> { buf.copy_from_slice(self.range(address, buf.len())?); Ok(()) }
    fn write(&mut self, address: u32, data: &[u8]) -> Result<(), Fault> { self.range(address, data.len())?.copy_from_slice(data); Ok(()) }
}
struct Store(Rc<RefCell<Vec<u8>>>);
impl SdStore for Store {
    fn len(&self) -> u64 { self.0.borrow().len() as u64 }
    fn read(&mut self, lba: u64, buf: &mut [u8; 512]) -> std::io::Result<()> {
        let at = usize::try_from(lba).ok().and_then(|v| v.checked_mul(512)).ok_or(std::io::ErrorKind::UnexpectedEof)?;
        buf.copy_from_slice(self.0.borrow().get(at..at + 512).ok_or(std::io::ErrorKind::UnexpectedEof)?); Ok(())
    }
    fn write(&mut self, lba: u64, buf: &[u8; 512]) -> std::io::Result<()> {
        let at = usize::try_from(lba).ok().and_then(|v| v.checked_mul(512)).ok_or(std::io::ErrorKind::UnexpectedEof)?;
        self.0.borrow_mut().get_mut(at..at + 512).ok_or(std::io::ErrorKind::UnexpectedEof)?.copy_from_slice(buf); Ok(())
    }
}
fn wr(host: &mut SdmmcS31, off: u32, value: u32) { host.write(BASE + off, value).unwrap(); }
fn rd(host: &mut SdmmcS31, off: u32) -> u32 { host.read(BASE + off).unwrap() }
fn configure(host: &mut SdmmcS31) {
    host.write(CLOCK, 9).unwrap(); host.write(MEMORY, 10).unwrap();
    host.write(PHASE, PHASE_CLOCKS | 7 << 1 | 3 << 5 | 7 << 9 | 1 << 16 | 1).unwrap();
    host.write(PAD_CTRL, 1).unwrap(); wr(host, reg::CLKENA, 1);
}
fn fixture() -> (SdmmcS31, Memory, Rc<RefCell<Vec<u8>>>) {
    let image = Rc::new(RefCell::new(vec![0; 1 << 20]));
    let mut host = SdmmcS31::default(); host.attach(Box::new(Store(image.clone())), SdCardConfig::default()).unwrap(); configure(&mut host);
    (host, Memory::default(), image)
}
fn cmd(host: &mut SdmmcS31, mem: &mut Memory, number: u8, arg: u32, flags: u32) -> u32 {
    wr(host, reg::RINTSTS, u32::MAX); wr(host, reg::CMDARG, arg); wr(host, reg::CMD, CMD_START | u32::from(number) | flags);
    host.run(mem, 1_000_000_000, SLOT, MPLL); rd(host, reg::RESP0)
}
fn ready(host: &mut SdmmcS31, mem: &mut Memory) {
    cmd(host, mem, 0, 0, 0); assert_eq!(cmd(host, mem, 8, 0x1aa, CMD_RESP), 0x1aa);
    for _ in 0..2 { cmd(host, mem, 55, 0, CMD_RESP); cmd(host, mem, 41, 0x40ff8000, CMD_RESP); }
    assert_eq!(rd(host, reg::RESP0), 0xc0ff8000);
    cmd(host, mem, 2, 0, CMD_RESP | CMD_RESP_LONG); let rca = cmd(host, mem, 3, 0, CMD_RESP) & 0xffff0000;
    cmd(host, mem, 9, rca, CMD_RESP | CMD_RESP_LONG); assert_eq!(rd(host, reg::RESP0 + 12) >> 30, 1);
    cmd(host, mem, 7, rca, CMD_RESP); cmd(host, mem, 55, rca, CMD_RESP); cmd(host, mem, 6, 2, CMD_RESP); wr(host, reg::CTYPE, 1);
}
fn transfer(host: &mut SdmmcS31, mem: &mut Memory, number: u8, buffer: u32) {
    for (i, v) in [des0::OWN | des0::FS | des0::LD, 512, buffer, 0].into_iter().enumerate() {
        mem.write(SRAM + 0x100 + i as u32 * 4, &v.to_le_bytes()).unwrap();
    }
    wr(host, reg::CTRL, CTRL_USE_INTERNAL_DMA | CTRL_INT_ENABLE); wr(host, reg::BMOD, BMOD_DE);
    wr(host, reg::DBADDR, SRAM + 0x100); wr(host, reg::BLKSIZ, 512); wr(host, reg::BYTCNT, 512); wr(host, reg::INTMASK, u32::MAX);
    cmd(host, mem, number, 8, CMD_RESP | CMD_DATA | if number == 24 { CMD_WRITE } else { 0 });
}

#[test]
fn s31_sdmmc_register_layout_clock_and_reset_values_are_chip_specific() {
    let mut host = SdmmcS31::default();
    assert_eq!(rd(&mut host, reg::VERID), 0x5432270a);
    assert_eq!(rd(&mut host, reg::CLK_EDGE_SEL), 1 << 9 | 1 << 17 | 1 << 23);
    assert_eq!(host.read(PAD_BASE + 0x1c), Ok(0x5ac)); assert_eq!(host.read(PAD_BASE + 0x2c), Ok(0x528));
    assert_eq!(host.read(MEMORY), Ok(6)); assert_eq!(host.read(CLOCK), Ok(0));
    assert_eq!(host.read(BASE + 4), Err(Fault::Unmapped)); assert_eq!(host.write(BASE + 1, 1), Err(Fault::Unmapped));
    wr(&mut host, reg::DLL_CONF, 2); assert_eq!(rd(&mut host, reg::DLL_CONF) & 2, 0, "no invented HS DLL calibration");
    configure(&mut host); assert_eq!(host.read(PHASE).unwrap() & 1, 0);
    assert_eq!(host.ciu_hz(MPLL), Some(62_500_000)); assert_eq!(host.ciu_hz(None), None);
    host.write(CLOCK, 13).unwrap(); assert_eq!(host.ciu_hz(MPLL), None, "unsupported root selection");
    host.write(CLOCK, 11).unwrap(); assert_eq!(host.ciu_hz(MPLL), MPLL);
    host.write(MEMORY, 14).unwrap(); assert_eq!(host.ciu_hz(MPLL), None);
    host.write(MEMORY, 15).unwrap(); assert_eq!(host.ciu_hz(MPLL), MPLL, "low power disabled by MODE=3");
}

#[test]
fn s31_sdmmc_idmac_writes_exported_store_and_reset_retains_card_contents() {
    let (mut host, mut mem, image) = fixture(); ready(&mut host, &mut mem);
    let pattern: Vec<u8> = (0..512).map(|i| (i % 251) as u8).collect(); mem.write(PSRAM + 0x400, &pattern).unwrap();
    transfer(&mut host, &mut mem, 24, PSRAM + 0x400);
    assert_eq!(rd(&mut host, reg::RINTSTS) & (int::DATA_OVER | int::DCRC | int::RTO), int::DATA_OVER); assert!(host.irq());
    assert_eq!(&image.borrow()[8 * 512..9 * 512], &pattern); assert_eq!(host.host.stats.bytes_written, 512);
    assert_eq!(mem.sram[0x103] & 0x80, 0, "IDMAC returns descriptor ownership");
    assert_eq!(rd(&mut host, reg::RAW_INTS), rd(&mut host, reg::RINTSTS));
    wr(&mut host, reg::RINTSTS, u32::MAX); assert!(!host.irq());
    host.reset(); assert!(host.present()); assert_eq!(host.read(CLOCK), Ok(0)); assert_eq!(host.host.stats.bytes_written, 512);
    configure(&mut host); ready(&mut host, &mut mem); transfer(&mut host, &mut mem, 17, SRAM + 0x400);
    assert_eq!(&mem.sram[0x400..0x600], &pattern); assert_eq!(host.host.stats.bytes_read, 512);
    let rca = 0xe6240000; cmd(&mut host, &mut mem, 32, 8, CMD_RESP); cmd(&mut host, &mut mem, 33, 8, CMD_RESP);
    cmd(&mut host, &mut mem, 38, 0, CMD_RESP); cmd(&mut host, &mut mem, 13, rca, CMD_RESP);
    assert_eq!(&image.borrow()[8 * 512..9 * 512], &[0; 512], "erase also reaches exported image");
}

#[test]
fn s31_sdmmc_clock_stall_and_cnnt_reset_do_not_complete_a_pending_command() {
    let (mut host, mut mem, _) = fixture(); host.write(CLOCK, 0).unwrap();
    cmd(&mut host, &mut mem, 8, 0x1aa, CMD_RESP); assert!(host.pending()); assert_eq!(rd(&mut host, reg::RINTSTS), 0);
    host.write(CLOCK, 9).unwrap(); host.run(&mut mem, 0, SLOT, None); assert!(host.pending());
    host.run(&mut mem, 0, SLOT, MPLL); assert!(!host.pending()); assert_eq!(rd(&mut host, reg::RESP0), 0x1aa);
    host.write(RESET, 1 << 30).unwrap(); cmd(&mut host, &mut mem, 8, 0x155, CMD_RESP);
    assert!(!host.pending()); assert_eq!(rd(&mut host, reg::RESP0), 0); assert!(host.present());
    host.write(RESET, 0).unwrap(); wr(&mut host, reg::CLKENA, 1);
    assert_eq!(cmd(&mut host, &mut mem, 8, 0x155, CMD_RESP), 0x155);
}

#[test]
fn s31_sdmmc_wrong_pads_power_slot_and_ejection_cannot_answer_for_the_card() {
    for bad in 0..7 {
        let (mut host, mut mem, image) = fixture(); let mut slot = SLOT; let mut card_num = 0;
        match bad {
            0 => { host.write(PAD_CTRL, 0).unwrap(); }
            1 => { host.write(PAD_BASE + 0x30, 0x4ac).unwrap(); }
            2 => slot = Some((SLOT0_PINS, false)),
            3 => slot = Some(([24, 25, 20, 21, 22, 19], true)),
            4 => card_num = 1 << 16,
            5 => wr(&mut host, reg::CLKENA, 0),
            _ => { assert!(host.eject().is_some()); assert!(!host.present()); assert_eq!(image.borrow().len(), 1 << 20); }
        }
        wr(&mut host, reg::CMDARG, 0x1aa); wr(&mut host, reg::CMD, CMD_START | card_num | 8 | CMD_RESP);
        host.run(&mut mem, 0, slot, MPLL); assert_ne!(rd(&mut host, reg::RINTSTS) & int::RTO, 0, "case {bad}");
        assert_eq!(rd(&mut host, reg::RESP0), 0);
    }
}

#[test]
fn s31_sdmmc_bad_dma_is_a_bus_error_and_bad_attach_preserves_existing_media() {
    let (mut host, mut mem, _) = fixture();
    let config = SdCardConfig { timed_bus: true, ..Default::default() };
    assert!(host.attach(Box::new(esp_periph::sdmmc::MemStore(vec![0; 1 << 20])), config).is_err());
    assert!(host.attach(Box::new(esp_periph::sdmmc::MemStore(vec![0; 513])), SdCardConfig::default()).is_err()); assert!(host.present());
    ready(&mut host, &mut mem); transfer(&mut host, &mut mem, 17, BASE);
    assert_ne!(rd(&mut host, reg::IDSTS) & idsts::FBE, 0);
    assert!(mem.range(u32::MAX, 512).is_err()); assert!(mem.range(SRAM + 4095, 512).is_err());
}
