use crate::device::{Device, WriteEffect};
use crate::regram::RegRam;

/// Which array a SPI command wrote (the SoC maps this to its own buffer ids).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DirtyMem { Flash, Psram }

// ------------------------------------------------------------------ SPI flash controller (SPI_MEM: SPI1 = command engine, SPI0 = cache path)
pub struct SpiMem {
    /// Whether a PSRAM device sits on CS1. True for the S3; the C3 has no PSRAM, and without
    /// this a flash command issued with CS0 disabled is misrouted to the PSRAM branch and
    /// answered with zeros (found on real C3 silicon: `E memspi: no response`).
    pub has_psram: bool,
    /// Classic ESP32 SPI_ADDR_REG is shifted out MSB first in user mode: for an address phase shorter
    /// than 32 bits the HAL writes `(addr << (32 - bitlen)) | ones` (IDF v5.5.4
    /// hal/esp32/include/hal/spi_flash_ll.h:365-377), so a 24-bit address sits in bits 31:8 and the
    /// ROM's SPIRead writes `addr << 8` for its 32-bit 0xBB/0xEB phases; only a full 32-bit address
    /// (4-byte-address commands, :368-370, not decoded here) stays right-aligned. The SPI_MEM
    /// controllers (S3/C3/C6) take the register right-aligned
    /// (esp32s3/include/hal/spimem_flash_ll.h:590-594). Dedicated flash commands (FLASH_READ/PP/SE/BE)
    /// keep the address in bits 23:0 and a length in 31:24 on both (hal/spi_flash_hal_iram.c:75-82;
    /// esp_rom/patches/esp_rom_spiflash.c:151-214).
    pub addr_msb_aligned: bool,
    pub regs: RegRam,
    /// (memory, offset, len) ranges the last command wrote, for the bus's page versions
    pub dirty: Vec<(DirtyMem, usize, usize)>,
    pub w: [u32; 16],
    pub pending_cmd: u32,
    pub status: u32,
    pub jedec: [u8; 3],
    pub is_spi1: bool,
    pub log: bool,
    /// octal PSRAM (APS6408-like) mode registers MR0..MR8, device on CS1 (SPI1 only).
    /// MR2's density field follows the array (`set_psram_size`).
    pub psram_mr: [u8; 9],
    /// The flash on CS0 is an octal SPI flash (Macronix MX25UM, as on ESP32-S3-WROOM-2), see
    /// `octal_flash_command`. False: the quad SPI flash of the other modules.
    pub octal_flash: bool,
    /// The octal flash's configuration register 2 at address 0, its protocol: 0 SPI (power-on),
    /// 1 STR OPI, 2 DTR OPI. ESP-IDF writes it with WRCR2 0x72 as an 8-bit SPI command with a
    /// 32-bit address 0 and one data byte (IDF v5.5.5
    /// components/spi_flash/esp32s3/spi_flash_oct_flash_init.c:110-128
    /// `s_set_flash_dtr_str_opi_mode`; v4.4.8 :113-131 the same) and reads it back with RDCR2
    /// (components/spi_flash/spi_flash_chip_mxic_opi.c:320-345, v5.5.5). A reset command
    /// (RSTEN, RST) clears it.
    pub flash_cr2: u8,
    // Only the classic adapter opts into phase-aware quad control. Other chips retain
    // their existing quad/octal behavior. Appended: no new state ahead of hot fields.
    classic_quad: Option<ClassicQuad>,
}

#[derive(Default)]
struct ClassicQuad {
    qpi: bool,
    reset_enabled: bool,
}

/// MR2[2:0], the density code of an AP Memory octal PSRAM for an array of `bytes`: 001 32 Mbit,
/// 011 64 Mbit, 101 128 Mbit, 111 256 Mbit, 110 512 Mbit (APS6408L-OBMx datasheet Rev. 3.9,
/// Table 12). ESP-IDF sizes the PSRAM from this field alone: IDF v5.5.5
/// components/esp_psram/esp32s3/esp_psram_impl_octal.c:69 (`density: 3` in MR2) and :371-375
/// (the five codes, `esp_psram_impl_enable`). IDF v4.4.8 (Arduino-ESP32 2.x)
/// components/esp_hw_support/port/esp32s3/opiram_psram.c:264-267 knows only the first four, so a
/// 64 MB array reads as size 0 there. None for a size the field has no code for. After
/// NickoScope's esp32sim fork, commit 2b52ad5 (github.com/NickoScope/TWIN-NickoScopeMatrix-64x128,
/// MIT).
pub fn opi_psram_density(bytes: usize) -> Option<u8> {
    [(4usize << 20, 0b001), (8 << 20, 0b011), (16 << 20, 0b101), (32 << 20, 0b111), (64 << 20, 0b110)].iter().find(|&&(n, _)| n == bytes).map(|&(_, d)| d)
}

/// The JEDEC ID (manufacturer, type, capacity) of a flash chip of `bytes`: quad SPI 0x20 0x40
/// log2(size) as before, octal Macronix C2 80 log2(size)+0x20 (MX25UM25645G datasheet: C2 80 39
/// for 32 MB; ESP-IDF takes `1 << (id - 0x20)`, IDF v5.5.5
/// components/spi_flash/spi_flash_chip_mxic_opi.c:41-52 `spi_flash_chip_mxic_opi_detect_size`;
/// v4.4.8 the same).
pub fn flash_jedec(octal: bool, bytes: usize) -> [u8; 3] {
    let cap = bytes.trailing_zeros() as u8;
    if octal { [0xc2, 0x80, cap + 0x20] } else { [0x20, 0x40, cap] }
}

