//! Differential decoder test: every instruction in objdump's disassembly of the C3, C6 and P4 mask
//! ROMs and of real firmware for each must decode to the same mnemonic and operands, and every word
//! objdump cannot decode (`.insn`) must decode as illegal.
//! The checked-in corpus (`tests/corpus/*.dis`, sampled from a real app and the mask ROM) runs
//! every time; `RISCV_DIS_FILES=/path/rom.dis:/path/app.dis` runs the ignored full-size test, and
//! `RISCV_P4_DIS_FILES` the same for listings of P4 (RV32IMAFC) code. The lists are split as
//! `PATH` is (`std::env::split_paths`: `:`, or `;` on Windows, where `:` is in every path).
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use riscv_rv32::decode::{decode, decode_for, Op, ISA_HWLP, ISA_PIE, ISA_ZB, ISA_ZC};
use riscv_rv32::disasm;
use riscv_rv32::state::{MISA_RV32IMAC, MISA_RV32IMAFC};

fn norm_num(t: &str) -> Option<i64> {
    let t = t.trim();
    if let Some(h) = t.strip_prefix("0x") { return i64::from_str_radix(h, 16).ok(); }
    if let Some(h) = t.strip_prefix("-0x") { return i64::from_str_radix(h, 16).ok().map(|v| -v); }
    if let Ok(v) = t.parse::<i64>() { return Some(v); }
    i64::from_str_radix(t, 16).ok()
}

fn same_operand(mine: &str, theirs: &str) -> bool {
    if mine == theirs { return true; }
    match (norm_num(mine), norm_num(theirs)) {
        (Some(a), Some(b)) => a == b || (a as u32) == (b as u32),
        _ => false,
    }
}

/// One instruction line of an objdump listing: address, bytes, length and objdump's text.
struct Line<'a> { addr: u32, hex: &'a str, bytes: [u8; 4], len: usize, theirs: &'a str }

fn lines(text: &str) -> impl Iterator<Item = Line<'_>> {
    text.lines().filter_map(|line| {
        // "40000000:\t0000006f          \tj\t40000000 <_start>"
        let l = line.trim_start();
        let (addr_s, rest) = l.split_once(":\t")?;
        let addr = u32::from_str_radix(addr_s.trim(), 16).ok()?;
        let (hex, text) = rest.split_once('\t')?;
        let hex = hex.trim();
        if hex.is_empty() || !hex.chars().all(|c| c.is_ascii_hexdigit()) { return None; }
        let word = u32::from_str_radix(hex, 16).ok()?;
        let len = hex.len() / 2;
        if len != 2 && len != 4 { return None; }
        // objdump appends " <sym+0x..>" and "# 0x.. <sym>" comments — not part of the operands
        let mut t = text.trim();
        if let Some(p) = t.find(" <") { t = &t[..p]; }
        if let Some(p) = t.find(" #") { t = &t[..p]; }
        if let Some(p) = t.find("  ") { t = &t[..p]; }
        let mut bytes = [0u8; 4];
        for (i, byte) in bytes.iter_mut().enumerate().take(len) { *byte = ((word >> (8 * i)) & 0xff) as u8; }
        Some(Line { addr, hex, bytes, len, theirs: t.trim() })
    })
}

fn read(file: &Path) -> String { std::fs::read_to_string(file).unwrap_or_else(|e| panic!("{}: {}", file.display(), e)) }

