//! Boards physically attached to ESP32-C3 GPIO/RMT, separate from the bare SoC.
pub use esp_soc::board::{Board,BoardModel,NoBoard};
pub mod stampc3;
pub use stampc3::StampC3;

pub fn make_board(name:&str)->Option<Board> {
    match name {"none"|"bare"=>Some(Box::new(NoBoard)),"stampc3"=>Some(Box::new(StampC3::new())),_=>None}
}

#[cfg(test)]
mod tests {
    #[test]
    fn c3_factory_does_not_accept_other_chip_boards() {
        for name in ["none","bare","stampc3"] {assert!(super::make_board(name).is_some());}
        for name in ["stamps3","stamps3a","nanoc6","esp32dev","invented"] {assert!(super::make_board(name).is_none());}
    }
}
