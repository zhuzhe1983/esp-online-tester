// rustc/clippy-driver --test this file with a caller-supplied prebuilt
// esp_periph rlib and dependency search directories. No dependency build.
#[path = "../esp32c5/src/mspi_regs.rs"]
mod mspi_regs;
#[path = "../esp32c5/src/quad_psram.rs"]
mod quad_psram;
#[path = "../esp32c5/src/mspi.rs"]
mod mspi;

// Compile references for owner APIs used by Bus/Soc outside these unit cases.
// They neither execute guest code nor suppress dead-code or other lint checks.
const _: fn(&mut mspi::Mspi) = mspi::Mspi::disable_mp_rom_compat;
const _: fn(&mspi::Mspi)->bool = mspi::Mspi::mp_rom_compat;
const _: fn(&mut mspi::Mspi,u32) = mspi::Mspi::set_xtal_hz;
const _: fn(&mspi::Mspi)->String = mspi::Mspi::psram_command_debug;
