#!/usr/bin/env python3
"""Copy the reproducible StopWatch fixture build into the public demo assets."""
from pathlib import Path
import hashlib
import json
import shutil

root = Path(__file__).resolve().parents[1]
build = root / "examples/platform-stopwatch/build"
public = root / "web/wasm/fw/public"
files = {
    "bootloader/bootloader.bin": "platform-stopwatch-bootloader.bin",
    "partition_table/partition-table.bin": "platform-stopwatch-ptable.bin",
    "platform_stopwatch.bin": "platform-stopwatch.bin",
}
for source in files:
    if not (build / source).is_file():
        raise SystemExit("Build the fixture with ESP-IDF v5.5.1 before packaging it.")
hashes = []
manifest_path = root / "web/wasm/fw/demo-stopwatch.json"
manifest = json.loads(manifest_path.read_text())
for source, name in files.items():
    shutil.copyfile(build / source, public / name)
    hashes.append(f"{hashlib.sha256((public / name).read_bytes()).hexdigest()}  {name}\n")
    manifest.setdefault("artifact_sha256", {})["public/" + name] = hashlib.sha256((public / name).read_bytes()).hexdigest()
(public / "platform-stopwatch.sha256").write_text("".join(hashes))
manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
print("Packaged StopWatch firmware and SHA-256 receipts.")