impl SpiMem {
    pub fn new(is_spi1: bool) -> Self { SpiMem { has_psram: true, addr_msb_aligned: false, regs: RegRam::new(), dirty: Vec::new(), w: [0; 16], pending_cmd: 0, status: 0x200, jedec: [0x20, 0x40, 0x17], is_spi1, log: false , psram_mr: [0x09, 0x0d, 0x8b, 0x00, 0x20, 0, 0, 0, 0x03], octal_flash: false, flash_cr2: 0, classic_quad: None } }
    /// The octal PSRAM on CS1 is `bytes` long, and MR2 says so. Device id and good-die bits keep
    /// their values. A size the field cannot encode leaves MR2 as it was (the reset value's
    /// 64 Mbit) and returns false.
    pub fn set_psram_size(&mut self, bytes: usize) -> bool {
        let Some(d) = opi_psram_density(bytes) else { return false };
        self.psram_mr[2] = (self.psram_mr[2] & !0x07) | d;
        true
    }
    /// Classic adapter only; presence is determined by the caller's backing slice.
    /// No backing allocation or CPU mapping is owned by the command engine.
    pub fn enable_classic_quad(&mut self) {
        self.has_psram = true;
        self.classic_quad = Some(ClassicQuad::default());
    }

    /// Classic mapped-cache qualification uses the same device mode as commands.
    pub fn classic_qpi_enabled(&self) -> bool {
        self.classic_quad.as_ref().is_some_and(|state| state.qpi)
    }

    // IDF v5.5.1 components/esp_psram/esp32/esp_psram_impl_quad.c:421-519:
    // normal exit is MOSI-only f5, ID is command 9f + 24 address bits,
    // entry is an 8-bit address-only 35. DCLK at 40 MHz PSRAM uses MOSI
    // 16-bit f500, or a two-bit zero command before address 9f000000 (32
    // bits, 64 MISO bits) / 35 (8 bits). This is not limited to rev0:
    // :936-940 selects DCLK when the PSRAM and flash CLK pins differ.
    // soc/esp32/register/soc/spi_reg.h (IDF v5.5.1): USER phases 31/30/28/27,
    // USER1 address length 26..31, USER2 command length 28..31.
    // Mode/reset gating below is an inferred functional model, not timing fidelity.
    fn classic_quad_opcode(&mut self, user: u32, user1: u32, user2: u32) -> Option<u32> {
        let command = user & (1 << 31) != 0;
        let address = user & (1 << 30) != 0;
        let mosi = user & (1 << 27) != 0;
        let miso = user & (1 << 28) != 0;
        let delay_command = command && (user2 >> 28) & 15 == 1 && user2 & 0xffff == 0;
        let delay_id = delay_command && address && (user1 >> 26) & 63 == 31
            && miso && !mosi && self.regs.read(0x28) == 63;
        let opcode = if delay_id {
            self.regs.read(4) >> 24
        } else if command && (user2 >> 28) & 15 == 7 {
            user2 & 0xff
        } else if (!command || delay_command) && address && (user1 >> 26) & 63 == 7 && !mosi && !miso {
            self.regs.read(4) >> 24
        } else if !command && !address && mosi && !miso && self.regs.read(0x24) == 15 && self.w[0] & 0xffff == 0xf500 {
            0xf5
        } else if !command && !address && mosi && !miso && self.regs.read(0x24) == 7 {
            self.w[0] & 0xff
        } else {
            self.classic_quad.as_mut().unwrap().reset_enabled = false;
            return None;
        };
        let state = self.classic_quad.as_mut().unwrap();
        let reset_enabled = std::mem::replace(&mut state.reset_enabled, false);
        let control = !miso && (!command || (delay_command && address && !mosi) || (!address && !mosi));
        match opcode {
            0x35 if control && !state.qpi => { state.qpi = true; None }
            0xf5 if control => { state.qpi = false; None }
            0x66 if control => { state.reset_enabled = true; None }
            0x99 if control => { if reset_enabled { state.qpi = false; } None }
            0x9f if !state.qpi && (delay_id || (command && address && (user1 >> 26) & 63 == 23 && miso && !mosi)) => Some(opcode),
            0x02 if !state.qpi && address && mosi && !miso => Some(opcode),
            0x03 | 0x0b if !state.qpi && address && miso && !mosi => Some(opcode),
            0x38 if state.qpi && address && mosi && !miso => Some(opcode),
            0xeb if state.qpi && address && miso && !mosi => Some(opcode),
            _ => None,
        }
    }
    pub fn read(&self, off: u32) -> u32 {
        match off {
            0x0 => 0,                                   // CMD: always idle after execution
            0x2c => self.status,                        // RD_STATUS
            0x54 => 0,                                  // FSM idle
            0x58..=0x94 => self.w[((off - 0x58) / 4) as usize],
            0xa4 => 0,                                  // SUS_STATUS
            0x3fc => 0x2101040,
            _ => self.regs.read(off),
        }
    }
    /// Returns true if a command must be executed against the flash array.
    pub fn write(&mut self, off: u32, v: u32) -> bool {
        match off {
            0x0 => { self.pending_cmd = v; return v & 0xffff_0003 != 0; }
            0x2c => {}   // RD_STATUS is written by hardware only
            0x58..=0x94 => self.w[((off - 0x58) / 4) as usize] = v,
            _ => self.regs.write(off, v),
        }
        false
    }
    fn w_bytes(&self, n: usize) -> Vec<u8> { let mut o = Vec::with_capacity(n); for i in 0..n { o.push((self.w[(i / 4) & 15] >> ((i % 4) * 8)) as u8); } o }
    fn set_w_bytes(&mut self, data: &[u8]) { for w in self.w.iter_mut() { *w = 0; } for (i, b) in data.iter().enumerate().take(64) { self.w[i / 4] |= (*b as u32) << ((i % 4) * 8); } }

