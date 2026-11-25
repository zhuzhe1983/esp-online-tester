#!/usr/bin/env python3
"""Build and statically verify an unchanged, non-WiFi C5 hello with a pinned IDF.

Uses caller-supplied installed tools; never installs, flashes, monitors, or runs a
guest. All copied sources, raw logs and binaries stay in this checkout's target/.
The Espressif source is CC0-1.0 (main) / Apache-2.0 (SDK); it is not vendored here.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import time


SDK_REVISION = "fcae32885b0296b32044cb99ecbdc50d98dddb83"
GCC_VERSION = "esp-14.2.0_20241119"
SOURCE_HASHES = {
    "CMakeLists.txt": "49af6567ee0b9c381654ad42f7ea0c40fe61a8ca0d9421625faa01ca21603009",
    "main/CMakeLists.txt": "116b04da44200b6ddeee8aed45c9fbfb5e487d727c7d29b028e79807e76137cc",
    "main/hello_world_main.c": "b2d1c8573307e010d276d2d0df537f1fe752416f8422e267aa8b1661c1744595",
}
PROFILES = {
    "idf551": {"version": "5.5.1", "revision": SDK_REVISION, "gcc": GCC_VERSION,
               "source_hashes": SOURCE_HASHES, "key_manager_sources": {}},
    "idf602": {"version": "6.0.2", "revision": "7101770dc6db2667b3c477cc31365dd1acd6db4e",
               "gcc": "esp-15.2.0_20251204",
               "source_hashes": {**SOURCE_HASHES,
                   "CMakeLists.txt": "5df271e4d0dd84f1d4765b274c6d53179cf33ca3f9c5eb5d21de69d88310d721"},
               "key_manager_sources": {
                   "components/esp_security/src/init.c": "0ec3693522cf58c738383e1a13110f6179631e5685c60397d4f72b9267393667",
                   "components/esp_hal_security/esp32c5/include/hal/key_mgr_ll.h": "9b0247c5bbca44453b0c76849dad38af990df665c3d629d450b4539358ff9a95"}},
}
DEFAULTS = "CONFIG_ESPTOOLPY_FLASHSIZE_32MB=y\n# CONFIG_SPIRAM is not set\n"


def require(ok: bool, message: str) -> None:
    if not ok:
        raise ValueError(message)


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def file_record(path: Path) -> dict:
    data = path.read_bytes()
    return {"bytes": len(data), "sha256": digest(data)}


def reserve_work_dir(repo: Path, requested: Path) -> Path:
    """Reject occupied/symlink spellings before resolving or writing anything."""
    target = repo / "target"
    raw = requested.expanduser().absolute()
    require(".." not in raw.parts, "work path must not contain parent traversal")
    require(raw != target, "work directory must not be the target root")
    require(raw.is_relative_to(target), "work directory must be inside this checkout's ignored target/")
    require(not os.path.lexists(raw), "work path already exists; all existing data is preserved")
    for parent in raw.parents:
        if parent == repo:
            break
        require(not parent.is_symlink(), "work parent must not be a symlink")
        require(not parent.exists() or parent.is_dir(), "work parent is not a directory")
    work = raw.resolve()
    require(work == raw, "work path must not resolve through aliases")
    # Only the conventional target root may need creation. Nested parents must
    # already exist; this avoids creating directories before reserving the leaf.
    require(raw.parent == target or raw.parent.is_dir(), "nested work parent must already exist")
    if not target.exists():
        target.mkdir(exist_ok=False)
    work.mkdir(exist_ok=False)
    return work


def write_json(path: Path, value: dict) -> None:
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    tmp.replace(path)


def git(sdk: Path, *args: str) -> str:
    env = dict(os.environ, GIT_OPTIONAL_LOCKS="0", GIT_TERMINAL_PROMPT="0")
    return subprocess.check_output(["git", "-C", str(sdk), *args], env=env, text=True).rstrip("\n")


def sdk_state(sdk: Path) -> dict:
    return {
        "revision": git(sdk, "rev-parse", "HEAD"),
        "tracked_status": git(sdk, "status", "--porcelain", "--untracked-files=no"),
        "submodules": git(sdk, "submodule", "status", "--recursive").splitlines(),
    }


def image_info(data: bytes) -> dict:
    # ESP-IDF 5.5.1 esp_app_format.h + esptool's ESP32FirmwareImage:
    # 24-byte header; <load_address, length> per segment; XOR seed 0xef;
    # checksum at the last byte of the next 16-byte block, then optional SHA256.
    require(len(data) >= 24 and data[0] == 0xe9, "not an Espressif image")
    require(0 < data[1] <= 16, "invalid segment count")
    pos, checksum, segments = 24, 0xef, []
    for _ in range(data[1]):
        require(pos + 8 <= len(data), "truncated segment header")
        address, size = struct.unpack_from("<II", data, pos)
        pos += 8
        require(size <= len(data) - pos, "truncated segment payload")
        payload = data[pos:pos + size]
        for byte in payload:
            checksum ^= byte
        segments.append({"load_address": f"0x{address:08x}", "bytes": size,
                         "file_offset": pos, "sha256": digest(payload)})
        pos += size
    checksum_offset = (pos // 16) * 16 + 15
    require(checksum_offset < len(data), "missing image checksum")
    require(data[checksum_offset] == checksum, "image XOR checksum mismatch")
    end = checksum_offset + 1
    appended = data[23] == 1
    require(data[23] in (0, 1), "unknown appended hash flag")
    image_hash = None
    if appended:
        require(end + 32 <= len(data), "missing appended image SHA256")
        image_hash = digest(data[:end])
        require(data[end:end + 32].hex() == image_hash, "appended image SHA256 mismatch")
        end += 32
    return {"entry": f"0x{struct.unpack_from('<I', data, 4)[0]:08x}",
            "chip_id": struct.unpack_from("<H", data, 12)[0],
            "flash_size_code": data[3] >> 4, "flash_frequency_code": data[3] & 15,
            "flash_mode_code": data[2], "min_revision_full": struct.unpack_from("<H", data, 15)[0],
            "max_revision_full": struct.unpack_from("<H", data, 17)[0],
            "segments": segments, "checksum_offset": checksum_offset,
            "checksum": checksum, "appended_sha256": image_hash,
            "validated_image_bytes": end, "trailing_bytes": len(data) - end}


def app_descriptor(data: bytes, image: dict) -> dict:
    # IDF 5.5.1 components/esp_app_format/include/esp_app_desc.h.
    start = image["segments"][0]["file_offset"]
    desc = data[start:start + 256]
    require(len(desc) == 256 and struct.unpack_from("<I", desc)[0] == 0xabcd5432,
            "missing first-segment app descriptor")
    def string(offset: int, size: int) -> str:
        return desc[offset:offset + size].split(b"\0", 1)[0].decode("utf-8")
    return {"magic": "0xabcd5432", "secure_version": struct.unpack_from("<I", desc, 4)[0],
            "version": string(16, 32), "project_name": string(48, 32),
            "compile_time": string(80, 16), "compile_date": string(96, 16),
            "idf_version": string(112, 32), "app_elf_sha256": desc[144:176].hex()}


def elf_info(data: bytes) -> dict:
    require(data[:7] == b"\x7fELF\x01\x01\x01" and len(data) >= 52, "not ELF32 little-endian")
    require(struct.unpack_from("<H", data, 18)[0] == 243, "ELF machine is not RISC-V")
    return {"class": 32, "byte_order": "little", "machine": 243,
            "entry": f"0x{struct.unpack_from('<I', data, 24)[0]:08x}"}


def partition_info(data: bytes) -> dict:
    # IDF 5.5.1 components/partition_table/gen_esp32part.py binary format.
    entries, md5_valid = [], False
    for pos in range(0, len(data) - 31, 32):
        row = data[pos:pos + 32]
        if row == b"\xff" * 32:
            break
        if row[:16] == b"\xeb\xeb" + b"\xff" * 14:
            require(row[16:] == hashlib.md5(data[:pos]).digest(), "partition table MD5 mismatch")
            md5_valid = True
            break
        magic, kind, subtype, offset, size, label, flags = struct.unpack("<HBBII16sI", row)
        require(magic == 0x50aa, "invalid partition entry")
        entries.append({"type": kind, "subtype": subtype, "offset": offset, "bytes": size,
                        "label": label.split(b"\0", 1)[0].decode(), "flags": flags})
    require(entries and md5_valid, "partition table has no validated MD5 entry")
    return {"entries": entries, "md5_valid": md5_valid}


def config_info(path: Path) -> dict:
    values = {}
    for line in path.read_text().splitlines():
        if line.startswith("CONFIG_") and "=" in line:
            key, value = line.split("=", 1)
            values[key] = value.strip('"')
    require(values.get("CONFIG_IDF_TARGET") == "esp32c5", "sdkconfig target is not C5")
    require(values.get("CONFIG_ESPTOOLPY_FLASHSIZE") == "32MB", "sdkconfig flash size is not 32MB")
    require(values.get("CONFIG_SPIRAM") != "y", "reference must not enable PSRAM")
    names = ["CONFIG_IDF_TARGET", "CONFIG_ESPTOOLPY_FLASHSIZE", "CONFIG_ESPTOOLPY_FLASHMODE",
             "CONFIG_ESPTOOLPY_FLASHFREQ", "CONFIG_XTAL_FREQ", "CONFIG_XTAL_FREQ_AUTO",
             "CONFIG_ESP_DEFAULT_CPU_FREQ_MHZ", "CONFIG_ESP_CONSOLE_UART_NUM",
             "CONFIG_ESP_CONSOLE_UART_DEFAULT", "CONFIG_COMPILER_LTO"]
    return {"selected": {name: values.get(name) for name in names}, "psram_enabled": False,
            "xtal_runtime_note": "C5 uses ROM/pin/eFuse autodetection; this is not a runtime XTAL measurement"}


def verify_flash_layout(merged: bytes, chunks: list[tuple[int, str, bytes]]) -> None:
    """Verify the complete RAW container, including bytes outside flash inputs."""
    boot = [row for row in chunks if row[1] == "bootloader/bootloader.bin"]
    require(len(boot) == 1 and boot[0][0] == 0x2000, "expected one C5 bootloader at 0x2000")
    require(0 < len(merged) <= 32 * 1024 * 1024, "merged image exceeds declared flash")
    cursor = 0
    for offset, name, data in sorted(chunks):
        require(offset >= cursor, "flash ranges overlap")
        require(data and offset + len(data) <= len(merged), "flash range is empty or exceeds merged image")
        require(all(byte == 0xff for byte in merged[cursor:offset]), "merged prefix/gap is not erased FF")
        require(merged[offset:offset + len(data)] == data, f"merged bytes differ for {name}")
        cursor = offset + len(data)
    require(cursor == len(merged), "merged image has bytes after the last flash file")


def inspect(work: Path) -> dict:
    profile_file = work / "build-profile.json"
    profile_name = json.loads(profile_file.read_text())["name"] if profile_file.exists() else "idf551"
    require(profile_name in PROFILES, "unknown pinned SDK profile")
    profile = PROFILES[profile_name]
    project, build = work / "project", work / "project/build"
    raw = json.loads((build / "flasher_args.json").read_text())
    merged_path = build / "merged.bin"
    merged = merged_path.read_bytes()
    artifacts = {"merged.bin": file_record(merged_path)}
    flash_files, images = [], {}
    app_path, app_image, ptable = None, None, None
    chunks = []
    for offset_text, name in sorted(raw["flash_files"].items(), key=lambda x: int(x[0], 0)):
        offset = int(offset_text, 0)
        path = (build / name).resolve()
        require(path.is_relative_to(build.resolve()), "flash input escaped build directory")
        data = path.read_bytes()
        chunks.append((offset, name, data))
    verify_flash_layout(merged, chunks)
    for offset, name, data in chunks:
        path = build / name
        record = {"path": name, "offset": offset, **file_record(path)}
        flash_files.append(record)
        artifacts[name] = file_record(path)
        if data[:1] == b"\xe9":
            info = image_info(data)
            require(info["chip_id"] == 23, f"image {name} is not ESP32-C5")
            require(info["flash_size_code"] == 5, f"image {name} does not declare 32MB")
            images[name] = info
            if name == "hello_world.bin":
                app_path, app_image = path, info
            if name.endswith("bootloader.bin"):
                require(offset == 0x2000, "C5 bootloader is not at 0x2000")
        elif "partition" in name:
            ptable = partition_info(data)
    require("bootloader/bootloader.bin" in images, "bootloader is not a validated Espressif image")
    require(app_path is not None and ptable is not None, "missing app or partition table")
    factory = [p for p in ptable["entries"] if p["type"] == 0 and p["subtype"] == 0]
    require(len(factory) == 1, "expected one factory app partition")
    app_offset = next(f["offset"] for f in flash_files if f["path"] == "hello_world.bin")
    require(factory[0]["offset"] == app_offset, "flash args and factory offset differ")
    require(app_path.stat().st_size <= factory[0]["bytes"], "app exceeds its partition")
    descriptor = app_descriptor(app_path.read_bytes(), app_image)
    elf_path, map_path = build / "hello_world.elf", build / "hello_world.map"
    elf = elf_info(elf_path.read_bytes())
    require(elf["entry"] == app_image["entry"], "ELF/image entry mismatch")
    require(descriptor["app_elf_sha256"] == file_record(elf_path)["sha256"], "descriptor/ELF SHA mismatch")
    require(descriptor["project_name"] == "hello_world" and descriptor["idf_version"].startswith("v" + profile["version"]),
            "unexpected app descriptor")
    map_text = map_path.read_text(errors="replace")
    map_symbols = {name: name in map_text for name in ["app_main", "vTaskDelay", "esp_restart", "esp_wifi_init"]}
    require(all(map_symbols[s] for s in ["app_main", "vTaskDelay", "esp_restart"]), "expected symbol absent from map")
    for name in ["hello_world.elf", "hello_world.map", "flasher_args.json", "project_description.json"]:
        artifacts[name] = file_record(build / name)
    artifacts["sdkconfig"] = file_record(project / "sdkconfig")
    artifacts["config/sdkconfig.h"] = file_record(build / "config/sdkconfig.h")
    return {"sdk_profile": profile_name, "artifacts": artifacts, "flash_files": flash_files, "images": images,
            "partitions": ptable, "app_descriptor": descriptor, "elf": elf,
            "map_text_contains": map_symbols, "configuration": config_info(project / "sdkconfig"),
            "static_verification_passed": True, "guest_executed": False}


def build_reference(args: argparse.Namespace) -> int:
    repo = Path(__file__).resolve().parent.parent
    work = reserve_work_dir(repo, args.work_dir)
    shutil.copy2(Path(__file__), work / "build-helper.py")
    profile = PROFILES[args.sdk_profile]
    source_hashes = profile["source_hashes"]
    write_json(work / "build-profile.json", {"name": args.sdk_profile, "jobs": args.jobs})
    sdk = args.idf_path.expanduser().resolve()
    # Keep the virtualenv interpreter spelling: resolving its symlink loses the venv.
    python = args.idf_python.expanduser().absolute()
    gcc_bin = args.gcc_bin.expanduser().resolve()
    tools_path = args.idf_tools_path.expanduser().resolve()
    ninja, cmake = args.ninja.expanduser().resolve(), args.cmake.expanduser().resolve()
    state_path = work / "state.json"
    state = {"status": "running", "phase": "preflight", "pid": os.getpid(), "pgid": os.getpgrp(),
             "argv": sys.argv, "cwd": str(repo), "started_unix": time.time(), "commands": []}
    write_json(state_path, state)
    print(f"BUILD_HANDLE pid={os.getpid()} pgid={os.getpgrp()} state={state_path}", flush=True)
    aliases = [(str(sdk), "$IDF_PATH"), (str(python), "$IDF_PYTHON"), (str(gcc_bin), "$GCC_BIN"),
               (str(tools_path), "$IDF_TOOLS_PATH"), (str(work), "$REFERENCE_DIR"),
               (str(repo), "$REPO"), (str(cmake), "$CMAKE"), (str(ninja), "$NINJA")]
    def normalized(text: str) -> str:
        for source, replacement in sorted(aliases, key=lambda p: -len(p[0])):
            text = text.replace(source, replacement)
        return text
    env = dict(os.environ)
    env.update(IDF_PATH=str(sdk), IDF_TOOLS_PATH=str(tools_path),
               IDF_PYTHON_ENV_PATH=str(python.parent.parent), PYTHON=str(python),
               IDF_COMPONENT_MANAGER="0", PYTHONDONTWRITEBYTECODE="1", CCACHE_DISABLE="1",
               CMAKE_BUILD_PARALLEL_LEVEL=str(args.jobs), MAKEFLAGS=f"-j{args.jobs}", GIT_OPTIONAL_LOCKS="0",
               GIT_TERMINAL_PROMPT="0", GIT_NO_LAZY_FETCH="1", C5_REFERENCE_NINJA=str(ninja),
               C5_REFERENCE_JOBS=str(args.jobs))
    commands = []
    before = None
    try:
        before = sdk_state(sdk)
        require(before["revision"] == profile["revision"], "SDK revision differs from pinned " + profile["version"])
        require(not before["tracked_status"], "SDK has tracked changes; no reset or update will be attempted")
        require(all(row and row[0] == " " for row in before["submodules"]), "SDK submodule revision/state differs")
        for path in [python, gcc_bin / "riscv32-esp-elf-gcc", ninja, cmake, sdk / "LICENSE"]:
            require(path.is_file(), f"required installed tool/source missing: {path}")
        require(profile["gcc"] in str(gcc_bin), "GCC install directory differs from pinned tool version")
        tools = json.loads((sdk / "tools/tools.json").read_text())
        expected = [v["name"] for t in tools["tools"] if t["name"] == "riscv32-esp-elf"
                    for v in t["versions"] if v.get("status") == "recommended"]
        require(expected == [profile["gcc"]], "SDK recommends a different GCC version")
        source = sdk / "examples/get-started/hello_world"
        for name, expected_hash in source_hashes.items():
            require(file_record(source / name)["sha256"] == expected_hash, f"official hello source mismatch: {name}")
        for name, expected_hash in profile["key_manager_sources"].items():
            require(file_record(sdk / name)["sha256"] == expected_hash, f"Key Manager source mismatch: {name}")
        project = work / "project"
        shutil.copytree(source, project, ignore=shutil.ignore_patterns("build", "sdkconfig", "sdkconfig.old", "__pycache__"))
        (work / "source-licenses").mkdir()
        shutil.copy2(sdk / "LICENSE", work / "source-licenses/ESP-IDF-LICENSE")
        (project / "sdkconfig.defaults").write_text(DEFAULTS)
        wrapper_dir = work / "tool-bin"
        wrapper_dir.mkdir()
        wrapper = wrapper_dir / "ninja"
        # idf.py invokes Ninja directly, and the bootloader is an ExternalProject.
        # Keep the nested bootloader at jobs1. These are build flags, not a
        # measurement or guarantee of the total process/thread concurrency.
        wrapper.write_text('#!/bin/sh\ncase "$PWD" in */project/build/bootloader|*/project/build/bootloader/*) j=1;; '
                           '*) j="$C5_REFERENCE_JOBS";; esac\nexec "$C5_REFERENCE_NINJA" "$@" -j"$j"\n')
        wrapper.chmod(0o755)
        env["PATH"] = os.pathsep.join([str(wrapper_dir), str(python.parent), str(gcc_bin), env.get("PATH", "")])
        def run(command: list[str], phase: str) -> None:
            state.update(phase=phase, current_command=command)
            state["commands"].append({"phase": phase, "argv": command})
            write_json(state_path, state)
            commands.append({"phase": phase, "argv": [normalized(part) for part in command]})
            print(f"PHASE {phase}: {json.dumps(command)}", flush=True)
            subprocess.run(command, cwd=project, env=env, check=True)
        run([str(gcc_bin / "riscv32-esp-elf-gcc"), "--version"], "compiler-version")
        run([str(python), str(sdk / "tools/idf.py"), "--version"], "idf-version-and-dependencies")
        run([str(python), str(sdk / "tools/idf.py"), "-C", str(project), "set-target", "esp32c5"], "configure-c5")
        run([str(cmake), "--build", str(project / "build"), "--parallel", str(args.jobs)], f"build-jobs{args.jobs}")
        run([str(python), str(sdk / "tools/idf.py"), "-C", str(project), "merge-bin", "--format", "raw",
             "-o", str(project / "build/merged.bin")], f"merge-bin-jobs{args.jobs}")
        result = inspect(work)
        require(not result["map_text_contains"]["esp_wifi_init"], "WiFi init unexpectedly occurs in reference map")
        for name, expected_hash in source_hashes.items():
            require(file_record(project / name)["sha256"] == expected_hash, f"build changed hello source: {name}")
        after = sdk_state(sdk)
        require(before == after, "SDK tracked/submodule state changed during build")
        result.update(schema=1, kind=f"independent-c5-{args.sdk_profile}-hello-reference", jobs=args.jobs,
                      nested_bootloader_jobs=1,
                      sdk={"repository": "https://github.com/espressif/esp-idf", "version": profile["version"],
                           "revision": profile["revision"], "tracked_clean_before_after": True,
                           "submodules": before["submodules"]},
                      source={"path": "examples/get-started/hello_world", "unchanged_sha256": source_hashes,
                              "main_license": "CC0-1.0", "sdk_license": file_record(work / "source-licenses/ESP-IDF-LICENSE"),
                              "defaults": DEFAULTS, "defaults_sha256": digest(DEFAULTS.encode()),
                              "key_manager_sources_sha256": profile["key_manager_sources"]},
                      toolchain={"recommended_version": profile["gcc"]}, commands=commands,
                      builder=file_record(work / "build-helper.py"),
                      limitations=["No guest/emulator execution or timing measurement",
                                   "Independent IDF " + profile["version"] + " reference, not FactoryOnly IDF 5.5.3 or its ELF",
                                   "No PSRAM enabled; no WiFi/RF/LCD/SD/input acceptance",
                                   "Hello counts down then intentionally restarts; runtime verification remains separate"],
                      private_capture_note="Raw logs, sources, ELF/map and binaries remain under ignored target/; paths are aliased here")
        write_json(work / "summary.json", result)
        state.update(status="complete", phase="static-verified", finished_unix=time.time())
        write_json(state_path, state)
        print("BUILD_COMPLETE summary.json static_verification_passed=true guest_executed=false", flush=True)
        return 0
    except Exception as error:
        state.update(status="failed", error=str(error), finished_unix=time.time())
        if before is not None:
            state["sdk_tracked_status_after_failure"] = git(sdk, "status", "--porcelain", "--untracked-files=no")
        write_json(state_path, state)
        print(f"BUILD_FAILED {error}", file=sys.stderr, flush=True)
        return 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    build = sub.add_parser("build", help="Use installed tools only; outputs must be in this checkout's target/")
    for flag in ["idf-path", "idf-python", "idf-tools-path", "gcc-bin", "ninja", "cmake", "work-dir"]:
        build.add_argument("--" + flag, type=Path, required=True)
    build.add_argument("--sdk-profile", choices=PROFILES, default="idf551", help="551 remains the default for existing commands")
    build.add_argument("--jobs", type=int, choices=range(1, 9), default=1, help="Outer compile jobs; nested bootloader stays at one")
    verify = sub.add_parser("inspect", help="Read and verify existing private outputs without building")
    verify.add_argument("--work-dir", type=Path, required=True)
    args = parser.parse_args()
    if args.action == "build":
        return build_reference(args)
    print(json.dumps(inspect(args.work_dir.expanduser().resolve()), indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
