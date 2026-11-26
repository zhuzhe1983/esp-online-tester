#!/usr/bin/env python3
"""Recheck the frozen Tab5 source audit; this does not execute P4 firmware."""
import argparse
import hashlib
import json
import pathlib
import subprocess
import tempfile
import urllib.request


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=pathlib.Path, default=pathlib.Path.cwd())
    parser.add_argument("--sources", type=pathlib.Path,
                        help="Directory containing the receipt's localFile downloads")
    parser.add_argument("--fetch", action="store_true",
                        help="Download missing pinned sources into --sources")
    parser.add_argument("--io-probe", action="store_true",
                        help="Run actual isolated PI4IO/GT911 source tests with rustc, without Cargo")
    args = parser.parse_args()
    if args.fetch and args.sources is None:
        parser.error("--fetch requires --sources")
    receipt = json.loads(pathlib.Path(__file__).with_name("source-receipts.json").read_text())
    revision = receipt["repositoryRevision"]
    snapshot = {}
    for row in receipt["repositoryFiles"]:
        data = subprocess.check_output(["git", "show", revision + ":" + row["path"]], cwd=args.repo)
        assert (len(data), sha(data)) == (row["bytes"], row["sha256"]), row["path"]
        snapshot[row["path"]] = data
    print(f"PASS: {len(snapshot)} frozen repository files match {revision}.")
    if args.sources is not None:
        args.sources.mkdir(parents=True, exist_ok=True)
        for row in receipt["sources"]:
            path = args.sources / row["localFile"]
            if args.fetch and not path.exists():
                request = urllib.request.Request(row["url"], headers={"User-Agent": "esp-online-tester-source-recheck/1"})
                with urllib.request.urlopen(request, timeout=45) as response:
                    data = response.read(8 * 1024 * 1024)
                path.write_bytes(data)
            data = path.read_bytes()
            assert (len(data), sha(data)) == (row["bytes"], row["sha256"]), row["id"]
        print(f"PASS: {len(receipt['sources'])} upstream downloads match recorded bytes and SHA-256.")
    if args.io_probe:
        # Extract the exact trait, not a substitute controller or a new device implementation.
        source = snapshot["esp-periph/src/i2c.rs"].decode()
        trait = source[source.index("pub trait I2cDevice {"):source.index("\npub const INT_END_DETECT")]
        with tempfile.TemporaryDirectory(prefix="tab5-io-probe-") as directory:
            root = pathlib.Path(directory)
            for name in ["pi4ioe5v6408", "gt911"]:
                (root / (name + ".rs")).write_bytes(snapshot["esp-periph/src/i2c/" + name + ".rs"])
            harness = '#![allow(dead_code)]\nmod i2c {\n' + trait
            for name in ["pi4ioe5v6408", "gt911"]:
                harness += '#[path=' + json.dumps(str(root / (name + ".rs"))) + '] pub mod ' + name + ';\n'
            harness += '''}
#[test]
fn tab5_lcd_reset_uses_input_pullup_release() {
    use i2c::{I2cDevice, pi4ioe5v6408::{Pi4ioState, Pi4ioe5v6408}};
    use std::sync::{Arc,Mutex};
    let state=Arc::new(Mutex::new(Pi4ioState::default()));
    let mut device=Pi4ioe5v6408::new(state.clone());
    // M5Tab5-UserDemo fe8aa051, bsp_io_expander_pi4ioe_init.
    for (reg,value) in [(1,255),(13,127),(11,127),(5,102),(7,0),(3,127)] {
        assert!(device.start(false)); assert!(device.write(reg)); assert!(device.write(value));
    }
    assert_eq!(state.lock().unwrap().drive(4),Some(false));
    assert!(device.start(false)); assert!(device.write(3)); assert!(device.write(111));
    assert_eq!(state.lock().unwrap().drive(4),None);
    assert_eq!(state.lock().unwrap().input() & 16,16);
    assert!(device.start(false)); assert!(device.write(5)); assert!(device.start(true));
    assert_eq!(device.read() & 16,0,"reset latch remains low while the input pull-up releases the net");
}
'''
            (root / "probe.rs").write_text(harness)
            print(subprocess.check_output(["rustc", "--version"], text=True).strip(), flush=True)
            subprocess.run(["rustc", "--edition=2021", "--test", str(root / "probe.rs"), "-o", str(root / "probe")], check=True)
            subprocess.run([str(root / "probe")], check=True)
        print("PASS: isolated device tests only; no P4 I2C MMIO, board wiring, DSI or guest firmware was exercised.")


if __name__ == "__main__":
    main()