    pub fn execute(&mut self, flash: &mut [u8], psram: &mut [u8]) {
        let cmd = self.pending_cmd; self.pending_cmd = 0;
        let user = self.regs.read(0x18); let user1 = self.regs.read(0x1c); let user2 = self.regs.read(0x20);
        let addr_reg = self.regs.read(0x4);
        let miso_bytes = ((self.regs.read(0x28) & 0x3ff) as usize + 8) / 8;
        let mosi_bytes = ((self.regs.read(0x24) & 0x3ff) as usize + 8) / 8;
        let addr_bits = ((user1 >> 26) & 0x3f) + 1;
        // User-command address: right-aligned on SPI_MEM; MSB-aligned on the classic ESP32, where the
        // flash sees the first 24 of the `addr_bits` shifted bits as the address: the HAL's DIO/QIO
        // phase is 28 bits (the continuous-mode bits follow, spi_flash_hal_common.inc:104-138), the
        // ROM's is 32, and the classic PSRAM driver sends an opcode as an 8-bit "address"
        // (esp_psram_impl_quad.c:495-512).
        let addr = if self.addr_msb_aligned {
            if addr_bits >= 24 { addr_reg >> 8 } else { addr_reg >> (32 - addr_bits) }
        } else if addr_bits > 24 { addr_reg } else { addr_reg & 0xff_ffff };
        // Dedicated flash commands (bits 31..22) take the address from bits 23:0 on the classic.
        let fixed = if self.addr_msb_aligned { addr_reg & 0xff_ffff } else { addr };
        let fsize = flash.len();
        let rd = |a: u32, n: usize| -> Vec<u8> { (0..n).map(|i| { let x = a as usize + i; if x < fsize { flash[x] } else { 0xff } }).collect() };
        // IDF v5.5.5 components/soc/esp32s3/register/soc/spi_mem_reg.h:
        // USR bit 18 (110-113), MISO/MOSI bits 28/27 (383-391), CS disable (560-570).
        let misc = self.regs.read(0x34);
        // A disabled classic CS0 must never fall through into flash, including
        // both-CS-disabled transactions. The default non-classic path is unchanged.
        if self.classic_quad.is_some() && cmd & (1 << 18) != 0 && misc & 3 == 3 {
            if user & (1 << 28) != 0 { self.set_w_bytes(&vec![0xff; miso_bytes]); }
            return;
        }
        if self.has_psram && cmd & (1 << 18) != 0 && misc & 1 != 0 && misc & 2 == 0 {   // USR command with CS0 disabled, CS1 enabled: PSRAM
            let mut c16 = user2 & 0xffff;
            let has_miso = user & (1 << 28) != 0; let has_mosi = user & (1 << 27) != 0;
            if self.log { eprintln!("[spi1] psram cmd {:#06x} addr {:#x} miso {} mosi {}", c16, addr, if has_miso { miso_bytes } else { 0 }, if has_mosi { mosi_bytes } else { 0 }); }
            let psize = psram.len();
            if psize == 0 {
                if has_miso { self.set_w_bytes(&vec![0xff; miso_bytes]); }
                return;
            }
            if self.classic_quad.is_some() {
                match self.classic_quad_opcode(user, user1, user2) {
                    Some(opcode) => c16 = opcode,
                    None => {
                        if has_miso { self.set_w_bytes(&vec![0xff; miso_bytes]); }
                        return;
                    }
                }
            }
            // IDF v5.5.5 components/esp_psram/device/esp_quad_psram_defs_ap.h:21-60;
            // v4.4.8 components/esp_hw_support/port/esp32s3/spiram_psram.c:46-83
            // use the same commands, KGD and 2/4/8 MiB density encoding.
            // Revision bits and absent-device 0xff are inferred model choices.
            match c16 {
                0x9f => {
                    let density = match psize {
                        0x200000 => 0x02, 0x400000 => 0x22, 0x800000 => 0x42,
                        _ => { self.set_w_bytes(&vec![0xff; miso_bytes]); return; }
                    };
                    self.set_w_bytes(&[0x0d, 0x5d, density][..miso_bytes.min(3)]);
                }
                0x4040 => { let i = (addr & 0xf) as usize; let d: Vec<u8> = (0..miso_bytes).map(|k| *self.psram_mr.get(i + k).unwrap_or(&0)).collect(); self.set_w_bytes(&d); }   // mode register read
                0xC0C0 => { let d = self.w_bytes(mosi_bytes); let i = (addr & 0xf) as usize; for (k, b) in d.iter().enumerate() { if i + k == 0 || i + k == 8 { self.psram_mr[i + k] = *b; } } }   // mode register write (MR0/MR8 writable)
                0x02 | 0x38 | 0x8080 => { let d = self.w_bytes(mosi_bytes); self.dirty.push((DirtyMem::Psram, addr as usize, d.len())); for (k, b) in d.iter().enumerate() { let x = addr as usize + k; if x < psize { psram[x] = *b; } } }   // sync write
                0x03 | 0x0b | 0xeb | 0x0000 => { let d: Vec<u8> = (0..miso_bytes).map(|k| { let x = addr as usize + k; if x < psize { psram[x] } else { 0 } }).collect(); self.set_w_bytes(&d); }   // sync read
                _ => { if has_miso { self.set_w_bytes(&vec![0u8; miso_bytes]); } }
            }
            return;
        }
        if cmd & (1 << 18) != 0 {   // USR: command from USER2
            let c = if user & (1 << 31) != 0 { (user2 & 0xff) as u8 } else { 0 };
            let has_addr = user & (1 << 30) != 0;
            let has_miso = user & (1 << 28) != 0;
            let has_mosi = user & (1 << 27) != 0;
            if self.octal_flash {
                // A 16-bit command (SPI_MEM_USER2 USR_COMMAND_BITLEN 15, bits 31:28; the value in
                // bits 15:0; IDF v5.5.5 components/soc/esp32s3/register/soc/spi_mem_reg.h:474-486)
                // whose second byte is the first one inverted is the octal form; the hardware sends
                // USR_COMMAND_VALUE LSB first, so WREN 06 F9 on the wire is 0xF906 in the register
                // (components/spi_flash/spi_flash_chip_mxic_opi.c:15 `CMD_OPI_FLASH_MXIC`,
                // components/esp_rom/esp32s3/include/esp32s3/rom/opi_flash.h:98-117 `*_OCT`).
                let raw = user2 & 0xffff;
                let opi = user & (1 << 31) != 0 && (user2 >> 28) & 0xf == 15 && (raw >> 8) as u8 == !(raw as u8);
                if self.log { eprintln!("[spi1] octal flash {} cmd {:#04x} addr {:#x}{} miso {} mosi {} (cr2 {})", if opi { "opi" } else { "spi" }, c, addr, if has_addr { "" } else { " (no addr)" }, if has_miso { miso_bytes } else { 0 }, if has_mosi { mosi_bytes } else { 0 }, self.flash_cr2); }
                self.octal_flash_command(c, opi, addr, if has_miso { miso_bytes } else { 0 }, if has_mosi { mosi_bytes } else { 0 }, flash);
                return;
            }
            if self.log { eprintln!("[spi1] usr cmd {:#04x} addr {:#x}{} miso {} mosi {}", c, addr, if has_addr { "" } else { " (no addr)" }, if has_miso { miso_bytes } else { 0 }, if has_mosi { mosi_bytes } else { 0 }); }
            // A 24-bit erase takes its address from the first 24 bits on the wire: the address
            // phase, then MOSI bytes. ESP-IDF v5.5.5 on the S3 sends an erase through
            // `spi_flash_hal_common_command` as 16 address bits plus the low byte as data
            // (components/hal/spi_flash_hal_common.inc:194-208, the PMS workaround); v5.5.4 has
            // no such split. Its quad erases below 16 MB use the dedicated FLASH_SE / FLASH_BE
            // bits instead (components/hal/spi_flash_hal_iram.c:42-47), which carry the whole address.
            let erase_addr = if has_mosi && addr_bits < 24 {
                self.w_bytes(((24 - addr_bits) / 8) as usize).iter().fold(addr, |a, &b| (a << 8) | u32::from(b))
            } else { addr };
            match c {
                0x03 | 0x0b | 0x3b | 0x6b | 0xbb | 0xeb => { let d = rd(addr, miso_bytes); self.set_w_bytes(&d); }
                0x9f => { let j = self.jedec; self.set_w_bytes(&j); }
                0x05 => { let s = self.status; self.set_w_bytes(&[s as u8]); }
                0x35 => { let s = self.status; self.set_w_bytes(&[(s >> 8) as u8]); }
                0x06 => self.status |= 0x02,                       // WREN: set WEL
                0x04 => self.status &= !0x02,                      // WRDI
                0x01 | 0x31 | 0x11 => self.status &= !0x02,        // WRSR*: latch consumed (keep QE set)
                0x15 => self.set_w_bytes(&[0x00]),
                0x02 | 0x32 | 0x38 => { let d = self.w_bytes(mosi_bytes); for (i, b) in d.iter().enumerate() { let x = addr as usize + i; if x < fsize { flash[x] &= *b; } } self.dirty.push((DirtyMem::Flash, addr as usize, d.len())); self.status &= !0x02; }
                0x20 => { let a = (erase_addr as usize) & !0xfff; for b in flash.iter_mut().take((a + 0x1000).min(fsize)).skip(a) { *b = 0xff; } self.dirty.push((DirtyMem::Flash, a, 0x1000)); self.status &= !0x02; }
                0x52 => { let a = (erase_addr as usize) & !0x7fff; for b in flash.iter_mut().take((a + 0x8000).min(fsize)).skip(a) { *b = 0xff; } self.dirty.push((DirtyMem::Flash, a, 0x8000)); self.status &= !0x02; }
                0xd8 => { let a = (erase_addr as usize) & !0xffff; for b in flash.iter_mut().take((a + 0x10000).min(fsize)).skip(a) { *b = 0xff; } self.dirty.push((DirtyMem::Flash, a, 0x10000)); self.status &= !0x02; }
                0xc7 | 0x60 => { for b in flash.iter_mut() { *b = 0xff; } self.dirty.push((DirtyMem::Flash, 0, fsize)); self.status &= !0x02; }
                _ => { if has_miso { self.set_w_bytes(&vec![0u8; miso_bytes]); } }
            }
            return;
        }
        if self.log && self.octal_flash && self.flash_cr2 != 0 { eprintln!("[spi1] octal flash: legacy command {:#010x} while in OPI mode (cr2 {})", cmd, self.flash_cr2); }
        if cmd & (1 << 31) != 0 { let d = rd(fixed, miso_bytes); self.set_w_bytes(&d); }                       // FLASH_READ
        if cmd & (1 << 28) != 0 { let j = self.jedec; self.set_w_bytes(&j); }                                   // RDID
        if cmd & (1 << 30) != 0 { self.status |= 0x02; }                                                         // WREN
        if cmd & (1 << 29) != 0 { self.status &= !0x02; }                                                        // WRDI
        if cmd & (1 << 26) != 0 { self.status &= !0x02; }                                                        // WRSR
        // RDSR (bit 27): RD_STATUS already reflects the live status register
        if cmd & (1 << 25) != 0 {                                                                                // PP
            let n = if addr_reg >> 24 != 0 { (addr_reg >> 24) as usize } else { mosi_bytes };
            let d = self.w_bytes(n);
            for (i, b) in d.iter().enumerate() { let x = (fixed & 0xff_ffff) as usize + i; if x < fsize { flash[x] &= *b; } }
            self.dirty.push((DirtyMem::Flash, (fixed & 0xff_ffff) as usize, d.len()));
            self.status &= !0x02;
        }
        if cmd & (1 << 24) != 0 { let a = (fixed as usize) & !0xfff; for b in flash.iter_mut().take((a + 0x1000).min(fsize)).skip(a) { *b = 0xff; } self.dirty.push((DirtyMem::Flash, a, 0x1000)); self.status &= !0x02; }      // SE
        if cmd & (1 << 23) != 0 { let a = (fixed as usize) & !0xffff; for b in flash.iter_mut().take((a + 0x10000).min(fsize)).skip(a) { *b = 0xff; } self.dirty.push((DirtyMem::Flash, a, 0x10000)); }    // BE
        if cmd & (1 << 22) != 0 { for b in flash.iter_mut() { *b = 0xff; } self.dirty.push((DirtyMem::Flash, 0, fsize)); }                          // CE
    }

