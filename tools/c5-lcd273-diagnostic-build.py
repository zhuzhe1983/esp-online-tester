#!/usr/bin/env python3
"""Build the independently authored LCD273 diagnostic; never run or flash it.

Caller supplies the verified IDF553/tool/component inputs. Existing references
are read only; all copied dependencies, logs and products use fresh target/.
"""
from __future__ import annotations
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess

APP = "c5_lcd273_diagnostic"
SDK_REV = "2c211b236707889e8400c4dc5644dd5c4ee071e0"
GCC = "esp-14.2.0_20251107"
CONFIG = {"bytes": 90677, "sha256": "5cb65d2d95d876a898a7729d3590120c4a87a1b9509be496cdbc1f6eaac94423"}
LOCK = {"bytes": 7066, "sha256": "f6f8e566fddb6b3cfadf2b150913291c31140d469b919c716bf6cde356087d1a"}


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def resolve_tool(value):
    """Resolve against the invoking cwd/PATH before adding our Ninja wrapper."""
    raw = os.path.expanduser(os.fspath(value))
    found = raw if os.path.dirname(raw) else shutil.which(raw)
    if not found:
        raise ValueError("executable not found: " + raw)
    try:
        path = Path(found).resolve(strict=True)
    except (OSError, RuntimeError) as error:
        raise ValueError("cannot resolve executable: " + raw) from error
    if not path.is_file() or not os.access(path, os.X_OK):
        raise ValueError("tool is not an executable file: " + raw)
    return path


def write_ninja_wrapper(path):
    path.write_text('#!/bin/sh\ncase "$PWD" in */build/bootloader|*/build/bootloader/*) j=1;; '
                    '*) j="$C5_DIAG_JOBS";; esac\nexec "$C5_DIAG_NINJA" "$@" -j"$j"\n')
    path.chmod(0o755)


def inspect(root, repo):
    ref = module("c5_ref", repo / "tools/c5-lcd273-reference-build.py")
    checks = module("c5_image", repo / "tools/c5-idf-hello-reference-build.py")
    require, record = ref.require, ref.record
    build = root / "build"
    raw = json.loads((build / "flasher_args.json").read_text())
    chunks = []
    for offset, name in raw["flash_files"].items():
        path = (build / name).resolve()
        require(path.is_relative_to(build.resolve()), "flash file outside build")
        chunks.append((int(offset, 0), name, path.read_bytes()))
    checks.verify_flash_layout((build / "merged.bin").read_bytes(), chunks)
    images, partitions = {}, None
    for offset, name, data in chunks:
        if name in (APP + ".bin", "bootloader/bootloader.bin"):
            images[name] = checks.image_info(data)
            require(images[name]["chip_id"] == 23 and images[name]["flash_size_code"] == 5,
                    "images must declare C5/32MiB")
            require(images[name]["trailing_bytes"] == 0, "image trailing bytes")
        elif name == "partition_table/partition-table.bin":
            partitions = checks.partition_info(data)
    require(len(images) == 2 and partitions, "required image/partition missing")
    app = checks.app_descriptor((build / (APP + ".bin")).read_bytes(), images[APP + ".bin"])
    elf = checks.elf_info((build / (APP + ".elf")).read_bytes())
    require(app["project_name"] == APP and app["version"] == "0.1.0" and app["idf_version"] == "v5.5.3",
            "diagnostic descriptor mismatch")
    require(app["app_elf_sha256"] == record(build / (APP + ".elf"))["sha256"], "ELF descriptor SHA mismatch")
    require(elf["entry"] == images[APP + ".bin"]["entry"], "ELF/image entry mismatch")
    app_offset = next(offset for offset, name, _ in chunks if name == APP + ".bin")
    factory = [x for x in partitions["entries"] if x["type"] == 0 and x["subtype"] == 0]
    require(len(factory) == 1 and factory[0]["offset"] == app_offset == 0x20000, "app partition offset mismatch")
    require((build / (APP + ".bin")).stat().st_size <= factory[0]["bytes"], "app exceeds partition")
    require(record(root / "sdkconfig") == record(root / "project/sdkconfig") == CONFIG, "manufacturer config bytes changed")
    require(record(root / "project/dependencies.lock") == LOCK, "manufacturer lock changed")
    source = repo / "firmware/c5-lcd273-diagnostic"
    own = {}
    for name in ["CMakeLists.txt", "main/CMakeLists.txt", "main/main.c", "LICENSE"]:
        own[name] = record(source / name)
        require(record(root / "project" / name) == own[name], "diagnostic source changed: " + name)
    names = {name for _, name, _ in chunks} | {"merged.bin", APP + ".elf", APP + ".map", "flasher_args.json", "project_description.json", "config/sdkconfig.h"}
    return {"kind": "independently-authored-lcd273-diagnostic", "guest_executed": False,
            "static_verification_passed": True, "artifacts": {name: record(build / name) for name in sorted(names)},
            "flash_files": [{"offset": offset, "path": name, **record(build / name)} for offset, name, _ in sorted(chunks)],
            "images": images, "partitions": partitions, "app_descriptor": app, "elf": elf,
            "own_source": own, "original_and_effective_sdkconfig": CONFIG, "dependency_lock": LOCK}


