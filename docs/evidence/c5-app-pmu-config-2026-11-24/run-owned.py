#!/usr/bin/env python3
"""Compile only the PMU/Bias/Analog component harness; no Cargo or ROM run."""
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
harness = out / "c5-app-pmu-harness.rs"
modules = [("boot_pmu.rs", "boot_pmu"), ("boot_bias.rs", "boot_bias"),
           ("boot_analog.rs", "boot_analog")]
harness.write_text("\n".join(
    "#[path=" + json.dumps(str(repo / "esp32c5/src" / source), ensure_ascii=False)
    + "] mod " + module + ";" for source, module in modules) + "\n")
binary = out / "c5-app-pmu-tests"
subprocess.run([args.rustc, "--edition=2021", "--test", str(harness),
                "-o", str(binary)], check=True)
result = subprocess.run([str(binary), "--nocapture", "--test-threads=1"],
                        capture_output=True, text=True)
text = result.stdout + result.stderr
(out / "owned-tests-raw.txt").write_text(text)
print(text, end="")
raise SystemExit(result.returncode)
