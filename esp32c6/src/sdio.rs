//! The ESP32-C6 SDIO slave: HINF, SLC and SLCHOST (0x60016000-0x60018FFF, IDF v5.5.3
//! components/soc/esp32c6/register/soc/reg_base.h:35-37), as ESP-IDF's sdio_slave driver programs
//! them (components/esp_driver_sdio/src/sdio_slave.c, hal/sdio_slave_hal.c,
//! hal/esp32c6/include/hal/sdio_slave_ll.h), and the I/O card a host sees on the other end of the
//! bus. The host side follows the SDIO Simplified Specification v3.00 (SD Specifications Part E1,
//! 2011): CMD5 and R4 (§3.2, §3.3), CMD3's R6 (§4.3), the I/O reset (§4.4), CMD52 and R5
//! (§5.1, §5.2), CMD53 (§5.3), the CCCR (§6.9), the FBR (§6.10), the bus states (§6.14), the
//! interrupt (§6.3, §8.1) and the CIS tuples (chapter 16). What sits behind function 1 is
//! Espressif's protocol (IDF v5.5.3 docs/en/api-reference/protocols/esp_sdio_slave_protocol.rst):
//! SLCHOST's registers at function 1 addresses 0x000-0x3FF and the data FIFO above them, where a
//! transfer to address A carries a packet of 0x1F800 - A bytes.
//!
//! The device sees one offset space: HINF at 0x0000, SLC at 0x1000, SLCHOST at 0x2000. The SLC's
//! "rx" link carries slave-to-host data (the driver's sending), its "tx" link host-to-slave data
//! (receiving). The DMA walks descriptors in SRAM (`DmaMem`) when the C6 starts a link and when
//! the host moves data, which is the moment the bytes cross the bus.
use emu_core::Fault;
use esp_periph::sdmmc::{SdCmdError, SdDataError, SdResponse};
use esp_periph::{Device, RegRam, WriteEffect};

/// Block offsets in this device's space.
pub const HINF: u32 = 0x0000;
pub const SLC: u32 = 0x1000;
pub const HOST: u32 = 0x2000;

/// SLC registers (soc/esp32c6/register/soc/sdio_slc_reg.h).
pub mod slc {
    pub const CONF0: u32 = 0x00; pub const INT_RAW: u32 = 0x04; pub const INT_ST: u32 = 0x08; pub const INT_ENA: u32 = 0x0c;
    pub const INT_CLR: u32 = 0x10; pub const RX_LINK: u32 = 0x3c; pub const RX_LINK_ADDR: u32 = 0x40; pub const TX_LINK: u32 = 0x44;
    pub const TX_LINK_ADDR: u32 = 0x48; pub const INTVEC_TOHOST: u32 = 0x5c; pub const TOKEN0: u32 = 0x60; pub const TOKEN1: u32 = 0x64;
    pub const RX_DSCR_CONF: u32 = 0xa8; pub const LEN_CONF: u32 = 0xf4; pub const LENGTH: u32 = 0xf8;
    /// SLCCONF0: SLC0_TX_RST, SLC0_RX_RST, SLC0_RX_AUTO_WRBACK.
    pub const TX_RST: u32 = 1 << 0; pub const RX_RST: u32 = 1 << 1; pub const RX_AUTO_WRBACK: u32 = 1 << 6;
    /// SLC0RX_LINK / SLC0TX_LINK: STOP, START, RESTART.
    pub const LINK_STOP: u32 = 1 << 28; pub const LINK_START: u32 = 1 << 29; pub const LINK_RESTART: u32 = 1 << 30;
    /// SLC0TOKENn: WDATA 11:0, WR 12, INC 13, INC_MORE 14; the count reads at 27:16.
    pub const TOKEN_WR: u32 = 1 << 12; pub const TOKEN_INC: u32 = 1 << 13; pub const TOKEN_INC_MORE: u32 = 1 << 14;
    /// SLC0_LEN_CONF: LEN_WDATA 19:0, LEN_WR 20, LEN_INC 21, LEN_INC_MORE 22, TX_NEW_PKT_IND 28 (reset 1).
    pub const LEN_WR: u32 = 1 << 20; pub const LEN_INC: u32 = 1 << 21; pub const LEN_INC_MORE: u32 = 1 << 22; pub const TX_NEW_PKT_IND: u32 = 1 << 28;
}
/// SLC0INT_RAW bits, the C6's side (sdio_slc_reg.h:262-462).
pub mod sint {
    pub const FRHOST: u32 = 0xff; pub const TX_OVF: u32 = 1 << 11; pub const TX_DONE: u32 = 1 << 14; pub const TX_SUC_EOF: u32 = 1 << 15;
    pub const RX_DONE: u32 = 1 << 16; pub const RX_EOF: u32 = 1 << 17;
}
/// SLCHOST registers (soc/esp32c6/register/soc/sdio_slc_host_reg.h, sdio_slc_host_struct.h).
pub mod host {
    pub const TOKEN_RDATA: u32 = 0x44; pub const INT_RAW: u32 = 0x50; pub const INT_ST: u32 = 0x58; pub const PKT_LEN: u32 = 0x60;
    pub const CONF_W7: u32 = 0x8c; pub const INT_CLR: u32 = 0xd4; pub const FUNC1_INT_ENA: u32 = 0xdc;
}
/// SLC0HOST_INT_RAW bits, the host's side (sdio_slc_host_reg.h:155-341).
pub mod hint {
    pub const TOHOST: u32 = 0xff; pub const RX_UDF: u32 = 1 << 16; pub const TX_OVF: u32 = 1 << 17; pub const NEW_PACKET: u32 = 1 << 23;
}
/// HINF_CFG_DATA0 reset value: DEVICE_ID_FN1 0x6666, USER_ID_FN1 0x0092 (sdio_hinf_reg.h:17-30).
pub const CFG_DATA0_RESET: u32 = 0x0092_6666;
/// HINF_CFG_DATA1 reset value: SDIO_ENABLE, SDIO_CD_ENABLE, SDIO_VER 0x232 (sdio_hinf_reg.h:36-150).
pub const CFG_DATA1_RESET: u32 = 1 | 1 << 4 | 0x232 << 12;
/// CFG_DATA1 bits the hardware drives: HIGHSPEED_MODE, IOENABLE2, CD_DISABLE, FUNC1_EPS, EMP,
/// IOENABLE1, FUNC2_EPS.
const CFG_DATA1_RO: u32 = 1 << 3 | 1 << 7 | 1 << 8 | 1 << 9 | 1 << 10 | 1 << 11 | 1 << 24;
const SDIO_ENABLE: u32 = 1 << 0; const IOREADY1: u32 = 1 << 1; const HIGHSPEED_ENABLE: u32 = 1 << 2; const IOREADY2: u32 = 1 << 5; const SDIO_INT_MASK: u32 = 1 << 6;

