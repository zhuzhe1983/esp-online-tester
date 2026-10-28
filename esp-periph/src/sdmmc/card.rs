//! An SD memory card on an SD bus, after the SD Physical Layer Simplified Specification v9.10
//! (SD spec below): the card state machine of §4.2 and §4.3 with the transitions of §4.8, the
//! responses of §4.9, the card status of §4.10.1 and the registers of chapter 5. It is a high
//! capacity card (CSD version 2.0, block addressing, 512-byte blocks) of the image's size, in
//! default or high speed, 1- or 4-bit, without UHS-I, locking or write protection.
use super::{SdCmdError, SdDataError, SdDevice, SdResponse};
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};

pub mod spi;

pub const BLOCK: usize = 512;

/// The blocks behind a card.
pub trait SdStore {
    /// Image size in bytes.
    fn len(&self) -> u64;
    fn is_empty(&self) -> bool { self.len() == 0 }
    fn read(&mut self, lba: u64, buf: &mut [u8; BLOCK]) -> std::io::Result<()>;
    fn write(&mut self, lba: u64, data: &[u8; BLOCK]) -> std::io::Result<()>;
    /// Blocks `lo..=hi` read 0 from now on.
    fn erase(&mut self, lo: u64, hi: u64) -> std::io::Result<()> {
        for lba in lo..=hi { self.write(lba, &[0; BLOCK])?; }
        Ok(())
    }
    /// What the CLI prints about it.
    fn describe(&self) -> String { format!("{} MiB in memory", self.len() >> 20) }
}

/// An image held in memory (tests, the browser build).
pub struct MemStore(pub Vec<u8>);
impl SdStore for MemStore {
    fn len(&self) -> u64 { self.0.len() as u64 }
    fn read(&mut self, lba: u64, buf: &mut [u8; BLOCK]) -> std::io::Result<()> {
        let at = lba as usize * BLOCK;
        buf.copy_from_slice(self.0.get(at..at + BLOCK).ok_or(std::io::ErrorKind::UnexpectedEof)?);
        Ok(())
    }
    fn write(&mut self, lba: u64, data: &[u8; BLOCK]) -> std::io::Result<()> {
        let at = lba as usize * BLOCK;
        self.0.get_mut(at..at + BLOCK).ok_or(std::io::ErrorKind::UnexpectedEof)?.copy_from_slice(data);
        Ok(())
    }
    fn erase(&mut self, lo: u64, hi: u64) -> std::io::Result<()> {
        self.0.get_mut(lo as usize * BLOCK..(hi as usize + 1) * BLOCK).ok_or(std::io::ErrorKind::UnexpectedEof)?.fill(0);
        Ok(())
    }
}

