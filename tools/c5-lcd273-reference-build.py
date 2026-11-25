#!/usr/bin/env python3
"""Acquire and build fixed, unchanged Waveshare C5 examples in ignored target/.

Never flashes, monitors, runs an emulator, or installs global tools. Downloads
are pinned public inputs. All raw source, tools and products remain private.
"""
from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import sys
import tarfile
import time
import urllib.request

SDK_REV = "2c211b236707889e8400c4dc5644dd5c4ee071e0"
VENDOR_REV = "418de818f4f40c5a04c4292dbe293bfaa1351068"
VENDOR_TREE_REV = "5282f0112760f1d53a79092548303df96602d4b8"
TREE_SHA = "ce4bda26e748697b1f2953864ba9a66c4e5ec556492589e2e96b06093e0cafa5"
VENDOR_RECEIPT_SHA = "1a1789395ea6e02f32b77ffdd3f11f99931e42fcf46b2bba17d04a004ca1c890"
TOOLS_SHA = "66ebeabd419b4e24d38f513042219e544ca8715b95c6095efa4d3da275a893f3"
GCC_NAME = "esp-14.2.0_20251107"
GCC_URL = ("https://github.com/espressif/crosstool-NG/releases/download/"
           + GCC_NAME + "/riscv32-esp-elf-14.2.0_20251107-aarch64-apple-darwin.tar.xz")
GCC_BYTES = 288327120
GCC_SHA = "cedfcdaf8f165bd3e11ca4ee46b71d5b561fab93407eb8203812f9c14b1c7714"
CASES = {"03_sd_card": (68, 1705249), "02_lvgl_demo": (65, 1702154)}


def require(ok, message):
    if not ok:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def record(path):
    data = path.read_bytes()
    return {"bytes": len(data), "sha256": digest(data)}


def write_json(path, value):
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    temporary.replace(path)


def fresh(repo, requested):
    target, raw = repo / "target", requested.expanduser().absolute()
    require(".." not in raw.parts and raw != target and raw.is_relative_to(target),
            "output must be a fresh child of this checkout's ignored target/")
    require(not os.path.lexists(raw), "output already exists; preserving all existing data")
    for parent in raw.parents:
        if parent == repo:
            break
        require(not parent.is_symlink(), "symlink parent is forbidden")
        require(not parent.exists() or parent.is_dir(), "output parent is not a directory")
    require(raw.resolve() == raw, "output must not resolve through aliases")
    require(raw.parent == target or raw.parent.is_dir(), "nested output parent must exist")
    if not target.exists():
        target.mkdir(exist_ok=False)
    raw.mkdir(exist_ok=False)
    return raw


def fetch(url, destination, *, expected_sha=None, expected_size=None):
    h, size = hashlib.sha256(), 0
    request = urllib.request.Request(url, headers={"User-Agent": "c5-reference-input/1"})
    with urllib.request.urlopen(request, timeout=60) as response, destination.open("xb") as output:
        while chunk := response.read(1024 * 1024):
            output.write(chunk)
            h.update(chunk)
            size += len(chunk)
    require(expected_size is None or size == expected_size, "download byte count mismatch: " + url)
    require(expected_sha is None or h.hexdigest() == expected_sha, "download SHA256 mismatch: " + url)
    return {"url": url, "bytes": size, "sha256": h.hexdigest()}


def run(argv, *, cwd, env=None, log):
    print(json.dumps({"command": list(map(str, argv)), "log": str(log)}, sort_keys=True), flush=True)
    with log.open("xb") as output:
        process = subprocess.Popen(list(map(str, argv)), cwd=cwd, env=env,
                                   stdout=output, stderr=subprocess.STDOUT)
        print(json.dumps({"child_pid": process.pid, "pgid": os.getpgid(process.pid)}, sort_keys=True), flush=True)
        code = process.wait()
    require(code == 0, f"command failed ({code}); retained log {log.name}")


def git(sdk, *args):
    env = dict(os.environ, GIT_TERMINAL_PROMPT="0", GIT_OPTIONAL_LOCKS="0")
    return subprocess.check_output(["git", "-C", str(sdk), *args], env=env, text=True).rstrip("\n")