/// The end of the function 1 FIFO window: a transfer to address A carries 0x1F800 - A bytes of a
/// packet (esp_sdio_slave_protocol.rst, "FIFO").
pub const FIFO_END: u32 = 0x1_f800;
/// Function 1 addresses below this reach SLCHOST's registers; ESP-Hosted and esp_serial_slave_link
/// mask register addresses with 0x3FF (esp_hosted 2.12.13 host/drivers/transport/sdio/sdio_reg.h,
/// ESP_ADDRESS_MASK).
pub const FIFO_START: u32 = 0x400;
/// The common CIS: what IDF's `sdmmc_io_print_cis_info` printed for an ESP32-C6 slave (esp_hosted
/// 2.12.13 docs/esp32_p4_function_ev_board.md, "Checking ESP-Hosted"). MANFID is CFG_DATA0's.
pub const CIS_ADDR: u32 = 0x1000;
const CIS_TAIL: [u8; 28] = [0x21, 0x02, 0x0c, 0x00, 0x22, 0x04, 0x00, 0x00, 0x02, 0x32, 0x1a, 0x05, 0x01, 0x01, 0x00, 0x02, 0x07,
                            0x1b, 0x08, 0xc1, 0x41, 0x30, 0x30, 0xff, 0xff, 0xff, 0xff, 0xff];
/// CCCR card capability (0x08): SDC, SMB, SRW, S4MI; full speed (§6.9 Table 6-2). Not read
/// from silicon.
pub const CARD_CAP: u8 = 0x17;
/// R4: two I/O functions (HINF has IOREADY1/2 and IOENABLE1/2), no memory, 2.7-3.6 V (§3.3).
pub const IO_FUNCTIONS: u32 = 2;
pub const IO_OCR: u32 = 0x00ff_8000;
/// R5 response flags (§5.2.1 Table 5-1): IO_CURRENT_STATE CMD and TRN, FUNCTION_NUMBER, OUT_OF_RANGE.
const R5_CMD: u32 = 0x10 << 8; const R5_TRN: u32 = 0x20 << 8; const R5_FUNCTION: u32 = 0x02 << 8; const R5_RANGE: u32 = 0x01 << 8;

/// Descriptors one host transfer may walk: a chain that loops back on itself ends there.
const MAX_DESCS: usize = 4096;

/// What the SLC DMA reaches: the C6's SRAM.
pub trait DmaMem {
    fn read(&mut self, addr: u32, buf: &mut [u8]) -> Result<(), Fault>;
    fn write(&mut self, addr: u32, data: &[u8]) -> Result<(), Fault>;
    fn read32(&mut self, addr: u32) -> Option<u32> { let mut b = [0; 4]; self.read(addr, &mut b).ok().map(|_| u32::from_le_bytes(b)) }
    fn write32(&mut self, addr: u32, v: u32) -> bool { self.write(addr, &v.to_le_bytes()).is_ok() }
}

/// A slice of memory at a base address.
pub struct Sram<'a> { pub base: u32, pub bytes: &'a mut [u8] }
impl Sram<'_> {
    fn span(&self, addr: u32, len: usize) -> Option<std::ops::Range<usize>> {
        let o = addr.checked_sub(self.base)? as usize;
        let end = o.checked_add(len)?;
        (end <= self.bytes.len()).then_some(o..end)
    }
}
impl DmaMem for Sram<'_> {
    fn read(&mut self, addr: u32, buf: &mut [u8]) -> Result<(), Fault> {
        let r = self.span(addr, buf.len()).ok_or(Fault::Unmapped)?;
        buf.copy_from_slice(&self.bytes[r]);
        Ok(())
    }
    fn write(&mut self, addr: u32, data: &[u8]) -> Result<(), Fault> {
        let r = self.span(addr, data.len()).ok_or(Fault::Unmapped)?;
        self.bytes[r].copy_from_slice(data);
        Ok(())
    }
}

/// An SLC descriptor (sdio_slave_ll.h, `sdio_slave_ll_desc_t`): size 13:0, length 27:14, a
/// software bit 28, sub-frame start 29, EOF 30, owner 31 (1: the hardware's); the buffer; the next.
#[derive(Clone, Copy, Debug)]
struct Desc { w0: u32, buf: u32, next: u32 }
impl Desc {
    const OWNER: u32 = 1 << 31; const EOF: u32 = 1 << 30;
    fn load(mem: &mut dyn DmaMem, addr: u32) -> Option<Desc> {
        if addr == 0 { return None; }
        let mut b = [0u8; 12];
        mem.read(addr, &mut b).ok()?;
        let w = |i: usize| u32::from_le_bytes(b[4 * i..4 * i + 4].try_into().unwrap());
        Some(Desc { w0: w(0), buf: w(1), next: w(2) })
    }
    fn size(&self) -> u32 { self.w0 & 0x3fff }
    fn length(&self) -> u32 { self.w0 >> 14 & 0x3fff }
    fn owned(&self) -> bool { self.w0 & Self::OWNER != 0 }
    fn eof(&self) -> bool { self.w0 & Self::EOF != 0 }
}

/// The slave-to-host link: the descriptor being sent and how much of it the host has taken.
#[derive(Default, Debug)]
struct RxLink { running: bool, cur: u32, off: u32 }
/// The host-to-slave link: the descriptor being filled, how full it is, and the last one closed,
/// whose next pointer a RESTART follows.
#[derive(Default, Debug)]
struct TxLink { running: bool, cur: u32, fill: u32, last: u32 }