/// An image file. Copy-on-write keeps written blocks, and erased ranges, in memory and never
/// modifies the file; write-through writes them to it.
pub struct FileStore { file: std::fs::File, len: u64, overlay: Option<HashMap<u64, Box<[u8; BLOCK]>>>, erased: Vec<(u64, u64)>, path: String }
impl FileStore {
    pub fn copy_on_write(path: &str) -> std::io::Result<Self> { Self::open(path, false) }
    pub fn write_through(path: &str) -> std::io::Result<Self> { Self::open(path, true) }
    fn open(path: &str, write: bool) -> std::io::Result<Self> {
        let file = std::fs::OpenOptions::new().read(true).write(write).open(path)?;
        let len = file.metadata()?.len();
        Ok(FileStore { file, len, overlay: (!write).then(HashMap::new), erased: Vec::new(), path: path.to_string() })
    }
}
impl SdStore for FileStore {
    fn len(&self) -> u64 { self.len }
    fn read(&mut self, lba: u64, buf: &mut [u8; BLOCK]) -> std::io::Result<()> {
        if let Some(b) = self.overlay.as_ref().and_then(|o| o.get(&lba)) { buf.copy_from_slice(&b[..]); return Ok(()); }
        if self.erased.iter().any(|&(lo, hi)| (lo..=hi).contains(&lba)) { buf.fill(0); return Ok(()); }
        self.file.seek(SeekFrom::Start(lba * BLOCK as u64))?;
        self.file.read_exact(buf)
    }
    fn write(&mut self, lba: u64, data: &[u8; BLOCK]) -> std::io::Result<()> {
        if let Some(o) = &mut self.overlay { o.insert(lba, Box::new(*data)); return Ok(()); }
        self.file.seek(SeekFrom::Start(lba * BLOCK as u64))?;
        self.file.write_all(data)
    }
    fn erase(&mut self, lo: u64, hi: u64) -> std::io::Result<()> {
        if let Some(o) = &mut self.overlay {
            o.retain(|lba, _| !(lo..=hi).contains(lba));
            self.erased.push((lo, hi));
            return Ok(());
        }
        let zeros = [0u8; 64 << 10];
        self.file.seek(SeekFrom::Start(lo * BLOCK as u64))?;
        let mut left = (hi - lo + 1) * BLOCK as u64;
        while left > 0 { let n = left.min(zeros.len() as u64) as usize; self.file.write_all(&zeros[..n])?; left -= n as u64; }
        Ok(())
    }
    fn describe(&self) -> String { format!("{} ({} MiB, {})", self.path, self.len >> 20, if self.overlay.is_some() { "copy-on-write" } else { "write-through" }) }
}

/// CURRENT_STATE of the card status (SD spec §4.10.1); `Ina` has no code.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CardState { Idle = 0, Ready = 1, Ident = 2, Stby = 3, Tran = 4, Data = 5, Rcv = 6, Prg = 7, Dis = 8, Ina = 15 }

/// Card status bits (SD spec §4.10.1).
pub mod status {
    pub const OUT_OF_RANGE: u32 = 1 << 31;
    pub const ADDRESS_ERROR: u32 = 1 << 30;
    pub const ERASE_SEQ_ERROR: u32 = 1 << 28;
    pub const ERASE_PARAM: u32 = 1 << 27;
    pub const ILLEGAL_COMMAND: u32 = 1 << 22;
    pub const CC_ERROR: u32 = 1 << 20;
    pub const ERROR: u32 = 1 << 19;
    pub const READY_FOR_DATA: u32 = 1 << 8;
    pub const APP_CMD: u32 = 1 << 5;
    /// Bits cleared by the response that reports them (clear condition C).
    pub const CLEAR_ON_READ: u32 = 0xfdf9_a028;
}
use status::*;

/// OCR (SD spec §5.1): 2.7-3.6 V, card capacity status, power-up done.
const OCR_VOLTAGE: u32 = 0x00ff_8000;
const OCR_CCS: u32 = 1 << 30;
const OCR_READY: u32 = 1 << 31;
/// CSD TRAN_SPEED (SD spec §5.3.3): 25 MHz, and 50 MHz once high speed is selected.
const TRAN_SPEED_DS: u32 = 0x32;
const TRAN_SPEED_HS: u32 = 0x5a;
/// CSD CCC: classes 0, 2, 4, 5, 8 and 10 (basic, block read, block write, erase,
/// application specific, switch), the ones this card implements.
const CCC: u32 = 0x535;
/// Any non-zero value; a card picks its own (SD spec §4.2.3).
const RCA: u16 = 0xe624;

/// What the card answers with, before it is put on the bus.
enum Reply { Silent, R1, R1b, R2(u128), R3(u32), R6, R7(u32) }

/// The data phase the card is in, if any.
enum Phase {
    None,
    /// `buf` goes out from `pos`; a multi-block read then loads `next` until `left` runs out.
    Read { buf: Vec<u8>, pos: usize, next: Option<u64>, left: Option<u32> },
    /// Blocks come in at `lba`; a single-block write ends after one.
    Write { lba: u64, buf: Vec<u8>, multi: bool, left: Option<u32> },
}

