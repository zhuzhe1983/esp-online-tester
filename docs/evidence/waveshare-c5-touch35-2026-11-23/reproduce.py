#!/usr/bin/env python3
"""Regenerate ignored native captures; optionally measure the unchanged ROM boundary."""
from pathlib import Path
import argparse
import hashlib
import json
import os
import subprocess

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "target/c5-touch35-reproduced")
    parser.add_argument("--rom", type=Path)
    parser.add_argument("--factory", type=Path)
    args = parser.parse_args()
    args.output = args.output.resolve()
    if args.rom:
        args.rom = args.rom.resolve()
    if args.factory:
        args.factory = args.factory.resolve()
    args.output.mkdir(parents=True, exist_ok=True)
    result = json.loads((HERE / "inputs-and-hashes.json").read_text())
    app = ROOT / "firmware/c5-touch35-probe/probe.bin"
    assert digest(app) == result["inputs"]["app"]["sha256"], "App bytes differ from measured input"
    env = os.environ.copy()
    env.setdefault("CARGO_BUILD_JOBS", "1")
    env.setdefault("CARGO_PROFILE_RELEASE_LTO", "false")
    env.setdefault("CARGO_TARGET_DIR", "target/c5-touch35-reproduction-build")
    command = ["cargo", "run", "-p", "esp32c5", "--release", "--example", "touch35_probe", "--", str(app), str(args.output)]
    subprocess.run(command, cwd=ROOT, env=env, check=True)
    for item in result["artifacts"]:
        if item["name"].endswith(".rgb565le") or item["name"] in ("uart.log", "execution.json", "board-report.txt"):
            assert digest(args.output / item["name"]) == item["sha256"], item["name"]
    if bool(args.rom) != bool(args.factory):
        parser.error("--rom and --factory must be supplied together")
    if args.rom:
        assert digest(args.rom) == result["inputs"]["rom"]["sha256"], "ROM differs"
        assert digest(args.factory) == result["inputs"]["factory"]["sha256"], "factory image differs"
        p = subprocess.run(["cargo", "run", "-p", "esp32c5", "--release", "--example", "factory_boundary", "--",
            "waveshare-c5-touch35", "32", "8", str(args.rom), str(args.factory), "1000000"],
            cwd=ROOT, env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, check=True)
        (args.output / "factory.log").write_text(p.stdout)
        assert "steps=6831" in p.stdout and "address: 1611333860" in p.stdout
        assert "uartBytes=0 nonzeroPixels=0" in p.stdout
    print("C5 Touch3.5 SRAM/input/SD captures reproduced; factory remains unverified.")

if __name__ == "__main__":
    main()
