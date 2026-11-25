#!/usr/bin/env python3
"""Check the actual C5 clock modules without workspace/shared builds."""
import argparse
import json
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[3])
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    source = args.repo.resolve() / "esp32c5/src"
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    harness = output / "sec-clock-harness.rs"
    harness.write_text("\n".join("#[path=" + json.dumps(str(source / (name + ".rs")), ensure_ascii=False)
                                 + "] mod " + name + ";" for name in
                                 ["clock_calibration", "clock_tree", "boot_clock"]) + "\n")
    commands = [["rustc", "+stable", "--edition=2021", "--test", str(harness), "-o", str(output / "sec-clock-tests")],
                [str(output / "sec-clock-tests"), "--test-threads=1"],
                ["rustup", "run", "stable", "clippy-driver", "--edition=2021", "--test", str(harness),
                 "-D", "warnings", "-o", str(output / "sec-clock-lint-tests")]]
    for index, command in enumerate(commands):
        result = subprocess.run(command, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        (output / f"check-{index}.txt").write_text(result.stdout)
        print(result.stdout, end="")
        result.check_returncode()


if __name__ == "__main__":
    main()