/// What makes one card differ from another.
#[derive(Clone, Debug)]
pub struct SdCardConfig {
    /// DAT0 busy after the last block of a write, after CMD12 ends a multi-block write, and
    /// after an erase, in nanoseconds of emulated time. 0: programming is instant.
    pub prg_ns: u64,
    /// ACMD41s answered busy before power-up is done.
    pub init_polls: u32,
    /// CID (SD spec §5.2): manufacturer ID, OEM/application ID, product name, revision, serial
    /// number (`None`: derived from the image size) and manufacturing year and month.
    pub mid: u8,
    pub oid: [u8; 2],
    pub name: [u8; 5],
    pub revision: u8,
    pub serial: Option<u32>,
    pub date: (u16, u8),
    /// the host's bus takes its time: commands and data at the card clock and width
    /// (`Sdmmc::timed`)
    pub timed_bus: bool,
}
impl Default for SdCardConfig {
    /// MID 0 (no manufacturer's), "ES", "SIMSD", revision 1.0, 2024-01.
    fn default() -> Self { SdCardConfig { prg_ns: 0, init_polls: 1, mid: 0, oid: *b"ES", name: *b"SIMSD", revision: 0x10, serial: None, date: (2024, 1), timed_bus: false } }
}

pub struct SdCard {
    store: Box<dyn SdStore>,
    pub config: SdCardConfig,
    /// Addressable 512-byte blocks: the image rounded down to the 512 KiB unit of C_SIZE.
    blocks: u64,
    state: CardState,
    rca: u16,
    status: u32,
    app_cmd: bool,
    /// a valid CMD8 since the last reset: the host speaks SD 2.00 and may set HCS
    if_cond: bool,
    init_left: u32,
    high_speed: bool,
    wide: bool,
    block_count: Option<u32>,
    erase: (Option<u64>, Option<u64>),
    data: Phase,
    written: u32,
    /// end of the programming period in progress, in nanoseconds
    busy_until: u64,
    serial: u32,
    pub log: bool,
    pub commands: u64,
    pub blocks_read: u64,
    pub blocks_written: u64,
}

impl SdCard {
    /// A card for the image; it must hold at least one 512 KiB capacity unit.
    pub fn new(store: Box<dyn SdStore>) -> Result<Self, String> { Self::with_config(store, SdCardConfig::default()) }

    pub fn with_config(store: Box<dyn SdStore>, config: SdCardConfig) -> Result<Self, String> {
        let units = store.len() / (512 << 10);
        if units == 0 { return Err(format!("an SD card image needs at least 512 KiB, this one has {} bytes", store.len())); }
        if units > 1 << 22 { return Err("an SD card image is limited to 2 TiB (CSD C_SIZE)".into()); }
        let blocks = units * 1024;
        // Without one configured, a serial number that follows the image, so two images read
        // as two cards.
        let serial = config.serial.unwrap_or((blocks as u32).wrapping_mul(0x9e37_79b9) ^ 0x5d1c_0b17);
        let init_left = config.init_polls;
        Ok(SdCard { store, config, blocks, state: CardState::Idle, rca: 0, status: 0, app_cmd: false, if_cond: false, init_left, high_speed: false, wide: false,
                    block_count: None, erase: (None, None), data: Phase::None, written: 0, busy_until: 0, serial, log: false, commands: 0, blocks_read: 0, blocks_written: 0 })
    }
    pub fn state(&self) -> CardState { self.state }
    pub fn blocks(&self) -> u64 { self.blocks }
    pub fn rca(&self) -> u16 { self.rca }
    pub fn store(&mut self) -> &mut dyn SdStore { &mut *self.store }
    pub fn describe(&self) -> String { self.store.describe() }
    pub fn serial(&self) -> u32 { self.serial }
    /// Above 32 GiB the card is SDXC, below it SDHC; there is no SDSC (byte-addressed) card.
    pub fn sdxc(&self) -> bool { self.blocks > 32 << 21 }

