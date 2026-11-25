#!/usr/bin/env python3
"""Use temporary fake tools to check resolution/delegation; no real build/guest."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import subprocess


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scratch", type=Path, required=True)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[3]
    helper = load("diag_tools", repo / "tools/c5-lcd273-diagnostic-build.py")
    ref = load("ref_output", repo / "tools/c5-lcd273-reference-build.py")
    root = ref.fresh(repo, args.scratch)
    bindir, wrappers = root / "bin", root / "wrapper"
    bindir.mkdir()
    wrappers.mkdir()
    for name in ["cmake", "ninja"]:
        fake = bindir / name
        fake.write_text('#!/bin/sh\nprintf "%s\\n" "FAKE_' + name.upper() + '" "$@"\n')
        fake.chmod(0o755)
    (bindir / "nonexec").write_text("not executable\n")
    (bindir / "directory").mkdir()
    (bindir / "cmake-link").symlink_to(bindir / "cmake")
    (root / "project").mkdir()
    (root / "build/bootloader").mkdir(parents=True)
    helper.write_ninja_wrapper(wrappers / "ninja")
    original_cwd, original_path = Path.cwd(), os.environ.get("PATH")
    results = []
    try:
        os.chdir(root)
        os.environ["PATH"] = str(bindir) + os.pathsep + (original_path or "")
        for spelling in ["ninja", "bin/ninja", "./bin/ninja", str(bindir / "ninja")]:
            assert helper.resolve_tool(spelling) == bindir / "ninja"
            results.append({"check": "resolve " + ("absolute ninja" if spelling.startswith("/") else spelling), "passed": True})
        for spelling in ["cmake", "bin/cmake", "./bin/cmake", "bin/cmake-link"]:
            assert helper.resolve_tool(spelling) == bindir / "cmake"
            results.append({"check": "resolve " + spelling, "passed": True})
        decoy = root / "path-bin"
        decoy.mkdir()
        (decoy / "ninja").write_text('#!/bin/sh\nexit 0\n')
        (decoy / "ninja").chmod(0o755)
        os.chdir(bindir)
        os.environ["PATH"] = str(decoy) + os.pathsep + (original_path or "")
        assert helper.resolve_tool("./ninja") == bindir / "ninja"
        results.append({"check": "explicit ./ninja uses invoking cwd even with another PATH ninja", "passed": True})
        assert helper.resolve_tool("ninja") == decoy / "ninja"
        results.append({"check": "basename ninja uses PATH rather than invoking cwd", "passed": True})
        os.chdir(root)
        os.environ["PATH"] = str(bindir) + os.pathsep + (original_path or "")
        for label, spelling in [("missing", "bin/missing"), ("directory", "bin/directory"),
                                ("non-executable relative file", "bin/nonexec"), ("non-executable PATH name", "nonexec")]:
            try:
                helper.resolve_tool(spelling)
            except ValueError:
                results.append({"check": label, "rejected": True})
            else:
                raise AssertionError(label + " accepted")
        real_ninja, real_cmake = helper.resolve_tool("ninja"), helper.resolve_tool("cmake")
        env = dict(os.environ, PATH=str(wrappers) + os.pathsep + os.environ["PATH"],
                   C5_DIAG_NINJA=str(real_ninja), C5_DIAG_JOBS="8")
        for label, cwd, jobs in [("wrapper reaches fake once from project cwd", root / "project", "-j8"),
                                 ("wrapper reaches fake once from nested bootloader cwd", root / "build/bootloader", "-j1")]:
            ran = subprocess.run(["ninja", "input"], cwd=cwd, env=env, text=True,
                                 capture_output=True, timeout=3, check=True)
            assert ran.stdout.splitlines() == ["FAKE_NINJA", "input", jobs]
            results.append({"check": label, "passed": True, "fake_invocations": 1})
        ran = subprocess.run([str(real_cmake), "--version"], cwd=root / "project", env=env,
                             text=True, capture_output=True, timeout=3, check=True)
        assert ran.stdout.splitlines() == ["FAKE_CMAKE", "--version"]
        results.append({"check": "absolute CMake works after changing cwd/PATH", "passed": True})
    finally:
        os.chdir(original_cwd)
        if original_path is None:
            os.environ.pop("PATH", None)
        else:
            os.environ["PATH"] = original_path
    print(json.dumps({"count": len(results), "checks": results, "all_passed": True,
                      "fake_tools_only": True, "real_build_executed": False, "guest_executed": False}, indent=2))


if __name__ == "__main__":
    main()
