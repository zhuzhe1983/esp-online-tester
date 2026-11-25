#!/usr/bin/env python3
"""Compile the actual TIMG and LP watchdog modules without a workspace build."""
import argparse
import json
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[3])
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    root = args.repo.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    modules = {"timg_wdt": root / "esp32c5/src/timg_wdt.rs",
               "lp_wdt": root / "esp32c5/src/lp_wdt.rs",
               "races": Path(__file__).resolve().with_name("controller-races.rs")}
    harness = output / "mwdt-harness.rs"
    harness.write_text("".join("#[path=" + json.dumps(str(path), ensure_ascii=False)
                               + "] mod " + name + ";\n" for name, path in modules.items()))
    commands = [["rustc", "+stable", "--edition=2021", "--test", str(harness), "-o", str(output / "mwdt-tests")],
                [str(output / "mwdt-tests"), "--test-threads=1"],
                ["rustup", "run", "stable", "clippy-driver", "--edition=2021", "--test", str(harness),
                 "-D", "warnings", "-o", str(output / "mwdt-lint-tests")]]
    for index, command in enumerate(commands):
        result = subprocess.run(command, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        (output / f"check-{index}.txt").write_text(result.stdout)
        print(result.stdout, end="")
        result.check_returncode()


if __name__ == "__main__":
    main()
