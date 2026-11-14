//! The SD/MMC host: the Synopsys DesignWare Mobile Storage Host core of the ESP32, ESP32-S3 and
//! ESP32-P4 / ESP32-S31 SDMMC block, its internal DMA controller (IDMAC), and the seam a card or an SDIO
//! function on one of its two slots implements (`SdDevice`).
//!
//! Registers: IDF v5.5.3 components/soc/esp32p4/register/hw_ver1/soc/sdmmc_struct.h and
//! sdmmc_reg.h, whose core block (0x000-0x1ff) has the offsets of soc/esp32/register/soc and
//! soc/esp32s3/register/soc/sdmmc_struct.h; `SdmmcLayout` holds what differs above it. What the
//! driver expects of them: components/esp_driver_sdmmc/src/sdmmc_host.c, sdmmc_transaction.c and
//! hal/esp32p4/include/hal/sdmmc_ll.h.
//!
//! A command runs to completion when the chip calls `run` after the CMD write that starts it:
//! the response, CMD_DONE, the data phase through the IDMAC, DATA_OVER and the auto-stop. The
//! IDMAC suspends on a descriptor the CPU still owns (DU) and resumes on a PLDMND write. Devices
//! see emulated time (`Sdmmc::now_ns`, set by the chip before each access) for their own busy
//! periods; the bus itself takes none unless `timed` is set (`bus_ns`): then CMD_DONE, DATA_OVER
//! and the auto-stop's ACD come when the bits are through at the slot's card clock and width,
//! from `advance`, which the chip calls at `next_event_ns`.
pub mod card;
pub use card::{FileStore, MemStore, SdCard, SdCardConfig, SdStore};
use crate::device::{Device, WriteEffect};
use crate::regram::RegRam;
use emu_core::Fault;

pub const SLOTS: usize = 2;

/// A response on the CMD line: the 32 bits of a 48-bit response (R1, R3, R6, R7, the SDIO R4
/// and R5) or the 128 bits of an R2, bit 0 first.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SdResponse { Short(u32), Long([u32; 4]) }

/// A command that did not get a good response.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SdCmdError {
    /// the device did not answer: the host's response timeout (RTO)
    NoResponse,
    /// a response with a bad CRC7 (RCRC when the command asks for the check)
    Crc(SdResponse),
    /// a response with a wrong index or end bit (RESP_ERR)
    Malformed(SdResponse),
}

/// Why a data block did not complete.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SdDataError {
    /// the device sent no data (a read) or no CRC status (a write): DRTO or EBE
    Timeout,
    /// a read block with a bad CRC16, or a write block refused with a CRC status error: DCRC
    Crc,
    /// a write block got no CRC status, the device not receiving: EBE
    NoCrcStatus,
}

/// Something on an SD bus: a memory card (`SdCard`) or an SDIO function (the P4's ESP-Hosted
/// co-processor on slot 1). The host sends commands and moves data; the device answers in the SD
/// protocol's terms and knows nothing about the host's registers or DMA. `now` is emulated time
/// in nanoseconds.
pub trait SdDevice {
    /// A command on CMD and the device's answer.
    fn command(&mut self, now: u64, cmd: u8, arg: u32) -> Result<SdResponse, SdCmdError>;
    /// The next `buf.len()` bytes of the data phase the last command started (card to host),
    /// which the bus has carried by `now`.
    fn read_data(&mut self, now: u64, buf: &mut [u8]) -> Result<(), SdDataError>;
    /// The next bytes of a write data phase (host to card).
    fn write_data(&mut self, now: u64, data: &[u8]) -> Result<(), SdDataError>;
    /// DAT0 held low (programming, an R1b busy period). Looking does not change the device.
    fn busy(&self, _now: u64) -> bool { false }
    /// When a busy period in progress ends, if the device can say.
    fn busy_until(&self) -> Option<u64> { None }
    /// How many data lines the device drives now, checked against the host's CTYPE.
    fn bus_width(&self) -> Option<u8> { None }
    /// The SDIO interrupt (DAT1 low).
    fn interrupt(&self) -> bool { false }
    /// An SDIO function, whose DAT1 carries its interrupt between transfers (SDIO Simplified
    /// Specification v3.00 §8.1.2).
    fn io(&self) -> bool { false }
    /// Asked as DAT1 goes low: true makes the host miss this edge, D1 still low (a fault).
    fn edge_lost(&self) -> bool { false }
    fn name(&self) -> &str;
    fn set_log(&mut self, _on: bool) {}
}

/// The memory the IDMAC reaches. A fault stops the transfer with a fatal bus error.
pub trait SdmmcMemory {
    fn read(&mut self, addr: u32, buf: &mut [u8]) -> Result<(), Fault>;
    fn write(&mut self, addr: u32, data: &[u8]) -> Result<(), Fault>;
}