def validate_tree(data):
    """Verify Git objects, accepting equivalent JSON formatting of the API tree."""
    tree = json.loads(data)
    require(not tree["truncated"] and tree["sha"] in (VENDOR_REV, VENDOR_TREE_REV),
            "wrong or truncated manufacturer recursive tree")
    groups, expected, paths = {}, {"": VENDOR_TREE_REV}, set()
    for item in tree["tree"]:
        path = PurePosixPath(item["path"])
        require(not path.is_absolute() and ".." not in path.parts and str(path) == item["path"], "invalid tree path")
        require(item["path"] not in paths, "duplicate tree path")
        paths.add(item["path"])
        parent, _, base = item["path"].rpartition("/")
        groups.setdefault(parent, []).append((base, item))
        if item["type"] == "tree":
            expected[item["path"]] = item["sha"]
    require(set(groups).issubset(expected), "tree contains orphan entries")
    for path, wanted in expected.items():
        rows = sorted(groups.get(path, []), key=lambda row: (row[0] + ("/" if row[1]["type"] == "tree" else "")).encode())
        payload = b"".join(("40000" if item["type"] == "tree" else item["mode"]).encode() + b" "
                           + base.encode() + b"\0" + bytes.fromhex(item["sha"]) for base, item in rows)
        actual = hashlib.sha1(b"tree " + str(len(payload)).encode() + b"\0" + payload).hexdigest()
        require(actual == wanted, "manufacturer Git tree object mismatch: " + (path or "/"))
    return tree