/// The I/O card's bus state (§6.14 Figure 6-2). `Inactive`: no CMD5 since power-on or the I/O
/// reset, so nothing but CMD5 is answered (§3.1.2).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum CardState { #[default] Inactive, Init, Standby, Command }

/// A CMD53 data phase: what is left of it, where it points, and for the FIFO how many bytes of the
/// packet are still to cross.
#[derive(Clone, Copy, Debug)]
struct Xfer { write: bool, func: u8, addr: u32, incr: bool, left: u32, packet: u32 }

/// What the end of a data phase changes on the C6's side.
#[derive(Clone, Copy, Debug)]
enum Effect { DescWord0 { at: u32, w0: u32 }, Int(u32) }

#[derive(Clone, Copy, Default, Debug)]
pub struct SdioStats {
    pub commands: u64, pub to_host: u64, pub from_host: u64, pub underflows: u64, pub overflows: u64,
    /// host reads of INT_RAW showing RX_NEW_PACKET while PKT_LEN still held the length the host
    /// last read: a packet announced before its length (a host PKT_LEN poll between the length
    /// write and the INT_RAW read also counts, so this is an upper bound)
    pub stale_new_packets: u64,
}

pub struct SdioSlave {
    cfg_data0: u32,
    cfg_data1: u32,
    hinf: RegRam,
    conf0: u32,
    int_raw: u32,
    int_ena: u32,
    token: [u32; 2],
    len: u32,
    rx: RxLink,
    tx: TxLink,
    /// an rx START or RESTART waits for the DMA to read the descriptor (`run_dma`)
    rx_kick: bool,
    /// an rx START found a packet: the host's RX_NEW_PACKET waits for the length that follows
    new_packet_armed: bool,
    /// PKT_LEN as the host last read it
    host_len: u32,
    /// while set, a data phase's hand-back (descriptor word 0, the C6's interrupt bits) waits for
    /// this machine cycle, when the bus has carried the data (`deferred`)
    defer_to: Option<u64>,
    deferred: Vec<(u64, Effect)>,
    slc: RegRam,
    host_raw: u32,
    host_ena: u32,
    host_ram: RegRam,
    /// the card the host sees
    pub state: CardState,
    rca: u16,
    ioe: u8,
    ien: u8,
    bus_ctrl: u8,
    ehs: bool,
    blksz: [u16; 3],
    xfer: Option<Xfer>,
    pub stats: SdioStats,
    /// TOKEN1 as the host reads it, held at this value (a test's fault: the receive buffers look
    /// exhausted while the driver goes on loading them)
    pub token1_hold: Option<u32>,
    /// receive buffers the host has filled since the driver last wrote TOKEN1
    tx_closed: u32,
    token1_base: u32,
    log: bool,
}

impl Default for SdioSlave { fn default() -> Self { Self::new() } }

impl SdioSlave {
    pub fn new() -> Self {
        let mut slc = RegRam::new();
        // SLC_RX_DSCR_CONF: INFOR_NO_REPLACE and RX_EOF_MODE (pop) reset to 1 (sdio_slc_reg.h:2675-2700).
        slc.write(slc::RX_DSCR_CONF, 1 << 1 | 1 << 3);
        SdioSlave {
            cfg_data0: CFG_DATA0_RESET, cfg_data1: CFG_DATA1_RESET, hinf: RegRam::new(), conf0: 0, int_raw: 0, int_ena: 0, token: [0; 2], len: 0,
            rx: RxLink::default(), tx: TxLink::default(), rx_kick: false, new_packet_armed: false, host_len: 0, defer_to: None, deferred: Vec::new(), slc, host_raw: 0, host_ena: 0, host_ram: RegRam::new(),
            state: CardState::Inactive, rca: 0, ioe: 0, ien: 0, bus_ctrl: 0, ehs: false, blksz: [512; 3], xfer: None,
            stats: SdioStats::default(), token1_hold: None, tx_closed: 0, token1_base: 0, log: false,
        }
    }

    /// The SLC0 interrupt (ETS_SLC0_INTR_SOURCE): SLC0INT_ST.
    pub fn irq(&self) -> bool { self.int_raw & self.int_ena != 0 }

    /// Function 1's interrupt is pending: SLC0HOST_INT_ST (INT_RAW under FUNC1_INT_ENA).
    fn func1_pending(&self) -> bool { self.host_raw & self.host_ena != 0 }

    /// DAT1 low: function 1 pending with IEN1 and IENM set (§6.3), unless HINF masks it.
    pub fn host_interrupt(&self) -> bool {
        self.cfg_data1 & SDIO_ENABLE != 0 && self.cfg_data1 & SDIO_INT_MASK == 0 && self.ien & 0x03 == 0x03 && self.func1_pending()
    }

    /// Bytes left in the CMD53 data phase in progress.
    pub fn transfer_left(&self) -> u32 { self.xfer.map_or(0, |x| x.left) }
    /// SLC0TOKEN1's count, as the driver has it.
    pub fn token1(&self) -> u32 { self.token[1] }
    /// TOKEN1 as an exhausted slave would show it: every buffer loaded since the driver last wrote
    /// TOKEN1 filled, none free.
    pub fn token1_exhausted(&self) -> u32 { self.token1_base.wrapping_add(self.tx_closed) & 0xfff }

    /// The data lines the card drives: 4 after CCCR Bus Width 10b, else 1 (§6.9).
    pub fn bus_width(&self) -> u8 { if self.bus_ctrl & 3 == 2 { 4 } else { 1 } }

    /// Work a register write left for the DMA.
    #[inline]
    pub fn dma_pending(&self) -> bool { self.rx_kick }

