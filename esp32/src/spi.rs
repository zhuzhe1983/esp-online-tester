//! Classic ESP32 SPI2/HSPI and SPI3/VSPI masters and a loopback fixture.
//! ESP-IDF v5.5.4 components/soc/esp32/register/soc/spi_reg.h:361-372 (MISO/MOSI bits 28/27), 1285-1334 (DMA links), 1414-1468 (DMA interrupt bits).
//! Transaction completion and timing behavior are inferred, not hardware-validated.
use esp_periph::{Device, RegRam, WriteEffect};
use esp_soc::{Board, BoardModel};

const USR: u32 = 1 << 18;
const USER_COMMAND: u32 = 1 << 31;
const USER_ADDR: u32 = 1 << 30;
const USER_MISO: u32 = 1 << 28;
const USER_MOSI: u32 = 1 << 27;

pub(crate) struct Transfer {
    pub tx: Vec<u8>,
    pub rx_len: usize,
    pub dma_tx_len: usize,
    pub dma_rx: bool,
    pub cs: Option<(usize, bool)>,
    pub keep_cs: bool,
}

/// The original ESP32 GP-SPI register block. Unlike later chips, its DMA link engine is part of
/// each SPI controller; the SoC bus walks those descriptors because it owns system memory.
pub struct ClassicGpSpi {
    regs: RegRam,
    w: [u32; 16],
    pending: Option<Transfer>,
    out_active: bool,
    in_active: bool,
    dma_int_ena: u32,
    dma_int_raw: u32,
    pub transfers: u64,
    log: bool,
}

impl Default for ClassicGpSpi {
    fn default() -> Self {
        Self::new()
    }
}

impl ClassicGpSpi {
    pub fn new() -> Self {
        Self {
            regs: RegRam::new(),
            w: [0; 16],
            pending: None,
            out_active: false,
            in_active: false,
            dma_int_ena: 0,
            dma_int_raw: 0,
            transfers: 0,
            log: false,
        }
    }

    pub fn read(&self, off: u32) -> u32 {
        match off {
            0x00 => (self.regs.read(0) & !USR) | if self.pending.is_some() { USR } else { 0 },
            0x80..=0xbc => self.w[((off - 0x80) / 4) as usize],
            0x10c => (self.out_active as u32) << 1 | self.in_active as u32,
            0x110 => self.dma_int_ena,
            0x114 => self.dma_int_raw,
            0x118 => self.dma_int_raw & self.dma_int_ena,
            _ => self.regs.read(off),
        }
    }

    pub fn write(&mut self, off: u32, v: u32) {
        match off {
            0x00 => {
                self.regs.write(0, v & !USR);
                if v & USR != 0 && self.pending.is_none() {
                    self.start();
                }
            }
            0x80..=0xbc => self.w[((off - 0x80) / 4) as usize] = v,
            0x104 => {
                self.regs.write(off, v & 0x000f_ffff);
                if v & (1 << 28) != 0 {
                    self.out_active = false;
                }
                if v & ((1 << 29) | (1 << 30)) != 0 {
                    self.out_active = true;
                }
            }
            0x108 => {
                self.regs.write(off, v & 0x001f_ffff);
                if v & (1 << 28) != 0 {
                    self.in_active = false;
                }
                if v & ((1 << 29) | (1 << 30)) != 0 {
                    self.in_active = true;
                }
            }
            0x110 => self.dma_int_ena = v,
            0x11c => self.dma_int_raw &= !v,
            _ => self.regs.write(off, v),
        }
    }

    fn start(&mut self) {
        let user = self.regs.read(0x1c);
        let mut tx = Vec::new();
        if user & USER_COMMAND != 0 {
            let n = ((((self.regs.read(0x24) >> 28) & 15) + 1) as usize).div_ceil(8);
            let command = self.regs.read(0x24);
            for i in 0..n {
                tx.push((command >> (8 * i)) as u8);
            }
        }
        if user & USER_ADDR != 0 {
            let bits = ((self.regs.read(0x20) >> 26) & 63) + 1;
            let n = bits.div_ceil(8) as usize;
            let address = (u64::from(self.regs.read(0x04)) << 32) | u64::from(self.regs.read(0x30));
            for i in 0..n {
                tx.push((address >> (56 - 8 * i)) as u8);
            }
        }

        let mosi_len = if user & USER_MOSI != 0 {
            ((self.regs.read(0x28) & 0x00ff_ffff) + 1).div_ceil(8) as usize
        } else {
            0
        };
        let dma_tx_len = if self.out_active { mosi_len } else { 0 };
        if dma_tx_len == 0 {
            let base = if user & (1 << 25) != 0 { 8 } else { 0 };
            for i in 0..mosi_len.min((16 - base) * 4) {
                tx.push((self.w[base + i / 4] >> (8 * (i % 4))) as u8);
            }
        }
        let rx_len = if user & USER_MISO != 0 {
            ((self.regs.read(0x2c) & 0x00ff_ffff) + 1).div_ceil(8) as usize
        } else {
            0
        };
        let pin = self.regs.read(0x34);
        let cs = (0..3)
            .find(|cs| pin & (1 << cs) == 0)
            .map(|cs| (cs, pin & (1 << (6 + cs)) != 0));
        self.pending = Some(Transfer {
            tx,
            rx_len,
            dma_tx_len,
            dma_rx: self.in_active && rx_len != 0,
            cs,
            keep_cs: pin & (1 << 30) != 0,
        });
    }

