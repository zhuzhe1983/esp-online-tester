//! The peripheral IP that Espressif reuses across chips (UART, USB-Serial/JTAG, systimer, timer
//! groups, GPIO, LEDC, MCPWM, RTC_CNTL, SAR ADC, efuse, SYSTEM, SPI_MEM, GDMA, SHA/AES/RSA, I2S,
//! RMT, GP-SPI, I2C, SDMMC, USB-OTG, the RISC-V chips' RNG), one file each, plus the plumbing
//! that mounts them: the `Device` trait every model implements and the `DeviceSet` table a chip
//! fills in once — dispatch, interrupt sources, clock ticks and timer deadlines all come from
//! that one table.
pub mod device;
pub mod mmio;
pub mod regram;
pub mod crypto;
pub mod i2c;
pub mod uart;
pub mod usb_serial_jtag;
pub mod systimer;
pub mod timg;
pub mod gpio;
pub mod ledc;
pub mod mcpwm;
pub mod rtc_cntl;
pub mod analog;
pub mod sar_adc;
pub mod efuse;
pub mod system;
pub mod spi_mem;
pub mod sha;
pub mod aes;
pub mod rsa;
pub mod gdma;
pub mod i2s;
pub mod rmt;
pub mod rmt_compact;
pub mod gpspi;
pub mod rng;
pub mod intmtx;
pub mod sdmmc;
pub mod usb_otg;

pub use device::{Device, WriteEffect};
pub use mmio::{DeviceSet, Dispatch, Misc, NO_SOURCE};
pub use regram::RegRam;
pub use uart::{Uart, UartLayout};
pub use usb_serial_jtag::UsbSerialJtag;
pub use systimer::Systimer;
pub use timg::{Timer, TimerGroup};
pub use gpio::Gpio;
pub use analog::{AdcObservation, AnalogInputs, AnalogSource};
pub use ledc::{Ledc, LedcLayout};
pub use mcpwm::Mcpwm;
pub use rtc_cntl::{reset_cause_name, RtcCntl, RST_POWERON, RST_RTCWDT_CPU, RST_RTCWDT_RTC, RST_RTCWDT_SYS, RST_SW_CPU, RST_SW_SYS};
pub use efuse::Efuse;
pub use system::SystemRegs;
pub use spi_mem::{flash_jedec, opi_psram_density, DirtyMem, SpiMem};
pub use sha::Sha;
pub use aes::Aes;
pub use rsa::Rsa;
pub use gdma::{read_desc, DmaDesc, Gdma, GdmaInCh, GdmaOutCh, DMA_ADDR_BASE, GDMA_CHANNELS, GDMA_CH_STRIDE};
pub use i2s::I2s;
pub use rmt::{Rmt, RmtTxCh, RMT_MEM_WORDS};
pub use gpspi::{GpSpi, GpSpiTransfer};
pub use rng::Rng;

/// Clocks the shared IP is specified against; the CPU clock differs per chip and is a parameter.
pub const APB_HZ: u64 = 80_000_000;
pub const XTAL_HZ: u64 = 40_000_000;
pub const SYSTIMER_HZ: u64 = 16_000_000;
pub const RTC_SLOW_HZ: u64 = 150_000;
#[doc(hidden)] pub use mmio::{__divider, __ClockDomain, __ClockTree, __Dividers};

mod clocked_queue;
pub mod regi2c;
pub use regi2c::Regi2c;