/// Every instruction in the listings must decode, for a core whose `misa` is `misa`, to objdump's
/// mnemonic and operands, and every `.insn` word objdump could not decode must be illegal. Xesppie
/// (`esp.*`; `esp.lp.*` with `ISA_HWLP`) is compared on a core with `ISA_PIE` where the decoder models the form; the forms it
/// leaves illegal, and every `esp.*` on other cores, are counted, not compared. Returns the case count.
fn check(files: &[PathBuf], misa: u32) -> usize {
    let (mut total, mut bad, mut unknown, mut xesppie) = (0usize, 0usize, 0usize, 0usize);
    let mut by_mnemonic: BTreeMap<String, (usize, usize, String)> = BTreeMap::new();
    for file in files {
        let text = read(file);
        for Line { addr, hex, bytes, len, theirs } in lines(&text) {
            if theirs.is_empty() { continue; }
            let insn = decode_for(misa, addr, bytes);
            let ext = if theirs.starts_with("esp.lp.") { ISA_HWLP } else { ISA_PIE };
            if theirs.starts_with("esp.") && (misa & ext == 0 || insn.is_illegal()) { xesppie += 1; continue; }
            if theirs.starts_with(".insn") {
                unknown += 1;
                let e = by_mnemonic.entry(".insn".to_string()).or_insert((0, 0, String::new()));
                e.0 += 1;
                if !insn.is_illegal() || insn.len as usize != len {
                    bad += 1; e.1 += 1;
                    if e.2.is_empty() { e.2 = format!("{:08x} {:>8}  objdump={:?}  ours={:?}", addr, hex, theirs, disasm::format_for(misa, &insn)); }
                }
                continue;
            }
            if theirs.starts_with('.') { continue; }
            let mine = disasm::format_for(misa, &insn);
            total += 1;

            let (tm, to) = theirs.split_once(char::is_whitespace).unwrap_or((theirs, ""));
            let (mm, mo) = mine.split_once('\t').unwrap_or((mine.as_str(), ""));
            let ok = tm == mm
                && insn.len as usize == len
                && to.split(',').zip(mo.split(',')).all(|(a, b)| same_operand(b.trim(), a.trim()))
                && to.split(',').count() == mo.split(',').count();
            let e = by_mnemonic.entry(tm.to_string()).or_insert((0, 0, String::new()));
            e.0 += 1;
            if !ok {
                bad += 1;
                e.1 += 1;
                if e.2.is_empty() { e.2 = format!("{:08x} {:>8}  objdump={:?}  ours={:?}", addr, hex, theirs, mine); }
            }
        }
    }
    assert!(total > 1000, "only {} instructions parsed — is the .dis file right?", total);
    if bad > 0 {
        let mut worst: Vec<_> = by_mnemonic.iter().filter(|(_, v)| v.1 > 0).collect();
        worst.sort_by_key(|(_, v)| std::cmp::Reverse(v.1));
        let mut msg = format!("{}/{} instructions mismatched ({} .insn words)\n", bad, total, unknown);
        for (m, (n, b, ex)) in worst.iter().take(15) {
            msg.push_str(&format!("  {:<12} {:>6}/{:<6}  {}\n", m, b, n, ex));
        }
        panic!("{}", msg);
    }
    let f = by_mnemonic.iter().filter(|(m, _)| is_f_text(m)).map(|(_, v)| v.0).sum::<usize>();
    eprintln!("decoder: {} instructions ({} F), 0 mismatches; {} .insn words illegal; {} Xesppie skipped", total, f, unknown, xesppie);
    total
}

/// objdump's mnemonics for the F instructions and the FP CSR aliases.
fn is_f_text(m: &str) -> bool {
    m.starts_with('f') && !m.starts_with("fence")
}

const C3_C6: &[&str] = &["rom.dis", "hello_world.dis", "c6-rom.dis", "c6-hello_world.dis"];
const P4: &[&str] = &["p4-rom.dis", "p4-idf.dis", "rv32f.dis"];
const S31: u32 = MISA_RV32IMAFC | ISA_ZB | ISA_ZC | ISA_PIE | ISA_HWLP;

fn corpus(names: &[&str]) -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus");
    names.iter().map(|n| dir.join(n)).collect()
}

/// The files a `*_DIS_FILES` variable lists.
fn listed(var: &str) -> Vec<PathBuf> {
    let list = std::env::var_os(var).unwrap_or_else(|| panic!("{var}=/path/rom.dis:/path/app.dis is required for this test"));
    std::env::split_paths(&list).collect()
}

/// Hermetic: the sampled corpora checked in next to this test (see the headers for provenance).
#[test]
fn decoder_matches_corpus() {
    let n = check(&corpus(C3_C6), 0);
    assert!(n >= 14000, "only {} corpus cases — the corpus files are incomplete", n);
}

/// The P4 corpora decoded as RV32IMAFC: the P4 mask ROM, ESP-IDF's P4 libraries and every F
/// encoding form assembled from `rv32f.S`.
#[test]
fn p4_decoder_matches_corpus() {
    let n = check(&corpus(P4), MISA_RV32IMAFC);
    assert!(n >= 8900, "only {} corpus cases — the corpus files are incomplete", n);
}

