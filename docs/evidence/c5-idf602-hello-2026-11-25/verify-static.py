#!/usr/bin/env python3
"""Exercise output-preservation and image guards; never build or run a guest."""

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
from types import SimpleNamespace


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-dir", required=True, type=Path)
    parser.add_argument("--historical-reference", action="append", default=[], type=Path)
    parser.add_argument("--case-dir", required=True, type=Path, help="Fresh directory under this checkout's target/")
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[3]
    spec = importlib.util.spec_from_file_location("c5_reference", repo / "tools/c5-idf-hello-reference-build.py")
    helper = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helper)
    scratch = helper.reserve_work_dir(repo, args.case_dir)
    results = []

    def snapshot():
        rows = {}
        for path in sorted(scratch.rglob("*")):
            name = str(path.relative_to(scratch))
            if path.is_symlink():
                rows[name] = ["symlink", os.readlink(path)]
            elif path.is_file():
                rows[name] = ["file", hashlib.sha256(path.read_bytes()).hexdigest()]
            else:
                rows[name] = ["directory"]
        return rows

    occupied = scratch / "occupied"
    occupied.mkdir()
    for name in ["canary", "build-helper.py", "state.json", "summary.json"]:
        (occupied / name).write_text("preserve " + name + "\n")
    (scratch / "file").write_text("preserve file\n")
    (scratch / "link").symlink_to(occupied, target_is_directory=True)
    (scratch / "dangling").symlink_to(scratch / "not-created")
    (scratch / "parent-link").symlink_to(occupied, target_is_directory=True)
    cases = [("existing directory", occupied), ("existing file", scratch / "file"),
             ("symlink", scratch / "link"), ("dangling symlink", scratch / "dangling"),
             ("symlink ancestor", scratch / "parent-link/new"), ("target root", repo / "target"),
             ("parent traversal", scratch / "../not-created"), ("outside target", repo / "not-created-by-reference")]
    for label, work in cases:
        before = snapshot()
        entries = sorted(p.name for p in (repo / "target").iterdir())
        try:
            helper.build_reference(SimpleNamespace(work_dir=work))
        except ValueError as error:
            results.append({"check": label, "rejected": True, "error": str(error), "canary_and_tree_unchanged": True})
        else:
            raise AssertionError(label + " was accepted")
        assert snapshot() == before and entries == sorted(p.name for p in (repo / "target").iterdir())
    fresh = scratch / "atomic-fresh"
    helper.reserve_work_dir(repo, fresh)
    try:
        helper.reserve_work_dir(repo, fresh)
    except ValueError:
        results.append({"check": "atomic fresh reservation refuses second writer", "rejected": True})
    else:
        raise AssertionError("second writer accepted")

    def reject(label, operation, data):
        try:
            operation(data)
        except ValueError as error:
            results.append({"check": label, "rejected": True, "error": str(error)})
        else:
            raise AssertionError(label + " was accepted")

    work = args.reference_dir.resolve()
    build = work / "project/build"
    raw = json.loads((build / "flasher_args.json").read_text())
    chunks = [(int(offset, 0), name, (build / name).read_bytes()) for offset, name in raw["flash_files"].items()]
    merged = (build / "merged.bin").read_bytes()
    reject("appended RAW byte", lambda value: helper.verify_flash_layout(value, chunks), merged + b"\xff")
    for label, index in [("RAW prefix mutation", 0), ("RAW gap mutation", 0x7fff)]:
        bad = bytearray(merged)
        bad[index] ^= 1
        reject(label, lambda value: helper.verify_flash_layout(value, chunks), bytes(bad))
    reject("missing bootloader", lambda value: helper.verify_flash_layout(merged, value),
           [row for row in chunks if row[1] != "bootloader/bootloader.bin"])
    reject("overlapping flash inputs", lambda value: helper.verify_flash_layout(merged, value),
           [(0x2000 if "partition" in name else offset, name, data) for offset, name, data in chunks])
    app = (build / "hello_world.bin").read_bytes()
    image = helper.image_info(app)
    for label, index in [("segment XOR mutation", image["segments"][0]["file_offset"] + 4),
                         ("appended SHA mutation", image["checksum_offset"] + 1)]:
        bad = bytearray(app)
        bad[index] ^= 1
        reject(label, helper.image_info, bytes(bad))
    bad = bytearray(app)
    bad[28:32] = b"\xff" * 4
    reject("oversized segment", helper.image_info, bytes(bad))
    bad = bytearray((build / "partition_table/partition-table.bin").read_bytes())
    bad[12] ^= 1
    reject("partition MD5 mutation", helper.partition_info, bytes(bad))
    bad = bytearray((build / "hello_world.elf").read_bytes())
    bad[18:20] = b"\x00\x00"
    reject("non-RISC-V ELF", helper.elf_info, bytes(bad))
    for directory in [*args.historical_reference, work]:
        original = json.loads((directory / "summary.json").read_text())
        assert helper.inspect(directory)["static_verification_passed"]
        for name, expected in original["artifacts"].items():
            path = directory / ("project" if name == "sdkconfig" else "project/build") / name
            assert helper.file_record(path) == expected
        results.append({"check": "read-only inspect and all retained artifact hashes " + original["sdk"]["version"], "passed": True})
    print(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()