def acquire(args):
    repo = Path(__file__).resolve().parent.parent
    tree_data = args.tree.read_bytes()
    tree = validate_tree(tree_data)
    root = fresh(repo, args.output)
    (root / "acquire-helper.py").write_bytes(Path(__file__).read_bytes())
    (root / "manufacturer-tree.json").write_bytes(tree_data)
    state = {"phase": "acquiring", "sdk_revision": SDK_REV, "vendor_revision": VENDOR_REV,
             "helper": record(root / "acquire-helper.py"), "pid": os.getpid(), "pgid": os.getpgrp(),
             "tree_capture": record(root / "manufacturer-tree.json"),
             "matches_original_tree_capture": digest(tree_data) == TREE_SHA}
    write_json(root / "state.json", state)
    print(json.dumps(state), flush=True)
    try:
        def compiler():
            metadata_url = f"https://raw.githubusercontent.com/espressif/esp-idf/{SDK_REV}/tools/tools.json"
            metadata = fetch(metadata_url, root / "idf-tools.json", expected_sha=TOOLS_SHA)
            item = next(x for x in json.loads((root / "idf-tools.json").read_text())["tools"]
                        if x["name"] == "riscv32-esp-elf")
            version = next(x for x in item["versions"] if x["name"] == GCC_NAME)
            artifact = version["macos-arm64"]
            require((artifact["url"], artifact["size"], artifact["sha256"]) ==
                    (GCC_URL, GCC_BYTES, GCC_SHA), "compiler metadata differs from pin")
            receipt = fetch(GCC_URL, root / "gcc.tar.xz", expected_sha=GCC_SHA, expected_size=GCC_BYTES)
            tool = root / "tools" / "tools" / "riscv32-esp-elf" / GCC_NAME
            tool.mkdir(parents=True, exist_ok=False)
            with tarfile.open(root / "gcc.tar.xz", "r:xz") as archive:
                archive.extractall(tool, filter="data")
            gcc = tool / "riscv32-esp-elf/bin/riscv32-esp-elf-gcc"
            text = subprocess.check_output([str(gcc), "--version"], text=True)
            require(GCC_NAME in text and "14.2.0" in text, "extracted compiler version mismatch")
            (root / "gcc-version.txt").write_text(text)
            return {"metadata": metadata, "artifact": receipt, "license": item["license"],
                    "gcc_version": text.splitlines()[0], "gcc_executable": record(gcc)}

        def sdk():
            env = dict(os.environ, GIT_TERMINAL_PROMPT="0", GIT_OPTIONAL_LOCKS="0")
            path = root / "esp-idf"
            run(["git", "clone", "--filter=blob:none", "--depth", "1", "--branch", "v5.5.3",
                 "--no-recurse-submodules", "https://github.com/espressif/esp-idf.git", path],
                cwd=root, env=env, log=root / "sdk-clone.log")
            require(git(path, "rev-parse", "HEAD") == SDK_REV, "SDK tag revision mismatch")
            run(["git", "submodule", "update", "--init", "--recursive", "--depth", "1", "--jobs", "8"],
                cwd=path, env=env, log=root / "sdk-submodules.log")
            status = git(path, "submodule", "status", "--recursive").splitlines()
            require(all(x.startswith(" ") for x in status), "submodule checkout mismatch")
            require(not git(path, "status", "--porcelain", "--untracked-files=no"), "SDK tracked files changed")
            return {"revision": SDK_REV, "submodules": status,
                    "license": record(path / "LICENSE"), "tracked_status": ""}

        def vendor():
            prefix = "example/ESP-IDF-V553/"
            selected = [x for x in tree["tree"] if x["type"] == "blob" and
                        (any(x["path"].startswith(prefix + c + "/") for c in CASES)
                         or x["path"] in ("LICENSE", "README.md"))]
            base = root / "vendor"
            base.mkdir(exist_ok=False)
            def one(item):
                rel = PurePosixPath(item["path"])
                require(not rel.is_absolute() and ".." not in rel.parts and item["mode"] == "100644",
                        "unexpected vendor source path/mode")
                output = base.joinpath(*rel.parts)
                output.parent.mkdir(parents=True, exist_ok=True)
                url = f"https://raw.githubusercontent.com/waveshareteam/ESP32-C5-LCD-2.73/{VENDOR_REV}/{rel}"
                receipt = fetch(url, output, expected_size=item["size"])
                data = output.read_bytes()
                blob = hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()
                require(blob == item["sha"], "vendor Git blob mismatch: " + str(rel))
                return {"path": str(rel), "git_blob": blob, **receipt}
            with ThreadPoolExecutor(max_workers=4) as pool:
                receipts = sorted(pool.map(one, selected), key=lambda x: x["path"])
            for case, expected in CASES.items():
                rows = [x for x in receipts if x["path"].startswith(prefix + case + "/")]
                require((len(rows), sum(x["bytes"] for x in rows)) == expected, "vendor case file count/size mismatch")
            write_json(root / "vendor-sources.json", {"revision": VENDOR_REV, "files": receipts})
            return {"revision": VENDOR_REV, "files": len(receipts), "receipt": record(root / "vendor-sources.json")}

        with ThreadPoolExecutor(max_workers=3) as pool:
            futures = {name: pool.submit(fn) for name, fn in [("compiler", compiler), ("sdk", sdk), ("vendor", vendor)]}
            results = {}
            failures = {}
            for name, future in futures.items():
                try:
                    results[name] = future.result()
                    print(json.dumps({"completed": name, "result": results[name]}, sort_keys=True), flush=True)
                except Exception as error:
                    failures[name] = str(error)
                    print(json.dumps({"failed": name, "error": str(error)}), flush=True)
        state.update(results=results, failures=failures, phase="failed" if failures else "acquired")
        write_json(root / "state.json", state)
        require(not failures, "one or more acquisition tasks failed; all data retained")
    except BaseException as error:
        state.update(phase="failed", error=str(error))
        write_json(root / "state.json", state)
        raise


def python_env(args):
    repo = Path(__file__).resolve().parent.parent
    sdk = args.sdk.expanduser().resolve()
    require(git(sdk, "rev-parse", "HEAD") == SDK_REV, "wrong SDK for Python environment")
    constraints = args.constraints.read_bytes()
    require(digest(constraints) == args.constraints_sha256, "constraints snapshot mismatch")
    root = fresh(repo, args.output)
    (root / "environment-helper.py").write_bytes(Path(__file__).read_bytes())
    (root / "espidf.constraints.v5.5.txt").write_bytes(constraints)
    shutil.copy2(sdk / "tools/requirements/requirements.core.txt", root / "requirements.core.txt")
    state = {"phase": "preparing-python", "pid": os.getpid(), "pgid": os.getpgrp(),
             "constraints": record(root / "espidf.constraints.v5.5.txt")}
    write_json(root / "state.json", state)
    env = dict(os.environ, PIP_CACHE_DIR=str(root / "pip-cache"), PIP_DISABLE_PIP_VERSION_CHECK="1",
               PYTHONDONTWRITEBYTECODE="1")
    try:
        run([args.base_python, "-m", "venv", root / "venv"], cwd=root, env=env, log=root / "venv.log")
        python = root / "venv/bin/python"
        run([python, "-m", "pip", "install", "-r", root / "requirements.core.txt", "-c",
             root / "espidf.constraints.v5.5.txt"], cwd=root, env=env, log=root / "pip-install.log")
        env["IDF_PYTHON_ENV_PATH"] = str(root / "venv")
        run([python, sdk / "tools/check_python_dependencies.py", "-r", root / "requirements.core.txt",
             "-c", root / "espidf.constraints.v5.5.txt"], cwd=root, env=env, log=root / "python-check.log")
        (root / "pip-freeze.txt").write_bytes(subprocess.check_output([python, "-m", "pip", "freeze"], env=env))
        state.update(phase="python-ready", python_version=subprocess.check_output([python, "--version"], text=True).strip(),
                     helper=record(root / "environment-helper.py"), freeze=record(root / "pip-freeze.txt"))
        write_json(root / "state.json", state)
    except BaseException as error:
        state.update(phase="failed", error=str(error))
        write_json(root / "state.json", state)
        raise