def build(args, repo):
    cmake, ninja = resolve_tool(args.cmake), resolve_tool(args.ninja)
    ref = module("c5_ref", repo / "tools/c5-lcd273-reference-build.py")
    require, record = ref.require, ref.record
    inputs = args.reference_inputs.expanduser().resolve()
    sdk = args.idf_path.expanduser().resolve()
    components = args.components_root.expanduser().resolve()
    pyroot = args.python_root.expanduser().resolve()
    gcc = args.gcc_bin.expanduser().resolve()
    vendor = inputs / "vendor/example/ESP-IDF-V553/02_lvgl_demo"
    baseline = ref.validate_sources(inputs, vendor, "02_lvgl_demo")
    require(ref.git(sdk, "rev-parse", "HEAD") == SDK_REV, "SDK revision differs")
    before = {"tracked": ref.git(sdk, "status", "--porcelain", "--untracked-files=no"),
              "submodules": ref.git(sdk, "submodule", "status", "--recursive").splitlines()}
    require(not before["tracked"] and all(x.startswith(" ") for x in before["submodules"]), "SDK dirty/gitlink mismatch")
    require(json.loads((components / "state.json").read_text())["phase"] == "components-ready", "components not ready")
    require(json.loads((pyroot / "state.json").read_text())["phase"] == "python-ready", "Python not ready")
    require(record(vendor / "sdkconfig") == CONFIG and record(vendor / "dependencies.lock") == LOCK, "fixed configuration mismatch")
    version = subprocess.check_output([gcc / "riscv32-esp-elf-gcc", "--version"], text=True)
    require(GCC in version, "wrong GCC package")
    root = ref.fresh(repo, args.output)
    shutil.copy2(Path(__file__), root / "build-helper.py")
    state = {"phase": "copying-private-inputs", "pid": os.getpid(), "pgid": os.getpgrp(), "commands": [], "jobs": args.jobs}
    ref.write_json(root / "state.json", state)
    try:
        project = root / "project"
        shutil.copytree(repo / "firmware/c5-lcd273-diagnostic", project)
        shutil.copytree(vendor / "components", project / "components")
        for name in ["sdkconfig", "dependencies.lock", "partitions.csv"]:
            shutil.copy2(vendor / name, project / name)
        # The manufacturer's empty main manifest is still part of the solver's
        # manifest hash. Preserve its presence and bytes; do not re-solve pins.
        shutil.copy2(vendor / "main/idf_component.yml", project / "main/idf_component.yml")
        shutil.copy2(vendor / "sdkconfig", root / "sdkconfig")
        # Component Manager gets its own writable cache and tool state. The
        # original package trees, SDK, compiler and Python are read-only inputs.
        shutil.copytree(components / "cache", root / "component-cache")
        (root / "tools").mkdir()
        (root / "tmp").mkdir()
        shutil.copy2(pyroot / "espidf.constraints.v5.5.txt", root / "tools/espidf.constraints.v5.5.txt")
        (root / "tool-bin").mkdir()
        wrapper = root / "tool-bin/ninja"
        write_ninja_wrapper(wrapper)
        python = pyroot / "venv/bin/python"
        env = dict(os.environ, IDF_PATH=str(sdk), IDF_TOOLS_PATH=str(root / "tools"),
                   IDF_PYTHON_ENV_PATH=str(pyroot / "venv"), PYTHON=str(python), PYTHONDONTWRITEBYTECODE="1",
                   IDF_COMPONENT_MANAGER="1", IDF_COMPONENT_CACHE_PATH=str(root / "component-cache"),
                   IDF_COMPONENT_NO_COLORS="1", TMPDIR=str(root / "tmp"), CCACHE_DISABLE="1",
                   CMAKE_BUILD_PARALLEL_LEVEL=str(args.jobs), MAKEFLAGS=f"-j{args.jobs}",
                   C5_DIAG_JOBS=str(args.jobs), C5_DIAG_NINJA=str(ninja),
                   GIT_OPTIONAL_LOCKS="0", GIT_TERMINAL_PROMPT="0", GIT_NO_LAZY_FETCH="1")
        env["PATH"] = os.pathsep.join([str(root / "tool-bin"), str(python.parent), str(gcc),
                                     str(cmake.parent), str(ninja.parent), env.get("PATH", "")])
        def phase(name, argv):
            state["phase"] = name
            state["commands"].append({"phase": name, "argv": list(map(str, argv))})
            ref.write_json(root / "state.json", state)
            ref.run(argv, cwd=project, env=env, log=root / (name + ".log"))
        phase("idf-version", [python, sdk / "tools/idf.py", "--version"])
        phase("reconfigure", [python, sdk / "tools/idf.py", "-C", project, "-B", root / "build",
                              "-D", "IDF_TARGET=esp32c5", "-D", "SDKCONFIG=" + str(root / "sdkconfig"), "reconfigure"])
        require(record(project / "dependencies.lock") == LOCK, "lock changed in configure")
        require(record(root / "sdkconfig") == CONFIG, "effective config changed in configure")
        phase("build", [cmake, "--build", root / "build", "--parallel", str(args.jobs)])
        phase("merge", [python, sdk / "tools/idf.py", "-C", project, "-B", root / "build", "merge-bin",
                        "--format", "raw", "-o", root / "build/merged.bin"])
        result = inspect(root, repo)
        for name, expected in baseline.items():
            if name.startswith("components/") or name in ["sdkconfig", "dependencies.lock", "partitions.csv", "main/idf_component.yml"]:
                require(record(project / name) == {key: expected[key] for key in ["bytes", "sha256"]}, "upstream input changed: " + name)
        ref.validate_sources(inputs, vendor, "02_lvgl_demo")
        after = {"tracked": ref.git(sdk, "status", "--porcelain", "--untracked-files=no"),
                 "submodules": ref.git(sdk, "submodule", "status", "--recursive").splitlines()}
        require(before == after, "SDK changed")
        result.update(sdk_revision=SDK_REV, vendor_revision=ref.VENDOR_REV, builder=record(root / "build-helper.py"),
                      source_baseline=baseline, sdk_clean_before_after=True, gcc_version=version.splitlines()[0],
                      jobs=args.jobs, nested_bootloader_jobs=1)
        ref.write_json(root / "summary.json", result)
        state["phase"] = "static-verified"
        ref.write_json(root / "state.json", state)
        print("BUILD_COMPLETE diagnostic static-verified guest_executed=false", flush=True)
    except BaseException as error:
        state.update(phase="failed", error=str(error))
        ref.write_json(root / "state.json", state)
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    command = sub.add_parser("build")
    for name in ["reference-inputs", "idf-path", "components-root", "python-root", "gcc-bin", "output"]:
        command.add_argument("--" + name, type=Path, required=True)
    # Preserve './ninja' as a path rather than losing './' through Path parsing.
    for name in ["cmake", "ninja"]:
        command.add_argument("--" + name, required=True)
    command.add_argument("--jobs", type=int, choices=range(1, 9), default=8)
    verify = sub.add_parser("inspect")
    verify.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parent.parent
    if args.action == "build":
        build(args, repo)
    else:
        print(json.dumps(inspect(args.output.expanduser().resolve(), repo), indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
