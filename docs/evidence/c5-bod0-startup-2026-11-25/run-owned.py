#!/usr/bin/env python3
"""Build only C5 BootBrownout/BootAnalog standalone tests; no Cargo."""
import argparse
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--repo", type=Path, required=True)
parser.add_argument("--output-dir", type=Path, required=True)
parser.add_argument("--compiler", default="rustc")
parser.add_argument("--lint", action="store_true")
args = parser.parse_args()
repo = args.repo.resolve()
out = args.output_dir.resolve()
out.mkdir(parents=True, exist_ok=True)
binary = out / ("c5-bod0-owned-lint" if args.lint else "c5-bod0-owned-tests")
command = [args.compiler, "--edition=2021", "--test",
           "docs/evidence/c5-bod0-startup-2026-11-25/owned-harness.rs",
           "-o", str(binary)]
if args.lint:
    command.extend(["-D", "warnings"])
subprocess.run(command, cwd=repo, check=True)
result = subprocess.run([str(binary), "--test-threads=1"], capture_output=True, text=True)
text = (result.stdout + result.stderr).strip() + "\n"
(out / ("owned-lint-tests.txt" if args.lint else "owned-tests.txt")).write_text(text)
print(text, end="")
raise SystemExit(result.returncode)
