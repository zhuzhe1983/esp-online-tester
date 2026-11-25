#!/usr/bin/env python3
"""Check the actual modem LP clock module without a shared build."""
import argparse
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[3])
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    source = args.repo.resolve() / "esp32c5/src/modem_lp_clock.rs"
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    commands = [["rustc", "+stable", "--edition=2021", "--test", str(source), "-o", str(output / "coex-tests")],
                [str(output / "coex-tests"), "--test-threads=1"],
                ["rustup", "run", "stable", "clippy-driver", "--edition=2021", "--test", str(source),
                 "-D", "warnings", "-o", str(output / "coex-lint-tests")]]
    for index, command in enumerate(commands):
        result = subprocess.run(command, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        (output / f"check-{index}.txt").write_text(result.stdout)
        print(result.stdout, end="")
        result.check_returncode()


if __name__ == "__main__":
    main()
