#!/usr/bin/env python3
"""Compile only C5 BootClock/ClockTree/calibration, without Cargo/Transport."""
import argparse
import json
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--repo", type=Path, required=True)
parser.add_argument("--output-dir", type=Path, required=True)
parser.add_argument("--rustc", default="rustc")
args = parser.parse_args()
repo = args.repo.resolve()
out = args.output_dir.resolve()
out.mkdir(parents=True, exist_ok=True)
harness = out / "c5-pll-div-clock-harness.rs"
harness.write_text("\n".join(
    "#[path=" + json.dumps(str(repo / "esp32c5/src" / (name + ".rs")))
    + "] mod " + name + ";"
    for name in ["clock_calibration", "clock_tree", "boot_clock"]) + "\n")
binary = out / "c5-pll-div-clock-tests"
subprocess.run([args.rustc, "--edition=2021", "--test", str(harness),
                "-o", str(binary)], check=True)
result = subprocess.run([str(binary), "--test-threads=1"], capture_output=True, text=True)
text = result.stdout + result.stderr
(out / "owned-tests-raw.txt").write_text(text)
print(text, end="")
raise SystemExit(result.returncode)
