//! Boards using ESP32-C61.
pub use esp_soc::board::{Board, BoardModel, NoBoard};
pub fn make_board(name: &str) -> Option<Board> { match name { "none" | "bare" => Some(Box::new(NoBoard)), _ => None } }
