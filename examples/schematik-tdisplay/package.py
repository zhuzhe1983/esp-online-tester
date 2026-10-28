#!/usr/bin/env python3
"""Package a completed pinned PlatformIO build for the static Pages demo."""
import gzip
import hashlib
import io
import os
from pathlib import Path
import re
import shutil
import tarfile
import urllib.request

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
PUBLIC = ROOT / "web/wasm/fw/public"
BUILD = HERE / ".pio/build/ttgo-t-display"
PIO = Path(os.environ.get("PLATFORMIO_CORE_DIR", Path.home() / ".platformio"))
CORE = PIO / "packages/framework-arduinoespressif32"
GFX = HERE / ".pio/libdeps/ttgo-t-display/GFX Library for Arduino"
PREFIX = "schematik-tdisplay-"


def fetch(url):
    with urllib.request.urlopen(url, timeout=60) as response:
        return response.read()


def source_files(directory, prefix):
    if not directory.is_dir():
        raise FileNotFoundError(directory)
    for path in sorted(directory.rglob("*")):
        if path.is_file() and not path.is_symlink():
            yield prefix + "/" + path.relative_to(directory).as_posix(), path.read_bytes()


def main():
    public_files = []
    for source, suffix in [("bootloader.bin", "bootloader.bin"),
                           ("partitions.bin", "ptable.bin"), ("firmware.bin", "app.bin")]:
        target = PUBLIC / (PREFIX + suffix)
        shutil.copyfile(BUILD / source, target)
        public_files.append(target)

    gfx_license = fetch("https://raw.githubusercontent.com/moononournation/Arduino_GFX/v1.6.9/license.txt")
    arduino_license = fetch("https://raw.githubusercontent.com/espressif/arduino-esp32/3.3.8/LICENSE.md")
    for data, expected in [(gfx_license, "3e7bede50526352926c2283e640f2823d9f9f2165d1852044c4fb1bbac283106"),
                           (arduino_license, "62e54861b30e953735dc187f036917c588665dbe3c5dd88449e34317608bbf5e")]:
        if hashlib.sha256(data).hexdigest() != expected:
            raise ValueError("Dependency licence SHA-256 mismatch")
    # Only allocated nonzero sections, excluding archive extraction/discarded lists.
    memory_map = (BUILD / "firmware.map").read_text().split("Linker script and memory map", 1)[1].split("Cross Reference Table", 1)[0]
    archives = sorted({Path(path).name for addr, size, path in re.findall(
        r"0x([0-9a-f]+)[ \t]+0x([0-9a-f]+)[ \t]+([^\n]+?\.a)\(", memory_map)
        if int(addr, 16) and int(size, 16)})
    inventory = "\n".join(archives) + "\n"
    (HERE / "linked-archives.txt").write_text(inventory)
    shared = (PUBLIC / "classic-hello-NOTICE.txt").read_text().split("ESP-IDF and Mbed TLS\n", 1)[1]
    notice = """Schematik T-Display firmware licences
====================================

Files: schematik-tdisplay-{bootloader,ptable,app}.bin.
Application/build recipe: examples/schematik-tdisplay (MIT, repository LICENSE).
Arduino-ESP32 3.3.8 core, SPI and Wire: LGPL-2.1-or-later, unmodified.
Arduino_GFX 1.6.9: BSD-2-Clause, unmodified; default glcdfont only.
ESP-IDF 5.5.4 and Mbed TLS crypto: Apache-2.0; FreeRTOS: MIT;
TLSF: BSD-3-Clause; Xtensa HAL: MIT; newlib: per-file licences;
GCC/libstdc++ runtime: GPLv3 with GCC Runtime Library Exception 3.1.
The retained esp_wifi, bt and coexist archives contribute startup/glue;
no libnet80211, libpp, libphy or libbtdm_app section is retained.
The allocated-section archive inventory is in examples/schematik-tdisplay/linked-archives.txt.
Bootloader: supplied by the pinned ESP-IDF/Arduino package (Apache-2.0,
FreeRTOS/runtime licences below). Partitions contain data, not code.

LGPL rebuild sources are provided alongside these images in
schematik-tdisplay-sources.tar.gz: Arduino core, SPI/Wire, board variants,
build scripts, application and pinned recipe. You may modify these libraries
and rebuild/relink the application for your own use; reverse engineering
for debugging those modifications is permitted. See the included README.

Application (MIT)
"""
    notice += (ROOT / "LICENSE").read_text()
    notice += "\nArduino_GFX 1.6.9\n" + gfx_license.decode()
    notice += "\nArduino-ESP32 3.3.8\n" + arduino_license.decode()
    notice += "\nESP-IDF and Mbed TLS\n" + shared
    (PUBLIC / (PREFIX + "NOTICE.txt")).write_text(notice.expandtabs(8))

    files = []
    for directory in ["cores", "variants/lilygo_t_display", "tools/partitions"]:
        files.extend(source_files(CORE / directory, "arduino-esp32/" + directory))
    for script in sorted((CORE / "tools").glob("*.py")):
        files.append(("arduino-esp32/tools/" + script.name, script.read_bytes()))
    for directory in ["SPI", "Wire"]:
        files.extend(source_files(CORE / "libraries" / directory, "arduino-esp32/libraries/" + directory))
    for name in ["platform.txt", "boards.txt", "package.json"]:
        if (CORE / name).exists():
            files.append(("arduino-esp32/" + name, (CORE / name).read_bytes()))
    files.extend(source_files(GFX / "src", "Arduino_GFX/src"))
    files.append(("Arduino_GFX/license.txt", gfx_license))
    files.append(("arduino-esp32/LICENSE.md", arduino_license))
    for name in ["src/main.cpp", "platformio.ini", "README.md", "linked-archives.txt", "verify_frames.py"]:
        files.append(("application/" + name, (HERE / name).read_bytes()))
    files.append(("application/LICENSE", (ROOT / "LICENSE").read_bytes()))
    target = PUBLIC / (PREFIX + "sources.tar.gz")
    # Deterministic metadata; do not leak local paths, uid, uname or timestamps.
    with target.open("wb") as output, gzip.GzipFile(fileobj=output, mode="wb", filename="", mtime=0) as zipped, tarfile.open(fileobj=zipped, mode="w") as archive:
        for name, data in sorted(files):
            info = tarfile.TarInfo(name)
            info.size = len(data)
            info.mode = 0o644
            archive.addfile(info, io.BytesIO(data))
    public_files.append(target)
    public_files.append(PUBLIC / (PREFIX + "NOTICE.txt"))
    sums = [(HERE / "src/main.cpp", "src/main.cpp")]
    sums += [(path, "../../web/wasm/fw/public/" + path.name) for path in public_files]
    (HERE / "SHA256SUMS").write_text("".join(hashlib.sha256(path.read_bytes()).hexdigest() + "  " + name + "\n" for path, name in sums))
    print("Packaged", ", ".join(path.name for path in public_files))


if __name__ == "__main__":
    main()