    /// CID (SD spec §5.2), from the configuration.
    pub fn cid(&self) -> u128 {
        let c = &self.config;
        let mut v = (c.mid as u128) << 120 | (u16::from_be_bytes(c.oid) as u128) << 104;
        for (i, &ch) in c.name.iter().enumerate() { v |= (ch as u128) << (96 - 8 * i); }
        let date = ((c.date.0.saturating_sub(2000) as u128 & 0xff) << 4) | (c.date.1 as u128 & 0xf);
        v |= (c.revision as u128) << 56 | (self.serial as u128) << 24 | date << 8;
        with_crc7(v)
    }

    /// CSD version 2.0 (SD spec §5.3.3): TAAC 1 ms, NSAC 0, 512-byte blocks,
    /// C_SIZE from the image, erase by block, R2W_FACTOR 4.
    pub fn csd(&self) -> u128 {
        let c_size = (self.blocks / 1024 - 1) as u128;
        let speed = if self.high_speed { TRAN_SPEED_HS } else { TRAN_SPEED_DS } as u128;
        let v = 1u128 << 126 | 0x0e << 112 | speed << 96 | (CCC as u128) << 84 | 9 << 80 | c_size << 48 | 1 << 46 | 0x7f << 39 | 2 << 26 | 9 << 22;
        with_crc7(v)
    }

    /// SCR (SD spec §5.6): structure 1.0, SD_SPEC 2 with SD_SPEC3 (version 3.0x),
    /// security 2.00 (SDHC) or 3.00 (SDXC), 1- and 4-bit, CMD23 supported.
    fn scr(&self) -> Vec<u8> { vec![0x02, (if self.sdxc() { 4 } else { 3 }) << 4 | 0x5, 0x80, 0x02, 0, 0, 0, 0] }

    /// SD status (SD spec §4.10.2): bus width, speed class 10, AU 4 MiB; the erase
    /// estimate fields 0, which tells the host to use its own timeout.
    fn ssr(&self) -> Vec<u8> {
        let mut s = vec![0u8; 64];
        set_bits(&mut s, 511, 510, if self.wide { 2 } else { 0 });
        set_bits(&mut s, 447, 440, 0x04);
        set_bits(&mut s, 431, 428, 0x9);
        s
    }

    /// The 64-byte switch function status (SD spec §4.3.10). Group 1 (access mode)
    /// offers default and high speed; the other groups their default function only.
    fn switch_status(&mut self, arg: u32) -> Vec<u8> {
        let set = arg & 1 << 31 != 0;
        let supported = |g: u32, f: u32| f == 0 || (g == 1 && f == 1);
        let mut chosen = [0u32; 7];
        let mut ok = true;
        for g in 1..=6u32 {
            let f = arg >> (4 * (g - 1)) & 0xf;
            chosen[g as usize] = if f == 0xf { if g == 1 { self.high_speed as u32 } else { 0 } } else if supported(g, f) { f } else { ok = false; 0xf };
        }
        if set && ok { self.high_speed = chosen[1] == 1; }
        let mut s = vec![0u8; 64];
        set_bits(&mut s, 511, 496, 100);
        for g in 1..=6u32 {
            let lo = 400 + 16 * (g - 1);
            set_bits(&mut s, lo + 15, lo, if g == 1 { 0x8003 } else { 0x8001 });
            let sel = 376 + 4 * (g - 1);
            set_bits(&mut s, sel + 3, sel, chosen[g as usize]);
        }
        set_bits(&mut s, 375, 368, 1);
        s
    }

    fn reset(&mut self) {
        self.state = CardState::Idle;
        self.rca = 0;
        self.status = 0;
        self.app_cmd = false;
        self.if_cond = false;
        self.init_left = self.config.init_polls;
        self.high_speed = false;
        self.wide = false;
        self.block_count = None;
        self.erase = (None, None);
        self.data = Phase::None;
        self.busy_until = 0;
    }

    /// The card status word of an R1 response: CURRENT_STATE is the state the command found
    /// the card in (SD spec §4.10.1).
    fn card_status(&self, at: CardState) -> u32 {
        let ready = if at == CardState::Prg { 0 } else { READY_FOR_DATA };
        self.status | (at as u32 & 0xf) << 9 | ready
    }