    /// A user command to the octal flash on CS0 (Macronix MX25UM25645G datasheet, command set;
    /// the commands ESP-IDF sends: IDF v5.5.5
    /// components/esp_rom/esp32s3/include/esp32s3/rom/opi_flash.h:84-117 `ROM_FLASH_CMD_*`,
    /// components/spi_flash/spi_flash_chip_mxic_opi.c:15-20 and
    /// components/spi_flash/esp32s3/spi_flash_oct_flash_init.c; v4.4.8 has the same command
    /// values). `c` is the command byte, `opi` whether it came in the 16-bit octal form. The chip
    /// decodes only the form of its current protocol (CR2): an 8-bit command while it is in OPI
    /// mode, or an octal one while it is in SPI mode, does nothing and reads 0xff. In DTR OPI the
    /// register reads give every byte twice and the driver keeps one of each (RDID
    /// spi_flash_chip_mxic_opi.c:97-130 `read_id`, "[MD, MD, MT, MT, MID, MID]"; RDSR and RDCR2
    /// :132-150 `read_reg`, :320-345), the array reads (the read command set up at :363-385,
    /// 8DTRD 0x11EE) do not. The DTR
    /// byte swap of the wire (`esp_rom_spi_set_dtr_swap_mode`, opi_flash.h:192, set in
    /// spi_flash_oct_flash_init.c:207-215) is not modelled: the data is always in order. Program
    /// and erase do not check the write-enable latch, as the quad model does not.
    fn octal_flash_command(&mut self, c: u8, opi: bool, addr: u32, miso: usize, mosi: usize, flash: &mut [u8]) {
        if opi != (self.flash_cr2 != 0) {
            if self.log { eprintln!("[spi1] octal flash: {} command {:#04x} ignored in {} mode", if opi { "OPI" } else { "SPI" }, c, if self.flash_cr2 != 0 { "OPI" } else { "SPI" }); }
            if miso > 0 { self.set_w_bytes(&vec![0xff; miso]); }
            return;
        }
        let dtr = self.flash_cr2 & 0x02 != 0;
        let fsize = flash.len();
        let reg = |s: &mut Self, v: &[u8]| { let d: Vec<u8> = if dtr { v.iter().flat_map(|&b| [b, b]).collect() } else { v.to_vec() }; s.set_w_bytes(&d) };
        // ESP-IDF 5.x on the S3 sends an erase's address with its low byte in a data phase, so
        // that the PMS check sees USR_MOSI (IDF v5.5.5 components/hal/spi_flash_hal_common.inc:194-208,
        // `#if CONFIG_IDF_TARGET_ESP32S3`); the chip takes it as the rest of the address. IDF
        // v4.4.8 has no such split (:1-179): its erases arrive with the whole address and no data.
        let erase_addr = self.w_bytes(mosi.min(4)).iter().fold(addr, |a, &b| (a << 8) | u32::from(b));
        let erase = |s: &mut Self, flash: &mut [u8], size: usize| {
            let a = (erase_addr as usize) & !(size - 1);
            for b in flash.iter_mut().take((a + size).min(fsize)).skip(a) { *b = 0xff; }
            s.dirty.push((DirtyMem::Flash, a, size));
            s.status &= !0x02;
        };
        match c {
            0x9f => { let j = self.jedec; reg(self, &j); }                                   // RDID
            0x05 => { let s = self.status as u8; reg(self, &[s]); }                           // RDSR
            0x15 => reg(self, &[0x00]),                                                       // RDCR: default output driver, dummy cycles
            0x71 => { let v = if addr == 0 { self.flash_cr2 } else { 0 }; reg(self, &[v]); } // RDCR2
            0x72 => {                                                                         // WRCR2: address 0 selects SPI / STR OPI / DTR OPI
                if addr == 0 && mosi > 0 { self.flash_cr2 = self.w_bytes(1)[0] & 0x03; }
                self.status &= !0x02;
            }
            0x06 => self.status |= 0x02,                                                      // WREN
            0x04 => self.status &= !0x02,                                                     // WRDI
            0x01 => self.status &= !0x02,                                                     // WRSR / WRCR (SPI mode)
            0x66 => {}                                                                        // RSTEN
            0x99 => { self.flash_cr2 = 0; self.status &= !0x02; }                             // RST: back to SPI
            0x03 | 0x0b | 0x0c | 0x13 | 0xec | 0xee => {                                      // READ, FAST_READ (4B), 8READ, 8DTRD
                let d: Vec<u8> = (0..miso).map(|i| { let x = addr as usize + i; if x < fsize { flash[x] } else { 0xff } }).collect();
                self.set_w_bytes(&d);
            }
            0x02 | 0x12 => {                                                                  // PP, PP4B
                let d = self.w_bytes(mosi);
                if self.log { eprintln!("[spi1] octal flash: program {:#x} {:02x?}", addr, &d[..d.len().min(16)]); }
                for (i, b) in d.iter().enumerate() { let x = addr as usize + i; if x < fsize { flash[x] &= *b; } }
                self.dirty.push((DirtyMem::Flash, addr as usize, d.len()));
                self.status &= !0x02;
            }
            0x20 | 0x21 => erase(self, flash, 0x1000),                                        // SE, SE4B
            0x52 | 0x5c => erase(self, flash, 0x8000),                                        // BE32K, BE32K4B
            0xd8 | 0xdc => erase(self, flash, 0x10000),                                       // BE, BE4B
            0x60 | 0xc7 => { for b in flash.iter_mut() { *b = 0xff; } self.dirty.push((DirtyMem::Flash, 0, fsize)); self.status &= !0x02; }   // CE
            _ => { if miso > 0 { self.set_w_bytes(&vec![0u8; miso]); } }
        }
    }
}

