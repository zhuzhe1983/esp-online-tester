#!/usr/bin/env python3
"""Compile and test only the actual C5 WiFi request and BootAnalog modules, without Cargo."""
import argparse
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--repo", type=Path, required=True)
parser.add_argument("--output-dir", type=Path, required=True)
parser.add_argument("--compiler", default="rustc")
parser.add_argument("--lint", action="store_true")
args = parser.parse_args()
out = args.output_dir.resolve()
out.mkdir(parents=True, exist_ok=True)
name = "owned-lint" if args.lint else "owned-tests"
command = [args.compiler, "--edition=2021", "--test", "docs/evidence/c5-wifi-clock-requests-2026-11-25/owned-harness.rs",
           "-o", str(out / name)]
if args.lint:
    command.extend(["-D", "warnings"])
subprocess.run(command, cwd=args.repo.resolve(), check=True)
result = subprocess.run([str(out / name), "--test-threads=1"], capture_output=True, text=True)
text = (result.stdout + result.stderr).strip() + "\n"
(out / (name + ".txt")).write_text(text)
print(text, end="")
raise SystemExit(result.returncode)