    /// An rx START or RESTART: the DMA fetches the descriptor. One the hardware owns is a packet
    /// to send: SLC0_RX_DONE once it is loaded, which the driver waits for with a one-byte
    /// descriptor (hal/sdio_slave_hal.c, `send_isr_invoker_enable`). The host's RX_NEW_PACKET
    /// waits for the PKT_LEN write the driver makes after the START (`send_new_packet`): raised at
    /// the START, the host's combined register read can see it with the old length. Guessed.
    pub fn run_dma(&mut self, mem: &mut dyn DmaMem) {
        if !std::mem::take(&mut self.rx_kick) || !self.rx.running { return; }
        if let Some(d) = Desc::load(mem, self.rx.cur) {
            if d.owned() {
                self.int_raw |= sint::RX_DONE;
                self.new_packet_armed = true;
                if self.log { eprintln!("[sdio] rx link at {:#010x}: {} bytes{}", self.rx.cur, d.length(), if d.eof() { ", EOF" } else { "" }); }
            }
        }
    }

    /// Apply `e` now, or queue it for the end of the data phase in progress.
    fn effect(&mut self, mem: &mut dyn DmaMem, e: Effect) {
        match self.defer_to {
            Some(at) => self.deferred.push((at, e)),
            None => self.apply(mem, e),
        }
    }

    fn apply(&mut self, mem: &mut dyn DmaMem, e: Effect) {
        match e {
            Effect::DescWord0 { at, w0 } => { mem.write32(at, w0); }
            Effect::Int(bits) => self.int_raw |= bits,
        }
    }

    /// The data calls that follow, until `None`, end on the bus at machine cycle `at`: their
    /// hand-back waits for it. The bytes themselves move at once; the buffers they fill or drain
    /// are the hardware's until then.
    pub fn defer_to(&mut self, at: Option<u64>) { self.defer_to = at; }

    /// The machine cycle the next queued hand-back is due.
    pub fn next_due(&self) -> Option<u64> { self.deferred.first().map(|d| d.0) }

    /// Apply the hand-backs due at or before machine cycle `now`, in the order they were queued.
    pub fn apply_due(&mut self, now: u64, mem: &mut dyn DmaMem) -> bool {
        let n = self.deferred.iter().take_while(|d| d.0 <= now).count();
        for (_, e) in self.deferred.drain(..n).collect::<Vec<_>>() { self.apply(mem, e); }
        n > 0
    }

    fn reset_rx(&mut self) { self.rx = RxLink::default(); self.rx_kick = false; self.new_packet_armed = false; }
    fn reset_tx(&mut self) { self.tx = TxLink::default(); }

    /// The I/O reset (CCCR RES, §4.4): the card's side back to its power-on state, CMD5 needed
    /// again. The SLC side and SLCHOST's counters stay as the C6's driver left them: the driver
    /// resets the counters itself and the host is to ask it to (sdio_slave.c, header comment;
    /// ESP-Hosted's slave does on the host's reset interrupt, `sdio_reset`).
    fn io_reset(&mut self) {
        (self.state, self.rca, self.ioe, self.ien, self.ehs, self.xfer) = (CardState::Inactive, 0, 0, 0, false, None);
        self.bus_ctrl &= 0x80;                        // CD Disable is not affected by a reset (§6.9)
        self.blksz = [512; 3];
    }

    // ---------------------------------------------------------------- the host's side
    /// A command on CMD and the card's answer (§6.14: which state takes which command).
    pub fn command(&mut self, cmd: u8, arg: u32, mem: &mut dyn DmaMem) -> Result<SdResponse, SdCmdError> {
        self.stats.commands += 1;
        if self.cfg_data1 & SDIO_ENABLE == 0 { return Err(SdCmdError::NoResponse); }
        let r = match (cmd, self.state) {
            (5, CardState::Inactive | CardState::Init) => {
                // §3.1.2: CMD5 with arg 0 asks for the OCR without starting initialization.
                if arg & IO_OCR != 0 { self.state = CardState::Init; }
                Ok(SdResponse::Short(1 << 31 | IO_FUNCTIONS << 28 | IO_OCR))
            }
            (3, CardState::Init | CardState::Standby) => {
                self.rca = 0x0001;
                self.state = CardState::Standby;
                Ok(SdResponse::Short((self.rca as u32) << 16))         // §4.3 Table 4-4: no errors, bits 12:0 zero
            }
            (7, CardState::Standby | CardState::Command) => {
                if arg >> 16 == self.rca as u32 && self.rca != 0 {
                    let was = self.state;
                    self.state = CardState::Command;
                    // R1b: the state as the command found it, stby 3 or tran 4, ready for data
                    Ok(SdResponse::Short(if was == CardState::Standby { 3 << 9 } else { 4 << 9 } | 1 << 8))
                } else { self.state = CardState::Standby; Err(SdCmdError::NoResponse) }
            }
            (52, CardState::Command) => Ok(SdResponse::Short(self.cmd52(arg, mem))),
            (53, CardState::Command) => Ok(SdResponse::Short(self.cmd53(arg))),
            _ => Err(SdCmdError::NoResponse),
        };
        if self.log { eprintln!("[sdio] CMD{} {:#010x} -> {:?} ({:?})", cmd, arg, r, self.state); }
        r
    }

    /// CMD52 (§5.1): [31] write, [30:28] function, [27] read after write, [25:9] address, [7:0] data.
    fn cmd52(&mut self, arg: u32, mem: &mut dyn DmaMem) -> u32 {
        let (write, func, raw, addr, data) = (arg >> 31 != 0, (arg >> 28 & 7) as u8, arg >> 27 & 1 != 0, arg >> 9 & 0x1_ffff, arg as u8);
        if func as u32 > IO_FUNCTIONS { return R5_CMD | R5_FUNCTION; }
        let out = if write {
            self.io_write(func, addr, data, mem);
            if raw { self.host_read(func, addr, mem) } else { data }
        } else { self.host_read(func, addr, mem) };
        R5_CMD | out as u32
    }

