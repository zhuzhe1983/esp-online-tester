//! H2 must execute on its actual CLI chip backend and reject unsupported boot modes.
use std::process::Command;
fn run(extra: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_esp32sim"))
        .args(["--chip", "h2", "--max-insns", "20000", "--app"])
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/../docs/evidence/waveshare-h2-sram-2026-11-23/h2-sram-digital-probe.bin"))
        .args(extra).output().expect("H2 CLI process")
}
#[test]
fn both_exact_h2_boards_execute_sram_uart_without_a_rom() {
    for board in ["waveshare-h2-zero", "waveshare-h2-dev-kit-n4"] {
        let result=run(&["--board",board,"--boot","app"]);
        let text=format!("{}{}",String::from_utf8_lossy(&result.stdout),String::from_utf8_lossy(&result.stderr));
        assert!(result.status.success(),"{text}");
        assert!(text.contains("H2 SRAM chip16 RGB8"),"{text}");
        assert!(!text.contains("ROM loaded"),"{text}");
    }
}
#[test]
fn h2_cli_refuses_rom_and_incorrect_physical_capacities() {
    for args in [["--boot","rom"],["--flash-mb","8"],["--psram-mb","8"]] {
        let result=run(&args);assert_eq!(result.status.code(),Some(2));
        assert!(String::from_utf8_lossy(&result.stderr).contains("H2"));
    }
}
