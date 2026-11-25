#!/usr/bin/env python3
"""Compile/test/lint only the standalone C5 software interrupt module."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    repo = args.repo.resolve()
    output = args.output_dir.resolve()
    source = "esp32c5/src/software_interrupt.rs"
    if not (repo / source).is_file():
        parser.error("--repo must contain esp32c5/src/software_interrupt.rs")
    output.mkdir(parents=True, exist_ok=True)

    def normalized(value):
        return value.replace(str(output), "<owned-output>").replace(str(repo), "<repo>")

    commands = [
        ["rustup", "run", "stable", "rustc", "--edition=2021", "--test", source,
         "-D", "warnings", "-o", str(output / "software-interrupt-tests")],
        [str(output / "software-interrupt-tests"), "--test-threads=1"],
        ["rustup", "run", "stable", "clippy-driver", "--edition=2021", "--test", source,
         "-D", "warnings", "-o", str(output / "software-interrupt-clippy")],
    ]
    results = []
    status = 0
    for index, command in enumerate(commands):
        result = subprocess.run(command, cwd=repo, text=True, stdout=subprocess.PIPE,
                                stderr=subprocess.STDOUT, check=False)
        raw = result.stdout.encode()
        text = normalized(result.stdout)
        (output / f"check-{index}.txt").write_text(text)
        results.append({"command": [normalized(value) for value in command],
                        "exitCode": result.returncode,
                        "rawOutputSha256": hashlib.sha256(raw).hexdigest(),
                        "normalizedOutputSha256": hashlib.sha256(text.encode()).hexdigest(),
                        "outputFile": f"check-{index}.txt"})
        print(text, end="")
        if result.returncode:
            status = result.returncode
            break
    versions = {}
    for name in ["rustc", "clippy-driver"]:
        result = subprocess.run(["rustup", "run", "stable", name, "--version"],
                                text=True, stdout=subprocess.PIPE,
                                stderr=subprocess.STDOUT, check=False)
        versions[name] = normalized(result.stdout.strip())
    summary = {"scope": "Standalone owned module; no Cargo, workspace, native guest, WASM or benchmark",
               "source": source,
               "sourceSha256": hashlib.sha256((repo / source).read_bytes()).hexdigest(),
               "tools": versions, "checks": results,
               "normalization": "Caller-supplied repository/output paths replaced by labels; no other text changed.",
               "exitCode": status}
    (output / "owned-checks.json").write_text(json.dumps(summary, indent=2) + "\n")
    raise SystemExit(status)


if __name__ == "__main__":
    main()
