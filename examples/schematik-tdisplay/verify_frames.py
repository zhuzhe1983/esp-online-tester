#!/usr/bin/env python3
"""Replay shipped firmware and assert the text pixels of every screen.

Requires a native release build, fetched ROM, PlatformIO dependencies and Pillow.
Run from the repo root: python3 examples/schematik-tdisplay/verify_frames.py OUT
OUT keeps replay scripts, console logs and actual (not synthesized) display PNGs.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
# Independent expected text: never derive the expected sequence from the sketch.
SCREENS = [
    ("", "schematik", ""),
    ("", "making hardware", "less hard"),
    ("", "from idea to", "blinking LED"),
    ("", "now with 100% fewer", "magic smoke"),
    ("", "it works in", "the simulator"),
    ("processor", "Xtensa LX6", "dual core, 240 MHz"),
    ("storage", "4 MB flash", ""),
    ("analog inputs", "18 ADC channels", ""),
    ("web demo", "running in", "WebAssembly"),
]


def expected_mask(index, font):
    image = Image.new("1", (240, 135))
    draw = ImageDraw.Draw(image)

    def text(value, y, size):
        x = (240 - len(value) * 6 * size) // 2
        assert x >= 0 and y + 8 * size <= 135, value
        for character in value:
            for col, byte in enumerate(font[ord(character) * 5:ord(character) * 5 + 5]):
                for row in range(8):
                    if byte & (1 << row):
                        left, top = x + col * size, y + row * size
                        draw.rectangle((left, top, left + size - 1, top + size - 1), fill=1)
            x += 6 * size

    if index == 0:
        text("schematik", (135 - 32) // 2, 4)
    else:
        title, first, second = SCREENS[index]
        text(title, 28, 1)
        text(first, 51, 2)
        text(second, 75, 2)
    return image


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=True)
    (out / "screens").mkdir(exist_ok=True)
    font_source = (HERE / ".pio/libdeps/ttgo-t-display/GFX Library for Arduino/src/font/glcdfont.h").read_text()
    font_source = font_source.split("PROGMEM = {", 1)[1].split("};", 1)[0]
    font_source = re.sub(r"//[^\n]*", "", font_source)
    font = bytes(int(value, 16) for value in re.findall(r"0x([0-9A-Fa-f]{2})", font_source))
    fw = ROOT / "web/wasm/fw"
    command = ["nice", "-n", "10", str(ROOT / "target/release/esp32sim"),
               "--chip", "esp32", "--board", "ttgo-tdisplay", "--rom", str(fw / "esp32_rev300_rom.elf"),
               "--boot", "rom", "--console", "uart0", "--flash-mb", "4"]
    for flag, name in [("bootloader", "bootloader"), ("ptable", "ptable"), ("app", "app")]:
        command += ["--" + flag, str(fw / ("public/schematik-tdisplay-" + name + ".bin"))]

    def run(name, script, stop, index=None):
        replay = out / (name + ".txt")
        replay.write_text(script)
        raw = out / (name + "-portrait.png")
        cmd = command + ["--script", str(replay), "--max-seconds", str(stop), "--tft-png", str(raw)]
        result = subprocess.run(cmd, capture_output=True, text=True, check=True, cwd=ROOT)
        (out / (name + ".log")).write_text(result.stdout + result.stderr)
        image = Image.open(raw).convert("RGB").transpose(Image.Transpose.ROTATE_90)
        image = image.resize((240, 135), Image.Resampling.NEAREST)
        image.save(out / "screens" / (name + ".png"))
        mask = image.convert("L").point(lambda x: 255 if x else 0).convert("1")
        if index is None:
            assert not mask.getbbox(), (name, "sleep must blank the panel")
        else:
            assert mask.tobytes() == expected_mask(index, font).tobytes(), (name, "text pixels/order/layout mismatch", SCREENS[index])
        return dict(name=name, expected=SCREENS[index] if index is not None else "blank",
                    rgb_sha256=hashlib.sha256(image.tobytes()).hexdigest()), result.stdout + result.stderr

    results = []
    for presses in range(len(SCREENS) + 1):
        script = "".join(f"{0.6 + n * 0.4:.2f} press 35 150\n" for n in range(presses))
        row, _ = run(f"{presses:02d}", script, 0.95 + presses * 0.4, presses % len(SCREENS))
        results.append(row)
        print("PASS", row["name"], row["expected"], flush=True)
    assert results[0]["rgb_sha256"] == results[-1]["rgb_sha256"], "wrap must restore wordmark"
    # Sleep from a noninitial message, verify parked CPUs twice, then wake to boot branding.
    script = "0.6 press 35 150\n1 press 0 150\n"
    sleep_logs = []
    for stop in [2, 2.5]:
        row, log = run("sleep-" + str(stop), script, stop)
        results.append(row)
        sleep_logs.append(re.findall(r"core([01]): pc=([0-9a-f]+)[^\n]+ insns=(\d+)", log))
    assert sleep_logs[0] and sleep_logs[0] == sleep_logs[1], sleep_logs
    row, log = run("wake", script + "3 press 0 150\n", 4, 0)
    assert "DEEPSLEEP_RESET" in log, "wake must reset through EXT0"
    assert row["rgb_sha256"] == results[0]["rgb_sha256"], "wake must restore wordmark"
    results.append(row)
    row, _ = run("power-again", script + "3 press 0 150\n4.5 press 0 150\n", 5.5)
    results.append(row)
    (out / "native-frames.json").write_text(json.dumps(results, indent=2) + "\n")
    print("PASS wrap, parked sleep, EXT0 wake, release/re-arm")


if __name__ == "__main__":
    main()
