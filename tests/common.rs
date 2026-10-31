//! Shared by the firmware tests in `cli/tests`, `esp32c6/tests` and `wasm/tests` (included with `#[path]`).
//! No dependencies beyond the workspace's own crates: a small SHA-256 stands in for a crate.
#![allow(dead_code)]
use std::path::{Path, PathBuf};
use std::process::Command;

/// Workspace root (this file lives in `<root>/tests/`).
pub fn root() -> PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf() }
pub fn golden(name: &str) -> PathBuf { root().join("tests/golden").join(name) }

/// The mask ROM ELF for `chip` (`esp32s3_rev0` / `esp32c3_rev3`), found as the CLI finds it
/// (`esp_soc::rom`: `ESP32SIM_ROM_DIR`, then ESP-IDF's `~/.espressif/tools/esp-rom-elfs`). These ELFs
/// are not checked in, so a test that needs one panics when it is missing — never a silent pass.
pub fn rom(name: &str) -> PathBuf { esp_soc::rom::require(&format!("{}_rom.elf", name)) }

pub struct Run { pub stdout: String, pub stderr: String, pub insns: u64 }

/// Run an emulator binary; stdout is the guest console, stderr the emulator's own report.
pub fn run(bin: &str, args: &[&str]) -> Run {
    let out = Command::new(bin).args(args).current_dir(root()).output().unwrap_or_else(|e| panic!("{}: {}", bin, e));
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(out.status.success(), "{} {:?} failed: {}\n{}", bin, args, out.status, tail(&stderr));
    let insns = insn_count(&stderr).unwrap_or_else(|| panic!("no `[emu] stop:` line in stderr:\n{}", tail(&stderr)));
    Run { stdout: String::from_utf8_lossy(&out.stdout).to_string(), stderr, insns }
}

/// Instructions executed, from the `[emu] stop:` line (both cores on the S3, one on the C3).
pub fn insn_count(stderr: &str) -> Option<u64> {
    let line = stderr.lines().find(|l| l.starts_with("[emu] stop:"))?;
    if let Some(i) = line.find("core0 ") {
        let mut it = line[i..].split_whitespace();
        let c0: u64 = it.nth(1)?.parse().ok()?; let c1: u64 = it.nth(2)?.parse().ok()?;
        return Some(c0 + c1);
    }
    let i = line.find(" — ")?; line[i + " — ".len()..].split_whitespace().next()?.parse().ok()
}

fn tail(s: &str) -> String { let n = s.len(); s[n.saturating_sub(1500)..].to_string() }

fn update_goldens() -> bool { matches!(std::env::var("UPDATE_GOLDENS").as_deref(), Ok("1" | "true")) }

/// Compare `actual` with the golden file; `UPDATE_GOLDENS=1` rewrites it instead.
pub fn expect_text(name: &str, actual: &str) {
    let p = golden(name);
    if update_goldens() { std::fs::write(&p, actual).unwrap(); return; }
    let want = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {} (run with UPDATE_GOLDENS=1 to create it)", p.display(), e));
    if want != actual {
        let got = p.with_extension("actual");
        std::fs::write(&got, actual).unwrap();
        let (mut ln, mut wl, mut al) = (0, want.lines(), actual.lines());
        loop {
            ln += 1;
            match (wl.next(), al.next()) {
                (Some(w), Some(a)) if w == a => continue,
                (w, a) => panic!("{} differs at line {}:\n  want: {:?}\n  got:  {:?}\n(full output in {})", name, ln, w, a, got.display()),
            }
        }
    }
}

pub fn expect_sha(name: &str, data: &[u8]) {
    let p = golden(name);
    let hex = sha256_hex(data);
    if update_goldens() { std::fs::write(&p, format!("{}\n", hex)).unwrap(); return; }
    let want = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {} (run with UPDATE_GOLDENS=1 to create it)", p.display(), e));
    assert_eq!(want.trim(), hex, "{}: {} bytes hash differently", name, data.len());
}

pub fn expect_u64(name: &str, v: u64) {
    let p = golden(name);
    if update_goldens() { std::fs::write(&p, format!("{}\n", v)).unwrap(); return; }
    let want: u64 = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {}", p.display(), e)).trim().parse().unwrap();
    assert_eq!(want, v, "{}", name);
}

/// A fresh output path on each call, including concurrent calls with the same filename.
pub fn tmp(name: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("esp32sim-test-{}/{id}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d.join(name)
}

pub fn sha256_hex(data: &[u8]) -> String { sha256(data).iter().map(|b| format!("{:02x}", b)).collect() }

pub fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2];
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut m = data.to_vec(); m.push(0x80); while m.len() % 64 != 56 { m.push(0); } m.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for chunk in m.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 { w[i] = u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap()); }
        for i in 16..64 { let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3); let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10); w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1); }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let t1 = hh.wrapping_add(e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25)).wrapping_add((e & f) ^ (!e & g)).wrapping_add(K[i]).wrapping_add(w[i]);
            let t2 = (a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22)).wrapping_add((a & b) ^ (a & c) ^ (b & c));
            hh = g; g = f; f = e; e = d.wrapping_add(t1); d = c; c = b; b = a; a = t1.wrapping_add(t2);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) { *x = x.wrapping_add(y); }
    }
    let mut out = [0u8; 32]; for (i, v) in h.iter().enumerate() { out[i * 4..i * 4 + 4].copy_from_slice(&v.to_be_bytes()); } out
}

#[test]
fn sha256_known_answer() { assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"); }

#[test]
fn concurrent_outputs_with_the_same_name_are_independent() {
    let workers: Vec<_> = (0u8..8).map(|value| std::thread::spawn(move || {
        let path = tmp("same.wav");
        std::fs::write(&path, [value]).unwrap();
        (path, value)
    })).collect();
    for worker in workers {
        let (path, value) = worker.join().unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), [value]);
        std::fs::remove_file(path).unwrap();
    }
}
