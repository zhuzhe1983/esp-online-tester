#!/usr/bin/env python3
"""Test the actual C5 clock modules without compiling the workspace."""
import argparse
import json
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[3])
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    root = args.repo.resolve() / "esp32c5/src"
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    harness = output / "clock-controls-harness.rs"
    modules = ["clock_calibration", "clock_tree", "boot_clock"]
    harness.write_text("\n".join("#[path=" + json.dumps(str(root / (m + ".rs")), ensure_ascii=False)
                                 + "] mod " + m + ";" for m in modules) + "\n")
    tests = output / "clock-controls-tests"
    lint_tests = output / "clock-controls-lint-tests"
    subprocess.run(["rustc", "+stable", "--edition=2021", "--test", str(harness),
                    "-o", str(tests)], check=True)
    subprocess.run([str(tests), "--test-threads=1"], check=True)
    subprocess.run(["rustup", "run", "stable", "clippy-driver", "--edition=2021",
                    "--test", str(harness), "-D", "warnings", "-o", str(lint_tests)], check=True)


if __name__ == "__main__":
    main()