    pub(crate) fn dma_links(&self) -> (u32, u32) {
        (
            0x3ff0_0000 | (self.regs.read(0x104) & 0x000f_ffff),
            0x3ff0_0000 | (self.regs.read(0x108) & 0x000f_ffff),
        )
    }

    pub(crate) fn pending(&self) -> Option<&Transfer> {
        self.pending.as_ref()
    }

    pub(crate) fn supply_dma_tx(&mut self, bytes: &[u8]) {
        if let Some(t) = self.pending.as_mut() {
            t.tx.extend_from_slice(&bytes[..bytes.len().min(t.dma_tx_len)]);
            t.dma_tx_len = 0;
        }
    }

    pub(crate) fn take_transfer(&mut self) -> Option<Transfer> {
        self.pending
            .as_ref()
            .is_some_and(|t| t.dma_tx_len == 0)
            .then(|| self.pending.take().unwrap())
    }

    pub(crate) fn idle_levels(&self, transfer: &Transfer) -> (bool, bool) {
        let clock = self.regs.read(0x34) & (1 << 29) != 0;
        let last = transfer.tx.last().copied().unwrap_or(0xff);
        let mosi = if self.regs.read(0x08) & (1 << 26) != 0 {
            last & 0x80 != 0
        } else {
            last & 1 != 0
        };
        (clock, mosi)
    }

    /// Cold opt-in phase metadata for a pin-aware SPI slave. ESP-IDF v4.4.7
    /// esp32/spi_reg.h USER/USER1/USER2, CTRL and SLAVE, also v5.5.4 offsets.
    /// This describes the existing transfer; it is not a second serializer.
    pub(crate) fn phase_snapshot(&self,transfer:&Transfer)->esp_soc::board::SpiQspiPins {
        let user=self.regs.read(0x1c);let control=self.regs.read(8);
        let command=user&USER_COMMAND!=0;let address=user&USER_ADDR!=0;
        let mosi=user&USER_MOSI!=0;let miso=user&USER_MISO!=0;
        esp_soc::board::SpiQspiPins {
            keep_cs:transfer.keep_cs,command_width:u8::from(command),address_width:u8::from(address),data_width:u8::from(mosi),read_width:u8::from(miso),
            command_bits:if command{((self.regs.read(0x24)>>28)&15)as u8+1}else{0},address_bits:if address{((self.regs.read(0x20)>>26)&63)as u8+1}else{0},
            data_bits:if mosi{(self.regs.read(0x28)&0xffffff)+1}else{0},read_bits:if miso{(self.regs.read(0x2c)&0xffffff)+1}else{0},
            dummy_cycles:if user&(1<<29)!=0{(self.regs.read(0x20)&255)as u16+1}else{0},sio:user&(1<<16)!=0,full_duplex:user&1!=0,
            clock_idle_high:self.regs.read(0x34)&(1<<29)!=0,clock_output_edge:user&(1<<7)!=0,
            unsupported_mode:control&((3<<25)|(3<<23)|(1<<20)|(1<<14))!=0||user&((15<<12)|(3<<10))!=0||self.regs.read(0x38)&(1<<30)!=0,
            ..Default::default()
        }
    }

    pub(crate) fn dma_fault(&mut self, tx: bool) {
        self.dma_int_raw |= if tx { 1 << 1 } else { 1 << 2 };
        if tx { self.out_active = false; } else { self.in_active = false; }
        self.pending = None;
    }

    pub(crate) fn finish(&mut self, transfer: Transfer, rx: &[u8], out_eof: u32, in_eof: u32) {
        if transfer.rx_len != 0 && !transfer.dma_rx {
            let base = if self.regs.read(0x1c) & (1 << 24) != 0 {
                8
            } else {
                0
            };
            esp_periph::fill_spi_w!(self.w, base, transfer.rx_len, rx);
        }
        if self.out_active {
            self.dma_int_raw |= (1 << 6) | (1 << 7) | (1 << 8);
            self.regs.write(0x134, out_eof);
            self.regs.write(0x138, out_eof);
        }
        if self.in_active {
            self.dma_int_raw |= (1 << 3) | (1 << 5);
            self.regs.write(0x124, in_eof);
        }
        self.out_active = false;
        self.in_active = false;
        self.regs.write(0x38, self.regs.read(0x38) | (1 << 4));
        self.transfers += 1;
        if self.log {
            eprintln!(
                "[spi] transfer tx={} rx={} dma_rx={}",
                transfer.tx.len(),
                transfer.rx_len,
                transfer.dma_rx
            );
        }
    }

