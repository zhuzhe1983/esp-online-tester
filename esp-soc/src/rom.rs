//! Where the mask ROM ELFs (Apache-2.0, espressif/esp-rom-elfs) are found on the host. They ship
//! with ESP-IDF and are not checked in. The CLI and every test look them up here, so the order is
//! the same everywhere: `$ESP32SIM_ROM_DIR/<file>`, then the newest
//! `~/.espressif/tools/esp-rom-elfs/*/<file>`. In the browser build neither exists and nothing is found.
use std::path::PathBuf;

/// The directory that holds the ROM ELFs, for example `tools/fetch-rom-elfs.sh`'s `web/wasm/fw`.
pub const DIR_VAR: &str = "ESP32SIM_ROM_DIR";

/// `file` (such as `esp32s3_rev0_rom.elf`, a `Soc::ROM_ELF`) in `$ESP32SIM_ROM_DIR`, else in the
/// newest ESP-IDF install that has it.
pub fn find(file: &str) -> Option<PathBuf> {
    find_in(file, std::env::var_os(DIR_VAR).map(PathBuf::from), std::env::var_os("HOME").map(PathBuf::from))
}

fn find_in(file: &str, dir: Option<PathBuf>, home: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(p) = dir.map(|d| d.join(file)).filter(|p| p.is_file()) { return Some(p) }
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(home?.join(".espressif/tools/esp-rom-elfs")).ok()?.flatten().map(|e| e.path()).collect();
    dirs.sort();
    dirs.into_iter().rev().map(|d| d.join(file)).find(|p| p.is_file())
}

/// `find` for tests and examples: a missing ROM panics naming both places, never a silent pass.
pub fn require(file: &str) -> PathBuf {
    find(file).unwrap_or_else(|| {
        let tried = std::env::var_os(DIR_VAR).map_or_else(|| "unset".into(), |d| format!("{:?}", PathBuf::from(d).join(file)));
        panic!("{file} not found ({DIR_VAR}: {tried}): set {DIR_VAR} to an absolute directory (tools/fetch-rom-elfs.sh DIR fetches them) or install ESP-IDF (~/.espressif/tools/esp-rom-elfs)")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rom_dir_first_then_newest_esp_idf_release_that_has_the_file() {
        let t = std::env::temp_dir().join(format!("esp32sim-rom-lookup-{}", std::process::id()));
        let idf = t.join("home/.espressif/tools/esp-rom-elfs");
        for d in ["dir", "home/.espressif/tools/esp-rom-elfs/20230320", "home/.espressif/tools/esp-rom-elfs/20241011", "home/.espressif/tools/esp-rom-elfs/20260528"] {
            std::fs::create_dir_all(t.join(d)).unwrap();
        }
        for f in ["dir/a_rom.elf", "home/.espressif/tools/esp-rom-elfs/20230320/a_rom.elf", "home/.espressif/tools/esp-rom-elfs/20230320/b_rom.elf",
                  "home/.espressif/tools/esp-rom-elfs/20241011/a_rom.elf", "home/.espressif/tools/esp-rom-elfs/20241011/b_rom.elf"] {
            std::fs::write(t.join(f), []).unwrap();
        }
        let (dir, home) = (Some(t.join("dir")), Some(t.join("home")));
        assert_eq!(find_in("a_rom.elf", dir.clone(), home.clone()), Some(t.join("dir/a_rom.elf")));
        // Not in the directory: the newest release that has it (20260528 is newer but empty).
        assert_eq!(find_in("b_rom.elf", dir.clone(), home.clone()), Some(idf.join("20241011/b_rom.elf")));
        assert_eq!(find_in("a_rom.elf", None, home.clone()), Some(idf.join("20241011/a_rom.elf")));
        assert_eq!(find_in("c_rom.elf", dir, home), None);
        assert_eq!(find_in("a_rom.elf", None, None), None);
        std::fs::remove_dir_all(t).unwrap();
    }
}