impl Device for SpiMem {
    fn read(&mut self, off: u32) -> u32 { SpiMem::read(self, off) }
    /// Only SPI1 is the command engine; SPI0 is the cache path and its command bit is ignored.
    fn write(&mut self, off: u32, v: u32) -> WriteEffect { if SpiMem::write(self, off, v) && self.is_spi1 { WriteEffect::SPI_EXEC } else { WriteEffect::NONE } }
    fn debug(&mut self, on: bool) { self.log = on; }
}

#[cfg(test)]
mod tests {
    use super::{DirtyMem, SpiMem};

    /// IDF v5.5.4 hal/esp32/include/hal/spi_flash_ll.h:365-378: the classic ESP32 shifts SPI_ADDR_REG out MSB first,
    /// so a 24-bit user address is written as `(addr << 8) | 0xff`, and the DIO read 0xBB uses a 28-bit address
    /// phase for the continuous-mode bits (spi_flash_hal_common.inc:104-138). Dedicated commands keep the address
    /// in bits 23:0 with the length in 31:24 (spi_flash_hal_iram.c:75-82). The SPI_MEM controllers take the user
    /// address right-aligned (esp32s3/include/hal/spimem_flash_ll.h:590-594).
    #[test]
    fn classic_user_address_is_msb_aligned_and_dedicated_commands_keep_bits_23_0() {
        let mut flash = vec![0xffu8; 0x400000];
        flash[0x200000..0x200004].copy_from_slice(&[0x11, 0x22, 0x33, 0x44]);
        let user_read = |spi: &mut SpiMem, flash: &mut [u8], addr_bits: u32, cmd: u32| {
            spi.write(0x34, 2);                                   // CS0: the flash
            spi.write(0x18, (1 << 31) | (1 << 30) | (1 << 28));   // USER: command, address, miso phases
            spi.write(0x1c, (addr_bits - 1) << 26);               // USER1: address bit length
            spi.write(0x20, cmd);                                 // USER2: command
            spi.write(0x04, (0x200000 << 8) | 0xff);             // ADDR as the classic HAL writes it
            spi.write(0x28, 31);                                  // MISO_DLEN: 32 bits
            spi.write(0x58, 0);
            spi.write(0, 1 << 18);
            spi.execute(flash, &mut []);
            spi.w[0]
        };
        let mut spi = SpiMem::new(true);
        spi.addr_msb_aligned = true;
        for (addr_bits, cmd) in [(24, 0x03), (28, 0xbb)] {
            assert_eq!(user_read(&mut spi, &mut flash, addr_bits, cmd), 0x44332211, "user read {cmd:#x}, {addr_bits}-bit address phase");
        }
        spi.write(0x04, (4 << 24) | 0x200100);                   // PP: length 4 at 0x200100
        spi.write(0x58, 0xa5a5a5a5);
        spi.write(0, 1 << 25);
        spi.execute(&mut flash, &mut []);
        assert_eq!(&flash[0x200100..0x200104], &[0xa5; 4]);
        spi.write(0x04, 0x200100);                               // SE: the sector holding both
        spi.write(0, 1 << 24);
        spi.execute(&mut flash, &mut []);
        assert_eq!(&flash[0x200000..0x200004], &[0xff; 4]);
        // The ROM's SPIRead: a 32-bit 0xEB phase with the address in bits 31:8 and a zero low byte.
        flash[0x200000..0x200004].copy_from_slice(&[0x11, 0x22, 0x33, 0x44]);
        spi.write(0x1c, 31 << 26);
        spi.write(0x20, 0xeb);
        spi.write(0x04, 0x200000 << 8);
        spi.write(0x58, 0);
        spi.write(0, 1 << 18);
        spi.execute(&mut flash, &mut []);
        assert_eq!(spi.w[0], 0x44332211, "ROM-style 32-bit phase");
        // A phase shorter than 24 bits takes its leading bits (the PSRAM driver's 8-bit opcode phase).
        flash[0x12..0x16].copy_from_slice(&[0xa1, 0xb2, 0xc3, 0xd4]);
        spi.write(0x1c, 7 << 26);
        spi.write(0x20, 0x03);
        spi.write(0x04, 0x12 << 24);
        spi.write(0x58, 0);
        spi.write(0, 1 << 18);
        spi.execute(&mut flash, &mut []);
        assert_eq!(spi.w[0], 0xd4c3b2a1, "8-bit phase");
        // Right-aligned decoding reads 0x200000ff, past a 4 MiB array.
        let mut spi = SpiMem::new(true);
        assert_eq!(user_read(&mut spi, &mut flash, 24, 0x03), 0xffffffff);
    }

