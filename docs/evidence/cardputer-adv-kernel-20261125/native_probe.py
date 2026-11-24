#!/usr/bin/env python3
"""Equal-work native Cardputer-Adv screen. Caller supplies binaries, ROM and merged Flash.

No device connection, clock/quantum change, firmware patch, storage clearing or fake output.
Counts include native JIT interpreter helpers; compiled blocks do not imply all-native ops.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import statistics
import subprocess
import tempfile
import time


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def run(binary, args, out, interpreter=False):
    png = out / "frame.png"
    png.unlink(missing_ok=True)
    command = [str(binary), *args, "--tft-png", str(png)]
    if interpreter:
        command.append("--no-jit")
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        start = time.perf_counter()
        child = subprocess.Popen(command, stdout=stdout, stderr=stderr)
        try:
            _, status, usage = os.wait4(child.pid, 0)
            child.returncode = os.waitstatus_to_exitcode(status)
        finally:
            if child.returncode is None:
                child.kill()
                child.wait()
        wall = time.perf_counter() - start
        stdout.seek(0)
        console = stdout.read()
        stderr.seek(0)
        text = stderr.read().decode(errors="replace")
    if child.returncode:
        raise RuntimeError(f"emulator exit {child.returncode}: {text[-1000:]}")
    stop = re.search(r"stop: .*?core0 (\d+) \+ core1 (\d+) insns", text)
    cache = re.search(r"blocks: (\d+) built \((\d+) cache flushes\) core0, (\d+) \((\d+)\) core1; jit: (\d+) compiled, (\d+) KB code", text)
    if not stop or not cache or not png.exists():
        raise RuntimeError("missing real stop/cache/frame output")
    return {
        "wall_s": wall, "user_s": usage.ru_utime, "system_s": usage.ru_stime,
        "peak_rss_bytes": usage.ru_maxrss * (1 if platform.system() == "Darwin" else 1024),
        "instructions": [int(x) for x in stop.groups()],
        "cache": dict(zip(["builds0", "flushes0", "builds1", "flushes1", "compiled", "code_kib"], map(int, cache.groups()))),
        "console_sha256": hashlib.sha256(console).hexdigest(), "frame_png_sha256": sha(png),
        "interpreter": interpreter,
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    for name in ["base", "new", "rom", "flash", "out"]:
        ap.add_argument("--" + name, type=Path, required=True)
    ap.add_argument("--seconds", type=float, default=2)
    ap.add_argument("--pairs", type=int, default=3)
    ap.add_argument("--oracle", action="store_true", help="also compare new --no-jit; never time it as a candidate speedup")
    a = ap.parse_args()
    if not 0 < a.seconds <= 300 or not 1 <= a.pairs <= 20:
        ap.error("seconds must be (0,300], pairs 1..20")
    args = ["--boot", "rom", "--rom", str(a.rom.resolve()), "--board", "cardputer-adv", "--flash-mb", "8", "--psram-mb", "0",
            "--flash-image", str(a.flash.resolve()), "--net", "none", "--max-seconds", str(a.seconds), "--no-dump"]
    result = {
        "platform": {"os": platform.system(), "arch": platform.machine()},
        "inputs": {n: {"bytes": getattr(a, n).stat().st_size, "sha256": sha(getattr(a, n))} for n in ["base", "new", "rom", "flash"]},
        "guest_seconds_limit": a.seconds, "native_default_quantum": 64, "pairs": a.pairs, "runs": [], "load_before": os.getloadavg(),
        "command": ["esp32sim", "--boot", "rom", "--rom", "$ROM", "--board", "cardputer-adv", "--flash-mb", "8", "--psram-mb", "0",
                    "--flash-image", "$FLASH", "--net", "none", "--max-seconds", str(a.seconds), "--no-dump", "--tft-png", "$TEMP/frame.png"],
    }
    with tempfile.TemporaryDirectory(prefix="cardputer-native-") as temp:
        for pair in range(a.pairs + 1):
            for side in (["new", "base"] if pair % 2 == 0 else ["base", "new"]):
                rec = run(getattr(a, side).resolve(), args, Path(temp))
                rec.update(side=side, pair=pair, warmup=pair == 0)
                result["runs"].append(rec)
                print(f"pair {pair} {side}: user {rec['user_s']:.6f}s, wall {rec['wall_s']:.6f}s", flush=True)
        if a.oracle:
            result["oracle"] = run(a.new.resolve(), args, Path(temp), interpreter=True)
    result["load_after"] = os.getloadavg()
    keys = lambda r: (tuple(r["instructions"]), r["console_sha256"], r["frame_png_sha256"])
    records = result["runs"] + ([result["oracle"]] if a.oracle else [])
    result["identical_work_console_frame"] = len({keys(r) for r in records}) == 1
    result["medians"] = {side: {key: statistics.median(r[key] for r in result["runs"] if r["side"] == side and not r["warmup"])
                                for key in ["user_s", "wall_s", "peak_rss_bytes"]} for side in ["base", "new"]}
    a.out.parent.mkdir(parents=True, exist_ok=True)
    a.out.write_text(json.dumps(result, indent=2) + "\n")
    if not result["identical_work_console_frame"]:
        raise SystemExit("work/output mismatch; receipt retained, no speed comparison accepted")


if __name__ == "__main__":
    main()
