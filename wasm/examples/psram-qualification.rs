//! Native ABI half of tools/psram-wasm-test.mjs (public fixture firmware only).
use esp32sim_wasm::*;
use std::{env, fs, path::PathBuf};
fn main() {
    let args: Vec<String> = env::args().collect();
    assert_eq!(args.len(), 5, "psram-qualification <firmware-dir> <p5|p6> <0|4 MiB> <jit:0|1>");
    let fw = PathBuf::from(&args[1]);
    let prefix = &args[2];
    let size = args[3].parse().unwrap();
    let jit = args[4].parse().unwrap();
    let rom = PathBuf::from(env::var_os("ESP32SIM_ROM_DIR").expect("set ESP32SIM_ROM_DIR"));
    // SAFETY: all pointers below are uniquely owned, byte buffers are live for each call,
    // output bytes are copied before the next mutating call, and delete consumes ownership.
    unsafe {
        let e = esp32sim_new(b"esp32".as_ptr(), 5, 4, size);
        assert!(!e.is_null());
        for (kind, path) in [(0, rom.join("esp32_rev300_rom.elf")),
            (1, fw.join(format!("{prefix}_probe.ino.bootloader.bin"))),
            (2, fw.join(format!("{prefix}_probe.ino.partitions.bin"))),
            (3, fw.join(format!("{prefix}_probe.ino.bin")))] {
            let bytes = fs::read(path).unwrap();
            assert_eq!(esp32sim_load(e, kind, bytes.as_ptr(), bytes.len()), 0);
        }
        esp32sim_set_jit(e, jit);
        assert_eq!(esp32sim_boot(e, 0), 0);
        for boot in 1..=2 {
            if boot == 2 { psram_tests::esp32sim_test_classic_reboot(e); }
            let mut transcript = String::new();
            for _ in 0..5000 {
                assert_eq!(esp32sim_run(e, 100_000, 0.0), 0);
                let n = esp32sim_out_take(e);
                for i in 0..n {
                    if esp32sim_out_kind(e, i) != 1 { continue; }
                    let s = std::str::from_utf8(std::slice::from_raw_parts(esp32sim_out_ptr(e, i), esp32sim_out_len(e, i))).unwrap();
                    println!("{{\"boot\":{boot},\"message\":{s}}}");
                    transcript.push_str(s);
                }
                if transcript.contains(&format!("{} result=", prefix.to_uppercase())) { break; }
            }
            assert!(transcript.contains(&format!("{} result={}", prefix.to_uppercase(), if size == 4 { "pass" } else { "no-psram" })), "guest result missing");
            let count = psram_tests::esp32sim_test_classic_jit_insns(e);
            println!("{{\"boot\":{boot},\"message\":{{\"t\":\"qualification\",\"jit_insns\":{count}}}}}");
        }
        esp32sim_delete(e);
    }
}