    /// CMD53 (§5.3): [31] write, [30:28] function, [27] block mode, [26] incrementing, [25:9]
    /// address, [8:0] byte or block count (0: 512 bytes; infinite blocks are not modelled).
    fn cmd53(&mut self, arg: u32) -> u32 {
        let (write, func, block, incr, addr, count) = (arg >> 31 != 0, (arg >> 28 & 7) as u8, arg >> 27 & 1 != 0, arg >> 26 & 1 != 0, arg >> 9 & 0x1_ffff, arg & 0x1ff);
        if func as u32 > IO_FUNCTIONS { return R5_CMD | R5_FUNCTION; }
        let left = if block { count * self.blksz[func as usize] as u32 } else if count == 0 { 512 } else { count };
        if left == 0 { return R5_CMD | R5_RANGE; }
        let packet = if func == 1 && addr >= FIFO_START { FIFO_END.saturating_sub(addr) } else { 0 };
        self.xfer = Some(Xfer { write, func, addr, incr, left, packet });
        R5_TRN
    }

    /// The card-to-host bytes of the CMD53 in progress.
    pub fn read_data(&mut self, buf: &mut [u8], mem: &mut dyn DmaMem) -> Result<(), SdDataError> {
        let Some(mut x) = self.xfer.filter(|x| !x.write) else { return Err(SdDataError::Timeout) };
        let n = (buf.len() as u32).min(x.left) as usize;
        if x.func == 1 && x.addr >= FIFO_START {
            let take = (n as u32).min(x.packet) as usize;
            let got = self.rx_pull(mem, &mut buf[..take]);
            // Reading past the link: zeros, RX_UDF, and the CMD53 succeeds. Guessed; silicon may
            // stall the bus and retry instead (SLC0_HOST_RD_RETRY_INT, SLC0_RD_RETRY_THRESHOLD).
            if got < take { self.host_raw |= hint::RX_UDF; self.stats.underflows += 1; }
            buf[got..].fill(0);                       // past the packet: zero filled
            x.packet -= take as u32;
            self.stats.to_host += got as u64;
        } else {
            for b in buf[..n].iter_mut() { *b = self.host_read(x.func, x.addr, mem); if x.incr { x.addr += 1; } }
            buf[n..].fill(0);
        }
        x.left -= n as u32;
        self.xfer = (x.left > 0).then_some(x);
        Ok(())
    }

    /// The host-to-card bytes of the CMD53 in progress.
    pub fn write_data(&mut self, data: &[u8], mem: &mut dyn DmaMem) -> Result<(), SdDataError> {
        let Some(mut x) = self.xfer.filter(|x| x.write) else { return Err(SdDataError::NoCrcStatus) };
        let n = (data.len() as u32).min(x.left) as usize;
        if x.func == 1 && x.addr >= FIFO_START {
            let take = (n as u32).min(x.packet) as usize;
            if take > 0 {
                x.packet -= take as u32;
                self.tx_push(mem, &data[..take], x.packet == 0);
                self.stats.from_host += take as u64;
            }
        } else {
            for &b in &data[..n] { self.io_write(x.func, x.addr, b, mem); if x.incr { x.addr += 1; } }
        }
        x.left -= n as u32;
        self.xfer = (x.left > 0).then_some(x);
        Ok(())
    }

    /// Bytes of the packets on the rx link, as far as there are owned descriptors. A descriptor the
    /// host has taken goes back (owner cleared when RX_AUTO_WRBACK is set) with RX_DONE, and
    /// RX_EOF after the one that ends a packet: the driver's "send finished" (rx EOF counted at the
    /// host's pop, SLC_RX_DSCR_CONF.RX_EOF_MODE 1).
    fn rx_pull(&mut self, mem: &mut dyn DmaMem, out: &mut [u8]) -> usize {
        let (mut n, mut steps) = (0, 0);
        while n < out.len() && self.rx.running && steps < MAX_DESCS {
            steps += 1;
            let Some(d) = Desc::load(mem, self.rx.cur).filter(|d| d.owned()) else { break };
            let k = (d.length().saturating_sub(self.rx.off) as usize).min(out.len() - n);
            if k > 0 && mem.read(d.buf.wrapping_add(self.rx.off), &mut out[n..n + k]).is_err() { out[n..n + k].fill(0); }
            n += k;
            self.rx.off += k as u32;
            if self.rx.off >= d.length() {
                if self.conf0 & slc::RX_AUTO_WRBACK != 0 { self.effect(mem, Effect::DescWord0 { at: self.rx.cur, w0: d.w0 & !Desc::OWNER }); }
                self.effect(mem, Effect::Int(sint::RX_DONE | if d.eof() { sint::RX_EOF } else { 0 }));
                if self.log && d.eof() { eprintln!("[sdio] rx EOF at {:#010x}", self.rx.cur); }
                (self.rx.cur, self.rx.off) = (d.next, 0);
            }
        }
        n
    }

    /// The descriptor the tx link fills next: the current one, or after the end of the list the
    /// next pointer of the last one closed, which the driver has extended and RESTARTed since.
    fn tx_desc(&mut self, mem: &mut dyn DmaMem) -> Option<Desc> {
        if !self.tx.running { return None; }
        if self.tx.cur == 0 && self.tx.last != 0 { self.tx.cur = Desc::load(mem, self.tx.last).map_or(0, |d| d.next); }
        Desc::load(mem, self.tx.cur).filter(|d| d.owned())
    }

    /// A packet's bytes into the receive buffers; `end` closes it with EOF. Without a buffer the
    /// bytes are lost (TX_OVF to both sides) and the CMD53 still succeeds: guessed, as the host
    /// is expected to wait for TOKEN1 and never get here.
    fn tx_push(&mut self, mem: &mut dyn DmaMem, mut data: &[u8], end: bool) {
        let mut steps = 0;
        while !data.is_empty() && steps < MAX_DESCS {
            steps += 1;
            let Some(d) = self.tx_desc(mem) else {
                self.host_raw |= hint::TX_OVF; self.effect(mem, Effect::Int(sint::TX_OVF)); self.stats.overflows += 1;
                if self.log { eprintln!("[sdio] tx overflow: {} bytes dropped", data.len()); }
                return;
            };
            let k = (d.size().saturating_sub(self.tx.fill) as usize).min(data.len());
            if k == 0 { self.tx_close(mem, d, false); continue; }
            if mem.write(d.buf.wrapping_add(self.tx.fill), &data[..k]).is_err() { self.effect(mem, Effect::Int(sint::TX_OVF)); }
            self.tx.fill += k as u32;
            data = &data[k..];
            if self.tx.fill >= d.size() && !(data.is_empty() && end) { self.tx_close(mem, d, false); }
        }
        if end { if let Some(d) = self.tx_desc(mem) { self.tx_close(mem, d, true); } }
    }

