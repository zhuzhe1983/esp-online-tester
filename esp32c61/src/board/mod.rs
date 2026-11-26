//! Boards using ESP32-C61.
pub mod corematrix;
pub mod cardkb2;
pub use esp_soc::board::{Board, BoardModel, NoBoard};
pub fn make_board(name: &str) -> Option<Board> { match name { "m5-cardkb2" | "m5stack-u215" => Some(Box::new(cardkb2::CardKb2::new())), "corematrix" | "m5stack-c158" => Some(Box::new(corematrix::CoreMatrix::new())), "none" | "bare" => Some(Box::new(NoBoard)), _ => None } }
