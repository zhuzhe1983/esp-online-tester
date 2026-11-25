#!/usr/bin/env python3
"""Compile the actual LP watchdog modules without a workspace build."""
import argparse
import json
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[3])
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    source = args.repo.resolve() / "esp32c5/src/lp_wdt.rs"
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    harness = output / "swd-fast-harness.rs"
    harness.write_text("#[path=" + json.dumps(str(source)) + "] mod lp_wdt;\n")
    tests = output / "swd-fast-tests"
    lint_tests = output / "swd-fast-lint-tests"
    subprocess.run(["rustc", "+stable", "--edition=2021", "--test", str(harness),
                    "-o", str(tests)], check=True)
    subprocess.run([str(tests), "--test-threads=1"], check=True)
    subprocess.run(["rustup", "run", "stable", "clippy-driver", "--edition=2021",
                    "--test", str(harness), "-D", "warnings", "-o", str(lint_tests)], check=True)


if __name__ == "__main__":
    main()
