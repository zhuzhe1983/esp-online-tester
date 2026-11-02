//! The machine around a chip, written once: the `Soc` implementations (classic ESP32, ESP32-S3,
//! ESP32-C3, ESP32-C6, ESP32-P4) plug their cores, memory map, peripherals and interrupt
//! controller into `Machine<S>`, which owns the scheduler, device time, console, action scripts,
//! the web UI protocol, real-time pacing, the image loaders and the board model. `devices` holds
//! the chip-neutral models a board is built from.
pub mod ble;
pub mod board;
pub mod camera;
mod console;
pub mod debug;
pub mod devices;
pub mod elf;
pub mod gpio;
pub mod host;
pub mod image;
pub mod json;
pub mod load;
pub mod machine;
pub mod nat;
pub mod net;
pub mod observe;
pub mod observers;
pub mod picture;
pub mod pins;
pub mod png;
pub mod rom;
pub mod soc;
pub mod uart;
pub mod usb_host;
pub mod web;
pub mod wifi;

pub use board::{Board, BoardModel, NoBoard};
pub use debug::DebugFlags;
pub use load::LoadKind;
pub use machine::{Console, Debug, Machine, Realtime, Script, ScriptAction};
pub use observe::{Ctx, Observer, Wants};
pub use soc::{CoreState, GpioState, RunUntil, Soc, SocBus, Stop};