/// What differs between the chips that carry the core.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SdmmcLayout {
    /// ESP32-P4: no PWREN or GPIO register; Espressif's wrapper from 0x800 (CLK_EDGE_SEL,
    /// RAW_INTS, DLL_CLK_CONF, DLL_CONF).
    Esp32p4,
    /// ESP32-S31: the same core and 0x800 wrapper offsets, checked against IDF v6.1
    /// fff9895c82d744c7237be8847347bdd1b07c6643 soc/esp32s31/register/soc/sdmmc_reg.h.
    /// The chip supplies its own clocks and dedicated pads; no HS DLL lock is synthesized.
    Esp32s31,
    /// ESP32 and ESP32-S3: PWREN at 0x04 and the CLOCK register at 0x800. Not wired to a chip
    /// yet; their reset values are unverified.
    Esp32,
}

/// Register offsets (sdmmc_struct.h).
pub mod reg {
    pub const CTRL: u32 = 0x00; pub const CLKDIV: u32 = 0x08; pub const CLKSRC: u32 = 0x0c; pub const CLKENA: u32 = 0x10;
    pub const TMOUT: u32 = 0x14; pub const CTYPE: u32 = 0x18; pub const BLKSIZ: u32 = 0x1c; pub const BYTCNT: u32 = 0x20;
    pub const INTMASK: u32 = 0x24; pub const CMDARG: u32 = 0x28; pub const CMD: u32 = 0x2c; pub const RESP0: u32 = 0x30;
    pub const MINTSTS: u32 = 0x40; pub const RINTSTS: u32 = 0x44; pub const STATUS: u32 = 0x48; pub const FIFOTH: u32 = 0x4c;
    pub const CDETECT: u32 = 0x50; pub const WRTPRT: u32 = 0x54; pub const TCBCNT: u32 = 0x5c; pub const TBBCNT: u32 = 0x60;
    pub const VERID: u32 = 0x6c; pub const HCON: u32 = 0x70; pub const RST_N: u32 = 0x78;
    pub const BMOD: u32 = 0x80; pub const PLDMND: u32 = 0x84; pub const DBADDR: u32 = 0x88; pub const IDSTS: u32 = 0x8c;
    pub const IDINTEN: u32 = 0x90; pub const DSCADDR: u32 = 0x94; pub const BUFADDR: u32 = 0x98; pub const CARDTHRCTL: u32 = 0x100;
    pub const CLK_EDGE_SEL: u32 = 0x800; pub const RAW_INTS: u32 = 0x804; pub const DLL_CONF: u32 = 0x80c;
}
/// CARDTHRCTL.BUSY_CLR_INT_EN: raise BCI (RINTSTS bit 13, SBE's) when a write's busy ends.
pub const CARDTHRCTL_BUSY_CLR_INT_EN: u32 = 1 << 1;

/// CTRL bits: the three resets clear themselves.
pub const CTRL_CONTROLLER_RESET: u32 = 1 << 0;
pub const CTRL_FIFO_RESET: u32 = 1 << 1;
pub const CTRL_DMA_RESET: u32 = 1 << 2;
pub const CTRL_INT_ENABLE: u32 = 1 << 4;
pub const CTRL_USE_INTERNAL_DMA: u32 = 1 << 25;
/// CMD bits.
pub const CMD_START: u32 = 1 << 31;
pub const CMD_UPDATE_CLK: u32 = 1 << 21;
pub const CMD_STOP_ABORT: u32 = 1 << 14;
pub const CMD_AUTO_STOP: u32 = 1 << 12;
pub const CMD_WRITE: u32 = 1 << 10;
pub const CMD_DATA: u32 = 1 << 9;
pub const CMD_RESP_CRC: u32 = 1 << 8;
pub const CMD_RESP_LONG: u32 = 1 << 7;
pub const CMD_RESP: u32 = 1 << 6;
/// RINTSTS / INTMASK / MINTSTS bits (hal/esp32p4/include/hal/sdmmc_ll.h SDMMC_LL_EVENT_*).
pub mod int {
    pub const CD: u32 = 1 << 0; pub const RESP_ERR: u32 = 1 << 1; pub const CMD_DONE: u32 = 1 << 2; pub const DATA_OVER: u32 = 1 << 3;
    pub const RCRC: u32 = 1 << 6; pub const DCRC: u32 = 1 << 7; pub const RTO: u32 = 1 << 8; pub const DRTO: u32 = 1 << 9;
    pub const HTO: u32 = 1 << 10; pub const HLE: u32 = 1 << 12; pub const SBE: u32 = 1 << 13; pub const BCI: u32 = 1 << 13; pub const ACD: u32 = 1 << 14;
    pub const EBE: u32 = 1 << 15; pub const SDIO_SLOT0: u32 = 1 << 16;
    pub const ALL: u32 = 0x3_ffff;
}
/// IDSTS / IDINTEN bits; NIS sums TI and RI, AIS sums FBE, DU and CES.
pub mod idsts {
    pub const TI: u32 = 1 << 0; pub const RI: u32 = 1 << 1; pub const FBE: u32 = 1 << 2; pub const DU: u32 = 1 << 4;
    pub const CES: u32 = 1 << 5; pub const NIS: u32 = 1 << 8; pub const AIS: u32 = 1 << 9;
    pub const W1C: u32 = 0x337;
    /// FBE_CODE (bits 12:10): host abort during transmission (1) or reception (2).
    pub const FBE_CODE_SHIFT: u32 = 10;
}
/// BMOD: software reset (self-clearing), descriptor skip length 6:2, IDMAC enable.
pub const BMOD_SWR: u32 = 1 << 0;
pub const BMOD_DE: u32 = 1 << 7;
/// IDMAC descriptor DES0 (soc/esp32p4/register/hw_ver1/soc/sdmmc_struct.h sdmmc_desc_t).
pub mod des0 {
    pub const DIC: u32 = 1 << 1; pub const LD: u32 = 1 << 2; pub const FS: u32 = 1 << 3; pub const CH: u32 = 1 << 4;
    pub const ER: u32 = 1 << 5; pub const CES: u32 = 1 << 30; pub const OWN: u32 = 1 << 31;
}
/// VERID reset value (sdmmc_reg.h SDHOST_VERSIONID default 1412572938).
pub const VERID_P4: u32 = 0x5432_270a;
/// HCON reset value from its fields (sdmmc_reg.h): card type 1, two cards (CARD_NUM 1), bus
/// type 1, 32-bit data (1), address width 19, DMA width 1, hold register 1, clock dividers 3.
pub const HCON: u32 = 1 | 1 << 1 | 1 << 6 | 1 << 7 | 19 << 10 | 1 << 18 | 1 << 22 | 3 << 24;

