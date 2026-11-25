#!/usr/bin/env python3
"""Compile/test only the C5 BootApm module; no Cargo or original-firmware run."""
import argparse
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
binary = out / "c5-apm-owned-tests"
subprocess.run([args.rustc, "--edition=2021", "--test", "esp32c5/src/boot_apm.rs",
                "-o", str(binary)], cwd=repo, check=True)
result = subprocess.run([str(binary), "--test-threads=1"], capture_output=True, text=True)
text = result.stdout + result.stderr
(out / "owned-tests-raw.txt").write_text(text)
print(text, end="")
raise SystemExit(result.returncode)