    /// Programming from `now` for the configured time (SD spec §4.3.4): prg, then tran.
    fn program(&mut self, now: u64) {
        self.busy_until = now.saturating_add(self.config.prg_ns);
        self.state = if self.config.prg_ns > 0 { CardState::Prg } else { CardState::Tran };
    }

    /// When programming has ended: prg goes to tran, dis (deselected while programming) to
    /// stby (SD spec §4.8).
    fn settle(&mut self, now: u64) {
        if now < self.busy_until { return; }
        match self.state { CardState::Prg => self.state = CardState::Tran, CardState::Dis => self.state = CardState::Stby, _ => {} }
    }

    fn load(&mut self, lba: u64) -> Option<Vec<u8>> {
        let mut b = [0u8; BLOCK];
        match self.store.read(lba, &mut b) {
            Ok(()) => { self.blocks_read += 1; Some(b.to_vec()) }
            Err(_) => { self.status |= CC_ERROR | ERROR; None }
        }
    }

    /// SD spec §4.8 (the card state transitions) and §4.7.4 (the commands).
    fn exec(&mut self, now: u64, cmd: u8, arg: u32, app: bool) -> Result<Reply, ()> {
        use CardState::*;
        let st = self.state;
        let mine = arg >> 16 == self.rca as u32;
        let tran = st == Tran;
        if app && matches!(cmd, 6 | 13 | 22 | 23 | 41 | 42 | 51) { self.status |= APP_CMD; }
        if app {
            match cmd {
                // ACMD6 SET_BUS_WIDTH: 00 one line, 10 four
                6 if tran => { match arg & 3 { 0 => self.wide = false, 2 => self.wide = true, _ => self.status |= ERROR } return Ok(Reply::R1); }
                // ACMD13 SD_STATUS, ACMD22 SEND_NUM_WR_BLOCKS, ACMD51 SEND_SCR: one block of data
                13 if tran => { self.data = Phase::Read { buf: self.ssr(), pos: 0, next: None, left: None }; self.state = Data; return Ok(Reply::R1); }
                22 if tran => { self.data = Phase::Read { buf: self.written.to_be_bytes().to_vec(), pos: 0, next: None, left: None }; self.state = Data; return Ok(Reply::R1); }
                51 if tran => { self.data = Phase::Read { buf: self.scr(), pos: 0, next: None, left: None }; self.state = Data; return Ok(Reply::R1); }
                // ACMD23 SET_WR_BLK_ERASE_COUNT (a pre-erase hint), ACMD42 SET_CLR_CARD_DETECT
                23 | 42 if tran => return Ok(Reply::R1),
                // ACMD41 SD_SEND_OP_COND (SD spec §4.2.3): an empty voltage window only asks; a
                // high capacity card stays busy for a host without HCS or without CMD8.
                41 if st == Idle => {
                    if arg & OCR_VOLTAGE == 0 { return Ok(Reply::R3(OCR_VOLTAGE)); }
                    if arg & OCR_CCS == 0 || !self.if_cond { return Ok(Reply::R3(OCR_VOLTAGE)); }
                    if self.init_left > 0 { self.init_left -= 1; return Ok(Reply::R3(OCR_VOLTAGE)); }
                    self.state = Ready;
                    return Ok(Reply::R3(OCR_READY | OCR_CCS | OCR_VOLTAGE));
                }
                // An application command index the card does not define is taken as the
                // standard command of that index (SD spec §4.3.9.1).
                _ => {}
            }
        }
        match cmd {
            // CMD0 GO_IDLE_STATE
            0 => { self.reset(); Ok(Reply::Silent) }
            // CMD2 ALL_SEND_CID
            2 if st == Ready => { self.state = Ident; Ok(Reply::R2(self.cid())) }
            // CMD3 SEND_RELATIVE_ADDR
            3 if st == Ident || st == Stby => { self.rca = RCA; self.state = Stby; Ok(Reply::R6) }
            // CMD4 SET_DSR: no DSR (CSD DSR_IMP 0), nothing to set
            4 if st == Stby => Ok(Reply::Silent),
            // CMD6 SWITCH_FUNC
            6 if tran => { let s = self.switch_status(arg); self.data = Phase::Read { buf: s, pos: 0, next: None, left: None }; self.state = Data; Ok(Reply::R1) }
            // CMD7 SELECT/DESELECT_CARD: a card not addressed deselects without answering; dis
            // is only left for prg while the card still programs (`settle` ends it at stby)
            7 => match (mine, st) {
                (true, Stby) => { self.state = Tran; Ok(Reply::R1b) }
                (true, Dis) => { self.state = Prg; Ok(Reply::R1b) }
                (false, Tran | Data) => { self.data = Phase::None; self.state = Stby; Ok(Reply::Silent) }
                (false, Prg) => { self.state = Dis; Ok(Reply::Silent) }
                (false, Stby | Dis) => Ok(Reply::Silent),
                _ => Err(()),
            },
            // CMD8 SEND_IF_COND: 2.7-3.6 V accepted, the check pattern echoed; anything else unanswered
            8 if st == Idle => {
                if arg >> 8 & 0xf != 1 { return Ok(Reply::Silent); }
                self.if_cond = true;
                Ok(Reply::R7(1 << 8 | (arg & 0xff)))
            }
            // CMD9 SEND_CSD, CMD10 SEND_CID
            9 if st == Stby => Ok(if mine { Reply::R2(self.csd()) } else { Reply::Silent }),
            10 if st == Stby => Ok(if mine { Reply::R2(self.cid()) } else { Reply::Silent }),
            // CMD12 STOP_TRANSMISSION
            12 if st == Data => { self.data = Phase::None; self.state = Tran; Ok(Reply::R1b) }
            12 if st == Rcv => { self.data = Phase::None; self.program(now); Ok(Reply::R1b) }
            // CMD13 SEND_STATUS
            13 if matches!(st, Stby | Tran | Data | Rcv | Prg | Dis) => Ok(if mine { Reply::R1 } else { Reply::Silent }),
            // CMD15 GO_INACTIVE_STATE
            15 if mine && st != Idle && st != Ready && st != Ident => { self.state = Ina; Ok(Reply::Silent) }
            // CMD16 SET_BLOCKLEN: read and write blocks are 512 bytes on this card regardless
            16 if tran => Ok(Reply::R1),
            // CMD17 READ_SINGLE_BLOCK, CMD18 READ_MULTIPLE_BLOCK: block address; out of range
            // answers OUT_OF_RANGE and sends nothing
            17 | 18 if tran => {
                let lba = arg as u64;
                let count = self.block_count.take();
                if lba >= self.blocks { self.status |= OUT_OF_RANGE; return Ok(Reply::R1); }
                let Some(buf) = self.load(lba) else { return Ok(Reply::R1) };
                let (next, left) = if cmd == 18 { (Some(lba + 1), count.map(|n| n.saturating_sub(1))) } else { (None, None) };
                self.data = Phase::Read { buf, pos: 0, next, left };
                self.state = Data;
                Ok(Reply::R1)
            }
            // CMD23 SET_BLOCK_COUNT (for the next CMD18 or CMD25)
            23 if tran => { self.block_count = Some(arg); Ok(Reply::R1) }
            // CMD24 WRITE_BLOCK, CMD25 WRITE_MULTIPLE_BLOCK
            24 | 25 if tran => {
                let lba = arg as u64;
                let count = self.block_count.take();
                if lba >= self.blocks { self.status |= OUT_OF_RANGE; return Ok(Reply::R1); }
                let left = if cmd == 25 { count } else { None };
                self.written = 0;
                self.data = Phase::Write { lba, buf: Vec::with_capacity(BLOCK), multi: cmd == 25, left };
                self.state = Rcv;
                Ok(Reply::R1)
            }
            // CMD32 ERASE_WR_BLK_START, CMD33 ERASE_WR_BLK_END, CMD38 ERASE (SD spec §4.3.5):
            // erased blocks read 0 (SCR DATA_STAT_AFTER_ERASE 0)
            32 | 33 if tran => {
                if arg as u64 >= self.blocks { self.status |= OUT_OF_RANGE; return Ok(Reply::R1); }
                if cmd == 32 { self.erase = (Some(arg as u64), None) } else { self.erase.1 = Some(arg as u64) }
                Ok(Reply::R1)
            }
            38 if tran => {
                let (Some(lo), Some(hi)) = std::mem::take(&mut self.erase) else { self.status |= ERASE_SEQ_ERROR; return Ok(Reply::R1b) };
                if hi < lo { self.status |= ERASE_PARAM; return Ok(Reply::R1b); }
                if self.store.erase(lo, hi).is_err() { self.status |= CC_ERROR | ERROR; }
                self.program(now);
                Ok(Reply::R1b)
            }
            // CMD55 APP_CMD: the next command is an application command
            55 if !matches!(st, Ready | Ident | Ina) => {
                if !mine && st != Idle { return Ok(Reply::Silent); }
                self.app_cmd = true;
                self.status |= APP_CMD;
                Ok(Reply::R1)
            }
            // Everything else, including CMD5/52/53 (SDIO, SD spec §4.7.4 "reserved for I/O
            // mode"), CMD11 (no 1.8 V) and the classes not in CCC, is illegal in SD mode.
            _ => Err(()),
        }
    }