/// The C3 and C6 decode the P4 corpora exactly as before F existed: every F instruction is
/// illegal with its raw bits, and everything else decodes the same with or without F.
#[test]
fn cores_without_f_reject_every_f_instruction() {
    let mut f = 0;
    for file in [corpus(P4), corpus(C3_C6)].concat() {
        let text = read(&file);
        for l in lines(&text) {
            let with_f = decode_for(MISA_RV32IMAFC, l.addr, l.bytes);
            for plain in [decode(l.addr, l.bytes), decode_for(MISA_RV32IMAC, l.addr, l.bytes)] {
                if with_f.op.is_f() {
                    assert_eq!((plain.op, plain.raw, plain.len), (Op::Illegal, with_f.raw, with_f.len), "{:08x} {}", l.addr, l.theirs);
                } else {
                    assert_eq!(plain, with_f, "{:08x} {}", l.addr, l.theirs);
                }
            }
            f += with_f.op.is_f() as usize;
        }
    }
    assert!(f > 2000, "only {} F instructions in the P4 corpora", f);
}

/// Every Zba/Zbb/Zbs and Zcb/Zcmp form assembled from `zbc.S`, decoded as the ESP32-S31 core.
#[test]
fn s31_decoder_matches_corpus() {
    let n = check(&corpus(&["zbc.dis"]), S31);
    assert!(n >= 1400, "only {} corpus cases — the corpus file is incomplete", n);
}

/// Without `ISA_ZB`/`ISA_ZC` every new form is illegal with its raw bits and length, and with them
/// every older corpus decodes the same: the extensions only take encodings that were illegal.
#[test]
fn cores_without_zb_zc_reject_them_and_older_code_decodes_the_same() {
    let mut new = 0;
    for file in corpus(&["zbc.dis"]) {
        for l in lines(&read(&file)) {
            let (with, plain) = (decode_for(S31, l.addr, l.bytes), decode_for(MISA_RV32IMAFC, l.addr, l.bytes));
            if with.op == Op::Illegal { assert!(plain.is_illegal(), "{:08x} {}", l.addr, l.theirs); continue; }
            // c.zext.b and c.not expand to andi/xori; they are new encodings all the same
            assert_eq!((plain.op, plain.raw, plain.len), (Op::Illegal, with.raw, with.len), "{:08x} {}", l.addr, l.theirs);
            new += 1;
        }
    }
    assert!(new >= 1400, "only {} new instructions", new);
    for file in [corpus(P4), corpus(C3_C6)].concat() {
        for l in lines(&read(&file)) {
            let plain = decode_for(MISA_RV32IMAFC, l.addr, l.bytes);
            if !plain.is_illegal() { assert_eq!(decode_for(S31, l.addr, l.bytes), plain, "{:08x} {}", l.addr, l.theirs); }
        }
    }
}

/// Every modelled Xespv2p2 form, operand value and sat/rm pair assembled from `pie.S`, decoded as
/// the ESP32-S31 core; each `esp.*` line of the subset must decode (none skipped), and the
/// flipped-bit words objdump leaves as `.insn` must be illegal.
#[test]
fn s31_pie_decoder_matches_corpus() {
    let n = check(&corpus(&["pie.dis"]), S31);
    assert!(n >= 1600, "only {} corpus cases — the corpus file is incomplete", n);
    let subset = ["esp.vmul.u16", "esp.vmul.u16.ld.incp", "esp.vadd.u16", "esp.vadd.u16.ld.incp", "esp.vsub.u16", "esp.vsub.u8",
        "esp.vmin.u16", "esp.vcmp.eq.u16", "esp.andq", "esp.orq", "esp.xorq", "esp.zero.q", "esp.vext.u8", "esp.vunzip.8",
        "esp.vzipt.8", "esp.vunzipt.8", "esp.src.q", "esp.vld.128.ip", "esp.vst.128.ip", "esp.vldbc.16.ip", "esp.ld.128.usar.ip",
        "esp.movx.w.sar", "esp.movx.r.sar", "esp.movx.w.cfg", "esp.movx.r.cfg", "esp.movi.32.a"];
    let (mut seen, text) = (std::collections::BTreeSet::new(), read(&corpus(&["pie.dis"])[0]));
    for l in lines(&text) {
        let m = l.theirs.split_whitespace().next().unwrap_or("");
        if subset.contains(&m) { assert!(!decode_for(S31, l.addr, l.bytes).is_illegal(), "{:08x} {}", l.addr, l.theirs); seen.insert(m); }
    }
    assert_eq!(seen.len(), subset.len(), "pie.dis lacks some of the subset");
}