    #[test]
    fn cs1_quad_id_tracks_capacity_and_keeps_octal_and_flash_ids() {
        for (size, id) in [(0, 0xffffff), (0x100000, 0xffffff), (0x200000, 0x025d0d), (0x400000, 0x225d0d), (0x800000, 0x425d0d)] {
            let mut spi = SpiMem::new(true);
            let mut psram = vec![0; size];
            spi.write(0x18, (1 << 31) | (1 << 28));
            spi.write(0x20, 0x9f);
            spi.write(0x28, 47);
            spi.write(0x34, 1);
            spi.write(0, 1 << 18);
            spi.execute(&mut [], &mut psram);
            assert_eq!(spi.w[0] & 0xffffff, id);
            if size == 0 {
                spi.write(0x20, 0x4040);
                spi.write(0, 1 << 18);
                spi.execute(&mut [], &mut psram);
                assert_eq!(spi.w[0], u32::MAX);
            } else {
                spi.write(0x20, 0x4040);
                spi.write(0, 1 << 18);
                spi.execute(&mut [], &mut psram);
                assert_eq!(spi.w[0] & 0xffffff, 0x8b0d09);
                for write in [0x02, 0x38, 0x8080] {
                    spi.write(0x24, 31);
                    let value = 0x12340000 | write;
                    spi.write(0x58, value);
                    spi.write(0x20, write);
                    spi.write(0, 1 << 18);
                    spi.execute(&mut [], &mut psram);
                    for read in [0x03, 0x0b, 0xeb, 0x0000] {
                        spi.write(0x58, 0);
                        spi.write(0x20, read);
                        spi.write(0, 1 << 18);
                        spi.execute(&mut [], &mut psram);
                        assert_eq!(spi.w[0], value);
                    }
                }
            }
            spi.write(0x34, 2);
            spi.write(0x20, 0x9f);
            spi.write(0, 1 << 18);
            spi.execute(&mut [], &mut psram);
            assert_eq!(spi.w[0] & 0xffffff, 0x174020);
        }
    }