    /// Hand a filled receive buffer to the driver: its length, EOF, owner cleared; TX_DONE, and
    /// TX_SUC_EOF at a packet's end (hal/sdio_slave_hal.c, `sdio_slave_hal_recv_has_next_item`).
    fn tx_close(&mut self, mem: &mut dyn DmaMem, d: Desc, eof: bool) {
        let w0 = (d.w0 & (0x3fff | 3 << 28)) | (self.tx.fill & 0x3fff) << 14 | if eof { Desc::EOF } else { 0 };
        self.effect(mem, Effect::DescWord0 { at: self.tx.cur, w0 });
        self.effect(mem, Effect::Int(sint::TX_DONE | if eof { sint::TX_SUC_EOF } else { 0 }));
        if self.log { eprintln!("[sdio] tx buffer {:#010x}: {} bytes{}", self.tx.cur, self.tx.fill, if eof { ", EOF" } else { "" }); }
        (self.tx.last, self.tx.cur, self.tx.fill) = (self.tx.cur, d.next, 0);
        self.tx_closed = self.tx_closed.wrapping_add(1);
    }

    /// A byte the host reads with CMD52 or a CMD53 below the FIFO, with the bookkeeping behind
    /// `SdioStats::stale_new_packets`. A CMD52 into the FIFO takes the packet's next byte.
    fn host_read(&mut self, func: u8, addr: u32, mem: &mut dyn DmaMem) -> u8 {
        if func == 1 {
            if addr >= FIFO_START {
                let mut b = [0u8];
                if self.rx_pull(mem, &mut b) == 0 { self.host_raw |= hint::RX_UDF; self.stats.underflows += 1; }
                else { self.stats.to_host += 1; }
                return b[0];
            }
            if addr == host::INT_RAW + 2 && self.host_raw & hint::NEW_PACKET != 0 && self.len == self.host_len { self.stats.stale_new_packets += 1; }
            if addr == host::PKT_LEN { self.host_len = self.len; }
        }
        self.io_read(func, addr)
    }

    /// A byte of a function's register space.
    fn io_read(&self, func: u8, addr: u32) -> u8 {
        match func {
            0 => self.cia_read(addr),
            1 if addr < FIFO_START => (self.host_word(addr & !3) >> ((addr & 3) * 8)) as u8,
            _ => 0,
        }
    }

    fn io_write(&mut self, func: u8, addr: u32, v: u8, mem: &mut dyn DmaMem) {
        match func {
            0 => self.cia_write(addr, v),
            1 if addr < FIFO_START => self.host_write_byte(addr, v),
            // one byte of a packet of 0x1F800 - addr bytes: the last one closes it
            1 if addr < FIFO_END => { self.tx_push(mem, &[v], addr == FIFO_END - 1); self.stats.from_host += 1; }
            _ => {}
        }
    }

    /// The Common I/O Area of function 0: CCCR (§6.9 Table 6-1), FBR (§6.10 Table 6-3), CIS.
    fn cia_read(&self, addr: u32) -> u8 {
        match addr {
            // HINF SDIO_VER 0x232 read as CCCR 2.00, SDIO 2.00, SD 2.00 (guessed split of the field)
            0x00 => (self.cfg_data1 >> 12) as u8,
            0x01 => (self.cfg_data1 >> 20 & 0xf) as u8,
            0x02 => self.ioe,
            0x03 => self.ior(),
            0x04 => self.ien,
            0x05 => (self.func1_pending() as u8) << 1,
            0x07 => self.bus_ctrl,
            0x08 => CARD_CAP,
            0x09..=0x0b => (CIS_ADDR >> (8 * (addr - 0x09))) as u8,
            0x10 | 0x11 => (self.blksz[0] >> (8 * (addr - 0x10))) as u8,
            0x13 => (self.cfg_data1 & HIGHSPEED_ENABLE != 0) as u8 | (self.ehs as u8) << 1,
            0x110 | 0x111 => (self.blksz[1] >> (8 * (addr - 0x110))) as u8,
            0x210 | 0x211 => (self.blksz[2] >> (8 * (addr - 0x210))) as u8,
            a if (CIS_ADDR..CIS_ADDR + 5 + 6).contains(&a) => {
                let id = self.cfg_data0;
                [0x01, 0x03, 0xd9, 0x01, 0xff, 0x20, 0x04, (id >> 16) as u8, (id >> 24) as u8, id as u8, (id >> 8) as u8][(a - CIS_ADDR) as usize]
            }
            a if (CIS_ADDR + 11..CIS_ADDR + 11 + CIS_TAIL.len() as u32).contains(&a) => CIS_TAIL[(a - CIS_ADDR - 11) as usize],
            _ => 0,
        }
    }

    fn cia_write(&mut self, addr: u32, v: u8) {
        match addr {
            0x02 => self.ioe = v & 0x06,
            0x04 => self.ien = v & 0x07,
            0x06 => {
                if v & 0x08 != 0 { self.io_reset(); if self.log { eprintln!("[sdio] I/O reset"); } }
                else if v & 7 != 0 && self.xfer.is_some_and(|x| x.func == v & 7) { self.xfer = None; }   // abort (§4.9)
            }
            0x07 => self.bus_ctrl = v & 0xe3,
            0x10 => self.blksz[0] = (self.blksz[0] & 0xff00) | v as u16,
            0x11 => self.blksz[0] = (self.blksz[0] & 0x00ff) | (v as u16) << 8,
            0x13 => self.ehs = v & 2 != 0 && self.cfg_data1 & HIGHSPEED_ENABLE != 0,
            0x110 => self.blksz[1] = (self.blksz[1] & 0xff00) | v as u16,
            0x111 => self.blksz[1] = (self.blksz[1] & 0x00ff) | (v as u16) << 8,
            0x210 => self.blksz[2] = (self.blksz[2] & 0xff00) | v as u16,
            0x211 => self.blksz[2] = (self.blksz[2] & 0x00ff) | (v as u16) << 8,
            _ => {}
        }
    }

