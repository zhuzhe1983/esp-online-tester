//! Boards using ESP32-C61.
pub mod corematrix;
pub use esp_soc::board::{Board, BoardModel, NoBoard};
pub fn make_board(name: &str) -> Option<Board> { match name { "corematrix" | "m5stack-c158" => Some(Box::new(corematrix::CoreMatrix::new())), "none" | "bare" => Some(Box::new(NoBoard)), _ => None } }