    /// R1, R1b and R6 report the card status and clear its reported bits; R2 carries CID or
    /// CSD, R3 the OCR and R7 the interface condition (SD spec §4.9).
    fn respond(&mut self, at: CardState, reply: Reply) -> Option<SdResponse> {
        let s = self.card_status(at);
        let r = match reply {
            Reply::Silent => return None,
            Reply::R2(v) => return Some(SdResponse::Long([v as u32, (v >> 32) as u32, (v >> 64) as u32, (v >> 96) as u32])),
            Reply::R3(v) | Reply::R7(v) => return Some(SdResponse::Short(v)),
            Reply::R1 | Reply::R1b => s,
            // R6 (SD spec §4.9.5): the new RCA and status bits 23, 22, 19 and 12:0
            Reply::R6 => (self.rca as u32) << 16 | (s >> 8 & 0xc000) | (s >> 6 & 0x2000) | (s & 0x1fff),
        };
        self.status &= !CLEAR_ON_READ;
        Some(SdResponse::Short(r))
    }
}

impl SdDevice for SdCard {
    fn command(&mut self, now: u64, cmd: u8, arg: u32) -> Result<SdResponse, SdCmdError> {
        self.commands += 1;
        self.settle(now);
        if self.state == CardState::Ina { return Err(SdCmdError::NoResponse); }
        let at = self.state;
        let app = std::mem::take(&mut self.app_cmd);
        if !app { self.status &= !APP_CMD; }
        let r = match self.exec(now, cmd, arg, app) {
            Ok(reply) => self.respond(at, reply).ok_or(SdCmdError::NoResponse),
            // SD spec §4.6.1: an illegal command is not answered and shows in the next response.
            Err(()) => { self.status |= ILLEGAL_COMMAND; Err(SdCmdError::NoResponse) }
        };
        if self.log { eprintln!("[sd] {}CMD{} arg {:#010x} in {:?} -> {:?}, now {:?}", if app { "A" } else { "" }, cmd, arg, at, r, self.state); }
        r
    }

