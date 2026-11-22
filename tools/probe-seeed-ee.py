#!/usr/bin/env python3
"""ROM boot pinned EE factory segments and retain compact, privacy-safe results.

--layouts is an explicit caller-supplied array with name, manifest and parts;
every part needs offset, bytes, sha256, localPath and url. No downloads or
modified firmware. Raw logs/captures stay only in the chosen output folder.
This records incomplete/blocked runs as such; a process exit is not UI proof.
"""
import argparse
from collections import Counter
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import subprocess
import time

from PIL import Image

ROM_SHA = "c0ce0f338d1de1bdc6efbef1591779a2a42c1ab7d759d3c6ae8ae63a7dd34cfd"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def checked_file(path, sha, size=None):
    data = path.read_bytes()
    if digest(data) != sha or (size is not None and len(data) != size):
        raise ValueError(f"Checksum/size mismatch: {path.name}")
    return data


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--cli", type=Path, required=True)
    p.add_argument("--rom", type=Path, required=True)
    p.add_argument("--layouts", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    p.add_argument("--boards", nargs="+")
    p.add_argument("--seconds", type=float, default=12)
    p.add_argument("--timeout", type=float, default=240)
    p.add_argument("--script", type=Path)
    args = p.parse_args()
    if args.seconds <= 0 or args.timeout <= 0:
        p.error("seconds and timeout must be positive")
    args.cli, args.rom, args.output = args.cli.resolve(), args.rom.resolve(), args.output.resolve()
    checked_file(args.rom, ROM_SHA)
    if args.output.exists():
        p.error("output must be a fresh directory; previous evidence is never overwritten")
    args.output.mkdir(parents=True)
    records = []
    for layout in json.loads(args.layouts.read_text()):
        match = re.fullmatch(r"SenseCraft_HMI_(EE0[2345])_(P[0-9]+_(?:MONO|QUAD|SP6))", layout["name"])
        if match is None:
            raise ValueError("Unexpected EE firmware target")
        board = "seeed-xiao-epaper-" + "-".join(match.groups()).lower().replace("_", "-")
        if args.boards and board not in args.boards:
            continue
        folder = args.output / board
        folder.mkdir()
        command = [str(args.cli), "--chip", "s3", "--board", board, "--rom", str(args.rom), "--boot", "rom", "--flash-mb", "16", "--psram-mb", "8", "--max-seconds", str(args.seconds), "--console", "both", "--no-dump", "--tft-png", str(folder / "frame.png")]
        occupied = []
        source = []
        for part in layout["parts"]:
            offset = int(str(part["offset"]), 0)
            file = Path(part["localPath"])
            checked_file(file, part["sha256"], part["bytes"])
            end = offset + part["bytes"]
            if not 0 <= offset < end <= 16 << 20 or any(offset < b and a < end for a, b in occupied):
                raise ValueError("Overlapping or out-of-bounds firmware segment")
            occupied.append((offset, end))
            command += ["--flash-at", f"0x{offset:x}={file}"]
            source.append({k: part[k] for k in ("offset", "url", "sha256", "bytes")})
        if args.script:
            command += ["--script", str(args.script.resolve())]
        started = time.monotonic()
        try:
            run = subprocess.run(command, capture_output=True, timeout=args.timeout)
            raw, code, timed_out = run.stdout + run.stderr, run.returncode, False
        except subprocess.TimeoutExpired as e:
            raw, code, timed_out = (e.stdout or b"") + (e.stderr or b""), None, True
        (folder / "raw.log").write_bytes(raw)
        text = raw.decode("utf-8", "replace")
        account = re.search(r"core0 (\d+) \+ core1 (\d+) insns.*?emulated ([\d.]+)s \((\d+) cycles\)", text)
        status = re.search(r"^\[" + re.escape(board) + r"\] (.+)$", text, re.M)
        record = {"board": board, "parts": source, "exitCode": code, "timedOut": timed_out, "wallSeconds": round(time.monotonic() - started, 3), "requestedGuestSeconds": args.seconds, "romBoot": "ROM boot from reset vector 0x40000400" in text, "consoleSha256": digest(raw), "consoleBytes": len(raw), "boardReport": status[1] if status else None, "chipResetCount": text.count("[emu] chip reset at"), "screenshot": None}
        if account:
            record.update(instructions=[int(account[1]), int(account[2])], virtualSeconds=float(account[3]), cycles=int(account[4]))
        png = folder / "frame.png"
        if png.exists():
            im = Image.open(png).convert("RGB")
            colors = Counter(im.getdata())
            record["screenshot"] = {"size": list(im.size), "sha256": digest(png.read_bytes()), "rgbSha256": digest(im.tobytes()), "colors": len(colors), "nonWhitePixels": im.width * im.height - colors[(255, 255, 255)]}
        records.append(record)
        receipt = {"schema": 1, "verifiedAt": datetime.now(timezone.utc).isoformat(), "cliSha256": digest(args.cli.read_bytes()), "romSha256": ROM_SHA, "layoutsSha256": digest(args.layouts.read_bytes()), "mode": "unmodified-official-segments-native-interpreter-rom-boot", "flashMiB": 16, "psramMiB": 8, "script": args.script.read_text().splitlines() if args.script else [], "scope": "digital board subset; inspect captures and firmware output before calling a factory UI verified", "privacy": "Raw console/captures may contain synthetic provisioning identifiers and are not committed. Retained hashes identify the unredacted captures without disclosing their content.", "runs": records}
        (args.output / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
        pixels = record["screenshot"]["nonWhitePixels"] if record["screenshot"] else None
        print(f"{board}: exit={code}, timeout={timed_out}, nonWhitePixels={pixels}", flush=True)
    if not records:
        raise ValueError("No requested board found in the supplied layouts")


if __name__ == "__main__":
    main()