/// Without `ISA_PIE` every PIE word is illegal with its raw bits, and with it the older corpora
/// decode the same.
#[test]
fn cores_without_pie_reject_it_and_older_code_decodes_the_same() {
    let (without, text) = (S31 & !ISA_PIE, read(&corpus(&["pie.dis"])[0]));
    for l in lines(&text) {
        let (with, plain) = (decode_for(S31, l.addr, l.bytes), decode_for(without, l.addr, l.bytes));
        if with.op.is_pie() { assert_eq!((plain.op, plain.raw, plain.len), (Op::Illegal, with.raw, with.len), "{:08x} {}", l.addr, l.theirs); }
        else { assert_eq!(plain, with, "{:08x} {}", l.addr, l.theirs); }
    }
    for file in [corpus(P4), corpus(C3_C6), corpus(&["zbc.dis"])].concat() {
        for l in lines(&read(&file)) { assert_eq!(decode_for(S31, l.addr, l.bytes), decode_for(without, l.addr, l.bytes), "{:08x} {}", l.addr, l.theirs); }
    }
}

/// The full listings of whatever `RISCV_DIS_FILES` points at.
#[test]
#[ignore = "set RISCV_DIS_FILES=/path/rom.dis:/path/app.dis (riscv32-esp-elf-objdump -d)"]
fn external_decoder_matches_objdump() {
    check(&listed("RISCV_DIS_FILES"), 0);
}

/// The same for listings of P4 code, decoded as RV32IMAFC.
#[test]
#[ignore = "set RISCV_P4_DIS_FILES=/path/p4rom.dis:/path/app.dis (riscv32-esp-elf-objdump -d)"]
fn external_p4_decoder_matches_objdump() {
    check(&listed("RISCV_P4_DIS_FILES"), MISA_RV32IMAFC);
}

/// Every Xesploop form, operand value and offset bit assembled from `hwlp.S`, decoded as the
/// ESP32-S31 core: each `esp.lp.*` line must decode (none skipped), every form must appear, and
/// the flipped-bit words objdump leaves as `.insn` must be illegal. `check` asserts > 1000 cases,
/// so the hwlp corpus runs together with the zbc one.
#[test]
fn s31_hwlp_decoder_matches_corpus() {
    check(&corpus(&["hwlp.dis", "zbc.dis"]), S31);
    let forms = ["esp.lp.setup", "esp.lp.setupi", "esp.lp.starti", "esp.lp.endi", "esp.lp.count", "esp.lp.counti"];
    let (mut seen, mut n, text) = (std::collections::BTreeSet::new(), 0, read(&corpus(&["hwlp.dis"])[0]));
    for l in lines(&text) {
        let m = l.theirs.split_whitespace().next().unwrap_or("");
        if forms.contains(&m) { assert!(!decode_for(S31, l.addr, l.bytes).is_illegal(), "{:08x} {}", l.addr, l.theirs); seen.insert(m); n += 1; }
    }
    assert_eq!(seen.len(), forms.len(), "hwlp.dis lacks some forms");
    assert!(n >= 290, "only {n} esp.lp cases");
}

/// Without `ISA_HWLP` every hardware-loop word is illegal with its raw bits, and with it the older
/// corpora decode the same.
#[test]
fn cores_without_hwlp_reject_it_and_older_code_decodes_the_same() {
    let (without, text) = (S31 & !ISA_HWLP, read(&corpus(&["hwlp.dis"])[0]));
    for l in lines(&text) {
        let (with, plain) = (decode_for(S31, l.addr, l.bytes), decode_for(without, l.addr, l.bytes));
        if with.op.is_hwlp() { assert_eq!((plain.op, plain.raw, plain.len), (Op::Illegal, with.raw, with.len), "{:08x} {}", l.addr, l.theirs); }
        else { assert_eq!(plain, with, "{:08x} {}", l.addr, l.theirs); }
    }
    for file in [corpus(P4), corpus(C3_C6), corpus(&["zbc.dis", "pie.dis"])].concat() {
        for l in lines(&read(&file)) { assert_eq!(decode_for(S31, l.addr, l.bytes), decode_for(without, l.addr, l.bytes), "{:08x} {}", l.addr, l.theirs); }
    }
}