    /// I/O Ready: a function is ready once the host enabled it and the driver set its IOREADY.
    fn ior(&self) -> u8 {
        ((self.ioe & 2 != 0 && self.cfg_data1 & IOREADY1 != 0) as u8) << 1 | ((self.ioe & 4 != 0 && self.cfg_data1 & IOREADY2 != 0) as u8) << 2
    }

    /// An SLCHOST register as both sides read it.
    fn host_word(&self, off: u32) -> u32 {
        match off {
            host::TOKEN_RDATA => (self.token1_hold.unwrap_or(self.token[1]) & 0xfff) << 16 | (self.token[0] & 0xfff),
            host::INT_RAW => self.host_raw,
            host::INT_ST => self.host_raw & self.host_ena,
            // LEN_CHECK (31:20) reads 0: what the hardware puts there is not documented
            host::PKT_LEN => self.len & 0xf_ffff,
            host::CONF_W7 => self.host_ram.read(off) & !0xff,  // the host's interrupt byte clears itself
            host::INT_CLR => 0,
            host::FUNC1_INT_ENA => self.host_ena,
            _ => self.host_ram.read(off),
        }
    }

    /// A byte the host writes through function 1. CONF_W7's low byte interrupts the slave
    /// (SLC_FRHOST_BIT0-7; esp_serial_slave_link 1.1.2 essl_sdio.c:497, "W7 is reserved for
    /// interrupts").
    fn host_write_byte(&mut self, addr: u32, b: u8) {
        let (off, sh) = (addr & 0x3fc, (addr & 3) * 8);
        let bits = (b as u32) << sh;
        match off {
            host::INT_CLR => self.host_raw &= !bits,
            host::CONF_W7 if sh == 0 => { self.int_raw |= b as u32; if self.log { eprintln!("[sdio] host interrupts the slave: {:#04x}", b); } }
            host::TOKEN_RDATA | host::INT_RAW | host::INT_ST | host::PKT_LEN => {}
            host::FUNC1_INT_ENA => self.host_ena = (self.host_ena & !(0xff << sh)) | bits,
            _ => { let w = self.host_ram.read(off); self.host_ram.write(off, (w & !(0xff << sh)) | bits); }
        }
    }

    // ---------------------------------------------------------------- the C6's side
    fn slc_read(&self, off: u32) -> u32 {
        match off {
            slc::CONF0 => self.conf0,
            slc::INT_RAW => self.int_raw,
            slc::INT_ST => self.int_raw & self.int_ena,
            slc::INT_ENA => self.int_ena,
            slc::INT_CLR | slc::INTVEC_TOHOST => 0,
            slc::RX_LINK | slc::TX_LINK => self.slc.read(off) & !(slc::LINK_STOP | slc::LINK_START | slc::LINK_RESTART),
            slc::TOKEN0 => (self.token[0] & 0xfff) << 16,
            slc::TOKEN1 => (self.token[1] & 0xfff) << 16,
            slc::LEN_CONF => slc::TX_NEW_PKT_IND,
            slc::LENGTH => self.len & 0xf_ffff,
            _ => self.slc.read(off),
        }
    }

    fn slc_write(&mut self, off: u32, v: u32) {
        match off {
            slc::CONF0 => {
                if v & slc::TX_RST != 0 { self.reset_tx(); }
                if v & slc::RX_RST != 0 { self.reset_rx(); }
                self.conf0 = v;
            }
            slc::INT_RAW | slc::INT_ST => {}
            slc::INT_ENA => self.int_ena = v,
            slc::INT_CLR => self.int_raw &= !v,
            slc::RX_LINK => {
                if v & slc::LINK_STOP != 0 { self.rx.running = false; self.new_packet_armed = false; }
                if v & slc::LINK_START != 0 { self.rx = RxLink { running: true, cur: self.slc.read(slc::RX_LINK_ADDR), off: 0 }; self.rx_kick = true; }
                if v & slc::LINK_RESTART != 0 && self.rx.running { self.rx_kick = true; }
                self.slc.write(off, v);
            }
            slc::TX_LINK => {
                if v & slc::LINK_STOP != 0 { self.tx.running = false; }
                if v & slc::LINK_START != 0 { self.tx = TxLink { running: true, cur: self.slc.read(slc::TX_LINK_ADDR), fill: 0, last: 0 }; }
                if v & slc::LINK_RESTART != 0 { self.tx.running = true; }
                self.slc.write(off, v);
            }
            slc::INTVEC_TOHOST => self.host_raw |= v & hint::TOHOST,
            slc::TOKEN0 | slc::TOKEN1 => {
                let t = &mut self.token[((off - slc::TOKEN0) / 4) as usize];
                let wdata = v & 0xfff;
                if v & slc::TOKEN_WR != 0 { *t = wdata; }
                if v & slc::TOKEN_WR != 0 && off == slc::TOKEN1 { (self.token1_base, self.tx_closed) = (wdata, 0); }
                if v & slc::TOKEN_INC != 0 { *t = (*t + 1) & 0xfff; }
                if v & slc::TOKEN_INC_MORE != 0 { *t = (*t + wdata) & 0xfff; }
            }
            slc::LEN_CONF => {
                let wdata = v & 0xf_ffff;
                if v & slc::LEN_WR != 0 { self.len = wdata; }
                if v & slc::LEN_INC != 0 { self.len = (self.len + 1) & 0xf_ffff; }
                if v & slc::LEN_INC_MORE != 0 { self.len = (self.len + wdata) & 0xf_ffff; }
                if v & (slc::LEN_WR | slc::LEN_INC | slc::LEN_INC_MORE) != 0 && std::mem::take(&mut self.new_packet_armed) {
                    self.host_raw |= hint::NEW_PACKET;
                }
            }
            _ => self.slc.write(off, v),
        }
    }

