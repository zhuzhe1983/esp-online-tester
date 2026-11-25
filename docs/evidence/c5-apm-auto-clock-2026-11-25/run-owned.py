#!/usr/bin/env python3
"""Run only the actual BootApm module's tests and Clippy; never invoke Cargo."""
import argparse
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--repo", type=Path, required=True)
parser.add_argument("--output-dir", type=Path, required=True)
parser.add_argument("--rustc", default="rustc")
parser.add_argument("--clippy-driver", default="clippy-driver")
args = parser.parse_args()
repo = args.repo.resolve()
out = args.output_dir.resolve()
out.mkdir(parents=True, exist_ok=True)
source = "esp32c5/src/boot_apm.rs"
binary = out / "c5-apm-auto-clock-tests"
for name, compiler in [("rustc", args.rustc), ("clippy", args.clippy_driver)]:
    result = subprocess.run([compiler, "--edition=2021", "--test", source,
                             "-D", "warnings", "-o", str(binary)],
                            cwd=repo, capture_output=True, text=True)
    text = result.stdout + result.stderr
    (out / (name + "-compile.txt")).write_text(text)
    print(text, end="")
    if result.returncode:
        raise SystemExit(result.returncode)
result = subprocess.run([str(binary), "--test-threads=1"], capture_output=True, text=True)
text = result.stdout + result.stderr
(out / "owned-tests-raw.txt").write_text(text)
print(text, end="")
raise SystemExit(result.returncode)