/// The descriptor the IDMAC is working through.
#[derive(Clone, Copy, Debug)]
struct Desc { addr: u32, des0: u32, bufs: [(u32, u32); 2], which: usize, off: u32 }

/// The data phase in progress.
#[derive(Debug)]
struct Transfer {
    slot: usize,
    write: bool,
    total: u32,
    /// bytes moved between memory and the FIFO
    moved: u32,
    /// bytes still to come from (or go to) the device
    card_left: u32,
    block: u32,
    fifo: Vec<u8>,
    auto_stop: bool,
    desc: Option<Desc>,
    /// the last descriptor (LD) has been closed
    chain_end: bool,
    suspended: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum Pending { #[default] None, Command, Resume }

#[derive(Clone, Copy, Default, Debug)]
pub struct SdmmcStats { pub commands: u64, pub timeouts: u64, pub bytes_read: u64, pub bytes_written: u64 }

pub struct Sdmmc {
    pub layout: SdmmcLayout,
    ctrl: u32, cmd: u32, cmdarg: u32, resp: [u32; 4], rintsts: u32, intmask: u32, ctype: u32, blksiz: u32, bytcnt: u32,
    bmod: u32, dbaddr: u32, idsts: u32, idinten: u32, dscaddr: u32, bufaddr: u32, tcbcnt: u32, tbbcnt: u32, resp_index: u32,
    /// the descriptor the IDMAC fetches next
    cursor: u32,
    ram: RegRam,
    /// CDETECT and WRTPRT: bit n is slot n's card_detect_n and write_protect input, which the
    /// chip resolves from its pin routing
    pub card_detect_n: u32,
    pub write_protect: u32,
    /// What is on the bus, and which of it each slot reaches (set by the chip before `run`).
    pub devices: Vec<Box<dyn SdDevice>>,
    pub route: [Option<usize>; SLOTS],
    /// Emulated time in nanoseconds, set by the chip before every access.
    pub now_ns: u64,
    xfer: Option<Transfer>,
    pending: Pending,
    /// slot of a finished write whose busy-clear interrupt is still to come
    bci_slot: Option<usize>,
    /// a register read raised an interrupt (BCI on a STATUS read)
    read_irq: bool,
    /// commands and data take their time on the bus; `ciu_hz` is the clock into the card-clock
    /// dividers, set by the chip
    pub timed: bool,
    pub ciu_hz: u64,
    /// RINTSTS bits raised at a time to come, and when the data phase in progress ends
    deferred: Vec<(u64, u32)>,
    data_due: u64,
    pub stats: SdmmcStats,
    log: bool,
    /// each slot's SDIO interrupt as last seen on DAT1
    sdio_irq: [bool; SLOTS],
}

impl Sdmmc {
    pub fn new(layout: SdmmcLayout) -> Self {
        let mut s = Sdmmc { layout, ctrl: 0, cmd: 0, cmdarg: 0, resp: [0; 4], rintsts: 0, intmask: 0, ctype: 0, blksiz: 0, bytcnt: 0,
            bmod: 0, dbaddr: 0, idsts: 0, idinten: 0, dscaddr: 0, bufaddr: 0, tcbcnt: 0, tbbcnt: 0, resp_index: 0, cursor: 0,
            ram: RegRam::new(), card_detect_n: 0, write_protect: 0, devices: Vec::new(), route: [None; SLOTS], now_ns: 0, xfer: None,
            pending: Pending::None, bci_slot: None, read_irq: false, timed: false, ciu_hz: 0, deferred: Vec::new(), data_due: 0,
            stats: SdmmcStats::default(), log: false, sdio_irq: [false; SLOTS] };
        s.reset_registers();
        s
    }

    /// The block's own reset (LP_CLKRST RST_EN_SDMMC on the P4): every register to its default;
    /// what is attached stays.
    pub fn reset_registers(&mut self) {
        self.ram = RegRam::new();
        // CMD.USE_HOLE_REG resets to 1, BLKSIZ and BYTCNT to 512.
        (self.ctrl, self.cmd, self.cmdarg, self.resp, self.rintsts, self.intmask, self.ctype) = (0, 1 << 29, 0, [0; 4], 0, 0, 0);
        (self.blksiz, self.bytcnt) = (0x200, 0x200);
        (self.bmod, self.dbaddr, self.idsts, self.idinten, self.dscaddr, self.bufaddr, self.cursor) = (0, 0, 0, 0, 0, 0, 0);
        (self.tcbcnt, self.tbbcnt, self.resp_index) = (0, 0, 0);
        self.ram.write(reg::TMOUT, 0xffff_ff40);                  // data 0xFFFFFF, response 64
        self.ram.write(reg::RST_N, 1);
        if matches!(self.layout, SdmmcLayout::Esp32p4 | SdmmcLayout::Esp32s31) { self.ram.write(reg::CLK_EDGE_SEL, 1 << 9 | 1 << 17 | 1 << 23); }   // EDGE_H 1, EDGE_N 1, CCLK_EN
        self.xfer = None;
        self.pending = Pending::None;
        self.bci_slot = None;
        self.deferred.clear();
    }

    /// The card clock of `slot`: CLKSRC picks one of the four CLKDIV dividers (0 divides by 1,
    /// n by 2n), as sdmmc_ll_get_card_clock_div reads them (hal/esp32p4/include/hal/sdmmc_ll.h).
    fn card_hz(&self, slot: usize) -> u64 {
        let src = self.ram.read(reg::CLKSRC) >> (2 * slot) & 3;
        let div = self.ram.read(reg::CLKDIV) >> (8 * src) & 0xff;
        self.ciu_hz / if div == 0 { 1 } else { 2 * div as u64 }
    }

    /// Nanoseconds `clocks` card clocks of `slot` take.
    fn bus_ns(&self, slot: usize, clocks: u64) -> u64 {
        let hz = self.card_hz(slot).max(1);
        (clocks as u128 * 1_000_000_000).div_ceil(hz as u128) as u64
    }

    /// The data phase on the wire (SD Physical Layer Simplified Specification v9.10 §3.6.1,
    /// §4.12): each block a start bit, its bits over the bus width, CRC16 and an end bit on every
    /// line, a write's CRC status token after it, and two clocks between blocks (NAC, NWR).
    fn data_ns(&self, slot: usize, total: u32, block: u32, write: bool) -> u64 {
        let width = self.width(slot) as u64;
        let blocks = total.div_ceil(block.max(1)) as u64;
        let per = 1 + (block as u64 * 8).div_ceil(width) + 16 + 1 + if write { 2 + 5 } else { 0 } + 2;
        self.bus_ns(slot, blocks * per)
    }

    /// Raise RINTSTS `bits` at `at` (at once unless the bus is timed and `at` is to come).
    fn raise(&mut self, at: u64, bits: u32) {
        if self.timed && at > self.now_ns { self.deferred.push((at, bits)); } else { self.rintsts |= bits; }
    }

    /// When the next deferred interrupt, or the end of the busy period a BCI waits for, is due.
    pub fn next_event_ns(&self) -> Option<u64> {
        let bits = self.deferred.iter().map(|d| d.0).min();
        let busy = self.bci_slot.and_then(|s| self.route[s]).and_then(|i| self.devices.get(i)).and_then(|d| d.busy_until());
        match (bits, busy) { (Some(a), Some(b)) => Some(a.min(b)), (a, b) => a.or(b) }
    }

    /// Time has reached `now`: raise what is due. True when RINTSTS changed.
    pub fn advance(&mut self, now: u64) -> bool {
        self.now_ns = now;
        let before = self.rintsts;
        let mut i = 0;
        while i < self.deferred.len() {
            if self.deferred[i].0 <= now { let (_, b) = self.deferred.swap_remove(i); self.rintsts |= b; } else { i += 1; }
        }
        if self.bci_slot.is_some() { self.busy_clear(); }
        self.read_irq = false;
        self.rintsts != before
    }

    /// Sample DAT1 of the slots an SDIO function sits on. RINTSTS's per-slot SDIO interrupt
    /// (bit 16 + slot) is set when DAT1 goes low: IDF treats it as edge triggered and reads the D1
    /// pin for the level (esp_driver_sdmmc/src/sdmmc_host.c, `sdmmc_host_io_int_wait`). True when
    /// RINTSTS changed.
    pub fn poll_sdio(&mut self) -> bool {
        let before = self.rintsts;
        for slot in 0..SLOTS {
            let dev = self.route[slot].and_then(|i| self.devices.get(i));
            let irq = dev.is_some_and(|d| d.io() && d.interrupt());
            if irq && !self.sdio_irq[slot] && !dev.is_some_and(|d| d.edge_lost()) { self.rintsts |= int::SDIO_SLOT0 << slot; }
            self.sdio_irq[slot] = irq;
        }
        self.rintsts != before
    }

    /// A command or an IDMAC resume is waiting for `run`.
    #[inline]
    pub fn pending(&self) -> bool { self.pending != Pending::None }

    /// Attach a device; returns its index for `route`.
    pub fn attach(&mut self, dev: Box<dyn SdDevice>) -> usize { self.devices.push(dev); self.devices.len() - 1 }
    /// `--debug sdmmc` is on.
    pub fn logging(&self) -> bool { self.log }

    /// A register read changed the interrupt line since the last call.
    pub fn take_read_irq(&mut self) -> bool { std::mem::take(&mut self.read_irq) }

    /// The controller's interrupt under CTRL.INT_ENABLE, and the IDMAC's: TI and RI under
    /// IDINTEN.NI, FBE, DU and CES under IDINTEN.AI.
    pub fn irq(&self) -> bool {
        let pending = self.idsts & self.idinten;
        (self.ctrl & CTRL_INT_ENABLE != 0 && self.rintsts & self.intmask & int::ALL != 0)
            || (pending & (idsts::TI | idsts::RI) != 0 && self.idinten & idsts::NIS != 0)
            || (pending & (idsts::FBE | idsts::DU | idsts::CES) != 0 && self.idinten & idsts::AIS != 0)
    }

    fn idmac_on(&self) -> bool { self.bmod & BMOD_DE != 0 && self.ctrl & CTRL_USE_INTERNAL_DMA != 0 }

    /// Data lines CTYPE gives a slot.
    fn width(&self, slot: usize) -> u8 { if self.ctype >> (16 + slot) & 1 != 0 { 8 } else if self.ctype >> slot & 1 != 0 { 4 } else { 1 } }

    fn device(&mut self, slot: usize) -> Option<&mut Box<dyn SdDevice>> {
        let i = (*self.route.get(slot)?)?;
        self.devices.get_mut(i)
    }

    /// Carry out what the last register write started.
    pub fn run(&mut self, mem: &mut dyn SdmmcMemory) {
        match std::mem::take(&mut self.pending) {
            Pending::Command => self.command(mem),
            Pending::Resume => self.pump(mem),
            Pending::None => {}
        }
    }

    fn send(&mut self, slot: usize, index: u8, arg: u32) -> Result<SdResponse, SdCmdError> {
        let now = self.now_ns;
        self.device(slot).map_or(Err(SdCmdError::NoResponse), |d| d.command(now, index, arg))
    }

    fn command(&mut self, mem: &mut dyn SdmmcMemory) {
        let c = self.cmd;
        self.cmd &= !CMD_START;
        let (index, slot) = ((c & 0x3f) as u8, ((c >> 16) & 0x1f) as usize);
        if c & CMD_STOP_ABORT != 0 { self.xfer = None; }
        self.stats.commands += 1;
        // card_num is five bits; nothing answers on a slot past the host's two.
        if slot >= SLOTS { self.stats.timeouts += 1; self.rintsts |= int::RTO | int::CMD_DONE; return; }
        let arg = self.cmdarg;
        let resp = self.send(slot, index, arg);
        if self.log { eprintln!("[sdmmc] slot {} CMD{} arg {:#010x} -> {:?}", slot, index, arg, resp); }
        let mut error = 0;
        // the command and its response on CMD: 48 bits, NCR, the 48- or 136-bit response, NCC
        // (SD Physical Layer Simplified Specification v9.10 §4.12.1); no response: the 64-clock timeout
        let resp_bits = if c & CMD_RESP == 0 { 0 } else if c & CMD_RESP_LONG != 0 { 136 } else { 48 };
        let cmd_end = if self.timed { self.now_ns + self.bus_ns(slot, 48 + 2 + resp_bits + 8) } else { self.now_ns };
        if c & CMD_RESP != 0 {
            let r = match resp {
                Err(SdCmdError::NoResponse) => {
                    self.stats.timeouts += 1;
                    let at = if self.timed { self.now_ns + self.bus_ns(slot, 48 + 64) } else { self.now_ns };
                    self.raise(at, int::RTO | int::CMD_DONE);
                    return;
                }
                Ok(r) => r,
                Err(SdCmdError::Crc(r)) => { if c & CMD_RESP_CRC != 0 { error = int::RCRC; } r }
                Err(SdCmdError::Malformed(r)) => { error = int::RESP_ERR; r }
            };
            match r { SdResponse::Short(v) => self.resp[0] = v, SdResponse::Long(v) => self.resp = v }
            self.resp_index = index as u32;
        }
        self.raise(cmd_end, int::CMD_DONE | error);
        if c & CMD_DATA != 0 && error == 0 { self.start_transfer(slot, c, cmd_end, mem); }
    }

    fn start_transfer(&mut self, slot: usize, c: u32, cmd_end: u64, mem: &mut dyn SdmmcMemory) {
        let total = self.bytcnt;
        let block = (self.blksiz & 0xffff).max(1);
        let width = self.width(slot);
        self.data_due = if self.timed { cmd_end + self.data_ns(slot, total, block, c & CMD_WRITE != 0) } else { self.now_ns };
        let mismatch = self.device(slot).and_then(|d| d.bus_width()).is_some_and(|w| w != width);
        let write = c & CMD_WRITE != 0;
        self.xfer = Some(Transfer { slot, write, total, moved: 0, card_left: total, block, fifo: Vec::new(), auto_stop: c & CMD_AUTO_STOP != 0, desc: None, chain_end: false, suspended: false });
        // On other data lines than the card's every block fails its CRC: a card still sends
        // what it was asked for, and refuses what it is sent.
        if mismatch {
            if !write {
                let mut left = total;
                while left > 0 {
                    let mut b = vec![0u8; block.min(left) as usize];
                    let now = if self.timed { self.data_due } else { self.now_ns };
                    if self.device(slot).is_none_or(|d| d.read_data(now, &mut b).is_err()) { break; }
                    left -= b.len() as u32;
                }
            }
            self.data_error(int::DCRC);
            return;
        }
        self.pump(mem);
    }

    /// End the data phase with an error: the interrupt, DATA_OVER after it (the DesignWare
    /// core reports data-over after a data error too) and the IDMAC's card error summary.
    fn data_error(&mut self, bit: u32) {
        self.raise(self.data_due, bit | int::DATA_OVER);
        self.idsts |= idsts::CES | idsts::AIS;
        self.finish(false);
    }

    /// The end of the data phase. The auto-stop goes out after an error as well: an
    /// assumption, which the IDF driver relies on to get the card out of its data state.
    fn finish(&mut self, ok: bool) {
        let Some(x) = self.xfer.take() else { return };
        if ok { self.raise(self.data_due, int::DATA_OVER); }
        self.tcbcnt = x.total - x.card_left;
        self.tbbcnt = x.moved;
        if x.auto_stop {
            // On a timed bus the auto-stop follows the data phase; a multi-block write programs from it.
            let sent = if self.timed { self.data_due } else { self.now_ns };
            let r = self.device(x.slot).map_or(Err(SdCmdError::NoResponse), |d| d.command(sent, 12, 0));
            if let Ok(SdResponse::Short(v)) = r { self.resp[1] = v; self.resp_index = 12; }
            let at = if self.timed { sent + self.bus_ns(x.slot, 48 + 2 + 48 + 8) } else { self.now_ns };
            self.raise(at, int::ACD);
        }
        if x.write && self.ram.read(reg::CARDTHRCTL) & CARDTHRCTL_BUSY_CLR_INT_EN != 0 { self.bci_slot = Some(x.slot); self.busy_clear(); }
    }

    /// BCI at the first look after the card that took the last write releases DAT0 (the
    /// host has no clock of its own to raise it at that moment).
    fn busy_clear(&mut self) -> bool {
        let Some(slot) = self.bci_slot else { return false };
        let now = self.now_ns;
        let busy = self.device(slot).is_some_and(|d| d.busy(now));
        if !busy { self.bci_slot = None; self.rintsts |= int::BCI; self.read_irq = true; }
        busy
    }

    fn fault(&mut self, write: bool) {
        self.idsts |= idsts::FBE | idsts::AIS | (if write { 1 } else { 2 }) << idsts::FBE_CODE_SHIFT;
        self.xfer = None;
    }

    /// Move data until the transfer is done, the IDMAC suspends on a CPU-owned descriptor, or
    /// it faults.
    fn pump(&mut self, mem: &mut dyn SdmmcMemory) {
        if !self.idmac_on() { return; }
        let Some(mut x) = self.xfer.take() else { return };
        x.suspended = false;
        loop {
            if x.moved == x.total { self.xfer = Some(x); self.finish(true); return; }
            if x.desc.is_none() {
                // Past the last descriptor with bytes left: the FIFO stops, as on silicon.
                if x.chain_end { self.xfer = Some(x); return; }
                let a = self.cursor;
                let mut w = [0u8; 16];
                if mem.read(a, &mut w).is_err() { self.xfer = Some(x); self.fault(false); return; }
                let word = |i: usize| u32::from_le_bytes(w[4 * i..4 * i + 4].try_into().unwrap());
                let (d0, d1) = (word(0), word(1));
                self.dscaddr = a;
                if d0 & des0::OWN == 0 { self.idsts |= idsts::DU | idsts::AIS; x.suspended = true; self.xfer = Some(x); return; }
                let second = if d0 & des0::CH != 0 { 0 } else { d1 >> 13 & 0x1fff };
                x.desc = Some(Desc { addr: a, des0: d0, bufs: [(word(2), d1 & 0x1fff), (word(3), second)], which: 0, off: 0 });
            }
            let d = x.desc.as_mut().unwrap();
            let (base, size) = d.bufs[d.which];
            let room = size - d.off.min(size);
            if room == 0 {
                if d.which == 0 && d.bufs[1].1 > 0 { d.which = 1; d.off = 0; continue; }
                if !self.close(&mut x, mem) { return; }
                continue;
            }
            let n = room.min(x.total - x.moved);
            let at = base.wrapping_add(d.off);
            if x.write {
                let mut chunk = vec![0u8; n as usize];
                if mem.read(at, &mut chunk).is_err() { self.xfer = Some(x); self.fault(true); return; }
                x.fifo.extend_from_slice(&chunk);
                let (slot, now) = (x.slot, if self.timed { self.data_due } else { self.now_ns });
                while x.card_left > 0 && (x.fifo.len() as u32 >= x.block.min(x.card_left)) {
                    let k = x.block.min(x.card_left) as usize;
                    let r = match self.device(slot) { Some(dev) => dev.write_data(now, &x.fifo[..k]), None => Err(SdDataError::NoCrcStatus) };
                    if let Err(e) = r { self.xfer = Some(x); self.data_error(if e == SdDataError::Crc { int::DCRC } else { int::EBE }); return; }
                    x.fifo.drain(..k);
                    x.card_left -= k as u32;
                    self.stats.bytes_written += k as u64;
                }
            } else {
                while (x.fifo.len() as u32) < n && x.card_left > 0 {
                    let k = x.block.min(x.card_left) as usize;
                    let mut b = vec![0u8; k];
                    let (slot, now) = (x.slot, if self.timed { self.data_due } else { self.now_ns });
                    let r = match self.device(slot) { Some(dev) => dev.read_data(now, &mut b), None => Err(SdDataError::Timeout) };
                    if let Err(e) = r { self.xfer = Some(x); self.data_error(if e == SdDataError::Crc { int::DCRC } else { int::DRTO }); return; }
                    x.fifo.extend_from_slice(&b);
                    x.card_left -= k as u32;
                    self.stats.bytes_read += k as u64;
                }
                if mem.write(at, &x.fifo[..n as usize]).is_err() { self.xfer = Some(x); self.fault(false); return; }
                x.fifo.drain(..n as usize);
            }
            let d = x.desc.as_mut().unwrap();
            d.off += n;
            x.moved += n;
            self.bufaddr = at.wrapping_add(n);
            if x.moved == x.total && !self.close(&mut x, mem) { return; }
        }
    }

    /// Hand a finished descriptor back to the CPU (OWN cleared), raise TI or RI unless DIC is
    /// set, and step to the next one: DES3 when chained, DBADDR after the end of a ring, else
    /// the next in the list (16 bytes plus BMOD's skip length in words).
    fn close(&mut self, x: &mut Transfer, mem: &mut dyn SdmmcMemory) -> bool {
        let Some(d) = x.desc.take() else { return true };
        if mem.write(d.addr, &(d.des0 & !des0::OWN).to_le_bytes()).is_err() { self.fault(x.write); return false; }
        if d.des0 & des0::DIC == 0 { self.idsts |= if x.write { idsts::TI } else { idsts::RI } | idsts::NIS; }
        let mut next = [0u8; 4];
        self.cursor = if d.des0 & des0::CH != 0 {
            if mem.read(d.addr.wrapping_add(12), &mut next).is_err() { self.fault(x.write); return false; }
            u32::from_le_bytes(next)
        } else if d.des0 & des0::ER != 0 { self.dbaddr } else { d.addr.wrapping_add(16 + 4 * (self.bmod >> 2 & 0x1f)) };
        if d.des0 & des0::LD != 0 { x.chain_end = true; }
        true
    }

    fn status(&mut self) -> u32 {
        // FIFO empty (2) and below the transmit watermark (1): the model keeps no FIFO between
        // register accesses. Command FSM idle; DAT3 high with a device on the active slot.
        let slot = (self.cmd >> 16 & 0x1f) as usize;
        let now = self.now_ns;
        let (present, busy) = if self.bci_slot == Some(slot) { (true, self.busy_clear()) } else {
            match self.device(slot) { Some(d) => (true, d.busy(now)), None => (false, false) }
        };
        1 << 1 | 1 << 2 | (present as u32) << 8 | (busy as u32) << 9 | (self.resp_index & 0x3f) << 11
    }
}

impl Default for Sdmmc { fn default() -> Self { Self::new(SdmmcLayout::Esp32p4) } }

impl Device for Sdmmc {
    fn read(&mut self, off: u32) -> u32 {
        match off {
            reg::CTRL => self.ctrl,
            reg::CTYPE => self.ctype,
            reg::BLKSIZ => self.blksiz,
            reg::BYTCNT => self.bytcnt,
            reg::INTMASK => self.intmask,
            reg::CMDARG => self.cmdarg,
            reg::CMD => self.cmd,
            0x30..=0x3c => self.resp[((off - reg::RESP0) / 4) as usize],
            reg::MINTSTS => self.rintsts & self.intmask & int::ALL,
            reg::RINTSTS => self.rintsts,
            reg::STATUS => self.status(),
            reg::CDETECT => self.card_detect_n & 3,
            reg::WRTPRT => self.write_protect & 3,
            reg::TCBCNT => self.tcbcnt,
            reg::TBBCNT => self.tbbcnt,
            reg::VERID => VERID_P4,
            reg::HCON => HCON,
            // PBL (10:8) mirrors FIFOTH's DMA_MULTIPLE_TRANSACTION_SIZE (30:28)
            reg::BMOD => self.bmod | (self.ram.read(reg::FIFOTH) >> 28 & 7) << 8,
            reg::PLDMND => 0,
            reg::DBADDR => self.dbaddr,
            reg::IDSTS => self.idsts,
            reg::IDINTEN => self.idinten,
            reg::DSCADDR => self.dscaddr,
            reg::BUFADDR => self.bufaddr,
            // the data FIFO: PIO transfers are not modelled
            0x200..=0x3ff => 0,
            reg::RAW_INTS if matches!(self.layout, SdmmcLayout::Esp32p4 | SdmmcLayout::Esp32s31) => self.rintsts,
            // DLL_CAL_END: calibration finishes at once
            reg::DLL_CONF if self.layout == SdmmcLayout::Esp32p4 => self.ram.read(off) | 1 << 1,
            _ => self.ram.read(off),
        }
    }

    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        match off {
            reg::CTRL => {
                if v & (CTRL_CONTROLLER_RESET | CTRL_DMA_RESET) != 0 { self.xfer = None; self.pending = Pending::None; self.deferred.clear(); }
                self.ctrl = v & !(CTRL_CONTROLLER_RESET | CTRL_FIFO_RESET | CTRL_DMA_RESET);
            }
            reg::CTYPE => self.ctype = v & 0x3_0003,
            reg::BLKSIZ => self.blksiz = v & 0xffff,
            reg::BYTCNT => self.bytcnt = v,
            reg::INTMASK => self.intmask = v & int::ALL,
            reg::CMDARG => self.cmdarg = v,
            reg::CMD => {
                self.cmd = v;
                if v & CMD_START != 0 {
                    // A clock-update command only loads CLKDIV, CLKSRC and CLKENA into the card
                    // clock: taken at once, no CMD_DONE.
                    if v & CMD_UPDATE_CLK != 0 { self.cmd &= !CMD_START; } else { self.pending = Pending::Command; }
                }
            }
            0x30..=0x3c | reg::MINTSTS | reg::STATUS | reg::CDETECT | reg::WRTPRT | reg::TCBCNT | reg::TBBCNT | reg::VERID | reg::HCON | reg::DSCADDR | reg::BUFADDR => {}
            reg::RINTSTS => self.rintsts &= !(v & int::ALL),
            reg::BMOD => {
                if v & BMOD_SWR != 0 { self.idsts = 0; self.dscaddr = 0; self.bufaddr = 0; self.cursor = self.dbaddr; }
                self.bmod = v & !(BMOD_SWR | 7 << 8);
            }
            reg::PLDMND => if self.xfer.as_ref().is_some_and(|x| x.suspended) { self.pending = Pending::Resume; },
            reg::DBADDR => { self.dbaddr = v; self.cursor = v; }
            reg::IDSTS => self.idsts &= !(v & idsts::W1C),
            reg::IDINTEN => self.idinten = v & idsts::W1C,
            reg::RAW_INTS if matches!(self.layout, SdmmcLayout::Esp32p4 | SdmmcLayout::Esp32s31) => {}
            _ => self.ram.write(off, v),
        }
        WriteEffect::NONE
    }

    fn irq_sources(&self) -> u64 { self.irq() as u64 }

    fn debug(&mut self, on: bool) {
        self.log = on;
        for d in &mut self.devices { d.set_log(on); }
    }
}