    fn irq(&self) -> bool {
        let slave = self.regs.read(0x38);
        slave & 0x1f & (slave >> 5) != 0
    }

    fn dma_irq(&self) -> bool {
        self.dma_int_raw & self.dma_int_ena != 0
    }
}

impl Device for ClassicGpSpi {
    fn read(&mut self, off: u32) -> u32 {
        ClassicGpSpi::read(self, off)
    }
    fn write(&mut self, off: u32, v: u32) -> WriteEffect {
        ClassicGpSpi::write(self, off, v);
        WriteEffect::NONE
    }
    fn irq_sources(&self) -> u64 {
        self.irq() as u64 | (self.dma_irq() as u64) << 1
    }
    fn debug(&mut self, on: bool) {
        self.log = on;
    }
}

pub(crate) fn signals(host: usize) -> (usize, usize, usize, [usize; 3]) {
    match host {
        2 => (8, 10, 9, [11, 61, 62]),
        3 => (63, 65, 64, [68, 69, 70]),
        _ => unreachable!(),
    }
}

pub fn make_board(name: &str) -> Option<Board> {
    match name {
        "esp32dev-loopback" | "loopback" => Some(Box::new(SpiLoopback)),
        _ => None,
    }
}

struct SpiLoopback;
impl BoardModel for SpiLoopback {
    fn name(&self) -> &'static str {
        "esp32dev-loopback"
    }
    fn spi_transfer(&mut self, _host: u8, tx: &[u8], rx_len: usize) -> Vec<u8> {
        (0..rx_len)
            .map(|i| tx.get(i).copied().unwrap_or(0xff))
            .collect()
    }
    fn report(&self) -> String {
        "[emu] esp32dev SPI loopback: MOSI connected to MISO".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_transfer_keeps_modes_cs_and_response_words() {
        let mut spi = ClassicGpSpi::new();
        spi.write(0x08, (1 << 26) | (1 << 25));
        spi.write(0x1c, USER_MOSI | USER_MISO | 1 | (1 << 7));
        spi.write(0x28, 23);
        spi.write(0x2c, 23);
        spi.write(0x34, (1 << 1) | (1 << 2));
        spi.write(0x80, 0x4433_2211);
        spi.write(0, USR);
        let transfer = spi.take_transfer().unwrap();
        assert_eq!(transfer.tx, [0x11, 0x22, 0x33]);
        assert_eq!((transfer.rx_len, transfer.cs), (3, Some((0, false))));
        spi.finish(transfer, &[0xa5, 0x5a, 0x3c], 0, 0);
        assert_eq!(spi.read(0x80), 0xff3c_5aa5);
        assert_eq!(spi.read(0) & USR, 0);
        assert_eq!(spi.read(0x08), (1 << 26) | (1 << 25));
        assert_eq!(spi.read(0x1c) & 0x81, 0x81);
    }

    #[test]
    fn command_address_and_dma_state_form_one_transaction() {
        let mut spi = ClassicGpSpi::new();
        spi.write(0x1c, USER_COMMAND | USER_ADDR | USER_MOSI);
        spi.write(0x20, 23 << 26);
        spi.write(0x24, (7 << 28) | 0x2c);
        spi.write(0x04, 0x1234_5600);
        spi.write(0x28, 15);
        spi.write(0x104, (1 << 29) | 0xabc);
        spi.write(0, USR);
        assert_eq!(spi.pending().unwrap().dma_tx_len, 2);
        spi.supply_dma_tx(&[0xde, 0xad]);
        let transfer = spi.take_transfer().unwrap();
        assert_eq!(transfer.tx, [0x2c, 0x12, 0x34, 0x56, 0xde, 0xad]);
        spi.finish(transfer, &[], 0x3ff0_0abc, 0);
        assert_eq!(spi.read(0x114) & 0x1c0, 0x1c0);
    }

    #[test]
    fn transaction_and_dma_interrupts_are_independent() {
        let mut spi = ClassicGpSpi::new();
        spi.write(0x38, 1 << 9);
        spi.write(0x110, 1 << 8);
        spi.write(0x1c, USER_MOSI);
        spi.write(0x28, 7);
        spi.write(0x104, 1 << 29);
        spi.write(0, USR);
        spi.supply_dma_tx(&[1]);
        let transfer = spi.take_transfer().unwrap();
        spi.finish(transfer, &[], 1, 0);
        assert_eq!(spi.irq_sources(), 3);
        spi.write(0x38, spi.read(0x38) & !(1 << 4));
        assert_eq!(spi.irq_sources(), 2);
        spi.write(0x11c, 1 << 8);
        assert_eq!(spi.irq_sources(), 0);
    }
}