def dependencies(args):
    repo = Path(__file__).resolve().parent.parent
    data = args.lock.read_bytes()
    require(digest(data) == "f6f8e566fddb6b3cfadf2b150913291c31140d469b919c716bf6cde356087d1a",
            "manufacturer dependency lock mismatch")
    root = fresh(repo, args.output)
    (root / "dependency-helper.py").write_bytes(Path(__file__).read_bytes())
    (root / "dependencies.lock").write_bytes(data)
    os.environ.update(IDF_TOOLS_PATH=str(root / "tools"), IDF_COMPONENT_CACHE_PATH=str(root / "cache"),
                      PYTHONDONTWRITEBYTECODE="1")
    from ruamel.yaml import YAML
    from idf_component_tools.archive_tools import unpack_archive
    from idf_component_tools.hash_tools.validate import validate_hash_eq_hashdir, validate_hashfile_eq_hashdir
    from idf_component_tools.registry.service_details import get_storage_client
    from idf_component_tools.sources.web_service import WebServiceSource
    from types import SimpleNamespace
    lock = YAML(typ="safe").load(data)
    require(lock["target"] == "esp32c5" and lock["dependencies"]["idf"]["version"] == "5.5.3",
            "unexpected locked target/SDK")
    source = WebServiceSource(system_cache_path=str(root / "cache"), registry_url="https://components.espressif.com/")
    state = {"phase": "acquiring-components", "pid": os.getpid(), "pgid": os.getpgrp(), "components": []}
    write_json(root / "state.json", state)
    try:
        for name, item in sorted(lock["dependencies"].items()):
            if name == "idf":
                continue
            require(item["source"]["type"] == "service", "non-registry lock source is not supported")
            version, expected = str(item["version"]), item["component_hash"]
            # CM 2.5.2 caches its storage_clients generator. A fresh read client
            # per component avoids reusing an exhausted iterator; no package or
            # SDK code is changed, and the same official source is consulted.
            client = get_storage_client(registry_url="https://components.espressif.com/",
                                        config_path=str(root / "tools/no-config.yml"))
            metadata = client.component(name, version)
            require(metadata["component_hash"] == expected, "registry metadata differs from manufacturer lock: " + name)
            short = name.replace("/", "__") + "-" + version
            metadata_path = root / (short + ".metadata.json")
            write_json(metadata_path, metadata)
            archive = root / (short + ".zip")
            download = fetch(metadata["download_url"], archive)
            unpacked = Path(source.component_cache_path(SimpleNamespace(name=name, version=version, component_hash=expected)))
            require(not os.path.lexists(unpacked), "component cache output already exists")
            unpack_archive(str(archive), str(unpacked))
            validate_hash_eq_hashdir(unpacked, expected)
            checksums = fetch(metadata["checksums_url"], root / (short + ".CHECKSUMS.json"))
            shutil.copy2(root / (short + ".CHECKSUMS.json"), unpacked / "CHECKSUMS.json")
            validate_hashfile_eq_hashdir(unpacked)
            licenses = {p.relative_to(unpacked).as_posix(): record(p) for p in sorted(unpacked.rglob("*"))
                        if p.is_file() and p.name.lower().startswith(("license", "copying", "notice"))}
            state["components"].append({"name": name, "version": version, "component_hash": expected,
                                        "archive": download, "metadata": record(metadata_path),
                                        "checksums": checksums, "licenses": licenses,
                                        "registry_license": metadata.get("license"),
                                        "hash_directory_validated": True, "checksums_validated": True})
            write_json(root / "state.json", state)
            print(json.dumps({"component_complete": name, "version": version, "bytes": download["bytes"]}), flush=True)
        state.update(phase="components-ready", helper=record(root / "dependency-helper.py"))
        write_json(root / "state.json", state)
    except BaseException as error:
        state.update(phase="failed", error=str(error))
        write_json(root / "state.json", state)
        raise