    /// MR`ma` and MR`ma+1` the way ESP-IDF reads them: 16-bit command 0x4040, 32 address bits,
    /// 16 data bits, CS1 alone (IDF v5.5.5 components/esp_psram/esp32s3/esp_psram_impl_octal.c:28
    /// and :180-205 `s_get_psram_mode_reg`; v4.4.8
    /// components/esp_hw_support/port/esp32s3/opiram_psram.c:34, :172 the same).
    fn psram_mr_read(s: &mut SpiMem, ma: u32) -> [u8; 2] {
        s.write(0x34, 1);                     // SPI_MEM_MISC: CS0_DIS set, CS1_DIS clear
        s.write(0x18, (1 << 31) | (1 << 30) | (1 << 28));   // SPI_MEM_USER: command, address, MISO
        s.write(0x1c, (32 - 1) << 26);        // SPI_MEM_USER1: USR_ADDR_BITLEN
        s.write(0x20, (15 << 28) | 0x4040);   // SPI_MEM_USER2: 16-bit OPI_PSRAM_REG_READ
        s.write(0x28, 16 - 1);                // SPI_MEM_MISO_DLEN
        s.write(0x4, ma);                     // SPI_MEM_ADDR
        assert!(s.write(0x0, 1 << 18), "USR starts the command");
        s.execute(&mut [], &mut [0u8; 16]);   // a chip is present: an empty array answers 0xff
        [s.w[0] as u8, (s.w[0] >> 8) as u8]
    }

    /// MR2[2:0] follows the array (APS6408L-OBMx Rev. 3.9 Table 12), up and down; device id,
    /// good-die and the other mode registers stay.
    #[test]
    fn opi_psram_mr2_density_follows_the_array_size() {
        let mut s = SpiMem::new(true);
        assert_eq!(psram_mr_read(&mut s, 2), [0x8b, 0x00], "reset value: 64 Mbit, MR3 after it");
        for (bytes, mr2) in [(16 << 20, 0x8d), (32 << 20, 0x8f), (4 << 20, 0x89), (64 << 20, 0x8e), (8 << 20, 0x8b), (16 << 20, 0x8d)] {
            assert!(s.set_psram_size(bytes));
            assert_eq!(psram_mr_read(&mut s, 2), [mr2, 0x00], "MR2 for {} MB", bytes >> 20);
        }
        assert_eq!(psram_mr_read(&mut s, 0), [0x09, 0x0d], "MR0 and MR1 untouched");
    }

    #[test]
    fn opi_psram_mr2_ignores_a_size_it_cannot_say() {
        let mut s = SpiMem::new(true);
        for bytes in [0, 2 << 20, 12 << 20, 128 << 20, (16 << 20) + 1] {
            assert!(!s.set_psram_size(bytes), "{bytes} bytes");
        }
        assert_eq!(psram_mr_read(&mut s, 2)[0], 0x8b);
    }

    #[test]
    fn flash_jedec_quad_and_octal() {
        assert_eq!(super::flash_jedec(false, 8 << 20), [0x20, 0x40, 0x17]);
        assert_eq!(super::flash_jedec(true, 16 << 20), [0xc2, 0x80, 0x38]);
        assert_eq!(super::flash_jedec(true, 32 << 20), [0xc2, 0x80, 0x39], "MX25UM25645G");
    }

    /// A CS0 user command as the ROM's `esp_rom_opiflash_exec_cmd` (IDF v5.5.5
    /// components/esp_rom/esp32s3/include/esp32s3/rom/opi_flash.h:217) and ESP-IDF's
    /// `spi_flash_hal_common_command` program it: an 8-bit command, or the 16-bit octal form
    /// (`cmd | !cmd << 8`, components/spi_flash/spi_flash_chip_mxic_opi.c:15).
    fn flash_cmd(s: &mut SpiMem, flash: &mut [u8], cmd: u8, opi: bool, addr: Option<(u32, u32)>, mosi: &[u8], miso: usize) -> Vec<u8> {
        let mut user = 1 << 31;
        if addr.is_some() { user |= 1 << 30; }
        if miso > 0 { user |= 1 << 28; }
        if !mosi.is_empty() { user |= 1 << 27; }
        s.write(0x34, 0);
        s.write(0x18, user);
        let (a, bits) = addr.unwrap_or((0, 24));
        s.write(0x1c, (bits - 1) << 26);
        s.write(0x20, if opi { (15 << 28) | u32::from(cmd) | (u32::from(!cmd) << 8) } else { (7 << 28) | u32::from(cmd) });
        s.write(0x4, a);
        s.write(0x24, (mosi.len() as u32 * 8).saturating_sub(1));
        s.write(0x28, (miso as u32 * 8).saturating_sub(1));
        s.w = [0; 16];
        for (i, b) in mosi.iter().enumerate() { s.w[i / 4] |= u32::from(*b) << ((i % 4) * 8); }
        assert!(s.write(0x0, 1 << 18));
        s.execute(flash, &mut []);
        s.w_bytes(miso)
    }

    fn octal_chip(bytes: usize) -> (SpiMem, Vec<u8>) {
        let mut s = SpiMem::new(true);
        s.octal_flash = true;
        s.jedec = super::flash_jedec(true, bytes);
        (s, (0..bytes).map(|i| (i >> 12) as u8).collect())
    }