    fn hinf_read(&self, off: u32) -> u32 {
        match off {
            0x00 => self.cfg_data0,
            0x04 => {
                let ro = (self.ehs as u32) << 3 | ((self.ioe & 4 != 0) as u32) << 7 | ((self.bus_ctrl & 0x80 != 0) as u32) << 8 | ((self.ioe & 2 != 0) as u32) << 11;
                (self.cfg_data1 & !CFG_DATA1_RO) | ro
            }
            _ => self.hinf.read(off),
        }
    }

    fn hinf_write(&mut self, off: u32, v: u32) {
        match off { 0x00 => self.cfg_data0 = v, 0x04 => self.cfg_data1 = v & !CFG_DATA1_RO, _ => self.hinf.write(off, v) }
    }

    fn host_write(&mut self, off: u32, v: u32) {
        match off {
            host::INT_CLR => self.host_raw &= !v,
            host::FUNC1_INT_ENA => self.host_ena = v,
            host::TOKEN_RDATA | host::INT_RAW | host::INT_ST | host::PKT_LEN => {}
            _ => self.host_ram.write(off, v),
        }
    }
}

impl Device for SdioSlave {
    fn read(&mut self, off: u32) -> u32 {
        match off & 0x3000 { HINF => self.hinf_read(off & 0xfff), SLC => self.slc_read(off & 0xfff), _ => self.host_word(off & 0xfff) }
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        match off & 0x3000 { HINF => self.hinf_write(off & 0xfff, v), SLC => self.slc_write(off & 0xfff, v), _ => self.host_write(off & 0xfff, v) }
        WriteEffect::NONE
    }
    fn irq_sources(&self) -> u64 { self.irq() as u64 }
    fn debug(&mut self, on: bool) { self.log = on; }
}

/// The host's access to the slave through the C6's bus: the DMA reaches SRAM, and the SLC0 line
/// is re-derived before the C6's next instruction.
impl crate::bus::SocBus {
    fn product_sdio_connected(&self)->bool {
        self.board.name()!="m5-stamp-addon-c6" || (18..=23).all(|pin|esp_soc::pins::ChipPins::C6.routes(&self.periph.gpio,&self.periph.io_mux).input_function(pin,0))
    }
    pub fn sdio_command(&mut self, cmd: u8, arg: u32) -> Result<SdResponse, SdCmdError> {
        if !self.product_sdio_connected() {return Err(SdCmdError::NoResponse);}
        let mut mem = Sram { base: crate::bus::SRAM_LOW, bytes: &mut self.sram };
        let r = self.periph.optional.sdio.command(cmd, arg, &mut mem);
        self.sdio_settle();
        r
    }
    /// The next card-to-host bytes. `done` is the machine cycle the bus finishes carrying them:
    /// the DMA's hand-back to the C6 waits for it when it is still to come.
    pub fn sdio_read_data(&mut self, buf: &mut [u8], done: u64) -> Result<(), SdDataError> {
        if !self.product_sdio_connected() {return Err(SdDataError::Timeout);}
        let mut mem = Sram { base: crate::bus::SRAM_LOW, bytes: &mut self.sram };
        let sdio = &mut self.periph.optional.sdio;
        sdio.defer_to((done > self.cycles).then_some(done));
        let r = sdio.read_data(buf, &mut mem);
        sdio.defer_to(None);
        self.sdio_settle();
        r
    }
    /// The next host-to-card bytes, `done` as for `sdio_read_data`.
    pub fn sdio_write_data(&mut self, data: &[u8], done: u64) -> Result<(), SdDataError> {
        if !self.product_sdio_connected() {return Err(SdDataError::Timeout);}
        let mut mem = Sram { base: crate::bus::SRAM_LOW, bytes: &mut self.sram };
        let sdio = &mut self.periph.optional.sdio;
        sdio.defer_to((done > self.cycles).then_some(done));
        let r = sdio.write_data(data, &mut mem);
        sdio.defer_to(None);
        self.sdio_settle();
        r
    }
    /// The machine cycle a queued hand-back is due.
    pub fn sdio_next_due(&self) -> Option<u64> { self.periph.optional.sdio.next_due() }
    /// Carry out every queued hand-back, as if the time had passed (a co-processor whose CPU has
    /// stopped while its DMA goes on).
    #[cold]
    #[inline(never)]
    pub fn sdio_apply_all(&mut self) {
        let mut mem = Sram { base: crate::bus::SRAM_LOW, bytes: &mut self.sram };
        if self.periph.optional.sdio.apply_due(u64::MAX, &mut mem) { self.sdio_settle(); }
    }
    /// Carry out the hand-backs that have come due.
    #[cold]
    #[inline(never)]
    pub fn sdio_apply_due(&mut self) {
        let mut mem = Sram { base: crate::bus::SRAM_LOW, bytes: &mut self.sram };
        if self.periph.optional.sdio.apply_due(self.cycles, &mut mem) { self.sdio_settle(); }
    }
    /// DAT1 as the slave drives it: low (true) while function 1 interrupts the host.
    pub fn sdio_interrupt(&self) -> bool { self.periph.optional.sdio.host_interrupt() }
    pub fn sdio_bus_width(&self) -> u8 { self.periph.optional.sdio.bus_width() }

    /// After a write to SLC: the DMA reads the descriptor a link START pointed it at.
    #[cold]
    #[inline(never)]
    pub(crate) fn sdio_slc_write(&mut self) {
        if !self.periph.optional.sdio.dma_pending() { return; }
        let mut mem = Sram { base: crate::bus::SRAM_LOW, bytes: &mut self.sram };
        self.periph.optional.sdio.run_dma(&mut mem);
        self.sdio_settle();
    }

    fn sdio_settle(&mut self) {
        esp_periph::Dispatch::refresh_optional(&mut self.periph, SLC_BLOCK);
        self.irq_dirty = true;
    }
}

/// The SLC block's number: its table entry carries the SLC0 source.
pub const SLC_BLOCK: u32 = 0x17;

#[cfg(test)]
mod tests;