    fn read_data(&mut self, _now: u64, out: &mut [u8]) -> Result<(), SdDataError> {
        let mut done = 0;
        while done < out.len() {
            let Phase::Read { buf, pos, next, left } = &mut self.data else { return Err(SdDataError::Timeout) };
            if *pos == buf.len() {
                let lba = match (*next, *left) { (Some(lba), l) if l != Some(0) => lba, _ => return Err(SdDataError::Timeout) };
                if lba >= self.blocks { self.status |= OUT_OF_RANGE; return Err(SdDataError::Timeout); }
                let Some(b) = self.load(lba) else { return Err(SdDataError::Timeout) };
                let Phase::Read { buf, pos, next, left } = &mut self.data else { unreachable!() };
                *buf = b; *pos = 0; *next = Some(lba + 1);
                if let Some(n) = left { *n -= 1; }
                continue;
            }
            let n = (out.len() - done).min(buf.len() - *pos);
            out[done..done + n].copy_from_slice(&buf[*pos..*pos + n]);
            *pos += n;
            done += n;
            // A single block, a register, or the last block CMD23 asked for: back to Tran.
            if *pos == buf.len() && (next.is_none() || *left == Some(0)) { self.data = Phase::None; self.state = CardState::Tran; break; }
        }
        if done < out.len() { return Err(SdDataError::Timeout); }
        Ok(())
    }