    /// The ESP-IDF octal flash bring-up (IDF v5.5.5
    /// components/spi_flash/esp32s3/spi_flash_oct_flash_init.c:110-128 and :188-215
    /// `s_flash_init_mxic`): SPI commands at power-on, WRCR2 0x02 at address 0 switches to DTR
    /// OPI, after which only the 16-bit commands work, register reads come twice
    /// (spi_flash_chip_mxic_opi.c:97-150), and a reset (RSTEN, RST) goes back to SPI, as the ROM
    /// does at boot.
    #[test]
    fn octal_flash_switches_to_dtr_opi_and_back() {
        let (mut s, mut flash) = octal_chip(32 << 20);
        assert_eq!(flash_cmd(&mut s, &mut flash, 0x9f, false, None, &[], 3), [0xc2, 0x80, 0x39], "SPI RDID");
        assert_eq!(flash_cmd(&mut s, &mut flash, 0x9f, true, None, &[], 3), [0xff; 3], "an octal command in SPI mode");
        flash_cmd(&mut s, &mut flash, 0x06, false, None, &[], 0);
        flash_cmd(&mut s, &mut flash, 0x72, false, Some((0, 32)), &[0x02], 0);
        assert_eq!(s.flash_cr2, 2);
        assert_eq!(flash_cmd(&mut s, &mut flash, 0x9f, true, Some((0, 32)), &[], 6), [0xc2, 0xc2, 0x80, 0x80, 0x39, 0x39], "DTR RDID: every byte twice");
        assert_eq!(flash_cmd(&mut s, &mut flash, 0x71, true, Some((0, 32)), &[], 2), [0x02, 0x02], "RDCR2: DTR OPI");
        assert_eq!(flash_cmd(&mut s, &mut flash, 0x9f, false, None, &[], 3), [0xff; 3], "a SPI command in OPI mode");
        flash_cmd(&mut s, &mut flash, 0x06, true, None, &[], 0);
        assert_eq!(flash_cmd(&mut s, &mut flash, 0x05, true, Some((0, 32)), &[], 2), [0x02, 0x02], "RDSR: WEL, twice");
        flash_cmd(&mut s, &mut flash, 0x66, true, None, &[], 0);
        flash_cmd(&mut s, &mut flash, 0x99, true, None, &[], 0);
        assert_eq!(s.flash_cr2, 0);
        assert_eq!(flash_cmd(&mut s, &mut flash, 0x05, false, None, &[], 1), [0x00], "SPI again, WEL clear");
    }

    /// 8DTRD, PP4B and SE4B in DTR OPI with 32-bit addresses past 16 MB. ESP-IDF 5.x sends an
    /// erase's address low byte as data (IDF v5.5.5 components/hal/spi_flash_hal_common.inc:194-208,
    /// the S3 PMS workaround: 24 address bits plus one MOSI byte).
    #[test]
    fn octal_flash_reads_programs_and_erases_above_16_mb() {
        let (mut s, mut flash) = octal_chip(32 << 20);
        s.flash_cr2 = 2;
        let at = 0x1ff_f000u32;
        assert_eq!(flash_cmd(&mut s, &mut flash, 0xee, true, Some((at, 32)), &[], 4), [0xff; 4], "8DTRD: the array, not doubled");
        flash_cmd(&mut s, &mut flash, 0x06, true, None, &[], 0);
        flash_cmd(&mut s, &mut flash, 0x21, true, Some((at >> 8, 24)), &[at as u8], 0);
        assert!(flash[at as usize..at as usize + 0x1000].iter().all(|&b| b == 0xff), "SE4B erased the sector");
        assert_eq!(flash[at as usize - 1], 0xfe, "and nothing before it");
        assert_eq!(flash[0x1_f000], 0x1f, "not the sector of the high bits alone");
        flash_cmd(&mut s, &mut flash, 0x12, true, Some((at + 4, 32)), &[0x12, 0x34, 0xa5], 0);
        assert_eq!(flash_cmd(&mut s, &mut flash, 0xee, true, Some((at, 32)), &[], 8), [0xff, 0xff, 0xff, 0xff, 0x12, 0x34, 0xa5, 0xff]);
        assert_eq!(s.dirty.last(), Some(&(DirtyMem::Flash, at as usize + 4, 3)));
    }

    /// A quad user-command erase in ESP-IDF v5.5.5's S3 form, 16 address bits plus the low byte as
    /// data (components/hal/spi_flash_hal_common.inc:194-208), erases the block of the whole
    /// address, as the same erase with a 24-bit address phase does.
    #[test]
    fn quad_user_erase_takes_the_address_low_byte_from_the_data_phase() {
        for (cmd, size) in [(0x20u8, 0x1000usize), (0x52, 0x8000), (0xd8, 0x10000)] {
            for split in [true, false] {
                let mut s = SpiMem::new(true);
                let mut flash: Vec<u8> = (0..4usize << 20).map(|i| (i >> 12) as u8).collect();
                let at = 0x23_0000u32;
                if split { flash_cmd(&mut s, &mut flash, cmd, false, Some((at >> 8, 16)), &[at as u8], 0); }
                else { flash_cmd(&mut s, &mut flash, cmd, false, Some((at, 24)), &[], 0); }
                let a = at as usize;
                assert!(flash[a..a + size].iter().all(|&b| b == 0xff), "{cmd:#04x} split={split}: the block at {at:#x}");
                assert_eq!(flash[a - 1], ((a - 1) >> 12) as u8, "{cmd:#04x} split={split}: nothing before it");
                assert_eq!(flash[a + size], ((a + size) >> 12) as u8, "{cmd:#04x} split={split}: nothing after it");
                assert_eq!(flash[0x2000], 0x02, "{cmd:#04x} split={split}: not the block of the high bits alone");
                assert_eq!(s.dirty.last(), Some(&(DirtyMem::Flash, a, size)));
            }
        }
    }
}
