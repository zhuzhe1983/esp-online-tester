#!/usr/bin/env python3
"""Check fresh-output guards and retained LCD273 images without building/running."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference", action="append", required=True, type=Path)
    parser.add_argument("--inputs", required=True, type=Path)
    parser.add_argument("--scratch", required=True, type=Path)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[3]
    helper = module("lcd273_build", repo / "tools/c5-lcd273-reference-build.py")
    checks = module("c5_images", repo / "tools/c5-idf-hello-reference-build.py")
    scratch = helper.fresh(repo, args.scratch)
    results = []

    def snapshot():
        return {p.relative_to(scratch).as_posix(): (["link", os.readlink(p)] if p.is_symlink()
                else ["file", hashlib.sha256(p.read_bytes()).hexdigest()] if p.is_file() else ["directory"])
                for p in sorted(scratch.rglob("*"))}

    occupied = scratch / "occupied"
    occupied.mkdir()
    for name in ["canary", "build-helper.py", "state.json", "summary.json"]:
        (occupied / name).write_text("preserve " + name + "\n")
    (scratch / "file").write_text("preserve file\n")
    (scratch / "link").symlink_to(occupied, target_is_directory=True)
    (scratch / "dangling").symlink_to(scratch / "not-created")
    (scratch / "parent-link").symlink_to(occupied, target_is_directory=True)
    empty = scratch / "empty"
    empty.mkdir()
    for label, path in [("existing directory with canaries", occupied), ("existing empty directory", empty),
                        ("existing file", scratch / "file"), ("symlink", scratch / "link"),
                        ("dangling symlink", scratch / "dangling"), ("symlink ancestor", scratch / "parent-link/new"),
                        ("target root", repo / "target"), ("parent traversal", scratch / "../not-created"),
                        ("outside ignored target", repo / "not-created-by-lcd273")]:
        before = snapshot()
        target_entries = sorted(p.name for p in (repo / "target").iterdir())
        try:
            helper.fresh(repo, path)
        except ValueError:
            pass
        else:
            raise AssertionError(label + " accepted")
        assert snapshot() == before and target_entries == sorted(p.name for p in (repo / "target").iterdir())
        results.append({"check": label, "rejected": True, "canaries_unchanged": True})
    leaf = helper.fresh(repo, scratch / "atomic")
    try:
        helper.fresh(repo, leaf)
    except ValueError:
        results.append({"check": "second writer rejected after atomic reservation", "passed": True})
    else:
        raise AssertionError("second writer accepted")

    def reject(label, function, value):
        try:
            function(value)
        except ValueError:
            results.append({"check": label, "rejected": True})
        else:
            raise AssertionError(label + " accepted")

    tree_bytes = (args.inputs / "manufacturer-tree.json").read_bytes()
    tree = helper.validate_tree(tree_bytes)
    helper.validate_tree(json.dumps(tree, indent=4).encode())
    results.append({"check": "all 382 Git tree objects and equivalent JSON serialization", "passed": True})
    bad_tree = json.loads(tree_bytes)
    bad_tree["tree"][0]["sha"] = "0" * 40
    reject("changed Git tree child object", helper.validate_tree, json.dumps(bad_tree).encode())
    for directory in args.reference:
        root = directory.resolve()
        saved = json.loads((root / "summary.json").read_text())
        current = helper.static_inspect(root)
        assert all(current[k] == saved[k] for k in current)
        helper.validate_sources(args.inputs.resolve(), root / "project", saved["case"])
        results.append({"check": saved["case"] + " inspect/source/artifact hashes unchanged", "passed": True})
    root = args.reference[0].resolve()
    build = root / "build"
    original_config = (root / "project/sdkconfig").read_bytes()
    effective_before = helper.record(root / "sdkconfig")
    non_whitelist = original_config.replace(b"CONFIG_SPIRAM_USE_MALLOC=y\n", b"# CONFIG_SPIRAM_USE_MALLOC is not set\n")
    assert non_whitelist != original_config
    for name, data in [("other-config-change", non_whitelist),
                       ("comment-bytes-change", original_config + b"\n# static byte-identity fixture\n")]:
        copied = helper.fresh(repo, scratch / name)
        (copied / "build").symlink_to(build, target_is_directory=True)
        (copied / "project").mkdir()
        (copied / "project/sdkconfig").write_bytes(original_config)
        (copied / "sdkconfig").write_bytes(data)
        if name == "comment-bytes-change":
            assert helper.config_values(copied / "sdkconfig") == helper.config_values(copied / "project/sdkconfig")
        reject(name + " rejected by complete inspect", helper.static_inspect, copied)
    assert helper.record(root / "sdkconfig") == effective_before
    assert (root / "project/sdkconfig").read_bytes() == original_config
    results.append({"check": "configuration fixtures left real reference bytes unchanged", "passed": True})
    raw = json.loads((build / "flasher_args.json").read_text())
    chunks = [(int(offset, 0), name, (build / name).read_bytes()) for offset, name in raw["flash_files"].items()]
    merged = (build / "merged.bin").read_bytes()
    reject("appended RAW byte", lambda value: checks.verify_flash_layout(value, chunks), merged + b"\xff")
    for label, index in [("RAW prefix mutation", 0), ("RAW gap mutation", 0x7fff)]:
        bad = bytearray(merged)
        bad[index] ^= 1
        reject(label, lambda value: checks.verify_flash_layout(value, chunks), bytes(bad))
    reject("missing bootloader", lambda value: checks.verify_flash_layout(merged, value),
           [x for x in chunks if x[1] != "bootloader/bootloader.bin"])
    reject("overlapping flash ranges", lambda value: checks.verify_flash_layout(merged, value),
           [(0x2000 if "partition" in name else offset, name, data) for offset, name, data in chunks])
    app = (build / "01_factory.bin").read_bytes()
    image = checks.image_info(app)
    for label, offset in [("image XOR mutation", image["segments"][0]["file_offset"] + 4),
                          ("image appended SHA mutation", image["checksum_offset"] + 1)]:
        bad = bytearray(app)
        bad[offset] ^= 1
        reject(label, checks.image_info, bytes(bad))
    bad = bytearray((build / "partition_table/partition-table.bin").read_bytes())
    bad[12] ^= 1
    reject("partition MD5 mutation", checks.partition_info, bytes(bad))
    bad = bytearray((build / "01_factory.elf").read_bytes())
    bad[18:20] = b"\0\0"
    reject("non-RISC-V ELF", checks.elf_info, bytes(bad))
    print(json.dumps({"checks": results, "count": len(results), "all_passed": True,
                      "guest_executed": False, "build_executed": False}, indent=2))


if __name__ == "__main__":
    main()