def validate_sources(inputs, project, case):
    require(record(inputs / "vendor-sources.json")["sha256"] == VENDOR_RECEIPT_SHA,
            "fixed vendor source receipt mismatch")
    receipt = json.loads((inputs / "vendor-sources.json").read_text())
    require(receipt["revision"] == VENDOR_REV, "wrong vendor source receipt")
    prefix = "example/ESP-IDF-V553/" + case + "/"
    rows = [x for x in receipt["files"] if x["path"].startswith(prefix)]
    require((len(rows), sum(x["bytes"] for x in rows)) == CASES[case], "incomplete vendor source receipt")
    for item in rows:
        rel = PurePosixPath(item["path"][len(prefix):])
        require(not rel.is_absolute() and ".." not in rel.parts, "invalid vendor receipt path")
        data = project.joinpath(*rel.parts).read_bytes()
        require(len(data) == item["bytes"] and digest(data) == item["sha256"], "vendor source changed: " + str(rel))
        require(hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest() == item["git_blob"],
                "vendor Git blob changed: " + str(rel))
    return {item["path"][len(prefix):]: {"bytes": item["bytes"], "sha256": item["sha256"],
            "git_blob": item["git_blob"]} for item in rows}


def config_values(path):
    return dict(line.split("=", 1) for line in path.read_text().splitlines()
                if line.startswith("CONFIG_") and "=" in line)


def static_inspect(root):
    spec = importlib.util.spec_from_file_location("c5_image_checks", Path(__file__).with_name("c5-idf-hello-reference-build.py"))
    checks = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(checks)
    build = root / "build"
    raw = json.loads((build / "flasher_args.json").read_text())
    merged = (build / "merged.bin").read_bytes()
    chunks = []
    for offset, name in raw["flash_files"].items():
        path = (build / name).resolve()
        require(path.is_relative_to(build.resolve()), "flash file escaped build directory")
        chunks.append((int(offset, 0), name, path.read_bytes()))
    checks.verify_flash_layout(merged, chunks)
    artifacts = {"merged.bin": record(build / "merged.bin")}
    images, partitions, flash = {}, None, []
    for offset, name, data in sorted(chunks):
        artifacts[name] = record(build / name)
        flash.append({"offset": offset, "path": name, **artifacts[name]})
        if name in ("bootloader/bootloader.bin", "01_factory.bin"):
            info = checks.image_info(data)
            require(info["chip_id"] == 23 and info["flash_size_code"] == 5, "image must declare C5 and 32MiB")
            require(info["trailing_bytes"] == 0, "image contains unvalidated trailing bytes")
            images[name] = info
        elif name == "partition_table/partition-table.bin":
            partitions = checks.partition_info(data)
    require(set(images) == {"bootloader/bootloader.bin", "01_factory.bin"} and partitions,
            "missing required boot/app/partition image")
    app = next(x for x in flash if x["path"] == "01_factory.bin")
    factory = [x for x in partitions["entries"] if x["type"] == 0 and x["subtype"] == 0]
    require(len(factory) == 1 and factory[0]["offset"] == app["offset"] == 0x20000,
            "factory partition / flash offset mismatch")
    require(app["bytes"] <= factory[0]["bytes"], "app exceeds partition")
    descriptor = checks.app_descriptor((build / "01_factory.bin").read_bytes(), images["01_factory.bin"])
    elf = checks.elf_info((build / "01_factory.elf").read_bytes())
    require(elf["entry"] == images["01_factory.bin"]["entry"], "ELF and app entry differ")
    require(descriptor["app_elf_sha256"] == record(build / "01_factory.elf")["sha256"], "descriptor/ELF SHA256 mismatch")
    require(descriptor["project_name"] == "01_factory" and descriptor["idf_version"].startswith("v5.5.3"),
            "unexpected descriptor identity")
    for name in ["01_factory.elf", "01_factory.map", "flasher_args.json", "project_description.json", "config/sdkconfig.h"]:
        artifacts[name] = record(build / name)
    current, original = config_values(root / "sdkconfig"), config_values(root / "project/sdkconfig")
    for name, expected in {"CONFIG_IDF_TARGET": '"esp32c5"', "CONFIG_ESPTOOLPY_FLASHSIZE": '"32MB"',
                           "CONFIG_SPIRAM": "y", "CONFIG_SPIRAM_MODE_QUAD": "y", "CONFIG_SPIRAM_SPEED_80M": "y",
                           "CONFIG_SPIRAM_MEMTEST": "y", "CONFIG_ESP_DEFAULT_CPU_FREQ_MHZ": "240"}.items():
        require(current.get(name) == expected, "required manufacturer configuration changed: " + name)
    delta = {key: {"original": original.get(key), "generated": current.get(key)}
             for key in sorted(set(original) | set(current)) if original.get(key) != current.get(key)}
    require(delta == {}, "effective sdkconfig values differ from manufacturer original")
    require(record(root / "sdkconfig") == record(root / "project/sdkconfig"),
            "effective sdkconfig bytes differ from manufacturer original")
    artifacts["sdkconfig.generated"] = record(root / "sdkconfig")
    return {"artifacts": artifacts, "flash_files": flash, "images": images, "partitions": partitions,
            "app_descriptor": descriptor, "elf": elf, "sdkconfig_value_delta": delta,
            "static_verification_passed": True, "guest_executed": False}


