#!/usr/bin/env python3
"""Inspect diagnostic products and reject mutated pure copies; no build/guest."""
import argparse
import importlib.util
import json
from pathlib import Path
import shutil


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference", type=Path, required=True)
    parser.add_argument("--scratch", type=Path, required=True)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[3]
    helper = load("diag_build", repo / "tools/c5-lcd273-diagnostic-build.py")
    ref = load("ref_build", repo / "tools/c5-lcd273-reference-build.py")
    root = args.reference.expanduser().resolve()
    current = helper.inspect(root, repo)
    original = json.loads((root / "summary.json").read_text())
    assert all(current[key] == original[key] for key in current)
    artifacts_before = dict(current["artifacts"])
    scratch = ref.fresh(repo, args.scratch)
    results = [{"check": "full inspect and all saved artifact/source/config hashes", "passed": True}]
    cases = [("non-whitelist CONFIG change", "sdkconfig", lambda b: b.replace(b"CONFIG_SPIRAM_USE_MALLOC=y\n", b"# CONFIG_SPIRAM_USE_MALLOC is not set\n")),
             ("configuration comment byte change", "sdkconfig", lambda b: b + b"\n# byte-only fixture\n"),
             ("own main source byte change", "project/main/main.c", lambda b: b + b"\n// source fixture\n"),
             ("upstream lock byte change", "project/dependencies.lock", lambda b: b + b"\n")]
    for index, (label, name, mutate) in enumerate(cases):
        copied = ref.fresh(repo, scratch / str(index))
        (copied / "build").symlink_to(root / "build", target_is_directory=True)
        (copied / "project/main").mkdir(parents=True)
        for source in ["sdkconfig", "project/sdkconfig", "project/dependencies.lock", "project/CMakeLists.txt",
                       "project/main/CMakeLists.txt", "project/main/main.c", "project/LICENSE"]:
            shutil.copy2(root / source, copied / source)
        data = (copied / name).read_bytes()
        bad = mutate(data)
        assert bad != data
        (copied / name).write_bytes(bad)
        try:
            helper.inspect(copied, repo)
        except ValueError as error:
            results.append({"check": label, "rejected": True, "reason": str(error)})
        else:
            raise AssertionError(label + " accepted")
    assert helper.inspect(root, repo)["artifacts"] == artifacts_before
    results.append({"check": "real reference remains unchanged after copy-only fixtures", "passed": True})
    print(json.dumps({"count": len(results), "checks": results, "all_passed": True,
                      "guest_executed": False, "build_executed": False}, indent=2))


if __name__ == "__main__":
    main()