    fn write_data(&mut self, now: u64, data: &[u8]) -> Result<(), SdDataError> {
        let mut rest = data;
        while !rest.is_empty() {
            let Phase::Write { lba, buf, multi, left } = &mut self.data else { return Err(SdDataError::NoCrcStatus) };
            let n = rest.len().min(BLOCK - buf.len());
            buf.extend_from_slice(&rest[..n]);
            rest = &rest[n..];
            if buf.len() < BLOCK { continue; }
            if *lba >= self.blocks { self.status |= OUT_OF_RANGE; self.data = Phase::None; self.state = CardState::Tran; return Err(SdDataError::NoCrcStatus); }
            let block: [u8; BLOCK] = buf[..].try_into().unwrap();
            let (at, multi) = (*lba, *multi);
            buf.clear();
            *lba += 1;
            let last = if let Some(n) = left { *n = n.saturating_sub(1); *n == 0 } else { !multi };
            if self.store.write(at, &block).is_err() { self.status |= CC_ERROR | ERROR; self.data = Phase::None; self.state = CardState::Tran; return Err(SdDataError::Crc); }
            self.blocks_written += 1;
            self.written += 1;
            if last { self.data = Phase::None; self.program(now); }
        }
        Ok(())
    }

    fn busy(&self, now: u64) -> bool { self.state == CardState::Prg && now < self.busy_until }
    fn busy_until(&self) -> Option<u64> { (self.state == CardState::Prg).then_some(self.busy_until) }
    fn bus_width(&self) -> Option<u8> { Some(if self.wide { 4 } else { 1 }) }
    fn name(&self) -> &str { "SD card" }
    fn set_log(&mut self, on: bool) { self.log = on; }
}

/// Bits `hi..=lo` of a big-endian register image, bit 0 being the last byte's LSB.
fn set_bits(buf: &mut [u8], hi: u32, lo: u32, v: u32) {
    let total = buf.len() as u32 * 8;
    for (k, bit) in (lo..=hi).enumerate() {
        let byte = ((total - 1 - bit) / 8) as usize;
        if v >> k & 1 != 0 { buf[byte] |= 1 << (bit % 8); } else { buf[byte] &= !(1 << (bit % 8)); }
    }
}

/// CID and CSD carry a CRC7 over bits 127:8 in bits 7:1 and a 1 in bit 0 (SD spec §4.5, §5.2).
fn with_crc7(v: u128) -> u128 {
    let bytes = v.to_be_bytes();
    (v & !0xff) | (crc7(&bytes[..15]) as u128) << 1 | 1
}

/// x^7 + x^3 + 1 (SD spec §4.5).
pub fn crc7(data: &[u8]) -> u8 {
    let mut crc = 0u8;
    for &b in data {
        for i in (0..8).rev() {
            let bit = (b >> i) & 1 ^ (crc >> 6) & 1;
            crc = (crc << 1) & 0x7f;
            if bit != 0 { crc ^= 0x09; }
        }
    }
    crc
}
