#!/usr/bin/env python3
"""ROM-boot the three pinned official Seeed factory ZIPs and verify real inputs.

Requires Pillow, a built esp32sim CLI and the official S3 ROM ELF. Download the
ZIPs as 0.96.zip, 1.14.zip and 1.47.zip into --firmware-dir; --download can do
this from the pinned public source. Binary images and captures stay in --output.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import subprocess
import time
from urllib.parse import quote
from urllib.request import urlopen
import zipfile

from PIL import Image, ImageChops

SOURCE = "https://raw.githubusercontent.com/Seeed-Projects/Display-Gadgets/ba7f69f223fb90b1766175afee4d26a045663a97/"
ROM_SHA = "c0ce0f338d1de1bdc6efbef1591779a2a42c1ab7d759d3c6ae8ae63a7dd34cfd"
COMMON = [
    (".ino.bootloader.bin", 0, "5403ba8cdf81cbb47f2debe13c0f5ff5903540075ccaf3fac65f0ee68213cb7d"),
    (".ino.partitions.bin", 0x8000, "8cee7a7c8e98c290ec9073c5cb31b33b3eefd0c4ddf461e9ccbac29e1a13e677"),
    ("boot_app0.bin", 0xe000, "f94c5d786a7a8fab06ac5d10e33bf37711a6697636dc037559ea19cc410a17f0"),
]
PROFILES = {
    "096": {"size": "0.96", "dimensions": [80, 160], "prefix": "0715_DashBoard_096_ESP32", "version": "v1.13.1", "zip": "64e023a67710a66405097f7487e0ac3e4fc080437c723b76ce160232c63d1216", "app": "8e3b2b01ee9f8f94636671a448e61c4e93307cc02ba00f2e0aededd3d4841770", "dim": "[BTN] KEY1 brightness=75%", "off": "[BTN] KEY2 backlight=OFF"},
    "114": {"size": "1.14", "dimensions": [135, 240], "prefix": "0715_DashBoard_114_ESP32", "version": "v1.2.4", "zip": "01b7d3b5641ee3e4be722a7c1f37217398a5926004eea59eca7241e71e02b680", "app": "64459863b91be6b775354a062bb53d489fdc0619dfee71f08f2356c08f4e61c4", "dim": "[BTN] USR1 cycle brightness pwm=192 label=75%", "off": "[BTN] USR2 toggle brightness pwm=0 label=0%"},
    "147": {"size": "1.47", "dimensions": [172, 320], "prefix": "0703_DashBoard_147_ESP32", "version": "v7.14", "zip": "d242496ffc36858239815878f136cbfb9afc4af597b393b4c757982afe21d6f1", "app": "69b2ce27c426fb0645f8a2dec991cac59f64de50ea80e470741c510ef00bbd24", "dim": "bl=191/75%", "off": "bl=0/0%"},
}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def checked(data, expected, label):
    actual = digest(data)
    if actual != expected:
        raise ValueError(f"{label}: SHA-256 mismatch: {actual}")
    return data


def image_stats(path):
    im = Image.open(path).convert("RGB")
    pixels = list(im.getdata())
    return im, {"size": list(im.size), "sha256": digest(path.read_bytes()), "rgbSha256": digest(im.tobytes()), "nonBlackPixels": sum(p != (0, 0, 0) for p in pixels), "colors": len(set(pixels)), "meanRgb": sum(sum(p) for p in pixels) / (3 * len(pixels))}


def run(args, ident, image, name, actions=""):
    folder = args.output / ident
    folder.mkdir(parents=True, exist_ok=True)
    png, script = folder / f"{name}.png", folder / f"{name}.script"
    cmd = [str(args.cli), "--chip", "s3", "--board", "seeed-xiao-ips-" + ident, "--rom", str(args.rom), "--boot", "rom", "--flash-image", str(image), "--flash-mb", "16", "--psram-mb", "8", "--max-seconds", "6", "--console", "both", "--no-dump", "--tft-png", str(png)]
    if actions:
        script.write_text(actions)
        cmd += ["--script", str(script)]
    started = time.monotonic()
    result = subprocess.run(cmd, capture_output=True, timeout=180)
    wall = time.monotonic() - started
    raw = result.stdout + result.stderr
    (folder / f"{name}.log").write_bytes(raw)
    text = raw.decode("utf-8", errors="replace")
    assert result.returncode == 0, f"{ident}/{name}: CLI exit {result.returncode}"
    assert "[BOOT]" in text and "ready" in text and PROFILES[ident]["version"] in text, f"{ident}/{name}: factory boot marker missing"
    assert "[IMU] not found" in text, "Unmodeled IMU must remain absent"
    assert "[emu] ROM boot from reset vector 0x40000400" in text
    frame, stats = image_stats(png)
    assert stats["size"] == [d * 3 for d in PROFILES[ident]["dimensions"]]
    stop = re.search(r"core0 (\d+) \+ core1 (\d+) insns.*?emulated ([\d.]+)s \((\d+) cycles\)", text)
    assert stop, "Missing instruction/cycle accounting"
    record = {"scenario": name, "actions": actions.splitlines(), "exitCode": result.returncode, "bootReady": True, "romBoot": True, "factoryVersion": PROFILES[ident]["version"], "wallSeconds": round(wall, 3), "instructions": [int(stop[1]), int(stop[2])], "virtualSeconds": float(stop[3]), "cycles": int(stop[4]), "consoleSha256": digest(raw), "consoleBytes": len(raw), "screenshot": stats}
    print(f"{ident}/{name}: boot ready, {stats['nonBlackPixels']} visible pixels", flush=True)
    return text, frame, record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cli", type=Path, default=Path("target/release/esp32sim"))
    parser.add_argument("--rom", type=Path, required=True)
    parser.add_argument("--firmware-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=Path("target/seeed-xiao-ips-probe"))
    parser.add_argument("--models", nargs="+", choices=list(PROFILES), default=list(PROFILES))
    parser.add_argument("--download", action="store_true")
    args = parser.parse_args()
    args.cli, args.rom, args.output = args.cli.resolve(), args.rom.resolve(), args.output.resolve()
    args.firmware_dir.mkdir(parents=True, exist_ok=True)
    args.output.mkdir(parents=True, exist_ok=True)
    checked(args.rom.read_bytes(), ROM_SHA, "S3 ROM ELF")
    receipt = {"schema": 1, "verifiedAt": datetime.now(timezone.utc).isoformat(), "romSha256": ROM_SHA, "cliSha256": digest(args.cli.read_bytes()), "sourceCommit": "ba7f69f223fb90b1766175afee4d26a045663a97", "mode": "native-interpreter-rom-boot-unmodified-official-firmware", "flashMiB": 16, "psramMiB": 8, "models": []}
    for ident in args.models:
        p = PROFILES[ident]
        zip_name = f"XIAO {p['size']}'' IPS Display (ESP32-S3) Factory Firmware.zip"
        url = SOURCE + quote("resources/firmware/" + zip_name, safe="/")
        archive = args.firmware_dir / (p["size"] + ".zip")
        if args.download and not archive.exists():
            archive.write_bytes(urlopen(url, timeout=60).read())
        checked(archive.read_bytes(), p["zip"], zip_name)
        image = bytearray(b"\xff" * (16 << 20))
        segments = []
        with zipfile.ZipFile(archive) as z:
            for suffix, offset, sha in COMMON + [(".ino.bin", 0x10000, p["app"])]:
                name = suffix if suffix == "boot_app0.bin" else p["prefix"] + suffix
                entries = [n for n in z.namelist() if Path(n).name == name]
                assert len(entries) == 1, f"Expected one exact {name}"
                data = checked(z.read(entries[0]), sha, name)
                image[offset:offset + len(data)] = data
                segments.append({"entry": entries[0], "offset": offset, "bytes": len(data), "sha256": sha})
        image_path = args.output / f"{ident}.merged.bin"
        image_path.write_bytes(image)
        model = {"board": "seeed-xiao-ips-" + ident, "source": {"url": url, "zipSha256": p["zip"], "segments": segments, "mergedSha256": digest(image)}, "scenarios": []}
        baseline, original, record = run(args, ident, image_path, "boot")
        assert record["screenshot"]["colors"] >= 5 and record["screenshot"]["nonBlackPixels"] > 300
        model["scenarios"].append(record)
        mean = record["screenshot"]["meanRgb"]
        a, b = ("button-a", "button-b") if ident == "114" else ("usr1", "usr2")
        text, _, record = run(args, ident, image_path, "dim", f"3 press {a} 1200\n")
        assert p["dim"] in text, f"{ident}: missing firmware brightness response"
        assert 0 < record["screenshot"]["meanRgb"] < mean * 0.9, "PWM must visibly dim the framebuffer"
        record["firmwareBrightness75"] = True
        record["brightnessRatio"] = record["screenshot"]["meanRgb"] / mean
        model["scenarios"].append(record)
        text, _, record = run(args, ident, image_path, "off", f"3 press {b} 1200\n")
        assert p["off"] in text, f"{ident}: missing firmware backlight-off response"
        assert record["screenshot"]["nonBlackPixels"] == 0, "Backlight off must blank physical display while GRAM remains"
        record["firmwareBacklightOff"] = True
        model["scenarios"].append(record)
        if ident == "114":
            text, changed, record = run(args, ident, image_path, "header", "3 press button-c 1200\n")
            assert "[BTN] USR3 header=Seeed" in text
            assert ImageChops.difference(original, changed).crop((0, 0, original.width, 90)).getbbox()
            record["firmwareHeaderChanged"] = True
            model["scenarios"].append(record)
        if ident == "147":
            assert "[TOUCH] AXS5106L BSP driver OK at 0x63" in baseline
            actions = "".join(f"{t/10:.1f} touch 20 301 1\n" for t in range(30, 60, 2))
            text, changed, record = run(args, ident, image_path, "touch", actions)
            assert "touch=Y x=20 y=301" in text, "Guest must decode mirrored raw I2C report into host coordinates"
            assert ImageChops.difference(original, changed).crop((0, 85 * 3, original.width, 110 * 3)).getbbox(), "Touch row must visibly change"
            record["firmwareTouchCoordinates"] = [20, 301]
            model["scenarios"].append(record)
            text, _, record = run(args, ident, image_path, "touch-release", actions.split("4.4 touch")[0] + "4.4 touch 20 301 0\n")
            assert "touch=Y x=20 y=301" in text
            assert re.search(r"touch=Y x=20 y=301.*touch=N x=-1 y=-1", text, re.S), "Guest must observe release after contact"
            record["firmwareTouchRelease"] = True
            model["scenarios"].append(record)
        receipt["models"].append(model)
        (args.output / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(f"PASS: {len(receipt['models'])} board profiles; receipt.json in the chosen output directory")


if __name__ == "__main__":
    main()