def build_case(args):
    repo = Path(__file__).resolve().parent.parent
    inputs = args.inputs.expanduser().resolve()
    components = args.components.expanduser().resolve()
    pyroot = args.python_root.expanduser().resolve()
    sdk = inputs / "esp-idf"
    require(json.loads((inputs / "state.json").read_text())["phase"] == "acquired", "input acquisition is incomplete")
    require(json.loads((components / "state.json").read_text())["phase"] == "components-ready", "locked components are incomplete")
    require(json.loads((pyroot / "state.json").read_text())["phase"] == "python-ready", "Python environment is incomplete")
    require(git(sdk, "rev-parse", "HEAD") == SDK_REV, "SDK revision mismatch")
    before = {"tracked": git(sdk, "status", "--porcelain", "--untracked-files=no"),
              "submodules": git(sdk, "submodule", "status", "--recursive").splitlines()}
    require(not before["tracked"] and all(x.startswith(" ") for x in before["submodules"]), "SDK is dirty or gitlinks mismatch")
    source = inputs / "vendor/example/ESP-IDF-V553" / args.case
    source_hashes = validate_sources(inputs, source, args.case)
    root = fresh(repo, args.output)
    shutil.copy2(Path(__file__), root / "build-helper.py")
    shutil.copytree(source, root / "project")
    shutil.copy2(source / "sdkconfig", root / "sdkconfig")
    state = {"phase": "preflight", "pid": os.getpid(), "pgid": os.getpgrp(), "case": args.case,
             "commands": [], "jobs": args.jobs, "nested_bootloader_jobs": 1}
    write_json(root / "state.json", state)
    python = pyroot / "venv/bin/python"
    tools = inputs / "tools"
    constraints = tools / "espidf.constraints.v5.5.txt"
    if constraints.exists():
        require(record(constraints) == record(pyroot / constraints.name), "private tools constraints mismatch")
    else:
        with constraints.open("xb") as output:
            output.write((pyroot / constraints.name).read_bytes())
    gcc = tools / "tools/riscv32-esp-elf" / GCC_NAME / "riscv32-esp-elf/bin"
    wrapper_dir = root / "tool-bin"
    wrapper_dir.mkdir(exist_ok=False)
    wrapper = wrapper_dir / "ninja"
    wrapper.write_text('#!/bin/sh\ncase "$PWD" in */build/bootloader|*/build/bootloader/*) j=1;; '
                       '*) j="$C5_REFERENCE_JOBS";; esac\nexec "$C5_REFERENCE_NINJA" "$@" -j"$j"\n')
    wrapper.chmod(0o755)
    env = dict(os.environ, IDF_PATH=str(sdk), IDF_TOOLS_PATH=str(tools), IDF_PYTHON_ENV_PATH=str(pyroot / "venv"),
               PYTHON=str(python), PYTHONDONTWRITEBYTECODE="1", IDF_COMPONENT_MANAGER="1",
               IDF_COMPONENT_CACHE_PATH=str(components / "cache"), IDF_COMPONENT_NO_COLORS="1",
               CMAKE_BUILD_PARALLEL_LEVEL=str(args.jobs), MAKEFLAGS=f"-j{args.jobs}", CCACHE_DISABLE="1",
               GIT_OPTIONAL_LOCKS="0", GIT_TERMINAL_PROMPT="0", C5_REFERENCE_JOBS=str(args.jobs),
               C5_REFERENCE_NINJA=str(args.ninja.expanduser().absolute()))
    env["PATH"] = os.pathsep.join([str(wrapper_dir), str(python.parent), str(gcc),
                                 str(args.cmake.parent), str(args.ninja.parent), env.get("PATH", "")])
    def phase(name, argv):
        state.update(phase=name)
        state["commands"].append({"phase": name, "argv": list(map(str, argv))})
        write_json(root / "state.json", state)
        run(argv, cwd=root / "project", env=env, log=root / (name + ".log"))
    try:
        phase("compiler-version", [gcc / "riscv32-esp-elf-gcc", "--version"])
        phase("idf-version", [python, sdk / "tools/idf.py", "--version"])
        phase("reconfigure", [python, sdk / "tools/idf.py", "-C", root / "project", "-B", root / "build",
                              "-D", "IDF_TARGET=esp32c5", "-D", "SDKCONFIG=" + str(root / "sdkconfig"), "reconfigure"])
        require(record(root / "project/dependencies.lock") == record(source / "dependencies.lock"), "component manager changed lock")
        phase("build", [args.cmake, "--build", root / "build", "--parallel", str(args.jobs)])
        phase("merge", [python, sdk / "tools/idf.py", "-C", root / "project", "-B", root / "build",
                        "merge-bin", "--format", "raw", "-o", root / "build/merged.bin"])
        result = static_inspect(root)
        validate_sources(inputs, root / "project", args.case)
        validate_sources(inputs, source, args.case)
        after = {"tracked": git(sdk, "status", "--porcelain", "--untracked-files=no"),
                 "submodules": git(sdk, "submodule", "status", "--recursive").splitlines()}
        require(before == after, "SDK state changed during build")
        result.update(schema=1, kind="unchanged-manufacturer-source-built-example", case=args.case,
                      sdk_revision=SDK_REV, vendor_revision=VENDOR_REV, source_hashes=source_hashes,
                      sdk_tracked_clean_before_after=True, dependency_lock_unchanged=True,
                      jobs=args.jobs, nested_bootloader_jobs=1, builder=record(root / "build-helper.py"))
        write_json(root / "summary.json", result)
        state.update(phase="static-verified")
        write_json(root / "state.json", state)
        print("BUILD_COMPLETE " + args.case + " static-verified guest_executed=false", flush=True)
    except BaseException as error:
        state.update(phase="failed", error=str(error))
        write_json(root / "state.json", state)
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="operation", required=True)
    prepare = sub.add_parser("acquire")
    prepare.add_argument("--tree", type=Path, required=True)
    prepare.add_argument("--output", type=Path, required=True)
    py = sub.add_parser("python-env")
    for name in ["base-python", "sdk", "constraints", "output"]:
        py.add_argument("--" + name, type=Path, required=True)
    py.add_argument("--constraints-sha256", required=True)
    deps = sub.add_parser("dependencies")
    deps.add_argument("--lock", type=Path, required=True)
    deps.add_argument("--output", type=Path, required=True)
    build = sub.add_parser("build")
    build.add_argument("--case", choices=CASES, required=True)
    for name in ["inputs", "components", "python-root", "output", "cmake", "ninja"]:
        build.add_argument("--" + name, type=Path, required=True)
    build.add_argument("--jobs", type=int, choices=range(1, 9), default=8)
    inspect = sub.add_parser("inspect")
    inspect.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.operation == "acquire":
        acquire(args)
    elif args.operation == "python-env":
        python_env(args)
    elif args.operation == "dependencies":
        dependencies(args)
    elif args.operation == "build":
        build_case(args)
    elif args.operation == "inspect":
        print(json.dumps(static_inspect(args.output.expanduser().resolve()), indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
